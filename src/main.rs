#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod downloader;
mod sanitizer;
mod state;
mod ui_chrome;
mod userscripts;

use downloader::{DownloadTask, EnqueueOutcome, NativeDownloadEngine, StreamProbeResult};
use rand::Rng;
use regex::Regex;
use sanitizer::{extract_host, sanitize_url};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{ElementState, Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::keyboard::KeyCode;
use tao::window::{Fullscreen, Icon, Window, WindowBuilder};
use userscripts::{list_user_scripts, load_approved_user_scripts, user_scripts_dir, UserScriptInfo};
use wry::{Rect, WebView, WebViewBuilder};

use state::{
    BookmarkEntry, BrowserStorage, GlobalSettings, HistoryEntry, SavedTab, SearchEngine,
    SitePolicy, APP_VERSION,
};

const CHROME_BAR_HEIGHT: f64 = 112.0;
const CONTENT_GUARD_JS: &str = include_str!("../assets/js/content_guard.js");

enum AppEvent {
    ChromeIpc(String),
    ContentIpc(usize, String),
    DownloadIntercepted(String),
    NewWindowBlockedOrOpened(String),
    NativeStreamCaptured(usize, String),
    StreamSizeResolved(usize, StreamProbeResult),
    TabAudioChanged(usize, bool),
    TabTitleChanged(usize, String),
    TabUrlChanged(usize, String),
    TabZoomChanged(usize, f64),
    RefreshChrome,
}

struct BrowserTab {
    id: usize,
    title: String,
    url: String,
    cookie: Option<String>,
    incognito: bool,
    pinned: bool,
    playing_audio: bool,
    muted: bool,
    reader_active: bool,
    zoom: f64,
    webview: WebView,
}

#[derive(Clone, Serialize)]
struct SniffedStreamItem {
    url: String,
    name: String,
    size_label: String,
}

#[derive(Serialize)]
struct TabSummary {
    id: usize,
    title: String,
    url: String,
    incognito: bool,
    pinned: bool,
    playing_audio: bool,
    muted: bool,
}

#[derive(Serialize)]
struct ChromeSnapshot {
    version: &'static str,
    ffmpeg_available: bool,
    ffmpeg_status: String,
    is_fullscreen: bool,
    has_closed_tabs: bool,
    find_current: usize,
    find_total: usize,
    tabs: Vec<TabSummary>,
    active_tab: usize,
    active_url: String,
    active_host: String,
    active_zoom: f64,
    active_reader: bool,
    site: SitePolicy,
    global: GlobalSettings,
    blocked_count: usize,
    streams: Vec<SniffedStreamItem>,
    bookmarks: Vec<BookmarkEntry>,
    history: Vec<HistoryEntry>,
    userscripts: Vec<UserScriptInfo>,
    duplicate_alert: Option<DownloadTask>,
    downloads: Vec<DownloadTask>,
}

fn generate_ipc_token() -> String {
    let mut rng = rand::thread_rng();
    let a: u64 = rng.gen();
    let b: u64 = rng.gen();
    format!("{:016x}{:016x}", a, b)
}

fn build_content_init_script(
    policy: &SitePolicy,
    global: &GlobalSettings,
    ipc_token: &str,
    user_js: &str,
) -> String {
    let mut script = CONTENT_GUARD_JS
        .replace("__SHUNNO_IPC_TOKEN__", ipc_token)
        .replace("__SHUNNO_ADBLOCK__", if policy.adblock { "true" } else { "false" })
        .replace("__SHUNNO_SPONSORBLOCK__", if policy.sponsorblock { "true" } else { "false" })
        .replace("__SHUNNO_REJECT_COOKIES__", if policy.auto_reject_cookies { "true" } else { "false" })
        .replace("__SHUNNO_BASKET__", if global.drop_basket_enabled { "true" } else { "false" })
        .replace("__SHUNNO_BG_MEDIA__", if global.background_media_play { "true" } else { "false" })
        .replace("__SHUNNO_FULL_DARK__", if global.full_dark_mode { "true" } else { "false" })
        .replace("__SHUNNO_JS_DISABLED__", if !policy.js_enabled { "true" } else { "false" })
        .replace("__SHUNNO_IMAGES_DISABLED__", if !policy.images_enabled { "true" } else { "false" })
        .replace("__SHUNNO_DESKTOP_UA__", if policy.desktop_ua { "true" } else { "false" });

    if !user_js.is_empty() {
        script.push('\n');
        script.push_str(user_js);
    }

    script
}

#[cfg(windows)]
fn apply_tab_zoom(webview: &WebView, factor: f64) {
    use wry::WebViewExtWindows;
    let _ = webview.zoom(factor);
    let controller = webview.controller();
    unsafe {
        let _ = controller.SetZoomFactor(factor);
    }
}

#[cfg(not(windows))]
fn apply_tab_zoom(webview: &WebView, factor: f64) {
    let _ = webview.zoom(factor);
}

#[cfg(windows)]
fn set_webview_muted(webview: &WebView, muted: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_8;
    use windows::core::Interface;
    use wry::WebViewExtWindows;

    let controller = webview.controller();
    unsafe {
        if let Ok(core) = controller.CoreWebView2() {
            if let Ok(core8) = core.cast::<ICoreWebView2_8>() {
                let _ = core8.SetIsMuted(muted);
                return;
            }
        }
    }
    let js = format!("document.querySelectorAll('video,audio').forEach(m => m.muted = {});", muted);
    let _ = webview.evaluate_script(&js);
}

#[cfg(not(windows))]
fn set_webview_muted(webview: &WebView, muted: bool) {
    let js = format!("document.querySelectorAll('video,audio').forEach(m => m.muted = {});", muted);
    let _ = webview.evaluate_script(&js);
}

#[cfg(windows)]
fn attach_native_webview2_hooks(
    webview: &WebView,
    tab_id: usize,
    adblock_enabled: bool,
    proxy: EventLoopProxy<AppEvent>,
) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL, ICoreWebView2_2, ICoreWebView2_8,
        ICoreWebView2WebResourceResponse,
    };
    use webview2_com::{IsDocumentPlayingAudioChangedEventHandler, WebResourceRequestedEventHandler};
    use windows::core::{w, Interface, PCWSTR};
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::System::Com::IStream;
    use wry::WebViewExtWindows;

    let controller = webview.controller();
    unsafe {
        // 0. Native Ctrl+Wheel / Pinch Zoom Synchronization
        use webview2_com::ZoomFactorChangedEventHandler;
        let mut zoom_token = Default::default();
        let zoom_proxy = proxy.clone();
        let zoom_handler = ZoomFactorChangedEventHandler::create(Box::new(move |sender, _| {
            if let Some(ctrl) = sender {
                let mut z = 1.0f64;
                if ctrl.ZoomFactor(&mut z).is_ok() {
                    let _ = zoom_proxy.send_event(AppEvent::TabZoomChanged(tab_id, z));
                }
            }
            Ok(())
        }));
        let _ = controller.add_ZoomFactorChanged(&zoom_handler, &mut zoom_token);

        let Ok(core) = controller.CoreWebView2() else {
            return;
        };

        // 1. Native Tab Audio Playing State Listener (ICoreWebView2_8)
        if let Ok(core8) = core.cast::<ICoreWebView2_8>() {
            let mut audio_token = Default::default();
            let audio_proxy = proxy.clone();
            let audio_handler = IsDocumentPlayingAudioChangedEventHandler::create(Box::new(move |sender, _| {
                if let Some(wv) = sender {
                    if let Ok(wv8) = wv.cast::<ICoreWebView2_8>() {
                        let mut playing = BOOL::default();
                        if wv8.IsDocumentPlayingAudio(&mut playing).is_ok() {
                            let _ = audio_proxy.send_event(AppEvent::TabAudioChanged(tab_id, playing.as_bool()));
                        }
                    }
                }
                Ok(())
            }));
            let _ = core8.add_IsDocumentPlayingAudioChanged(&audio_handler, &mut audio_token);
        }

        // 2. Native AdBlocker & Stream Sniffer (ICoreWebView2_2)
        let Ok(core2) = core.cast::<ICoreWebView2_2>() else {
            return;
        };
        let Ok(env) = core2.Environment() else {
            return;
        };

        // Only intercept known ad/tracker URL patterns so 99% of page traffic runs at full native Chromium speed
        if adblock_enabled {
            let ad_filters = [
                w!("*://*.doubleclick.net/*"),
                w!("*://*.googlesyndication.com/*"),
                w!("*://*.googleadservices.com/*"),
                w!("*://*.adnxs.com/*"),
                w!("*://*.adsrvr.org/*"),
                w!("*://*.taboola.com/*"),
                w!("*://*.outbrain.com/*"),
                w!("*://*.popads.net/*"),
                w!("*://*.popcash.net/*"),
                w!("*://*.propellerads.com/*"),
                w!("*://*.exoclick.com/*"),
                w!("*://*.youtube.com/pagead/*"),
                w!("*://*.youtube.com/api/stats/ads*"),
                w!("*://*.youtube.com/ptracking*"),
                w!("*://*.youtube.com/get_midroll_*"),
                w!("*://*.googlevideo.com/initplayback*oad*"),
            ];
            for pat in ad_filters {
                let _ = core.AddWebResourceRequestedFilter(pat, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL);
            }

            let mut token = Default::default();
            let env_for_ad = env.clone();
            let ad_proxy = proxy.clone();
            let handler = WebResourceRequestedEventHandler::create(Box::new(move |_sender, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let empty_resp: ICoreWebView2WebResourceResponse = env_for_ad.CreateWebResourceResponse(
                    None::<&IStream>,
                    403,
                    w!("Blocked by Shunno AdBlock"),
                    PCWSTR::null(),
                )?;
                let _ = args.SetResponse(&empty_resp);
                let _ = ad_proxy.send_event(AppEvent::ContentIpc(tab_id, "INTERNAL|AD_BLOCKED".to_string()));
                Ok(())
            }));

            let _ = core.add_WebResourceRequested(&handler, &mut token);
        }

        // 3. Native WebView2 DownloadStarting Interceptor (routes all browser downloads to Downloads)
        use webview2_com::DownloadStartingEventHandler;
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_4;
        if let Ok(core4) = core.cast::<ICoreWebView2_4>() {
            let mut dl_token = Default::default();
            let dl_proxy = proxy.clone();
            let dl_handler = DownloadStartingEventHandler::create(Box::new(move |_sender, args| {
                if let Some(args) = args {
                    if let Ok(op) = args.DownloadOperation() {
                        let mut uri_ptr = windows::core::PWSTR::null();
                        if op.Uri(&mut uri_ptr).is_ok() {
                            let uri = webview2_com::take_pwstr(uri_ptr);
                            if uri.starts_with("http") {
                                let _ = args.SetCancel(true);
                                let _ = args.SetHandled(true);
                                let _ = dl_proxy.send_event(AppEvent::DownloadIntercepted(uri));
                            }
                        }
                    }
                }
                Ok(())
            }));
            let _ = core4.add_DownloadStarting(&dl_handler, &mut dl_token);
        }
        // 4. Native WebView2 ContextMenuRequested Hook: Replace "Open link in new window" with "Open link in new tab"
        use webview2_com::ContextMenuRequestedEventHandler;
        use webview2_com::CustomItemSelectedEventHandler;
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND, ICoreWebView2Environment9, ICoreWebView2_11,
        };
        if let (Ok(core11), Ok(env9)) = (core.cast::<ICoreWebView2_11>(), env.cast::<ICoreWebView2Environment9>()) {
            let mut ctx_token = Default::default();
            let ctx_proxy = proxy.clone();
            let ctx_handler = ContextMenuRequestedEventHandler::create(Box::new(move |_sender, args| {
                let Some(args) = args else { return Ok(()); };
                let Ok(target) = args.ContextMenuTarget() else { return Ok(()); };
                let mut has_link = windows::Win32::Foundation::BOOL::default();
                let _ = target.HasLinkUri(&mut has_link);
                if has_link.as_bool() {
                    let mut link_ptr = windows::core::PWSTR::null();
                    if target.LinkUri(&mut link_ptr).is_ok() {
                        let link_uri = webview2_com::take_pwstr(link_ptr);
                        if link_uri.starts_with("http") {
                            if let Ok(items) = args.MenuItems() {
                                let mut count = 0u32;
                                let _ = items.Count(&mut count);
                                let mut idx = 0u32;
                                while idx < count {
                                    if let Ok(item) = items.GetValueAtIndex(idx) {
                                        let mut name_ptr = windows::core::PWSTR::null();
                                        if item.Name(&mut name_ptr).is_ok() {
                                            let name = webview2_com::take_pwstr(name_ptr);
                                            if name.eq_ignore_ascii_case("openLinkInNewWindow") {
                                                let _ = items.RemoveValueAtIndex(idx);
                                                count -= 1;
                                                continue;
                                            }
                                        }
                                    }
                                    idx += 1;
                                }

                                if let Ok(new_tab_item) = env9.CreateContextMenuItem(
                                    w!("Open link in new tab"),
                                    None::<&IStream>,
                                    COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND,
                                ) {
                                    let mut sel_token = Default::default();
                                    let sel_proxy = ctx_proxy.clone();
                                    let sel_uri = link_uri.clone();
                                    let sel_handler = CustomItemSelectedEventHandler::create(Box::new(move |_s, _| {
                                        let _ = sel_proxy.send_event(AppEvent::NewWindowBlockedOrOpened(sel_uri.clone()));
                                        Ok(())
                                    }));
                                    let _ = new_tab_item.add_CustomItemSelected(&sel_handler, &mut sel_token);
                                    let _ = items.InsertValueAtIndex(0, &new_tab_item);
                                }
                            }
                        }
                    }
                }
                Ok(())
            }));
            let _ = core11.add_ContextMenuRequested(&ctx_handler, &mut ctx_token);
        }
    }
}

#[cfg(not(windows))]
fn attach_native_webview2_hooks(
    _webview: &WebView,
    _tab_id: usize,
    _adblock_enabled: bool,
    _proxy: EventLoopProxy<AppEvent>,
) {
}

fn create_tab_webview(
    window: &Window,
    tab_id: usize,
    url: &str,
    policy: &SitePolicy,
    global: &GlobalSettings,
    ipc_token: &str,
    proxy: EventLoopProxy<AppEvent>,
) -> wry::Result<WebView> {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let content_h = (size.height - CHROME_BAR_HEIGHT).max(100.0);
    let host = extract_host(url);
    let user_js = load_approved_user_scripts(&host, &global.approved_script_hashes);
    let init_script = build_content_init_script(policy, global, ipc_token, &user_js);

    let ipc_proxy = proxy.clone();
    let dl_proxy = proxy.clone();
    let win_proxy = proxy.clone();
    let title_proxy = proxy.clone();
    let nav_proxy = proxy.clone();

    let wv = WebViewBuilder::new_as_child(window)
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, CHROME_BAR_HEIGHT).into(),
            size: LogicalSize::new(size.width.max(400.0), content_h).into(),
        })
        .with_url(if url == "shunno://newtab" { "about:blank" } else { url })
        .with_initialization_script(&init_script)
        .with_document_title_changed_handler(move |new_title| {
            let _ = title_proxy.send_event(AppEvent::TabTitleChanged(tab_id, new_title));
        })
        .with_navigation_handler(move |nav_url| {
            if nav_url.starts_with("http://") || nav_url.starts_with("https://") {
                let _ = nav_proxy.send_event(AppEvent::TabUrlChanged(tab_id, nav_url));
            }
            true
        })
        .with_ipc_handler(move |req| {
            let _ = ipc_proxy.send_event(AppEvent::ContentIpc(tab_id, req.body().clone()));
        })
        .with_download_started_handler(move |dl_url, _dest| {
            let _ = dl_proxy.send_event(AppEvent::DownloadIntercepted(dl_url));
            false
        })
        .with_new_window_req_handler(move |popup_url| {
            let _ = win_proxy.send_event(AppEvent::NewWindowBlockedOrOpened(popup_url));
            false
        })
        .build()?;

    attach_native_webview2_hooks(&wv, tab_id, policy.adblock, proxy);

    let _ = wv.set_visible(true);
    Ok(wv)
}

/// Non-Overlapping Split Layout:
/// When a drawer/popover is open, `chrome_wv` occupies the top bar + right docked panel `(w - panel_w, CHROME_BAR_HEIGHT, panel_w, content_h)`
/// while the active tab WebView occupies the left viewport `(0, CHROME_BAR_HEIGHT, w - panel_w, content_h)`.
/// Because WebView2 HWNDs never overlap, every button in the menu/downloads panel is 100% clickable AND the live webpage remains visible!
#[cfg(windows)]
fn install_native_win32_window_features(window: &Window, chrome_wv: &WebView) {
    use tao::platform::windows::WindowExtWindows;
    use webview2_com::Microsoft::Web::WebView2::Win32::{COREWEBVIEW2_COLOR, ICoreWebView2Controller2};
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Shell::{DefSubclassProc, SetCurrentProcessExplicitAppUserModelID, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowLongW, GetWindowRect, LoadImageW, SendMessageW, SetClassLongPtrW,
        SetWindowLongW, SetWindowPos, GCLP_HICON, GCLP_HICONSM, GWL_EXSTYLE, GWL_STYLE,
        HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT,
        HTTOPRIGHT, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED, SWP_FRAMECHANGED,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WM_NCCALCSIZE, WM_NCHITTEST, WM_SETICON,
        WS_EX_APPWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
    };
    use wry::WebViewExtWindows;

    let hwnd = HWND(window.hwnd() as isize);
    unsafe { let _ = SetCurrentProcessExplicitAppUserModelID(windows::core::w!("Shunno.Browser.Desktop")); }

    unsafe {
        // 1. Enable WS_THICKFRAME + WS_EX_APPWINDOW so borderless window supports mouse resizing & Windows Snap!
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let new_style = style | WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0 | WS_SYSMENU.0;
        SetWindowLongW(hwnd, GWL_STYLE, new_style as i32);

        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongW(hwnd, GWL_EXSTYLE, (ex_style | WS_EX_APPWINDOW.0) as i32);

        let _ = SetWindowPos(
            hwnd,
            HWND(0),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );

        // 2. Load the embedded EXE icon resource (#1 compiled by winres) onto both Window & Class so Taskbar never shows a black circle!
        if let Ok(hinst) = GetModuleHandleW(None) {
            if let Ok(hicon_big) = LoadImageW(
                hinst,
                PCWSTR(1 as *const u16),
                IMAGE_ICON,
                48,
                48,
                LR_SHARED,
            ) {
                SetClassLongPtrW(hwnd, GCLP_HICON, hicon_big.0);
                let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_BIG as usize), LPARAM(hicon_big.0));
            }
            if let Ok(hicon_sm) = LoadImageW(
                hinst,
                PCWSTR(1 as *const u16),
                IMAGE_ICON,
                16,
                16,
                LR_DEFAULTSIZE | LR_SHARED,
            ) {
                SetClassLongPtrW(hwnd, GCLP_HICONSM, hicon_sm.0);
                let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_SMALL as usize), LPARAM(hicon_sm.0));
            }
        }

        // 3. Make chrome_wv WebView2 background transparent (A=0) so overlays float cleanly over the live webpage without flicker!
        let chrome_ctrl = chrome_wv.controller();
        if let Ok(ctrl2) = chrome_ctrl.cast::<ICoreWebView2Controller2>() {
            let _ = ctrl2.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR { A: 0, R: 0, G: 0, B: 0 });
        }

        // 4. Subclass window procedure for 8-directional mouse resize on borderless frame
        unsafe extern "system" fn borderless_resize_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
            _uid: usize,
            _ref_data: usize,
        ) -> LRESULT {
            match msg {
                WM_NCCALCSIZE => {
                    if wparam.0 != 0 {
                        return LRESULT(0);
                    }
                }
                WM_NCHITTEST => {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    let mut rc = RECT::default();
                    if GetWindowRect(hwnd, &mut rc).is_ok() {
                        let border = 8;
                        let on_left = x >= rc.left && x < rc.left + border;
                        let on_right = x <= rc.right && x > rc.right - border;
                        let on_top = y >= rc.top && y < rc.top + border;
                        let on_bottom = y <= rc.bottom && y > rc.bottom - border;

                        if on_top && on_left { return LRESULT(HTTOPLEFT as isize); }
                        if on_top && on_right { return LRESULT(HTTOPRIGHT as isize); }
                        if on_bottom && on_left { return LRESULT(HTBOTTOMLEFT as isize); }
                        if on_bottom && on_right { return LRESULT(HTBOTTOMRIGHT as isize); }
                        if on_left { return LRESULT(HTLEFT as isize); }
                        if on_right { return LRESULT(HTRIGHT as isize); }
                        if on_top { return LRESULT(HTTOP as isize); }
                        if on_bottom { return LRESULT(HTBOTTOM as isize); }
                        let _ = GetClientRect(hwnd, &mut rc);
                        return LRESULT(HTCLIENT as isize);
                    }
                }
                _ => {}
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }

        let _ = SetWindowSubclass(hwnd, Some(borderless_resize_proc), 1, 0);
    }
}

#[cfg(not(windows))]
fn install_native_win32_window_features(_window: &Window, _chrome_wv: &WebView) {}

#[cfg(windows)]
fn bring_chrome_above_tabs(chrome_wv: &WebView) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE};
    use wry::WebViewExtWindows;

    let ctrl = chrome_wv.controller();
    unsafe {
        let mut parent = HWND::default();
        if ctrl.ParentWindow(&mut parent).is_ok() && parent.0 != 0 {
            let _ = SetWindowPos(parent, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
}

#[cfg(not(windows))]
fn bring_chrome_above_tabs(_chrome_wv: &WebView) {}

fn layout_webviews(
    window: &Window,
    chrome_wv: &WebView,
    tabs: &[BrowserTab],
    active_idx: usize,
    drawer_open: bool,
    engine_menu_open: bool,
) {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let w = size.width.max(400.0);
    let h = size.height.max(300.0);
    let is_fs = window.fullscreen().is_some();
    let is_max = window.is_maximized() || is_fs;
    let grip = if is_max { 0.0 } else { 3.0 };

    let top_bar_h = if is_fs { 0.0 } else { CHROME_BAR_HEIGHT };
    let inner_w = (w - grip * 2.0).max(200.0);
    let inner_h = (h - grip).max(200.0);
    let content_h = (inner_h - top_bar_h).max(80.0);

    let is_home = tabs
        .get(active_idx)
        .map(|t| t.url == "shunno://newtab")
        .unwrap_or(true);

    let chrome_h = if is_fs {
        0.0
    } else if drawer_open || engine_menu_open || is_home {
        inner_h
    } else {
        top_bar_h
    };

    let _ = chrome_wv.set_visible(!is_fs);
    let _ = chrome_wv.set_bounds(Rect {
        position: LogicalPosition::new(grip, 0.0).into(),
        size: LogicalSize::new(inner_w, chrome_h).into(),
    });

    for (idx, tab) in tabs.iter().enumerate() {
        let _ = tab.webview.set_bounds(Rect {
            position: LogicalPosition::new(grip, top_bar_h).into(),
            size: LogicalSize::new(inner_w, content_h).into(),
        });
        let show_web = idx == active_idx && tab.url != "shunno://newtab";
        let _ = tab.webview.set_visible(show_web);
    }

    if !is_fs && (drawer_open || engine_menu_open) {
        bring_chrome_above_tabs(chrome_wv);
    }
}

fn main() -> wry::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to start Tokio runtime");
    let rt_handle = rt.handle().clone();

    let ipc_token = generate_ipc_token();
    let mut global_settings = BrowserStorage::load_global_settings();
    let mut site_policies = BrowserStorage::load_site_policies();
    let mut bookmarks = BrowserStorage::load_bookmarks();
    let mut history = BrowserStorage::load_history();
    let engine = Arc::new(NativeDownloadEngine::new(
        global_settings.speed_limit_bps,
        global_settings.auto_clean_completed,
        global_settings.max_concurrent_downloads,
    ));

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Shunno Browser")
        .with_decorations(false)
        .with_inner_size(LogicalSize::new(1240.0, 780.0))
        .build(&event_loop)
        .unwrap();

    let chrome_proxy = proxy.clone();
    let chrome_wv = WebViewBuilder::new_as_child(&window)
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: LogicalSize::new(1240.0, CHROME_BAR_HEIGHT).into(),
        })
        .with_transparent(true)
        .with_html(ui_chrome::CHROME_HTML)
        .with_ipc_handler(move |req| {
            let _ = chrome_proxy.send_event(AppEvent::ChromeIpc(req.body().clone()));
        })
        .build()?;

    install_native_win32_window_features(&window, &chrome_wv);

    let next_tab_id = Arc::new(AtomicUsize::new(1));
    let mut tabs: Vec<BrowserTab> = Vec::new();
    let mut active_tab_idx: usize = 0;
    let mut drawer_open = false;
    let mut engine_menu_open = false;
    let mut blocked_count: usize = 0;
    let mut sniffed_streams: Vec<SniffedStreamItem> = Vec::new();
    let mut duplicate_alert: Option<DownloadTask> = None;
    let mut closed_tabs: Vec<SavedTab> = Vec::new();
    let mut find_current: usize = 0;
    let mut find_total: usize = 0;
    let mut had_active_downloads = false;

    let saved_session = BrowserStorage::load_session();
    let initial_urls: Vec<String> = if saved_session.is_empty() {
        vec!["shunno://newtab".to_string()]
    } else {
        saved_session.into_iter().map(|t| t.url).collect()
    };

    for u in initial_urls {
        let host = extract_host(&u);
        let pol = site_policies.get(&host).cloned().unwrap_or_default();
        let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
        if let Ok(wv) = create_tab_webview(&window, tid, &u, &pol, &global_settings, &ipc_token, proxy.clone()) {
            let init_title = if u == "shunno://newtab" {
                "New Tab".to_string()
            } else if host.is_empty() {
                u.clone()
            } else {
                host
            };
            tabs.push(BrowserTab {
                id: tid,
                title: init_title,
                url: u,
                cookie: None,
                incognito: false,
                pinned: false,
                playing_audio: false,
                muted: false,
                reader_active: false,
                zoom: 1.0,
                webview: wv,
            });
        }
    }

    // Periodic queue scheduler pump & UI progress refresh
    let ticker_proxy = proxy.clone();
    let ticker_engine = engine.clone();
    let ticker_rt = rt_handle.clone();
    rt_handle.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(750)).await;
            ticker_engine.pump_scheduler(&ticker_rt);
            let has_active_or_queued = ticker_engine
                .tasks
                .lock()
                .map(|l| {
                    l.iter().any(|t| {
                        t.status == "DOWNLOADING" || t.status == "MUXING" || t.status == "QUEUED"
                    })
                })
                .unwrap_or(false);
            let ff_busy = ticker_engine
                .ffmpeg_install_status
                .lock()
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if has_active_or_queued || ff_busy {
                let _ = ticker_proxy.send_event(AppEvent::RefreshChrome);
            }
        }
    });

    let push_chrome_state = |window: &Window,
                             chrome_wv: &WebView,
                             tabs: &[BrowserTab],
                             active_idx: usize,
                             site_policies: &HashMap<String, SitePolicy>,
                             global_settings: &GlobalSettings,
                             blocked_count: usize,
                             sniffed_streams: &[SniffedStreamItem],
                             bookmarks: &[BookmarkEntry],
                             history: &[HistoryEntry],
                             duplicate_alert: &Option<DownloadTask>,
                             has_closed_tabs: bool,
                             find_current: usize,
                             find_total: usize,
                             engine: &NativeDownloadEngine| {
        let active_url = tabs.get(active_idx).map(|t| t.url.clone()).unwrap_or_default();
        let active_zoom = tabs.get(active_idx).map(|t| t.zoom).unwrap_or(1.0);
        let active_reader = tabs.get(active_idx).map(|t| t.reader_active).unwrap_or(false);
        let active_host = extract_host(&active_url);
        let site = site_policies.get(&active_host).cloned().unwrap_or_default();
        let downloads = engine.tasks.lock().map(|l| l.clone()).unwrap_or_default();
        let scripts = list_user_scripts(&global_settings.approved_script_hashes);

        let ff_status = engine.ffmpeg_install_status.lock().map(|s| s.clone()).unwrap_or_default();
        let snap = ChromeSnapshot {
            version: APP_VERSION,
            ffmpeg_available: BrowserStorage::find_ffmpeg().is_some(),
            ffmpeg_status: ff_status,
            is_fullscreen: window.fullscreen().is_some(),
            has_closed_tabs,
            find_current,
            find_total,
            tabs: tabs
                .iter()
                .map(|t| TabSummary {
                    id: t.id,
                    title: t.title.clone(),
                    url: t.url.clone(),
                    incognito: t.incognito,
                    pinned: t.pinned,
                    playing_audio: t.playing_audio,
                    muted: t.muted,
                })
                .collect(),
            active_tab: active_idx,
            active_url,
            active_host,
            active_zoom,
            active_reader,
            site,
            global: global_settings.clone(),
            blocked_count,
            streams: sniffed_streams.to_vec(),
            bookmarks: bookmarks.to_vec(),
            history: history.to_vec(),
            userscripts: scripts,
            duplicate_alert: duplicate_alert.clone(),
            downloads,
        };

        if let Ok(json) = serde_json::to_string(&snap) {
            let js = format!("if (window.renderChromeState) window.renderChromeState({});", json);
            let _ = chrome_wv.evaluate_script(&js);
        }
    };

    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
            }

            Event::WindowEvent {
                event: WindowEvent::Focused(false),
                ..
            } => {
                if global_settings.auto_pip_on_blur {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let _ = tab.webview.evaluate_script("if (window.__shunnoTogglePiP) window.__shunnoTogglePiP();");
                    }
                }
            }

            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { event: key_ev, .. },
                ..
            } => {
                if key_ev.physical_key == KeyCode::F11 && key_ev.state == ElementState::Pressed {
                    let is_fs = window.fullscreen().is_some();
                    window.set_fullscreen(if is_fs { None } else { Some(Fullscreen::Borderless(None)) });
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                }
            }

            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                let to_save: Vec<SavedTab> = tabs
                    .iter()
                    .filter(|t| !t.incognito)
                    .map(|t| SavedTab {
                        title: t.title.clone(),
                        url: t.url.clone(),
                    })
                    .collect();
                BrowserStorage::save_session(&to_save);
                *control_flow = ControlFlow::Exit;
            }

            Event::UserEvent(AppEvent::RefreshChrome) => {
                let active_now = engine
                    .tasks
                    .lock()
                    .map(|l| l.iter().any(|t| t.status == "DOWNLOADING" || t.status == "MUXING" || t.status == "QUEUED"))
                    .unwrap_or(false);
                if active_now {
                    had_active_downloads = true;
                } else if had_active_downloads {
                    had_active_downloads = false;
                    match global_settings.on_queue_complete.as_str() {
                        "OPEN_FOLDER" => {
                            let dir = BrowserStorage::downloads_root_dir();
                            let _ = Command::new("explorer.exe").arg(dir).spawn();
                        }
                        "EXIT_APP" => {
                            *control_flow = ControlFlow::Exit;
                            return;
                        }
                        _ => {}
                    }
                }
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &duplicate_alert,
                    !closed_tabs.is_empty(),
                    find_current,
                    find_total,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::TabTitleChanged(tab_id, new_title)) => {
                if !new_title.trim().is_empty() {
                    if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                        tab.title = new_title;
                    }
                    push_chrome_state(
                        &chrome_wv,
                        &tabs,
                        active_tab_idx,
                        &site_policies,
                        &global_settings,
                        blocked_count,
                        &sniffed_streams,
                        &bookmarks,
                        &history,
                        &duplicate_alert,
                        &engine,
                    );
                }
            }

            Event::UserEvent(AppEvent::TabUrlChanged(tab_id, new_url)) => {
                if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.url = new_url.clone();
                    if tab.title.is_empty() || tab.title == "New Tab" || tab.title.starts_with("http") {
                        let h = extract_host(&new_url);
                        if !h.is_empty() {
                            tab.title = h;
                        }
                    }
                }
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &duplicate_alert,
                    !closed_tabs.is_empty(),
                    find_current,
                    find_total,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::TabZoomChanged(tab_id, new_zoom)) => {
                if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.zoom = new_zoom;
                }
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &duplicate_alert,
                    !closed_tabs.is_empty(),
                    find_current,
                    find_total,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::TabAudioChanged(tab_id, playing)) => {
                if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.playing_audio = playing;
                }
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &duplicate_alert,
                    !closed_tabs.is_empty(),
                    find_current,
                    find_total,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::NativeStreamCaptured(tab_id, stream_url)) => {
                if !sniffed_streams.iter().any(|s| s.url == stream_url) {
                    let name = stream_url
                        .split('?')
                        .next()
                        .and_then(|p| p.rsplit('/').next())
                        .unwrap_or("Media Stream")
                        .to_string();
                    sniffed_streams.insert(
                        0,
                        SniffedStreamItem {
                            url: stream_url.clone(),
                            name,
                            size_label: "Probing...".to_string(),
                        },
                    );
                    if sniffed_streams.len() > 25 {
                        sniffed_streams.truncate(25);
                    }
                    let tab_info = tabs
                        .iter()
                        .find(|t| t.id == tab_id)
                        .map(|t| (t.url.clone(), t.cookie.clone()));
                    let (ref_url, cookie_val) = match tab_info {
                        Some((u, c)) => (Some(u), c),
                        None => (None, None),
                    };
                    let eng = engine.clone();
                    let px = proxy.clone();
                    rt_handle.spawn(async move {
                        let res = eng.probe_stream_metadata(stream_url, ref_url, cookie_val).await;
                        let _ = px.send_event(AppEvent::StreamSizeResolved(tab_id, res));
                    });
                }
            }

            Event::UserEvent(AppEvent::StreamSizeResolved(tab_id, probe)) => {
                if let Some(item) = sniffed_streams.iter_mut().find(|s| s.url == probe.url) {
                    item.size_label = probe.size_label.clone();
                    if let Some(ref rname) = probe.resolved_name {
                        item.name = rname.clone();
                    }
                }
                if let Some(tab) = tabs.iter().find(|t| t.id == tab_id) {
                    if let Ok(json) = serde_json::to_string(&probe) {
                        let js = format!(
                            "if (window.__shunnoOnStreamProbed) window.__shunnoOnStreamProbed({});",
                            json
                        );
                        let _ = tab.webview.evaluate_script(&js);
                    }
                }
            }

            Event::UserEvent(AppEvent::DownloadIntercepted(dl_url)) => {
                let (ref_url, cookie_val) = tabs
                    .get(active_tab_idx)
                    .map(|t| (Some(t.url.clone()), t.cookie.clone()))
                    .unwrap_or((None, None));
                match engine.enqueue(&rt_handle, dl_url, global_settings.default_threads, ref_url, cookie_val, "CHECK", None) {
                    EnqueueOutcome::Started(_) => { duplicate_alert = None; }
                    EnqueueOutcome::DuplicateDetected(existing) => { duplicate_alert = Some(existing); }
                }
                drawer_open = true;
                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                let _ = chrome_wv.evaluate_script("toggleDrawer('downloads');");
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &duplicate_alert,
                    !closed_tabs.is_empty(),
                    find_current,
                    find_total,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::NewWindowBlockedOrOpened(popup_url)) => {
                let lower = popup_url.to_ascii_lowercase();
                let is_ad_popup = lower.contains("popads")
                    || lower.contains("popcash")
                    || lower.contains("doubleclick")
                    || lower.contains("propellerads");
                if global_settings.block_popups && is_ad_popup {
                    blocked_count += 1;
                } else if popup_url.starts_with("http") {
                    let clean = sanitize_url(&popup_url, &global_settings);
                    let host = extract_host(&clean);
                    let pol = site_policies.get(&host).cloned().unwrap_or_default();
                    let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                    if let Ok(wv) = create_tab_webview(&window, tid, &clean, &pol, &global_settings, &ipc_token, proxy.clone()) {
                        tabs.push(BrowserTab {
                            id: tid,
                            title: clean.clone(),
                            url: clean,
                            cookie: None,
                            incognito: false,
                            pinned: false,
                            playing_audio: false,
                            muted: false,
                            reader_active: false,
                            zoom: 1.0,
                            webview: wv,
                        });
                        active_tab_idx = tabs.len() - 1;
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                    }
                }
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &duplicate_alert,
                    !closed_tabs.is_empty(),
                    find_current,
                    find_total,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::ContentIpc(tab_id, raw_msg)) => {
                // Verify per-launch cryptographic IPC token before processing any message from content WebView
                let msg = if let Some(inner) = raw_msg.strip_prefix("INTERNAL|") {
                    inner
                } else if let Some((tok, body)) = raw_msg.split_once('|') {
                    if tok != ipc_token {
                        return;
                    }
                    body
                } else {
                    return;
                };

                if msg == "AD_BLOCKED" {
                    blocked_count += 1;
                } else if let Some(fc) = msg.strip_prefix("FIND_COUNT:") {
                    if let Some((cur_s, tot_s)) = fc.split_once('|') {
                        find_current = cur_s.parse().unwrap_or(0);
                        find_total = tot_s.parse().unwrap_or(0);
                    }
                } else if let Some(r_flag) = msg.strip_prefix("READER_STATE:") {
                    if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                        tab.reader_active = r_flag == "1";
                    }
                } else if let Some(aud_flag) = msg.strip_prefix("AUDIO_STATE:") {
                    if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                        tab.playing_audio = aud_flag == "1";
                    }
                } else if let Some(probe_json) = msg.strip_prefix("PROBE_STREAM:") {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(probe_json) {
                        let target_url = val.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let referer = val.get("referer").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let cookie = val.get("cookie").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
                        if target_url.starts_with("http") {
                            let eng = engine.clone();
                            let px = proxy.clone();
                            rt_handle.spawn(async move {
                                let res = eng.probe_stream_metadata(target_url, referer, cookie).await;
                                let _ = px.send_event(AppEvent::StreamSizeResolved(tab_id, res));
                            });
                        }
                    }
                } else if let Some(float_json) = msg.strip_prefix("FLOAT_DL:") {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(float_json) {
                        let dl_url = val.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let threads = val.get("threads").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
                        let referer = val.get("referer").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let cookie = val.get("cookie").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
                        if dl_url.starts_with("http") {
                            match engine.enqueue(&rt_handle, dl_url, threads, referer, cookie, "CHECK", None) {
                                EnqueueOutcome::Started(_) => { duplicate_alert = None; }
                                EnqueueOutcome::DuplicateDetected(existing) => { duplicate_alert = Some(existing); }
                            }
                            drawer_open = true;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                            let _ = chrome_wv.evaluate_script("toggleDrawer('downloads');");
                        }
                    }
                } else if let Some(key_cmd) = msg.strip_prefix("KEY:") {
                    match key_cmd {
                        "zoom_in" => {
                            if let Some(tab) = tabs.get_mut(active_tab_idx) {
                                tab.zoom = (tab.zoom + 0.1).min(3.0);
                                apply_tab_zoom(&tab.webview, tab.zoom);
                            }
                        }
                        "zoom_out" => {
                            if let Some(tab) = tabs.get_mut(active_tab_idx) {
                                tab.zoom = (tab.zoom - 0.1).max(0.4);
                                apply_tab_zoom(&tab.webview, tab.zoom);
                            }
                        }
                        "zoom_reset" => {
                            if let Some(tab) = tabs.get_mut(active_tab_idx) {
                                tab.zoom = 1.0;
                                apply_tab_zoom(&tab.webview, tab.zoom);
                            }
                        }
                        "f11" => {
                            let is_fs = window.fullscreen().is_some();
                            window.set_fullscreen(if is_fs { None } else { Some(Fullscreen::Borderless(None)) });
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                        }
                        "next_tab" => {
                            if !tabs.is_empty() {
                                active_tab_idx = (active_tab_idx + 1) % tabs.len();
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                            }
                        }
                        "prev_tab" => {
                            if !tabs.is_empty() {
                                active_tab_idx = if active_tab_idx == 0 { tabs.len() - 1 } else { active_tab_idx - 1 };
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                            }
                        }
                        "t" => {
                            let start = "shunno://newtab".to_string();
                            let pol = SitePolicy::default();
                            let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                            if let Ok(wv) = create_tab_webview(&window, tid, &start, &pol, &global_settings, &ipc_token, proxy.clone()) {
                                tabs.push(BrowserTab { id: tid, title: "New Tab".to_string(), url: start, cookie: None, incognito: false, pinned: false, playing_audio: false, muted: false, reader_active: false, zoom: 1.0, webview: wv });
                                active_tab_idx = tabs.len() - 1;
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                            }
                        }
                        "w" => {
                            if tabs.len() <= 1 {
                                BrowserStorage::save_session(&[]);
                                *control_flow = ControlFlow::Exit;
                                return;
                            } else if active_tab_idx < tabs.len() {
                                tabs.remove(active_tab_idx);
                                if active_tab_idx >= tabs.len() { active_tab_idx = tabs.len() - 1; }
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                            }
                        }
                        "j" => {
                            drawer_open = true;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                            let _ = chrome_wv.evaluate_script("toggleDrawer('downloads');");
                        }
                        _ => {}
                    }
                } else if let Some(html_payload) = msg.strip_prefix("OFFLINE_HTML:") {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(html_payload) {
                        let title = val.get("title").and_then(|v| v.as_str()).unwrap_or("snapshot");
                        let html = val.get("html").and_then(|v| v.as_str()).unwrap_or("");
                        let safe_name = downloader::sanitize_download_filename(&format!("{}.html", title.replace(' ', "_")));
                        let out = BrowserStorage::downloads_root_dir().join(&safe_name);
                        let _ = fs::write(out, html);
                    }
                } else if let Some(grab_json) = msg.strip_prefix("GRAB_RESULTS:") {
                    if let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(grab_json) {
                        for it in items.into_iter().rev() {
                            let u = it.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let nm = it.get("name").and_then(|v| v.as_str()).unwrap_or("Media Stream").to_string();
                            if u.starts_with("http") && !sniffed_streams.iter().any(|s| s.url == u) {
                                sniffed_streams.insert(
                                    0,
                                    SniffedStreamItem {
                                        url: u.clone(),
                                        name: nm,
                                        size_label: "Probing...".to_string(),
                                    },
                                );
                                let (ref_url, cookie_val) = tabs
                                    .iter()
                                    .find(|t| t.id == tab_id)
                                    .map(|t| (Some(t.url.clone()), t.cookie.clone()))
                                    .unwrap_or((None, None));
                                let eng = engine.clone();
                                let px = proxy.clone();
                                rt_handle.spawn(async move {
                                    let res = eng.probe_stream_metadata(u, ref_url, cookie_val).await;
                                    let _ = px.send_event(AppEvent::StreamSizeResolved(tab_id, res));
                                });
                            }
                        }
                        if sniffed_streams.len() > 40 {
                            sniffed_streams.truncate(40);
                        }
                    }
                    drawer_open = true;
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                    let _ = chrome_wv.evaluate_script("toggleDrawer('streams');");
                } else if let Some(stream_url) = msg.strip_prefix("STREAM:") {
                    if !sniffed_streams.iter().any(|s| s.url == stream_url) {
                        let name = stream_url
                            .split('?')
                            .next()
                            .and_then(|p| p.rsplit('/').next())
                            .unwrap_or("Media Stream")
                            .to_string();
                        sniffed_streams.insert(
                            0,
                            SniffedStreamItem {
                                url: stream_url.to_string(),
                                name,
                                size_label: "Probing...".to_string(),
                            },
                        );
                        if sniffed_streams.len() > 25 {
                            sniffed_streams.truncate(25);
                        }
                    }
                } else if let Some(meta_json) = msg.strip_prefix("META:") {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(meta_json) {
                        let title = val.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let url = val.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let ck = val.get("cookie").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
                        if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                            if ck.is_some() {
                                tab.cookie = ck;
                            }
                            if !title.is_empty() {
                                tab.title = title.clone();
                            }
                            if !url.is_empty() {
                                tab.url = url.clone();
                                if !tab.incognito {
                                    let now = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .map(|d| d.as_secs())
                                        .unwrap_or(0);
                                    history.insert(
                                        0,
                                        HistoryEntry {
                                            title: if title.is_empty() { url.clone() } else { title },
                                            url,
                                            visited_at: now,
                                        },
                                    );
                                    BrowserStorage::save_history(&history);
                                }
                            }
                        }
                    }
                }
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &duplicate_alert,
                    !closed_tabs.is_empty(),
                    find_current,
                    find_total,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::ChromeIpc(cmd)) => {
                if cmd == "WIN_DRAG" {
                    let _ = window.drag_window();
                } else if cmd == "WIN_MINIMIZE" {
                    window.set_minimized(true);
                } else if cmd == "WIN_MAXIMIZE" {
                    window.set_maximized(!window.is_maximized());
                } else if cmd == "WIN_CLOSE" {
                    let to_save: Vec<SavedTab> = tabs
                        .iter()
                        .filter(|t| !t.incognito)
                        .map(|t| SavedTab {
                            title: t.title.clone(),
                            url: t.url.clone(),
                        })
                        .collect();
                    BrowserStorage::save_session(&to_save);
                    *control_flow = ControlFlow::Exit;
                    return;
                } else if cmd == "ZOOM_IN" || cmd == "ZOOM_OUT" || cmd == "ZOOM_RESET" {
                    if let Some(tab) = tabs.get_mut(active_tab_idx) {
                        if cmd == "ZOOM_IN" {
                            tab.zoom = (tab.zoom + 0.1).min(3.0);
                        } else if cmd == "ZOOM_OUT" {
                            tab.zoom = (tab.zoom - 0.1).max(0.4);
                        } else {
                            tab.zoom = 1.0;
                        }
                        apply_tab_zoom(&tab.webview, tab.zoom);
                    }
                } else if let Some(payload) = cmd.strip_prefix("ADD_SHORTCUT:") {
                    if let Some((t, u)) = payload.split_once('|') {
                        let clean_u = sanitize_url(u, &global_settings);
                        if !t.trim().is_empty() && clean_u.starts_with("http") {
                            global_settings.quick_shortcuts.push(state::QuickShortcut {
                                title: t.trim().to_string(),
                                url: clean_u,
                            });
                            BrowserStorage::save_global_settings(&global_settings);
                        }
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("DEL_SHORTCUT:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < global_settings.quick_shortcuts.len() {
                            global_settings.quick_shortcuts.remove(idx);
                            BrowserStorage::save_global_settings(&global_settings);
                        }
                    }
                } else if cmd == "TOGGLE_FULLSCREEN" {
                    let is_fs = window.fullscreen().is_some();
                    window.set_fullscreen(if is_fs { None } else { Some(Fullscreen::Borderless(None)) });
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                } else if cmd.starts_with("DRAWER_OPEN:") {
                    drawer_open = true;
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                } else if cmd == "DRAWER_CLOSE" {
                    drawer_open = false;
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                } else if cmd == "TRIGGER_PIP" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let _ = tab.webview.evaluate_script("if (window.__shunnoTogglePiP) window.__shunnoTogglePiP();");
                    }
                } else if cmd == "BACK" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let _ = tab.webview.evaluate_script("history.back();");
                    }
                } else if cmd == "FORWARD" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let _ = tab.webview.evaluate_script("history.forward();");
                    }
                } else if cmd == "RELOAD" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let _ = tab.webview.evaluate_script("location.reload();");
                    }
                } else if let Some(raw_nav) = cmd.strip_prefix("NAV:") {
                    let clean = sanitize_url(raw_nav, &global_settings);
                    sniffed_streams.clear();
                    if let Some(tab) = tabs.get_mut(active_tab_idx) {
                        tab.url = clean.clone();
                        if clean == "shunno://newtab" {
                            tab.title = "New Tab".to_string();
                            let _ = tab.webview.load_url("about:blank");
                        } else {
                            let h = extract_host(&clean);
                            if !h.is_empty() {
                                tab.title = h;
                            }
                            tab.playing_audio = false;
                            let _ = tab.webview.load_url(&clean);
                        }
                    }
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                } else if cmd == "NEW_TAB" || cmd == "NEW_INCOGNITO" {
                    let is_incog = cmd == "NEW_INCOGNITO";
                    let start = "shunno://newtab".to_string();
                    let pol = SitePolicy::default();
                    let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                    if let Ok(wv) = create_tab_webview(&window, tid, &start, &pol, &global_settings, &ipc_token, proxy.clone()) {
                        tabs.push(BrowserTab {
                            id: tid,
                            title: if is_incog { "Private Tab".to_string() } else { "New Tab".to_string() },
                            url: start,
                            cookie: None,
                            incognito: is_incog,
                            pinned: false,
                            playing_audio: false,
                            muted: false,
                            reader_active: false,
                            zoom: 1.0,
                            webview: wv,
                        });
                        active_tab_idx = tabs.len() - 1;
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                    }
                } else if cmd == "NEXT_TAB" {
                    if !tabs.is_empty() {
                        active_tab_idx = (active_tab_idx + 1) % tabs.len();
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                    }
                } else if cmd == "PREV_TAB" {
                    if !tabs.is_empty() {
                        active_tab_idx = if active_tab_idx == 0 { tabs.len() - 1 } else { active_tab_idx - 1 };
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                    }
                } else if cmd == "REFRESH_UI" {
                    // Triggers immediate push_chrome_state at the bottom of ChromeIpc
                } else if let Some(payload) = cmd.strip_prefix("REORDER_TAB:") {
                    if let Some((from_s, to_s)) = payload.split_once('|') {
                        if let (Ok(from), Ok(to)) = (from_s.parse::<usize>(), to_s.parse::<usize>()) {
                            if from < tabs.len() && to < tabs.len() && from != to {
                                let moved = tabs.remove(from);
                                tabs.insert(to, moved);
                                if active_tab_idx == from {
                                    active_tab_idx = to;
                                } else if from < active_tab_idx && active_tab_idx <= to {
                                    active_tab_idx -= 1;
                                } else if to <= active_tab_idx && active_tab_idx < from {
                                    active_tab_idx += 1;
                                }
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                            }
                        }
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("SWITCH_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < tabs.len() {
                            active_tab_idx = idx;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                        }
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("TOGGLE_MUTE_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if let Some(tab) = tabs.get_mut(idx) {
                            tab.muted = !tab.muted;
                            set_webview_muted(&tab.webview, tab.muted);
                        }
                    }
                } else if cmd == "CLOSE_ACTIVE_TAB" {
                    if tabs.len() <= 1 {
                        BrowserStorage::save_session(&[]);
                        *control_flow = ControlFlow::Exit;
                        return;
                    } else if active_tab_idx < tabs.len() {
                        let removed = tabs.remove(active_tab_idx);
                        if !removed.incognito && removed.url != "shunno://newtab" {
                            closed_tabs.push(SavedTab { title: removed.title, url: removed.url });
                        }
                        if active_tab_idx >= tabs.len() {
                            active_tab_idx = tabs.len() - 1;
                        }
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("CLOSE_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if tabs.len() <= 1 {
                            BrowserStorage::save_session(&[]);
                            *control_flow = ControlFlow::Exit;
                            return;
                        } else if idx < tabs.len() {
                            let removed = tabs.remove(idx);
                            if !removed.incognito && removed.url != "shunno://newtab" {
                                closed_tabs.push(SavedTab { title: removed.title, url: removed.url });
                            }
                            if active_tab_idx >= tabs.len() {
                                active_tab_idx = tabs.len() - 1;
                            }
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                        }
                    }
                } else if cmd == "REOPEN_CLOSED_TAB" {
                    if let Some(saved) = closed_tabs.pop() {
                        let host = extract_host(&saved.url);
                        let pol = site_policies.get(&host).cloned().unwrap_or_default();
                        let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                        if let Ok(wv) = create_tab_webview(&window, tid, &saved.url, &pol, &global_settings, &ipc_token, proxy.clone()) {
                            tabs.push(BrowserTab {
                                id: tid,
                                title: saved.title,
                                url: saved.url,
                                cookie: None,
                                incognito: false,
                                pinned: false,
                                playing_audio: false,
                                muted: false,
                                reader_active: false,
                                zoom: 1.0,
                                webview: wv,
                            });
                            active_tab_idx = tabs.len() - 1;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                        }
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("TOGGLE_PIN_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < tabs.len() {
                            tabs[idx].pinned = !tabs[idx].pinned;
                            let tab_item = tabs.remove(idx);
                            let target_idx = if tab_item.pinned {
                                tabs.iter().take_while(|t| t.pinned).count()
                            } else {
                                tabs.len()
                            };
                            tabs.insert(target_idx, tab_item);
                            active_tab_idx = target_idx;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                        }
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("SET_SEARCH_ENGINE:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < global_settings.search_engines.len() {
                            global_settings.active_search_engine = idx;
                            BrowserStorage::save_global_settings(&global_settings);
                        }
                    }
                } else if let Some(payload) = cmd.strip_prefix("ADD_SEARCH_ENGINE:") {
                    if let Some((name, qurl)) = payload.split_once('|') {
                        let clean_name = name.trim().to_string();
                        let mut clean_url = qurl.trim().to_string();
                        if !clean_name.is_empty() && clean_url.starts_with("http") {
                            if !clean_url.contains("%s") {
                                clean_url.push_str("%s");
                            }
                            global_settings.search_engines.push(SearchEngine {
                                name: clean_name,
                                query_url: clean_url,
                            });
                            global_settings.active_search_engine = global_settings.search_engines.len() - 1;
                            BrowserStorage::save_global_settings(&global_settings);
                        }
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("DEL_SEARCH_ENGINE:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if global_settings.search_engines.len() > 1 && idx < global_settings.search_engines.len() {
                            global_settings.search_engines.remove(idx);
                            if global_settings.active_search_engine >= global_settings.search_engines.len() {
                                global_settings.active_search_engine = 0;
                            }
                            BrowserStorage::save_global_settings(&global_settings);
                        }
                    }
                } else if let Some(sha) = cmd.strip_prefix("TOGGLE_SCRIPT:") {
                    if let Some(pos) = global_settings.approved_script_hashes.iter().position(|h| h == sha) {
                        global_settings.approved_script_hashes.remove(pos);
                    } else {
                        global_settings.approved_script_hashes.push(sha.to_string());
                    }
                    BrowserStorage::save_global_settings(&global_settings);
                } else if let Some(gkey) = cmd.strip_prefix("TOGGLE_GLOBAL:") {
                    let mut reload_active = false;
                    match gkey {
                        "full_dark" => {
                            global_settings.full_dark_mode = !global_settings.full_dark_mode;
                            reload_active = true;
                        }
                        "drop_basket" => {
                            global_settings.drop_basket_enabled = !global_settings.drop_basket_enabled;
                            reload_active = true;
                        }
                        "bg_audio" => global_settings.background_media_play = !global_settings.background_media_play,
                        "auto_pip" => global_settings.auto_pip_on_blur = !global_settings.auto_pip_on_blur,
                        "auto_clean" => {
                            global_settings.auto_clean_completed = !global_settings.auto_clean_completed;
                            engine.auto_clean_completed.store(global_settings.auto_clean_completed, Ordering::SeqCst);
                        }
                        "strip_tracking" => global_settings.strip_tracking_params = !global_settings.strip_tracking_params,
                        "block_popups" => global_settings.block_popups = !global_settings.block_popups,
                        _ => {}
                    }
                    BrowserStorage::save_global_settings(&global_settings);

                    if reload_active {
                        if let Some(tab) = tabs.get(active_tab_idx) {
                            let host = extract_host(&tab.url);
                            let pol = site_policies.get(&host).cloned().unwrap_or_default();
                            let current_url = tab.url.clone();
                            let prev_cookie = tab.cookie.clone();
                            let is_incog = tab.incognito;
                            let is_muted = tab.muted;
                            let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                            if let Ok(new_wv) = create_tab_webview(&window, tid, &current_url, &pol, &global_settings, &ipc_token, proxy.clone()) {
                                if is_muted { set_webview_muted(&new_wv, true); }
                                tabs[active_tab_idx] = BrowserTab {
                                    id: tid,
                                    title: tab.title.clone(),
                                    url: current_url,
                                    cookie: prev_cookie,
                                    incognito: is_incog,
                                    pinned: tab.pinned,
                                    playing_audio: false,
                                    muted: is_muted,
                                    reader_active: false,
                                    zoom: tab.zoom,
                                    webview: new_wv,
                                };
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                            }
                        }
                    }
                } else if let Some(skey) = cmd.strip_prefix("TOGGLE_SITE:") {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let host = extract_host(&tab.url);
                        let pol = site_policies.entry(host.clone()).or_default();
                        match skey {
                            "adblock" => pol.adblock = !pol.adblock,
                            "js_enabled" => pol.js_enabled = !pol.js_enabled,
                            "images_enabled" => pol.images_enabled = !pol.images_enabled,
                            "desktop_ua" => pol.desktop_ua = !pol.desktop_ua,
                            "sponsorblock" => pol.sponsorblock = !pol.sponsorblock,
                            "auto_reject_cookies" => pol.auto_reject_cookies = !pol.auto_reject_cookies,
                            _ => {}
                        }
                        let updated_pol = pol.clone();
                        BrowserStorage::save_site_policies(&site_policies);

                        let current_url = tab.url.clone();
                        let prev_cookie = tab.cookie.clone();
                        let is_incog = tab.incognito;
                        let is_muted = tab.muted;
                        let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                        if let Ok(new_wv) = create_tab_webview(&window, tid, &current_url, &updated_pol, &global_settings, &ipc_token, proxy.clone()) {
                            if is_muted { set_webview_muted(&new_wv, true); }
                            if (tab.zoom - 1.0).abs() > f64::EPSILON { apply_tab_zoom(&new_wv, tab.zoom); }
                            tabs[active_tab_idx] = BrowserTab {
                                id: tid,
                                title: tab.title.clone(),
                                url: current_url,
                                cookie: prev_cookie,
                                incognito: is_incog,
                                pinned: tab.pinned,
                                playing_audio: false,
                                muted: is_muted,
                                reader_active: false,
                                zoom: tab.zoom,
                                webview: new_wv,
                            };
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                        }
                    }
                } else if let Some(spd) = cmd.strip_prefix("SET_PLAYBACK_SPEED:") {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let js = format!("document.querySelectorAll('video,audio').forEach(m => m.playbackRate = {});", spd);
                        let _ = tab.webview.evaluate_script(&js);
                    }
                } else if cmd == "READER_MODE" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let _ = tab.webview.evaluate_script("if (window.__shunnoToggleReader) window.__shunnoToggleReader();");
                    }
                } else if let Some(em_flag) = cmd.strip_prefix("ENGINE_MENU:") {
                    engine_menu_open = em_flag == "1";
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open, engine_menu_open);
                } else if cmd == "INSTALL_FFMPEG" {
                    engine.install_ffmpeg_bundle(&rt_handle);
                } else if let Some(mc_str) = cmd.strip_prefix("SET_MAX_CONCURRENT:") {
                    if let Ok(mc) = mc_str.parse::<usize>() {
                        global_settings.max_concurrent_downloads = mc.clamp(1, 8);
                        engine.max_concurrent.store(global_settings.max_concurrent_downloads, Ordering::SeqCst);
                        BrowserStorage::save_global_settings(&global_settings);
                        engine.pump_scheduler(&rt_handle);
                    }
                } else if let Some(oc_str) = cmd.strip_prefix("SET_ON_COMPLETE:") {
                    global_settings.on_queue_complete = oc_str.trim().to_string();
                    BrowserStorage::save_global_settings(&global_settings);
                } else if let Some(query) = cmd.strip_prefix("FIND:") {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let escaped = query.replace('\\', "\\\\").replace('\'', "\\'");
                        let js = format!("if (window.__shunnoFindOnPage) window.__shunnoFindOnPage('{}', 'next');", escaped);
                        let _ = tab.webview.evaluate_script(&js);
                    }
                } else if cmd == "FIND_NEXT" || cmd == "FIND_PREV" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let dir = if cmd == "FIND_PREV" { "prev" } else { "next" };
                        let js = format!("if (window.__shunnoFindOnPage) window.__shunnoFindOnPage(window.__shunnoFindLastQuery || '', '{}');", dir);
                        let _ = tab.webview.evaluate_script(&js);
                    }
                } else if cmd == "ADD_BOOKMARK" || cmd.starts_with("ADD_BOOKMARK:") {
                    let folder_name = cmd
                        .strip_prefix("ADD_BOOKMARK:")
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .unwrap_or("General")
                        .to_string();
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        if tab.url != "shunno://newtab" && !bookmarks.iter().any(|b| b.url == tab.url) {
                            let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                            bookmarks.insert(0, BookmarkEntry {
                                title: tab.title.clone(),
                                url: tab.url.clone(),
                                folder: folder_name,
                                created_at: now,
                            });
                            BrowserStorage::save_bookmarks(&bookmarks);
                        }
                    }
                } else if let Some(payload) = cmd.strip_prefix("SET_BOOKMARK_FOLDER:") {
                    if let Some((idx_str, new_folder)) = payload.split_once('|') {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if idx < bookmarks.len() && !new_folder.trim().is_empty() {
                                bookmarks[idx].folder = new_folder.trim().to_string();
                                BrowserStorage::save_bookmarks(&bookmarks);
                            }
                        }
                    }
                } else if let Some(payload) = cmd.strip_prefix("RENAME_BOOKMARK:") {
                    if let Some((idx_str, new_title)) = payload.split_once('|') {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if idx < bookmarks.len() && !new_title.trim().is_empty() {
                                bookmarks[idx].title = new_title.trim().to_string();
                                BrowserStorage::save_bookmarks(&bookmarks);
                            }
                        }
                    }

                } else if let Some(idx_str) = cmd.strip_prefix("DEL_BOOKMARK:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < bookmarks.len() {
                            bookmarks.remove(idx);
                            BrowserStorage::save_bookmarks(&bookmarks);
                        }
                    }
                } else if cmd == "EXPORT_TXT_BOOKMARKS" {
                    let _ = BrowserStorage::export_portable_txt_bookmarks(&bookmarks);
                } else if cmd == "IMPORT_TXT_BOOKMARKS" {
                    let _ = BrowserStorage::import_portable_txt_bookmarks(&mut bookmarks);
                } else if cmd == "CLEAR_HISTORY" {
                    history.clear();
                    BrowserStorage::save_history(&history);
                } else if cmd == "SAVE_OFFLINE_HTML" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let js = format!(
                            "if (window.ipc) window.ipc.postMessage('{}|OFFLINE_HTML:' + JSON.stringify({{ title: document.title, html: '<!DOCTYPE html>' + document.documentElement.outerHTML }}));",
                            ipc_token
                        );
                        let _ = tab.webview.evaluate_script(&js);
                    }
                } else if cmd == "OPEN_SCRIPTS_FOLDER" {
                    let dir = user_scripts_dir();
                    let _ = Command::new("explorer.exe").arg(dir).spawn();
                } else if cmd == "OPEN_BIN_FOLDER" {
                    let dir = BrowserStorage::ffmpeg_bin_dir();
                    let _ = Command::new("explorer.exe").arg(dir).spawn();
                } else if cmd == "OPEN_DOWNLOADS_FOLDER" {
                    let dir = BrowserStorage::downloads_root_dir();
                    let _ = Command::new("explorer.exe").arg(dir).spawn();
                } else if cmd == "GRAB_PAGE_LINKS" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let _ = tab.webview.evaluate_script("if (window.__shunnoGrabAllPageLinks) window.__shunnoGrabAllPageLinks();");
                    }
                } else if cmd == "CLEAR_COMPLETED_DL" {
                    engine.clean_stale_and_completed();
                } else if cmd == "DISMISS_DUP" {
                    duplicate_alert = None;
                } else if let Some(ren_url) = cmd.strip_prefix("DUP_RENAME:") {
                    duplicate_alert = None;
                    let (ref_url, cookie_val) = tabs
                        .get(active_tab_idx)
                        .map(|t| (Some(t.url.clone()), t.cookie.clone()))
                        .unwrap_or((None, None));
                    let _ = engine.enqueue(&rt_handle, ren_url.to_string(), global_settings.default_threads, ref_url, cookie_val, "RENAME", None);
                } else if let Some(ov_url) = cmd.strip_prefix("DUP_OVERWRITE:") {
                    duplicate_alert = None;
                    let (ref_url, cookie_val) = tabs
                        .get(active_tab_idx)
                        .map(|t| (Some(t.url.clone()), t.cookie.clone()))
                        .unwrap_or((None, None));
                    let _ = engine.enqueue(&rt_handle, ov_url.to_string(), global_settings.default_threads, ref_url, cookie_val, "OVERWRITE", None);
                } else if let Some(payload) = cmd.strip_prefix("ENQUEUE_SCHED:") {
                    let parts: Vec<&str> = payload.splitn(3, '|').collect();
                    if parts.len() == 3 {
                        let threads = parts[0].parse::<usize>().unwrap_or(16);
                        let delay_s = parts[1].parse::<u64>().ok().filter(|&d| d > 0);
                        let raw_urls = parts[2];
                        let (ref_url, cookie_val) = tabs
                            .get(active_tab_idx)
                            .map(|t| (Some(t.url.clone()), t.cookie.clone()))
                            .unwrap_or((None, None));
                        for line in raw_urls.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
                            match engine.enqueue(&rt_handle, line.to_string(), threads, ref_url.clone(), cookie_val.clone(), "CHECK", delay_s) {
                                EnqueueOutcome::Started(_) => {}
                                EnqueueOutcome::DuplicateDetected(ex) => { duplicate_alert = Some(ex); }
                            }
                        }
                    }
                } else if let Some(limit_str) = cmd.strip_prefix("SET_SPEED_LIMIT:") {
                    if let Ok(bps) = limit_str.parse::<u64>() {
                        global_settings.speed_limit_bps = bps;
                        engine.speed_limit_bps.store(bps, Ordering::SeqCst);
                        BrowserStorage::save_global_settings(&global_settings);
                    }
                } else if let Some(payload) = cmd.strip_prefix("MUX_DASH:") {
                    if let Some((v_url, a_url)) = payload.split_once('|') {
                        let (ref_url, cookie_val, title) = tabs
                            .get(active_tab_idx)
                            .map(|t| (Some(t.url.clone()), t.cookie.clone(), t.title.clone()))
                            .unwrap_or((None, None, "Shunno_Muxed_Video".to_string()));
                        engine.enqueue_dash_mux(
                            &rt_handle,
                            v_url.trim().to_string(),
                            a_url.trim().to_string(),
                            title,
                            ref_url,
                            cookie_val,
                        );
                    }
                } else if let Some(payload) = cmd.strip_prefix("ENQUEUE_DL:") {
                    if let Some((threads_str, raw_urls)) = payload.split_once('|') {
                        let threads = threads_str.parse::<usize>().unwrap_or(16);
                        let (ref_url, cookie_val) = tabs
                            .get(active_tab_idx)
                            .map(|t| (Some(t.url.clone()), t.cookie.clone()))
                            .unwrap_or((None, None));
                        let re = Regex::new(r"\[(\d+)-(\d+)\]").unwrap();
                        for line in raw_urls.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
                            if let Some(caps) = re.captures(line) {
                                let s_str = caps.get(1).map(|m| m.as_str()).unwrap_or("1");
                                let e_str = caps.get(2).map(|m| m.as_str()).unwrap_or("1");
                                if let (Ok(start), Ok(end)) = (s_str.parse::<u32>(), e_str.parse::<u32>()) {
                                    let width = s_str.len();
                                    let full_m = caps.get(0).map(|m| m.as_str()).unwrap_or("");
                                    for i in start..=end.min(start + 100) {
                                        let num = format!("{:0width$}", i, width = width);
                                        let expanded = line.replace(full_m, &num);
                                        match engine.enqueue(&rt_handle, expanded, threads, ref_url.clone(), cookie_val.clone(), "CHECK", None) {
                                            EnqueueOutcome::Started(_) => {}
                                            EnqueueOutcome::DuplicateDetected(ex) => { duplicate_alert = Some(ex); }
                                        }
                                    }
                                    continue;
                                }
                            }
                            match engine.enqueue(&rt_handle, line.to_string(), threads, ref_url.clone(), cookie_val.clone(), "CHECK", None) {
                                EnqueueOutcome::Started(_) => {}
                                EnqueueOutcome::DuplicateDetected(ex) => { duplicate_alert = Some(ex); }
                            }
                        }
                    }
                } else if let Some(id_str) = cmd.strip_prefix("DL_PAUSE:") {
                    if let Ok(id) = id_str.parse::<u64>() {
                        engine.pause(id);
                    }
                } else if let Some(id_str) = cmd.strip_prefix("DL_RESUME:") {
                    if let Ok(id) = id_str.parse::<u64>() {
                        engine.resume_or_restart(&rt_handle, id, None);
                    }
                } else if let Some(payload) = cmd.strip_prefix("DL_FIX:") {
                    if let Some((id_str, new_url)) = payload.split_once('|') {
                        if let Ok(id) = id_str.parse::<u64>() {
                            engine.resume_or_restart(&rt_handle, id, Some(new_url.to_string()));
                        }
                    }
                } else if let Some(id_str) = cmd.strip_prefix("DL_DEL:") {
                    if let Ok(id) = id_str.parse::<u64>() {
                        engine.delete(id);
                    }
                } else if let Some(path) = cmd.strip_prefix("DL_SHOW:") {
                    if !path.is_empty() {
                        let _ = Command::new("explorer.exe").arg(format!("/select,{}", path)).spawn();
                    }
                }

                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &duplicate_alert,
                    !closed_tabs.is_empty(),
                    find_current,
                    find_total,
                    &engine,
                );
            }

            _ => {}
        }
    });
}
