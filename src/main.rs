#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod downloader;
mod sanitizer;
mod state;
mod ui_chrome;
mod userscripts;

use downloader::{DownloadTask, EnqueueOutcome, NativeDownloadEngine, StreamProbeResult};
use rand::Rng;
use sanitizer::{extract_host, sanitize_url};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{ElementState, Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::keyboard::KeyCode;
#[cfg(not(windows))]
use tao::window::Fullscreen;
use tao::window::{Icon, Window, WindowBuilder};
use userscripts::{list_user_scripts, load_approved_user_scripts, user_scripts_dir, UserScriptInfo};
use wry::{Rect, WebContext, WebView, WebViewBuilder};

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
    NativeDownloadStarted {
        id: u64,
        url: String,
        file_name: String,
        saved_path: String,
        total_bytes: i64,
    },
    NativeDownloadProgress {
        id: u64,
        received: u64,
        total: i64,
        speed_bps: u64,
    },
    NativeDownloadFinished {
        id: u64,
        ok: bool,
        final_path: Option<String>,
        err_msg: Option<String>,
    },
    /// WebView2 download paused by the user, or interrupted but resumable.
    NativeDownloadPaused {
        id: u64,
        reason: Option<String>,
    },
    /// Raised by the native ad filter. Never derived from page-supplied IPC text.
    AdBlocked,
    DownloadWithCookies {
        url: String,
        threads: usize,
        referer: Option<String>,
        cookie: Option<String>,
        dup_mode: String,
        delay_secs: Option<u64>,
        open_drawer: bool,
    },
    /// (opener tab id, url)
    NewWindowBlockedOrOpened(usize, String),
    NativeStreamCaptured(usize, String, String, String),
    StreamSizeResolved(usize, StreamProbeResult),
    TabAudioChanged(usize, bool),
    TabTitleChanged(usize, String),
    TabUrlChanged(usize, String),
    TabZoomChanged(usize, f64),
    ToggleFullscreen,
    SleepCheck,
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
    sleeping: bool,
    last_active_secs: u64,
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
    sleeping: bool,
}

#[derive(Serialize)]
struct ChromeSnapshot {
    version: &'static str,
    ffmpeg_available: bool,
    ffmpeg_status: String,
    ytdlp_status: String,
    is_fullscreen: bool,
    has_closed_tabs: bool,
    find_current: usize,
    find_total: usize,
    tabs: Vec<TabSummary>,
    active_tab: usize,
    split_tab: Option<usize>,
    active_url: String,
    active_host: String,
    active_zoom: f64,
    active_reader: bool,
    site_data_cleared: bool,
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

fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// A page may only make the native engine contact private/LAN hosts when the page itself is on that host.
fn page_may_target(page_url: Option<&str>, target: &str) -> bool {
    if sanitizer::is_public_http_url(target) {
        return true;
    }
    let target_host = extract_host(target);
    !target_host.is_empty() && page_url.map(|p| extract_host(p) == target_host).unwrap_or(false)
}

// Live WebView2 download operations, keyed by Shunno task id. WebView2 raises all of its
// events on the UI thread, which is also the event loop thread, so a thread_local is sufficient.
#[cfg(windows)]
thread_local! {
    static NATIVE_OPS: std::cell::RefCell<HashMap<u64, webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2DownloadOperation>> =
        std::cell::RefCell::new(HashMap::new());
}

#[cfg(windows)]
fn native_op_call(id: u64, action: &str) -> bool {
    NATIVE_OPS.with(|ops| {
        let mut ops = ops.borrow_mut();
        let Some(op) = ops.get(&id).cloned() else {
            return false;
        };
        let ok = unsafe {
            match action {
                "pause" => op.Pause().is_ok(),
                "resume" => op.Resume().is_ok(),
                "cancel" => op.Cancel().is_ok(),
                _ => false,
            }
        };
        if action == "cancel" {
            ops.remove(&id);
        }
        ok
    })
}

#[cfg(not(windows))]
fn native_op_call(_id: u64, _action: &str) -> bool {
    false
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
fn set_explicit_app_user_model_id() {
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(windows::core::w!("Shunno.Browser.Desktop"));
    }
}

#[cfg(not(windows))]
fn set_explicit_app_user_model_id() {}

fn load_embedded_window_icon() -> Option<Icon> {
    const ICO_BYTES: &[u8] = include_bytes!("../assets/app.ico");
    if ICO_BYTES.len() < 22 {
        return None;
    }
    let count = u16::from_le_bytes([ICO_BYTES[4], ICO_BYTES[5]]) as usize;
    for target_size in [48u32, 32u32, 24u32] {
        for i in 0..count {
            let off = 6 + i * 16;
            if off + 16 > ICO_BYTES.len() {
                break;
            }
            let w = ICO_BYTES[off] as u32;
            let h = ICO_BYTES[off + 1] as u32;
            let bytes_in_res = u32::from_le_bytes([
                ICO_BYTES[off + 8],
                ICO_BYTES[off + 9],
                ICO_BYTES[off + 10],
                ICO_BYTES[off + 11],
            ]) as usize;
            let img_offset = u32::from_le_bytes([
                ICO_BYTES[off + 12],
                ICO_BYTES[off + 13],
                ICO_BYTES[off + 14],
                ICO_BYTES[off + 15],
            ]) as usize;

            if w == target_size && h == target_size && img_offset + 40 + (w * h * 4) as usize <= ICO_BYTES.len() && bytes_in_res >= 40 {
                let pixel_start = img_offset + 40;
                let row_bytes = (w * 4) as usize;
                let mut rgba = vec![0u8; (w * h * 4) as usize];
                for y in 0..(h as usize) {
                    let src_row = pixel_start + ((h as usize - 1 - y) * row_bytes);
                    let dst_row = y * row_bytes;
                    for x in 0..(w as usize) {
                        let s = src_row + x * 4;
                        let d = dst_row + x * 4;
                        rgba[d] = ICO_BYTES[s + 2];     // R
                        rgba[d + 1] = ICO_BYTES[s + 1]; // G
                        rgba[d + 2] = ICO_BYTES[s];     // B
                        rgba[d + 3] = ICO_BYTES[s + 3]; // A
                    }
                }
                if let Ok(icon) = Icon::from_rgba(rgba, w, h) {
                    return Some(icon);
                }
            }
        }
    }
    None
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(windows)]
fn suspend_tab_webview(webview: &WebView) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
    use webview2_com::TrySuspendCompletedHandler;
    use windows::core::Interface;
    use wry::WebViewExtWindows;

    let _ = webview.set_visible(false);
    let controller = webview.controller();
    unsafe {
        if let Ok(core) = controller.CoreWebView2() {
            if let Ok(core3) = core.cast::<ICoreWebView2_3>() {
                let handler = TrySuspendCompletedHandler::create(Box::new(move |_res, _ok| Ok(())));
                let _ = core3.TrySuspend(&handler);
            }
        }
    }
}

#[cfg(not(windows))]
fn suspend_tab_webview(_webview: &WebView) {}

#[cfg(windows)]
fn resume_tab_webview(webview: &WebView) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
    use windows::core::Interface;
    use wry::WebViewExtWindows;

    let controller = webview.controller();
    unsafe {
        if let Ok(core) = controller.CoreWebView2() {
            if let Ok(core3) = core.cast::<ICoreWebView2_3>() {
                let _ = core3.Resume();
            }
        }
    }
}

#[cfg(not(windows))]
fn resume_tab_webview(_webview: &WebView) {}

#[cfg(windows)]
fn snap_window_to_fraction(window: &Window, x_f: i32, y_f: i32, w_f: i32, h_f: i32) {
    use tao::platform::windows::WindowExtWindows;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, ShowWindow, SWP_NOZORDER, SW_RESTORE};

    let hwnd = HWND(window.hwnd() as isize);
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(hmon, &mut mi).as_bool() {
            let work = mi.rcWork;
            let total_w = work.right - work.left;
            let total_h = work.bottom - work.top;
            let nx = work.left + (total_w * x_f) / 1000;
            let ny = work.top + (total_h * y_f) / 1000;
            let nw = (total_w * w_f) / 1000;
            let nh = (total_h * h_f) / 1000;
            let _ = SetWindowPos(hwnd, HWND(0), nx, ny, nw, nh, SWP_NOZORDER);
        }
    }
}

#[cfg(not(windows))]
fn snap_window_to_fraction(_window: &Window, _x_f: i32, _y_f: i32, _w_f: i32, _h_f: i32) {}

#[cfg(windows)]
fn apply_native_fullscreen(window: &Window, enable: bool) {
    use std::sync::{Mutex, OnceLock};
    use tao::platform::windows::WindowExtWindows;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowPlacement, SetWindowPlacement, SetWindowPos, HWND_TOP, SWP_FRAMECHANGED,
        SWP_NOOWNERZORDER, WINDOWPLACEMENT,
    };

    static SAVED_WP: OnceLock<Mutex<Option<WINDOWPLACEMENT>>> = OnceLock::new();
    let wp_lock = SAVED_WP.get_or_init(|| Mutex::new(None));
    let hwnd = HWND(window.hwnd() as isize);

    unsafe {
        if enable {
            let mut wp = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            if GetWindowPlacement(hwnd, &mut wp).is_ok() {
                if let Ok(mut g) = wp_lock.lock() {
                    *g = Some(wp);
                }
            }
            let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(hmon, &mut mi).as_bool() {
                let m = mi.rcMonitor;
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOP,
                    m.left,
                    m.top,
                    m.right - m.left,
                    m.bottom - m.top,
                    SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
                );
            }
        } else {
            let saved = wp_lock.lock().ok().and_then(|mut g| g.take());
            if let Some(wp) = saved {
                let _ = SetWindowPlacement(hwnd, &wp);
                let _ = SetWindowPos(
                    hwnd,
                    HWND(0),
                    0,
                    0,
                    0,
                    0,
                    windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                        | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                        | windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER
                        | SWP_FRAMECHANGED,
                );
            } else {
                window.set_fullscreen(None);
            }
        }
    }
}

#[cfg(not(windows))]
fn apply_native_fullscreen(window: &Window, enable: bool) {
    window.set_fullscreen(if enable {
        Some(Fullscreen::Borderless(None))
    } else {
        None
    });
}

#[cfg(windows)]
fn focus_webview_programmatically(webview: &WebView) {
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC;
    use wry::WebViewExtWindows;
    let ctrl = webview.controller();
    unsafe {
        let _ = ctrl.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
    }
}

#[cfg(not(windows))]
fn focus_webview_programmatically(_webview: &WebView) {}

#[cfg(windows)]
fn print_tab_to_pdf(webview: &WebView, page_title: &str) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_7;
    use webview2_com::PrintToPdfCompletedHandler;
    use windows::core::{Interface, PCWSTR};
    use wry::WebViewExtWindows;

    let controller = webview.controller();
    unsafe {
        let Ok(core) = controller.CoreWebView2() else { return; };
        let Ok(core7) = core.cast::<ICoreWebView2_7>() else { return; };

        let pdf_dir = BrowserStorage::downloads_root_dir().join("PDFs");
        let _ = fs::create_dir_all(&pdf_dir);
        let raw_stem = if page_title.trim().is_empty() || page_title == "New Tab" {
            "shunno_page".to_string()
        } else {
            page_title.trim().chars().take(60).collect::<String>()
        };
        let safe_name = downloader::sanitize_download_filename(&format!("{}_{}.pdf", raw_stem, now_epoch_secs()));
        let out_path = pdf_dir.join(safe_name);
        let out_str = out_path.to_string_lossy().to_string();
        let wide_path: Vec<u16> = out_str.encode_utf16().chain(Some(0)).collect();

        let reveal_path = out_str.clone();
        let handler = PrintToPdfCompletedHandler::create(Box::new(move |res, is_successful| {
            if res.is_ok() && is_successful {
                let _ = Command::new("explorer.exe")
                    .args(["/select,", &reveal_path])
                    .spawn();
            }
            Ok(())
        }));

        let _ = core7.PrintToPdf(PCWSTR::from_raw(wide_path.as_ptr()), None, &handler);
    }
}

#[cfg(not(windows))]
fn print_tab_to_pdf(_webview: &WebView, _page_title: &str) {}

#[cfg(windows)]
fn capture_tab_screenshot_to_file(webview: &WebView) {
    use webview2_com::CapturePreviewCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::Com::StructuredStorage::{CreateStreamOnHGlobal, GetHGlobalFromStream};
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use wry::WebViewExtWindows;

    let controller = webview.controller();
    unsafe {
        let Ok(core) = controller.CoreWebView2() else { return; };
        let Ok(stream) = CreateStreamOnHGlobal(HGLOBAL::default(), true) else { return; };
        let stream_clone = stream.clone();
        let handler = CapturePreviewCompletedHandler::create(Box::new(move |res| {
            if res.is_ok() {
                if let Ok(hglobal) = GetHGlobalFromStream(&stream_clone) {
                    let size = GlobalSize(hglobal);
                    let ptr = GlobalLock(hglobal);
                    if !ptr.is_null() && size > 0 {
                        let bytes = std::slice::from_raw_parts(ptr as *const u8, size).to_vec();
                        let _ = GlobalUnlock(hglobal);
                        let shots_dir = BrowserStorage::downloads_root_dir().join("Screenshots");
                        let _ = fs::create_dir_all(&shots_dir);
                        let out_path = shots_dir.join(format!("shunno_shot_{}.png", now_epoch_secs()));
                        if fs::write(&out_path, &bytes).is_ok() {
                            let _ = Command::new("explorer.exe")
                                .args(["/select,", &out_path.to_string_lossy()])
                                .spawn();
                        }
                    }
                }
            }
            Ok(())
        }));
        let _ = core.CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
            &stream,
            &handler,
        );
    }
}

#[cfg(not(windows))]
fn capture_tab_screenshot_to_file(_webview: &WebView) {}

fn merge_cookie_headers(primary: Option<String>, fallback: Option<String>) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    let mut parts = Vec::new();
    for src in [primary, fallback].into_iter().flatten() {
        for raw_pair in src.split(';') {
            let trimmed = raw_pair.trim();
            if trimmed.is_empty() {
                continue;
            }
            let key = trimmed.split('=').next().unwrap_or("").trim().to_ascii_lowercase();
            if !key.is_empty() && seen.insert(key) {
                parts.push(trimmed.to_string());
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

#[cfg(windows)]
fn dispatch_download_with_httponly_cookies(
    webview: Option<&WebView>,
    url: String,
    threads: usize,
    referer: Option<String>,
    fallback_cookie: Option<String>,
    dup_mode: &str,
    delay_secs: Option<u64>,
    open_drawer: bool,
    proxy: EventLoopProxy<AppEvent>,
) {
    use webview2_com::GetCookiesCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_2;
    use windows::core::{Interface, PCWSTR};
    use wry::WebViewExtWindows;

    let dup_mode_owned = dup_mode.to_string();
    if let Some(wv) = webview {
        let controller = wv.controller();
        unsafe {
            if let Ok(core) = controller.CoreWebView2() {
                if let Ok(core2) = core.cast::<ICoreWebView2_2>() {
                    if let Ok(cookie_mgr) = core2.CookieManager() {
                        let cookie_uri = url.strip_prefix(downloader::YTDLP_PREFIX).unwrap_or(&url);
                        let wide_uri: Vec<u16> = cookie_uri.encode_utf16().chain(Some(0)).collect();
                        let url_cb = url.clone();
                        let ref_cb = referer.clone();
                        let fb_cb = fallback_cookie.clone();
                        let dup_cb = dup_mode_owned.clone();
                        let px_cb = proxy.clone();

                        let handler = GetCookiesCompletedHandler::create(Box::new(move |res, list| {
                            let mut native_pairs = Vec::new();
                            if res.is_ok() {
                                if let Some(cookie_list) = list {
                                    let mut count = 0u32;
                                    if cookie_list.Count(&mut count).is_ok() {
                                        for i in 0..count {
                                            if let Ok(c) = cookie_list.GetValueAtIndex(i) {
                                                let mut name_ptr = windows::core::PWSTR::null();
                                                let mut val_ptr = windows::core::PWSTR::null();
                                                if c.Name(&mut name_ptr).is_ok() && c.Value(&mut val_ptr).is_ok() {
                                                    let name = webview2_com::take_pwstr(name_ptr);
                                                    let val = webview2_com::take_pwstr(val_ptr);
                                                    if !name.is_empty() {
                                                        native_pairs.push(format!("{}={}", name, val));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            let native_hdr = if native_pairs.is_empty() {
                                None
                            } else {
                                Some(native_pairs.join("; "))
                            };
                            let merged = merge_cookie_headers(native_hdr, fb_cb.clone());
                            let _ = px_cb.send_event(AppEvent::DownloadWithCookies {
                                url: url_cb.clone(),
                                threads,
                                referer: ref_cb.clone(),
                                cookie: merged,
                                dup_mode: dup_cb.clone(),
                                delay_secs,
                                open_drawer,
                            });
                            Ok(())
                        }));

                        if cookie_mgr
                            .GetCookies(PCWSTR::from_raw(wide_uri.as_ptr()), &handler)
                            .is_ok()
                        {
                            return;
                        }
                    }
                }
            }
        }
    }

    let _ = proxy.send_event(AppEvent::DownloadWithCookies {
        url,
        threads,
        referer,
        cookie: fallback_cookie,
        dup_mode: dup_mode_owned,
        delay_secs,
        open_drawer,
    });
}

#[cfg(not(windows))]
fn dispatch_download_with_httponly_cookies(
    _webview: Option<&WebView>,
    url: String,
    threads: usize,
    referer: Option<String>,
    fallback_cookie: Option<String>,
    dup_mode: &str,
    delay_secs: Option<u64>,
    open_drawer: bool,
    proxy: EventLoopProxy<AppEvent>,
) {
    let _ = proxy.send_event(AppEvent::DownloadWithCookies {
        url,
        threads,
        referer,
        cookie: fallback_cookie,
        dup_mode: dup_mode.to_string(),
        delay_secs,
        open_drawer,
    });
}


/// Applies password autosave and tracking prevention to a tab's WebView2 settings/profile.
#[cfg(windows)]
fn apply_privacy_settings(webview: &WebView, save_passwords: bool, strict_tracking: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Profile3, ICoreWebView2Settings4, ICoreWebView2_13, COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BALANCED,
        COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_STRICT,
    };
    use windows::core::Interface;
    use wry::WebViewExtWindows;
    unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else { return };
        if let Ok(settings) = core.Settings() {
            if let Ok(s4) = settings.cast::<ICoreWebView2Settings4>() {
                let _ = s4.SetIsPasswordAutosaveEnabled(save_passwords);
                let _ = s4.SetIsGeneralAutofillEnabled(save_passwords);
            }
        }
        if let Ok(c13) = core.cast::<ICoreWebView2_13>() {
            if let Ok(profile) = c13.Profile() {
                if let Ok(p3) = profile.cast::<ICoreWebView2Profile3>() {
                    let level = if strict_tracking {
                        COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_STRICT
                    } else {
                        COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BALANCED
                    };
                    let _ = p3.SetPreferredTrackingPreventionLevel(level);
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn apply_privacy_settings(_webview: &WebView, _save_passwords: bool, _strict_tracking: bool) {}

/// Deletes every password saved by the built-in password manager (normal profile).
#[cfg(windows)]
fn clear_saved_passwords(webview: &WebView) {
    use webview2_com::ClearBrowsingDataCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Profile2, ICoreWebView2_13, COREWEBVIEW2_BROWSING_DATA_KINDS_GENERAL_AUTOFILL,
        COREWEBVIEW2_BROWSING_DATA_KINDS_PASSWORD_AUTOSAVE,
    };
    use windows::core::Interface;
    use wry::WebViewExtWindows;
    unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else { return };
        let Ok(c13) = core.cast::<ICoreWebView2_13>() else { return };
        let Ok(profile) = c13.Profile() else { return };
        let Ok(p2) = profile.cast::<ICoreWebView2Profile2>() else { return };
        let handler = ClearBrowsingDataCompletedHandler::create(Box::new(|_| Ok(())));
        let _ = p2.ClearBrowsingData(
            COREWEBVIEW2_BROWSING_DATA_KINDS_PASSWORD_AUTOSAVE | COREWEBVIEW2_BROWSING_DATA_KINDS_GENERAL_AUTOFILL,
            &handler,
        );
    }
}

#[cfg(not(windows))]
fn clear_saved_passwords(_webview: &WebView) {}

#[cfg(windows)]
fn clear_httponly_cookies_for_url(webview: &WebView, url: &str) {
    use webview2_com::GetCookiesCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_2;
    use windows::core::{Interface, PCWSTR};
    use wry::WebViewExtWindows;

    if !url.starts_with("http") {
        return;
    }
    let controller = webview.controller();
    unsafe {
        if let Ok(core) = controller.CoreWebView2() {
            if let Ok(core2) = core.cast::<ICoreWebView2_2>() {
                if let Ok(cookie_mgr) = core2.CookieManager() {
                    let wide_uri: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
                    let mgr_cb = cookie_mgr.clone();
                    let handler = GetCookiesCompletedHandler::create(Box::new(move |res, list| {
                        if res.is_ok() {
                            if let Some(cookie_list) = list {
                                let mut count = 0u32;
                                if cookie_list.Count(&mut count).is_ok() {
                                    for i in 0..count {
                                        if let Ok(c) = cookie_list.GetValueAtIndex(i) {
                                            let _ = mgr_cb.DeleteCookie(&c);
                                        }
                                    }
                                }
                            }
                        }
                        Ok(())
                    }));
                    let _ = cookie_mgr.GetCookies(PCWSTR::from_raw(wide_uri.as_ptr()), &handler);
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn clear_httponly_cookies_for_url(_webview: &WebView, _url: &str) {}

#[cfg(windows)]
fn attach_f11_accelerator_hook(webview: &WebView, proxy: EventLoopProxy<AppEvent>) {
    use webview2_com::AcceleratorKeyPressedEventHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN,
    };
    use wry::WebViewExtWindows;

    let controller = webview.controller();
    unsafe {
        let mut token = Default::default();
        let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_sender, args| {
            if let Some(args) = args {
                let mut kind = Default::default();
                let mut vk = 0u32;
                if args.KeyEventKind(&mut kind).is_ok() && args.VirtualKey(&mut vk).is_ok() {
                    if vk == 0x7A
                        && (kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                            || kind == COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN)
                    {
                        let _ = args.SetHandled(true);
                        let _ = proxy.send_event(AppEvent::ToggleFullscreen);
                    }
                }
            }
            Ok(())
        }));
        let _ = controller.add_AcceleratorKeyPressed(&handler, &mut token);
    }
}

#[cfg(not(windows))]
fn attach_f11_accelerator_hook(_webview: &WebView, _proxy: EventLoopProxy<AppEvent>) {}

fn expand_wildcard_range(line: &str) -> Option<Vec<String>> {
    let open = line.find('[')?;
    let close = line[open + 1..].find(']')? + open + 1;
    let inner = &line[open + 1..close];
    let (s_str, e_str) = inner.split_once('-')?;
    if s_str.is_empty() || e_str.is_empty() || !s_str.chars().all(|c| c.is_ascii_digit()) || !e_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let start: u32 = s_str.parse().ok()?;
    let end: u32 = e_str.parse().ok()?;
    let width = s_str.len();
    let full_m = &line[open..=close];
    let mut out = Vec::new();
    for i in start..=end.min(start.saturating_add(100)) {
        let num = format!("{:0width$}", i, width = width);
        out.push(line.replace(full_m, &num));
    }
    Some(out)
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

        // 0b. Native Full-URL Synchronization (captures SPA pushState/replaceState, YouTube watch URLs, redirects)
        use webview2_com::SourceChangedEventHandler;
        let mut src_token = Default::default();
        let src_proxy = proxy.clone();
        let src_handler = SourceChangedEventHandler::create(Box::new(move |sender, _| {
            if let Some(wv) = sender {
                let mut uri_ptr = windows::core::PWSTR::null();
                if wv.Source(&mut uri_ptr).is_ok() {
                    let uri = webview2_com::take_pwstr(uri_ptr);
                    if uri.starts_with("http://") || uri.starts_with("https://") {
                        let _ = src_proxy.send_event(AppEvent::TabUrlChanged(tab_id, uri));
                    }
                }
            }
            Ok(())
        }));
        let _ = core.add_SourceChanged(&src_handler, &mut src_token);

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
                let _ = ad_proxy.send_event(AppEvent::AdBlocked);
                Ok(())
            }));

            let _ = core.add_WebResourceRequested(&handler, &mut token);
        }



        // 2b. Native IDM-Class HTTP Response Media Sniffer (ICoreWebView2_2::add_WebResourceResponseReceived)
        use webview2_com::WebResourceResponseReceivedEventHandler;
        let mut resp_token = Default::default();
        let resp_proxy = proxy.clone();
        let resp_handler = WebResourceResponseReceivedEventHandler::create(Box::new(move |_sender, args| {
            let Some(args) = args else { return Ok(()); };
            let Ok(req) = args.Request() else { return Ok(()); };
            let mut uri_ptr = windows::core::PWSTR::null();
            if req.Uri(&mut uri_ptr).is_err() { return Ok(()); }
            let raw_uri = webview2_com::take_pwstr(uri_ptr);
            if !raw_uri.starts_with("http") { return Ok(()); }

            let Ok(resp) = args.Response() else { return Ok(()); };
            let Ok(headers) = resp.Headers() else { return Ok(()); };

            let mut ct_ptr = windows::core::PWSTR::null();
            let content_type = if headers.GetHeader(w!("content-type"), &mut ct_ptr).is_ok() {
                webview2_com::take_pwstr(ct_ptr).to_ascii_lowercase()
            } else {
                String::new()
            };

            let mut cl_ptr = windows::core::PWSTR::null();
            let content_len: u64 = if headers.GetHeader(w!("content-length"), &mut cl_ptr).is_ok() {
                webview2_com::take_pwstr(cl_ptr).trim().parse::<u64>().unwrap_or(0)
            } else {
                0
            };

            let lower_uri = raw_uri.to_ascii_lowercase();
            let is_media_ct = content_type.starts_with("video/")
                || content_type.starts_with("audio/")
                || content_type.contains("mpegurl")
                || content_type.contains("dash+xml")
                || content_type.contains("x-matroska");

            let is_large_enough = content_len == 0 || content_len >= 65_536 || content_type.contains("mpegurl") || content_type.contains("dash+xml");

            if is_media_ct && is_large_enough {
                let mut clean_uri = raw_uri;
                if lower_uri.contains("googlevideo.com/videoplayback") {
                    // Strip transient MSE byte-range parameters so NativeDownloadEngine downloads the complete stream
                    let mut parts: Vec<&str> = Vec::new();
                    if let Some((base, q)) = clean_uri.split_once('?') {
                        for kv in q.split('&') {
                            if !kv.starts_with("range=") && !kv.starts_with("rbuf=") && !kv.starts_with("rn=") {
                                parts.push(kv);
                            }
                        }
                        clean_uri = format!("{}?{}", base, parts.join("&"));
                    }
                }
                let ext = if content_type.contains("mpegurl") {
                    ".M3U8"
                } else if content_type.contains("dash+xml") {
                    ".MPD"
                } else if content_type.contains("webm") {
                    ".WEBM"
                } else if content_type.contains("matroska") {
                    ".MKV"
                } else if content_type.starts_with("audio/") {
                    ".M4A"
                } else {
                    ".MP4"
                };
                let size_label = if content_len > 0 {
                    downloader::format_human_size(content_len)
                } else {
                    "Stream".to_string()
                };
                let _ = resp_proxy.send_event(AppEvent::NativeStreamCaptured(
                    tab_id,
                    clean_uri,
                    ext.to_string(),
                    size_label,
                ));
            }
            Ok(())
        }));
        let _ = core2.add_WebResourceResponseReceived(&resp_handler, &mut resp_token);


        // 3. Native WebView2 Download Engine Bridge (ICoreWebView2_4::add_DownloadStarting)
        // Keeps WebView2's native download stream alive (100% compatible with Cloudflare, GDrive, HubCloud, POST, blob:)
        // while routing files cleanly into ShunnoBrowser\<Category> and displaying live progress in Shunno's Downloads panel.
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED, COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED,
            COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED, COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED, ICoreWebView2_4,
        };
        use webview2_com::{
            BytesReceivedChangedEventHandler, DownloadStartingEventHandler,
            StateChangedEventHandler,
        };
        if let Ok(core4) = core.cast::<ICoreWebView2_4>() {
            let mut dl_token = Default::default();
            let dl_proxy = proxy.clone();
            let dl_handler = DownloadStartingEventHandler::create(Box::new(move |_sender, args| {
                let Some(args) = args else { return Ok(()); };
                let Ok(op) = args.DownloadOperation() else { return Ok(()); };

                // Hide Edge's default popup while keeping the native download running
                let _ = args.SetHandled(true);

                let mut uri_ptr = windows::core::PWSTR::null();
                let uri = if op.Uri(&mut uri_ptr).is_ok() {
                    webview2_com::take_pwstr(uri_ptr)
                } else {
                    String::new()
                };

                let mut default_path_ptr = windows::core::PWSTR::null();
                let default_name = if args.ResultFilePath(&mut default_path_ptr).is_ok() {
                    let p = webview2_com::take_pwstr(default_path_ptr);
                    std::path::Path::new(&p)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("download.bin")
                        .to_string()
                } else {
                    "download.bin".to_string()
                };

                let safe_name = downloader::sanitize_download_filename(&default_name);
                let category = downloader::detect_download_category(&safe_name);
                let target_dir = BrowserStorage::downloads_root_dir().join(&category);
                let _ = fs::create_dir_all(&target_dir);
                let mut dest_path = target_dir.join(&safe_name);
                if dest_path.exists() {
                    let stem = dest_path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
                    let ext = dest_path.extension().and_then(|s| s.to_str()).unwrap_or("bin");
                    dest_path = target_dir.join(format!("{}_{}.{}", stem, now_epoch_secs() % 100_000, ext));
                }
                let dest_str = dest_path.to_string_lossy().to_string();
                let wide_dest: Vec<u16> = dest_str.encode_utf16().chain(Some(0)).collect();
                let _ = args.SetResultFilePath(PCWSTR::from_raw(wide_dest.as_ptr()));

                let mut total_bytes: i64 = -1;
                let _ = op.TotalBytesToReceive(&mut total_bytes);

                let dl_id = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(1)
                    .wrapping_mul(1000)
                    + rand::random::<u16>() as u64 % 1000;
                // Keep the operation so the Downloads panel can really pause, resume and cancel it.
                NATIVE_OPS.with(|ops| {
                    ops.borrow_mut().insert(dl_id, op.clone());
                });

                let _ = dl_proxy.send_event(AppEvent::NativeDownloadStarted {
                    id: dl_id,
                    url: uri,
                    file_name: safe_name,
                    saved_path: dest_str.clone(),
                    total_bytes,
                });

                let mut b_token = Default::default();
                let b_proxy = dl_proxy.clone();
                let last_time = std::sync::Mutex::new((std::time::Instant::now(), 0u64));
                let b_handler = BytesReceivedChangedEventHandler::create(Box::new(move |sender, _| {
                    if let Some(dl_op) = sender {
                        let mut rec: i64 = 0;
                        let mut tot: i64 = -1;
                        let _ = dl_op.BytesReceived(&mut rec);
                        let _ = dl_op.TotalBytesToReceive(&mut tot);
                        let rec_u = rec.max(0) as u64;
                        if let Ok(mut guard) = last_time.lock() {
                            let elapsed_ms = guard.0.elapsed().as_millis() as u64;
                            if elapsed_ms >= 300 {
                                let spd = (rec_u.saturating_sub(guard.1) * 1000) / elapsed_ms.max(1);
                                *guard = (std::time::Instant::now(), rec_u);
                                let _ = b_proxy.send_event(AppEvent::NativeDownloadProgress {
                                    id: dl_id,
                                    received: rec_u,
                                    total: tot,
                                    speed_bps: spd,
                                });
                            }
                        }
                    }
                    Ok(())
                }));
                let _ = op.add_BytesReceivedChanged(&b_handler, &mut b_token);

                let mut s_token = Default::default();
                let s_proxy = dl_proxy.clone();
                let final_p = dest_str;
                let s_handler = StateChangedEventHandler::create(Box::new(move |sender, _| {
                    if let Some(dl_op) = sender {
                        let mut st = Default::default();
                        if dl_op.State(&mut st).is_ok() {
                            if st == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED {
                                NATIVE_OPS.with(|ops| {
                                    ops.borrow_mut().remove(&dl_id);
                                });
                                let _ = s_proxy.send_event(AppEvent::NativeDownloadFinished {
                                    id: dl_id,
                                    ok: true,
                                    final_path: Some(final_p.clone()),
                                    err_msg: None,
                                });
                            } else if st == COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED {
                                let mut reason = Default::default();
                                let _ = dl_op.InterruptReason(&mut reason);
                                let mut can_resume = windows::Win32::Foundation::BOOL::default();
                                let _ = dl_op.CanResume(&mut can_resume);
                                if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED {
                                    // Pause() reports itself as an interruption; it is not a failure.
                                    let _ = s_proxy.send_event(AppEvent::NativeDownloadPaused { id: dl_id, reason: None });
                                } else if reason != COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED && can_resume.as_bool() {
                                    let _ = s_proxy.send_event(AppEvent::NativeDownloadPaused {
                                        id: dl_id,
                                        reason: Some(format!("Interrupted (reason {}), press Resume", reason.0)),
                                    });
                                } else {
                                    NATIVE_OPS.with(|ops| {
                                        ops.borrow_mut().remove(&dl_id);
                                    });
                                    let msg = if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED {
                                        "Cancelled".to_string()
                                    } else {
                                        format!("Download interrupted (reason {})", reason.0)
                                    };
                                    let _ = s_proxy.send_event(AppEvent::NativeDownloadFinished {
                                        id: dl_id,
                                        ok: false,
                                        final_path: None,
                                        err_msg: Some(msg),
                                    });
                                }
                            }
                        }
                    }
                    Ok(())
                }));
                let _ = op.add_StateChanged(&s_handler, &mut s_token);

                Ok(())
            }));
            let _ = core4.add_DownloadStarting(&dl_handler, &mut dl_token);
        }

        // 4. Native WebView2 ContextMenuRequested Hook:
        //    - Replace "Open link in new window" with "Open link in new tab"
        //    - Add "Download link with Shunno" on all HTTP(S) webpage links
        //    - Add "Download media with Shunno" on right-clicked media/image sources
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
                let Ok(items) = args.MenuItems() else { return Ok(()); };

                let mut insert_pos = 0u32;
                let mut added_link_uri = String::new();

                let mut has_link = windows::Win32::Foundation::BOOL::default();
                let _ = target.HasLinkUri(&mut has_link);
                if has_link.as_bool() {
                    let mut link_ptr = windows::core::PWSTR::null();
                    if target.LinkUri(&mut link_ptr).is_ok() {
                        let link_uri = webview2_com::take_pwstr(link_ptr);
                        if link_uri.starts_with("http://") || link_uri.starts_with("https://") {
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
                                    let _ = sel_proxy.send_event(AppEvent::NewWindowBlockedOrOpened(tab_id, sel_uri.clone()));
                                    Ok(())
                                }));
                                let _ = new_tab_item.add_CustomItemSelected(&sel_handler, &mut sel_token);
                                let _ = items.InsertValueAtIndex(insert_pos, &new_tab_item);
                                insert_pos += 1;
                            }

                            if let Ok(dl_link_item) = env9.CreateContextMenuItem(
                                w!("Download link with Shunno"),
                                None::<&IStream>,
                                COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND,
                            ) {
                                let mut dl_sel_token = Default::default();
                                let dl_sel_proxy = ctx_proxy.clone();
                                let dl_sel_uri = link_uri.clone();
                                let dl_sel_handler = CustomItemSelectedEventHandler::create(Box::new(move |_s, _| {
                                    let _ = dl_sel_proxy.send_event(AppEvent::DownloadIntercepted(dl_sel_uri.clone()));
                                    Ok(())
                                }));
                                let _ = dl_link_item.add_CustomItemSelected(&dl_sel_handler, &mut dl_sel_token);
                                let _ = items.InsertValueAtIndex(insert_pos, &dl_link_item);
                                insert_pos += 1;
                            }

                            added_link_uri = link_uri;
                        }
                    }
                }

                let mut has_src = windows::Win32::Foundation::BOOL::default();
                let _ = target.HasSourceUri(&mut has_src);
                if has_src.as_bool() {
                    let mut src_ptr = windows::core::PWSTR::null();
                    if target.SourceUri(&mut src_ptr).is_ok() {
                        let src_uri = webview2_com::take_pwstr(src_ptr);
                        if (src_uri.starts_with("http://") || src_uri.starts_with("https://"))
                            && src_uri != added_link_uri
                        {
                            if let Ok(dl_media_item) = env9.CreateContextMenuItem(
                                w!("Download media with Shunno"),
                                None::<&IStream>,
                                COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND,
                            ) {
                                let mut m_token = Default::default();
                                let m_proxy = ctx_proxy.clone();
                                let m_uri = src_uri.clone();
                                let m_handler = CustomItemSelectedEventHandler::create(Box::new(move |_s, _| {
                                    let _ = m_proxy.send_event(AppEvent::DownloadIntercepted(m_uri.clone()));
                                    Ok(())
                                }));
                                let _ = dl_media_item.add_CustomItemSelected(&m_handler, &mut m_token);
                                let _ = items.InsertValueAtIndex(insert_pos, &dl_media_item);
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
    web_context: &mut WebContext,
    tab_id: usize,
    url: &str,
    policy: &SitePolicy,
    global: &GlobalSettings,
    ipc_token: &str,
    proxy: EventLoopProxy<AppEvent>,
    incognito: bool,
) -> wry::Result<WebView> {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let content_h = (size.height - CHROME_BAR_HEIGHT).max(100.0);
    let host = extract_host(url);
    let user_js = load_approved_user_scripts(&host, &global.approved_script_hashes);
    let init_script = build_content_init_script(policy, global, ipc_token, &user_js);

    let ipc_proxy = proxy.clone();
    let win_proxy = proxy.clone();
    let title_proxy = proxy.clone();
    let nav_proxy = proxy.clone();

    let wv = WebViewBuilder::new_as_child(window)
        .with_web_context(web_context)
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, CHROME_BAR_HEIGHT).into(),
            size: LogicalSize::new(size.width.max(400.0), content_h).into(),
        })
        .with_url(if url == "shunno://newtab" { "about:blank" } else { url })
        // Private tabs get a separate in-memory WebView2 InPrivate session: no shared cookies, storage, cache or passwords.
        .with_incognito(incognito)
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

        .with_new_window_req_handler(move |popup_url| {
            let lower = popup_url.to_ascii_lowercase();
            if popup_url.is_empty()
                || lower.starts_with("about:blank")
                || lower.starts_with("javascript:")
                || lower.contains("accounts.google.com")
                || lower.contains("oauth")
            {
                return true;
            }
            let _ = win_proxy.send_event(AppEvent::NewWindowBlockedOrOpened(tab_id, popup_url));
            false
        })
        .build()?;

    attach_f11_accelerator_hook(&wv, proxy.clone());
    attach_native_webview2_hooks(&wv, tab_id, policy.adblock, proxy);
    apply_privacy_settings(&wv, global.save_passwords && !incognito, global.strict_tracking);

    if (policy.zoom - 1.0).abs() > f64::EPSILON {
        apply_tab_zoom(&wv, policy.zoom);
    }
    let _ = wv.set_visible(true);
    Ok(wv)
}

/// Non-Overlapping Split Layout:
/// When a drawer/popover is open, `chrome_wv` occupies the top bar + right docked panel `(w - panel_w, CHROME_BAR_HEIGHT, panel_w, content_h)`
/// while the active tab WebView occupies the left viewport `(0, CHROME_BAR_HEIGHT, w - panel_w, content_h)`.
/// Because WebView2 HWNDs never overlap, every button in the menu/downloads panel is 100% clickable AND the live webpage remains visible!
#[cfg(windows)]
fn install_native_win32_window_features(window: &Window, chrome_wv: &WebView) {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use tao::platform::windows::WindowExtWindows;
    use webview2_com::Microsoft::Web::WebView2::Win32::{COREWEBVIEW2_COLOR, ICoreWebView2Controller2};
    use windows::core::Interface;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, CreateIconFromResourceEx, GetClientRect, GetWindowLongW, GetWindowRect, IsZoomed, SendMessageW,
        SetClassLongPtrW, SetWindowLongPtrW, SetWindowLongW, SetWindowPos, GCLP_HICON,
        GCLP_HICONSM, GWL_EXSTYLE, GWL_STYLE, GWLP_WNDPROC, HTBOTTOM, HTBOTTOMLEFT,
        HTBOTTOMRIGHT, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, ICON_BIG,
        ICON_SMALL, LR_DEFAULTCOLOR, SWP_FRAMECHANGED, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, WM_NCCALCSIZE, WM_NCHITTEST, WM_SETICON, WNDPROC,
        WS_EX_APPWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
    };
    use wry::WebViewExtWindows;

    static ORIG_WNDPROC: AtomicIsize = AtomicIsize::new(0);
    let hwnd = HWND(window.hwnd() as isize);

    unsafe {

        // 0. Native Windows 11 Rounded Corners (DWMWCP_ROUND)
        let corner_pref = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner_pref as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&corner_pref) as u32,
        );

        // 1. Enable WS_THICKFRAME + WS_EX_APPWINDOW so borderless window supports mouse resizing & Windows Snap
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

        // 2. Apply full-brightness embedded icon directly onto Window & Class (GCLP_HICON / WM_SETICON)
        const ICO_BYTES: &[u8] = include_bytes!("../assets/app.ico");
        if ICO_BYTES.len() >= 22 {
            let count = u16::from_le_bytes([ICO_BYTES[4], ICO_BYTES[5]]) as usize;
            for i in 0..count {
                let off = 6 + i * 16;
                if off + 16 > ICO_BYTES.len() { break; }
                let w = ICO_BYTES[off] as i32;
                if w == 0 { continue; }
                let sz = u32::from_le_bytes([ICO_BYTES[off + 8], ICO_BYTES[off + 9], ICO_BYTES[off + 10], ICO_BYTES[off + 11]]) as usize;
                let img_off = u32::from_le_bytes([ICO_BYTES[off + 12], ICO_BYTES[off + 13], ICO_BYTES[off + 14], ICO_BYTES[off + 15]]) as usize;
                if img_off + sz <= ICO_BYTES.len() {
                    let slice = &ICO_BYTES[img_off..img_off + sz];
                    if let Ok(hicon) = CreateIconFromResourceEx(slice, true, 0x00030000, w, w, LR_DEFAULTCOLOR) {
                        if w == 48 || w == 32 {
                            SetClassLongPtrW(hwnd, GCLP_HICON, hicon.0);
                            let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_BIG as usize), LPARAM(hicon.0));
                        } else if w == 16 || w == 24 {
                            SetClassLongPtrW(hwnd, GCLP_HICONSM, hicon.0);
                            let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_SMALL as usize), LPARAM(hicon.0));
                        }
                    }
                }
            }
        }


        // 3. Make chrome_wv WebView2 background transparent (A=0) so overlays float cleanly over the live webpage
        let chrome_ctrl = chrome_wv.controller();
        if let Ok(ctrl2) = chrome_ctrl.cast::<ICoreWebView2Controller2>() {
            let _ = ctrl2.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR { A: 0, R: 0, G: 0, B: 0 });
        }

        // 4. Chain custom WndProc for 8-directional borderless mouse resizing
        unsafe extern "system" fn borderless_resize_wndproc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            match msg {
                WM_NCCALCSIZE => {
                    if wparam.0 != 0 {
                        return LRESULT(0);
                    }
                }
                WM_NCHITTEST => {
                    if IsZoomed(hwnd).as_bool() {
                        return LRESULT(HTCLIENT as isize);
                    }
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
            let prev_ptr = ORIG_WNDPROC.load(Ordering::Relaxed);
            if prev_ptr != 0 {
                let prev_fn: WNDPROC = std::mem::transmute(prev_ptr);
                CallWindowProcW(prev_fn, hwnd, msg, wparam, lparam)
            } else {
                LRESULT(0)
            }
        }

        let prev = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, borderless_resize_wndproc as *const () as usize as isize);
        if prev != 0 {
            ORIG_WNDPROC.store(prev, Ordering::Relaxed);
        }
    }
}

#[cfg(not(windows))]
fn install_native_win32_window_features(_window: &Window, _chrome_wv: &WebView) {}

#[cfg(windows)]
fn bring_chrome_above_tabs(chrome_wv: &WebView, tabs: &[BrowserTab]) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE};
    use wry::WebViewExtWindows;

    let ctrl = chrome_wv.controller();
    unsafe {
        let mut chrome_hwnd = HWND::default();
        if ctrl.ParentWindow(&mut chrome_hwnd).is_ok() && chrome_hwnd.0 != 0 {
            let _ = SetWindowPos(chrome_hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            for tab in tabs {
                let mut tab_hwnd = HWND::default();
                if tab.webview.controller().ParentWindow(&mut tab_hwnd).is_ok()
                    && tab_hwnd.0 != 0
                    && tab_hwnd.0 != chrome_hwnd.0
                {
                    let _ = SetWindowPos(tab_hwnd, chrome_hwnd, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn bring_chrome_above_tabs(_chrome_wv: &WebView, _tabs: &[BrowserTab]) {}

#[cfg(windows)]
fn bring_active_tab_above_chrome(tab_wv: &WebView, chrome_wv: &WebView) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE};
    use wry::WebViewExtWindows;

    unsafe {
        let mut tab_hwnd = HWND::default();
        let mut chrome_hwnd = HWND::default();
        if tab_wv.controller().ParentWindow(&mut tab_hwnd).is_ok()
            && chrome_wv.controller().ParentWindow(&mut chrome_hwnd).is_ok()
            && tab_hwnd.0 != 0
            && chrome_hwnd.0 != 0
            && tab_hwnd.0 != chrome_hwnd.0
        {
            let _ = SetWindowPos(tab_hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            let _ = SetWindowPos(chrome_hwnd, tab_hwnd, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
}

#[cfg(not(windows))]
fn bring_active_tab_above_chrome(_tab_wv: &WebView, _chrome_wv: &WebView) {}



fn layout_webviews(
    window: &Window,
    chrome_wv: &WebView,
    tabs: &[BrowserTab],
    active_idx: usize,
    split_idx: Option<usize>,
    drawer_open: bool,
    engine_menu_open: bool,
    palette_open: bool,
    is_fs: bool,
) {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let w = size.width.max(400.0);
    let h = size.height.max(300.0);
    let is_max = window.is_maximized() || is_fs;
    let grip = if is_max { 0.0 } else { 3.0 };

    let top_bar_h = if is_fs { 0.0 } else { CHROME_BAR_HEIGHT };
    let inner_w = (w - grip * 2.0).max(200.0);
    let inner_h = (h - grip).max(200.0);
    let content_h = (inner_h - top_bar_h).max(80.0);

    let valid_split = split_idx.filter(|&s| s < tabs.len() && s != active_idx);
    let is_home = valid_split.is_none()
        && tabs
            .get(active_idx)
            .map(|t| t.url == "shunno://newtab")
            .unwrap_or(true);

    let overlay_active = drawer_open || engine_menu_open || palette_open;
    let chrome_h = if is_fs {
        if is_home { inner_h } else { 0.0 }
    } else if overlay_active || is_home {
        inner_h
    } else {
        top_bar_h
    };

    let _ = chrome_wv.set_visible(!is_fs || is_home);
    let _ = chrome_wv.set_bounds(Rect {
        position: LogicalPosition::new(grip, 0.0).into(),
        size: LogicalSize::new(inner_w, chrome_h).into(),
    });

    // Full-width webpage (zero splitting): Menu & Downloads float over the live webpage at 10% transparency.
    let web_w = inner_w;

    for (idx, tab) in tabs.iter().enumerate() {
        if let Some(s_idx) = valid_split {
            let half_w = ((web_w - 2.0) / 2.0).max(100.0);
            if idx == active_idx && tab.url != "shunno://newtab" {
                let _ = tab.webview.set_bounds(Rect {
                    position: LogicalPosition::new(grip, top_bar_h).into(),
                    size: LogicalSize::new(half_w, content_h).into(),
                });
                let _ = tab.webview.set_visible(true);
                continue;
            } else if idx == s_idx && tab.url != "shunno://newtab" {
                let _ = tab.webview.set_bounds(Rect {
                    position: LogicalPosition::new(grip + half_w + 2.0, top_bar_h).into(),
                    size: LogicalSize::new(half_w, content_h).into(),
                });
                let _ = tab.webview.set_visible(true);
                continue;
            }
        }

        let _ = tab.webview.set_bounds(Rect {
            position: LogicalPosition::new(grip, top_bar_h).into(),
            size: LogicalSize::new(web_w, content_h).into(),
        });
        let show_web = idx == active_idx && tab.url != "shunno://newtab";
        let _ = tab.webview.set_visible(show_web);
    }

    if !is_fs {
        if overlay_active || is_home {
            bring_chrome_above_tabs(chrome_wv, tabs);
        } else if let Some(active_tab) = tabs.get(active_idx) {
            if active_tab.url != "shunno://newtab" {
                bring_active_tab_above_chrome(&active_tab.webview, chrome_wv);
            }
        }
    }
}

fn main() -> wry::Result<()> {
    set_explicit_app_user_model_id();

    // Store WebView2 user profile cleanly inside %LOCALAPPDATA%\ShunnoBrowser\WebView2
    // so ShunnoBrowser.exe stays 100% standalone with zero folder beside the executable.
    let wv2_data_dir = BrowserStorage::app_data_dir().join("WebView2");
    let _ = fs::create_dir_all(&wv2_data_dir);
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &wv2_data_dir);
    let mut web_context = WebContext::new(Some(wv2_data_dir));

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
        .with_window_icon(load_embedded_window_icon())
        .with_inner_size(LogicalSize::new(1240.0, 780.0))
        .build(&event_loop)
        .unwrap();

    let chrome_proxy = proxy.clone();
    let chrome_wv = WebViewBuilder::new_as_child(&window)
        .with_web_context(&mut web_context)
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
    attach_f11_accelerator_hook(&chrome_wv, proxy.clone());

    let next_tab_id = Arc::new(AtomicUsize::new(1));
    let mut tabs: Vec<BrowserTab> = Vec::new();
    let mut active_tab_idx: usize = 0;
    let mut drawer_open = false;
    let mut engine_menu_open = false;
    let mut palette_open = false;
    let mut is_fullscreen = false;
    let mut last_fs_toggle = std::time::Instant::now() - Duration::from_secs(5);
    let mut split_tab_idx: Option<usize> = None;
    let mut blocked_count: usize = 0;
    let mut sniffed_streams: Vec<SniffedStreamItem> = Vec::new();
    let mut duplicate_alert: Option<DownloadTask> = None;
    let mut closed_tabs: Vec<SavedTab> = Vec::new();
    let mut find_current: usize = 0;
    let mut find_total: usize = 0;
    let mut had_active_downloads = false;
    let mut site_data_cleared = false;
    // Tabs for which the user asked "Save page"; OFFLINE_HTML from any other tab is ignored.
    let mut pending_offline_saves: std::collections::HashSet<usize> = std::collections::HashSet::new();

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
        if let Ok(wv) = create_tab_webview(&window, &mut web_context, tid, &u, &pol, &global_settings, &ipc_token, proxy.clone(), false) {
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
                sleeping: false,
                last_active_secs: now_epoch_secs(),
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
        let mut sleep_ticks: u32 = 0;
        let mut was_busy = false;
        loop {
            tokio::time::sleep(Duration::from_millis(750)).await;
            sleep_ticks = sleep_ticks.wrapping_add(1);
            if sleep_ticks % 80 == 0 {
                let _ = ticker_proxy.send_event(AppEvent::SleepCheck);
            }
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
            let tool_busy = |m: &std::sync::Mutex<String>| {
                m.lock()
                    .map(|s| s.starts_with("Downloading") || s.starts_with("Verifying") || s.starts_with("Extracting"))
                    .unwrap_or(false)
            };
            let busy = has_active_or_queued
                || tool_busy(&ticker_engine.ffmpeg_install_status)
                || tool_busy(&ticker_engine.ytdlp_install_status);
            // Also refresh once on the busy -> idle edge; otherwise the final COMPLETED/FAILED
            // state never reaches the panel and the "when queue finishes" action never runs.
            if busy || was_busy {
                let _ = ticker_proxy.send_event(AppEvent::RefreshChrome);
            }
            was_busy = busy;
        }
    });

    // Probe FFmpeg once asynchronously on Tokio so push_chrome_state NEVER spawns a blocking subprocess on the UI thread!
    let ffmpeg_available_flag = Arc::new(AtomicBool::new(false));
    {
        let ff_flag = ffmpeg_available_flag.clone();
        let ff_px = proxy.clone();
        rt_handle.spawn(async move {
            let found = BrowserStorage::find_ffmpeg().is_some();
            ff_flag.store(found, Ordering::Relaxed);
            if found {
                let _ = ff_px.send_event(AppEvent::RefreshChrome);
            }
        });
    }
    let cached_scripts = Arc::new(std::sync::Mutex::new(list_user_scripts(&global_settings.approved_script_hashes)));
    let last_chrome_json = Arc::new(std::sync::Mutex::new(String::new()));
    let cached_scripts_for_push = cached_scripts.clone();
    let push_chrome_state = move |_window: &Window,
                             chrome_wv: &WebView,
                             tabs: &[BrowserTab],
                             active_idx: usize,
                             split_tab: Option<usize>,
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
                             site_data_cleared: bool,
                             is_fullscreen: bool,
                             engine: &NativeDownloadEngine| {
        let active_url = tabs.get(active_idx).map(|t| t.url.clone()).unwrap_or_default();
        let active_zoom = tabs.get(active_idx).map(|t| t.zoom).unwrap_or(1.0);
        let active_reader = tabs.get(active_idx).map(|t| t.reader_active).unwrap_or(false);
        let active_host = extract_host(&active_url);
        let site = site_policies.get(&active_host).cloned().unwrap_or_default();
        let downloads = engine.tasks.lock().map(|l| l.clone()).unwrap_or_default();
        let scripts = cached_scripts_for_push.lock().map(|s| s.clone()).unwrap_or_default();

        let ff_status = engine.ffmpeg_install_status.lock().map(|s| s.clone()).unwrap_or_default();
        if ff_status == "Installed!" || ff_status == "Ready" {
            ffmpeg_available_flag.store(true, Ordering::Relaxed);
        }
        let snap = ChromeSnapshot {
            version: APP_VERSION,
            ffmpeg_available: ffmpeg_available_flag.load(Ordering::Relaxed),
            ffmpeg_status: ff_status,
            ytdlp_status: engine.ytdlp_install_status.lock().map(|s| s.clone()).unwrap_or_default(),
            is_fullscreen,
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
                    sleeping: t.sleeping,
                })
                .collect(),
            active_tab: active_idx,
            split_tab,
            active_url,
            active_host,
            active_zoom,
            active_reader,
            site_data_cleared,
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
            if let Ok(mut prev) = last_chrome_json.lock() {
                if *prev == json {
                    return;
                }
                *prev = json.clone();
            }
            let js = format!("if (window.renderChromeState) window.renderChromeState({});", json);
            let _ = chrome_wv.evaluate_script(&js);
        }
    };

    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
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
                    let _ = proxy.send_event(AppEvent::ToggleFullscreen);
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

            Event::UserEvent(AppEvent::SleepCheck) => {
                let now = now_epoch_secs();
                let mut changed = false;
                for (idx, tab) in tabs.iter_mut().enumerate() {
                    let is_visible_pane = idx == active_tab_idx || split_tab_idx == Some(idx);
                    if !is_visible_pane
                        && !tab.pinned
                        && !tab.playing_audio
                        && !tab.sleeping
                        && tab.url != "shunno://newtab"
                        && now.saturating_sub(tab.last_active_secs) >= 900
                    {
                        suspend_tab_webview(&tab.webview);
                        tab.sleeping = true;
                        changed = true;
                    }
                }
                if changed {
                    push_chrome_state(
                        &window,
                        &chrome_wv,
                        &tabs,
                        active_tab_idx,
                        split_tab_idx,
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
                        site_data_cleared,
                        is_fullscreen,
                        &engine,
                    );
                }
            }

            Event::UserEvent(AppEvent::ToggleFullscreen) => {
                if last_fs_toggle.elapsed() < Duration::from_millis(300) {
                    return;
                }
                last_fs_toggle = std::time::Instant::now();
                is_fullscreen = !is_fullscreen;
                apply_native_fullscreen(&window, is_fullscreen);
                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                if is_fullscreen {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        if tab.url != "shunno://newtab" {
                            focus_webview_programmatically(&tab.webview);
                        } else {
                            focus_webview_programmatically(&chrome_wv);
                        }
                    }
                } else {
                    focus_webview_programmatically(&chrome_wv);
                }
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    split_tab_idx,
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
                    site_data_cleared,
                    is_fullscreen,
                    &engine,
                );
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
                    split_tab_idx,
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
                    site_data_cleared,
                    is_fullscreen,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::TabTitleChanged(tab_id, new_title)) => {
                if !new_title.trim().is_empty() {
                    if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                        tab.title = new_title;
                    }
                    push_chrome_state(
                        &window,
                        &chrome_wv,
                        &tabs,
                        active_tab_idx,
                        split_tab_idx,
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
                        site_data_cleared,
                        is_fullscreen,
                        &engine,
                    );
                }
            }

            Event::UserEvent(AppEvent::TabUrlChanged(tab_id, new_url)) => {
                if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.url = new_url.clone();
                    let h = extract_host(&new_url);
                    if !h.is_empty() {
                        if tab.title.is_empty() || tab.title == "New Tab" || tab.title.starts_with("http") {
                            tab.title = h.clone();
                        }
                        let saved_zoom = site_policies.get(&h).map(|p| p.zoom).unwrap_or(1.0);
                        if (tab.zoom - saved_zoom).abs() > 0.01 {
                            tab.zoom = saved_zoom;
                            apply_tab_zoom(&tab.webview, saved_zoom);
                        }
                    }
                }
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    split_tab_idx,
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
                    site_data_cleared,
                    is_fullscreen,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::TabZoomChanged(tab_id, new_zoom)) => {
                if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.zoom = new_zoom;
                    let host = extract_host(&tab.url);
                    if !host.is_empty() && tab.url != "shunno://newtab" {
                        let pol = site_policies.entry(host).or_default();
                        if (pol.zoom - new_zoom).abs() > 0.01 {
                            pol.zoom = new_zoom;
                            BrowserStorage::save_site_policies(&site_policies);
                        }
                    }
                }
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    split_tab_idx,
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
                    site_data_cleared,
                    is_fullscreen,
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
                    split_tab_idx,
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
                    site_data_cleared,
                    is_fullscreen,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::NativeStreamCaptured(tab_id, stream_url, ext, size_label)) => {
                if !sniffed_streams.iter().any(|s| s.url == stream_url) {
                    let tab_title = tabs
                        .iter()
                        .find(|t| t.id == tab_id)
                        .map(|t| t.title.clone())
                        .filter(|t| !t.is_empty() && t != "New Tab")
                        .unwrap_or_else(|| "Media Stream".to_string());
                    let display_name = format!("{} ({})", tab_title, ext.trim_start_matches('.'));
                    sniffed_streams.insert(
                        0,
                        SniffedStreamItem {
                            url: stream_url.clone(),
                            name: display_name.clone(),
                            size_label: size_label.clone(),
                        },
                    );
                    if sniffed_streams.len() > 25 {
                        sniffed_streams.truncate(25);
                    }

                    if let Some(tab) = tabs.iter().find(|t| t.id == tab_id) {
                        let payload = serde_json::json!({
                            "url": stream_url,
                            "ext": ext,
                            "name": display_name,
                            "size_label": size_label,
                        });
                        if let Ok(js_arg) = serde_json::to_string(&payload) {
                            let js = format!("if (window.__shunnoOnNativeStream) window.__shunnoOnNativeStream({});", js_arg);
                            let _ = tab.webview.evaluate_script(&js);
                        }
                    }

                    push_chrome_state(
                        &window,
                        &chrome_wv,
                        &tabs,
                        active_tab_idx,
                        split_tab_idx,
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
                        site_data_cleared,
                        is_fullscreen,
                        &engine,
                    );
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

            Event::UserEvent(AppEvent::NativeDownloadStarted {
                id,
                url,
                file_name,
                saved_path,
                total_bytes,
            }) => {
                engine.register_native_download(id, url, file_name, saved_path, total_bytes);
                drawer_open = true;
                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                let _ = chrome_wv.evaluate_script("if (window.openDrawer) window.openDrawer('downloads');");
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    split_tab_idx,
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
                    site_data_cleared,
                    is_fullscreen,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::NativeDownloadProgress {
                id,
                received,
                total,
                speed_bps,
            }) => {
                engine.update_native_download_progress(id, received, total, speed_bps);
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    split_tab_idx,
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
                    site_data_cleared,
                    is_fullscreen,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::NativeDownloadFinished {
                id,
                ok,
                final_path,
                err_msg,
            }) => {
                engine.finish_native_download(id, ok, final_path, err_msg);
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    split_tab_idx,
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
                    site_data_cleared,
                    is_fullscreen,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::NativeDownloadPaused { id, reason }) => {
                engine.set_native_status(id, "PAUSED", reason);
                let _ = proxy.send_event(AppEvent::RefreshChrome);
            }

            Event::UserEvent(AppEvent::AdBlocked) => {
                blocked_count += 1;
            }

            Event::UserEvent(AppEvent::DownloadIntercepted(dl_url)) => {
                let active_tab = tabs.get(active_tab_idx);
                let (ref_url, cookie_val, wv_opt) = active_tab
                    .map(|t| (Some(t.url.clone()), t.cookie.clone(), Some(&t.webview)))
                    .unwrap_or((None, None, None));
                dispatch_download_with_httponly_cookies(
                    wv_opt,
                    dl_url,
                    global_settings.default_threads,
                    ref_url,
                    cookie_val,
                    "CHECK",
                    None,
                    true,
                    proxy.clone(),
                );
            }

            Event::UserEvent(AppEvent::DownloadWithCookies {
                url,
                threads,
                referer,
                cookie,
                dup_mode,
                delay_secs,
                open_drawer,
            }) => {
                match engine.enqueue(&rt_handle, url, threads, referer, cookie, &dup_mode, delay_secs) {
                    EnqueueOutcome::Started => { duplicate_alert = None; }
                    EnqueueOutcome::DuplicateDetected(existing) => { duplicate_alert = Some(existing); }
                }
                if open_drawer {
                    drawer_open = true;
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                    let _ = chrome_wv.evaluate_script("if (window.openDrawer) window.openDrawer('downloads');");
                }
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    split_tab_idx,
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
                    site_data_cleared,
                    is_fullscreen,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::NewWindowBlockedOrOpened(opener_id, popup_url)) => {
                let opener_private = tabs.iter().find(|t| t.id == opener_id).map(|t| t.incognito).unwrap_or(false);
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
                    if let Ok(wv) = create_tab_webview(&window, &mut web_context, tid, &clean, &pol, &global_settings, &ipc_token, proxy.clone(), opener_private) {
                        tabs.push(BrowserTab {
                            id: tid,
                            title: clean.clone(),
                            url: clean,
                            cookie: None,
                            incognito: opener_private,
                            pinned: false,
                            playing_audio: false,
                            muted: false,
                            reader_active: false,
                            sleeping: false,
                            last_active_secs: now_epoch_secs(),
                            zoom: 1.0,
                            webview: wv,
                        });
                        active_tab_idx = tabs.len() - 1;
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                    }
                }
                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    split_tab_idx,
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
                    site_data_cleared,
                    is_fullscreen,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::ContentIpc(tab_id, raw_msg)) => {
                // Every content message must carry the per-launch token. There is deliberately no
                // token-free prefix: content IPC is page-reachable, so a bypass prefix is a bypass for every site.
                let Some((tok, msg)) = raw_msg.split_once('|') else {
                    return;
                };
                if !constant_time_eq(tok, &ipc_token) {
                    return;
                }
                let page_url = tabs.iter().find(|t| t.id == tab_id).map(|t| t.url.clone());

                if msg == "AD_BLOCKED" {
                    blocked_count += 1;
                } else if let Some(fc) = msg.strip_prefix("FIND_COUNT:") {
                    if let Some((cur_s, tot_s)) = fc.split_once('|') {
                        find_current = cur_s.parse().unwrap_or(0);
                        find_total = tot_s.parse().unwrap_or(0);
                    }
                } else if msg.starts_with("SITE_DATA_CLEARED:") {
                    site_data_cleared = true;
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
                        if target_url.starts_with("http") && page_may_target(page_url.as_deref(), &target_url) {
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
                        // "page": the player streams via blob:/MSE, so let yt-dlp resolve the page URL.
                        let via_page = val.get("via").and_then(|v| v.as_str()) == Some("page");
                        let referer = val.get("referer").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let cookie = val.get("cookie").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
                        if dl_url.starts_with("http") && page_may_target(page_url.as_deref(), &dl_url) {
                            let wv_opt = tabs.iter().find(|t| t.id == tab_id).map(|t| &t.webview);
                            let routed = if via_page { format!("{}{}", downloader::YTDLP_PREFIX, dl_url) } else { dl_url };
                            dispatch_download_with_httponly_cookies(
                                wv_opt,
                                routed,
                                threads,
                                referer,
                                cookie,
                                "CHECK",
                                None,
                                true,
                                proxy.clone(),
                            );
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
                            let _ = proxy.send_event(AppEvent::ToggleFullscreen);
                        }
                        "next_tab" => {
                            if !tabs.is_empty() {
                                active_tab_idx = (active_tab_idx + 1) % tabs.len();
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            }
                        }
                        "prev_tab" => {
                            if !tabs.is_empty() {
                                active_tab_idx = if active_tab_idx == 0 { tabs.len() - 1 } else { active_tab_idx - 1 };
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            }
                        }
                        "t" => {
                            let start = "shunno://newtab".to_string();
                            let pol = SitePolicy::default();
                            let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                            if let Ok(wv) = create_tab_webview(&window, &mut web_context, tid, &start, &pol, &global_settings, &ipc_token, proxy.clone(), false) {
                                tabs.push(BrowserTab { id: tid, title: "New Tab".to_string(), url: start, cookie: None, incognito: false, pinned: false, playing_audio: false, muted: false, reader_active: false, sleeping: false, last_active_secs: now_epoch_secs(), zoom: 1.0, webview: wv });
                                active_tab_idx = tabs.len() - 1;
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
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
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            }
                        }
                        "k" => {
                            palette_open = true;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            let _ = chrome_wv.evaluate_script("if (window.openPalette) window.openPalette();");
                        }
                        "j" => {
                            drawer_open = true;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            let _ = chrome_wv.evaluate_script("if (window.openDrawer) window.openDrawer('downloads');");
                        }
                        _ => {}
                    }
                } else if let Some(html_payload) = msg.strip_prefix("OFFLINE_HTML:") {
                    // Only honoured right after the user pressed "Save page" for this tab.
                    if !pending_offline_saves.remove(&tab_id) {
                        return;
                    }
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
                            if u.starts_with("http") && page_may_target(page_url.as_deref(), &u) && !sniffed_streams.iter().any(|s| s.url == u) {
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
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
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
                        if let Some(ua_str) = val.get("ua").and_then(|v| v.as_str()) {
                            engine.set_user_agent(ua_str);
                        }
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
                                    let hist_snapshot = history.clone();
                                    rt_handle.spawn(async move {
                                        BrowserStorage::save_history(&hist_snapshot);
                                    });
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
                    split_tab_idx,
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
                    site_data_cleared,
                    is_fullscreen,
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
                        let host = extract_host(&tab.url);
                        if !host.is_empty() && tab.url != "shunno://newtab" {
                            site_policies.entry(host).or_default().zoom = tab.zoom;
                            BrowserStorage::save_site_policies(&site_policies);
                        }
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
                    let _ = proxy.send_event(AppEvent::ToggleFullscreen);
                } else if cmd.starts_with("DRAWER_OPEN:") {
                    drawer_open = true;
                    if cmd == "DRAWER_OPEN:controls" {
                        if let Ok(mut cs) = cached_scripts.lock() {
                            *cs = list_user_scripts(&global_settings.approved_script_hashes);
                        }
                    }
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                } else if cmd == "DRAWER_CLOSE" {
                    drawer_open = false;
                    site_data_cleared = false;
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
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
                    site_data_cleared = false;
                    drawer_open = false;
                    engine_menu_open = false;
                    palette_open = false;
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
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                } else if cmd == "NEW_TAB" || cmd == "NEW_INCOGNITO" {
                    let is_incog = cmd == "NEW_INCOGNITO";
                    let start = "shunno://newtab".to_string();
                    let pol = SitePolicy::default();
                    let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                    if let Ok(wv) = create_tab_webview(&window, &mut web_context, tid, &start, &pol, &global_settings, &ipc_token, proxy.clone(), is_incog) {
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
                            sleeping: false,
                            last_active_secs: now_epoch_secs(),
                            zoom: 1.0,
                            webview: wv,
                        });
                        active_tab_idx = tabs.len() - 1;
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                    }
                } else if cmd == "NEXT_TAB" {
                    if !tabs.is_empty() {
                        active_tab_idx = (active_tab_idx + 1) % tabs.len();
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                    }
                } else if cmd == "PREV_TAB" {
                    if !tabs.is_empty() {
                        active_tab_idx = if active_tab_idx == 0 { tabs.len() - 1 } else { active_tab_idx - 1 };
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
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
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            }
                        }
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("SWITCH_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < tabs.len() {
                            active_tab_idx = idx;
                            drawer_open = false;
                            engine_menu_open = false;
                            palette_open = false;
                            if split_tab_idx == Some(idx) {
                                split_tab_idx = None;
                            }
                            if tabs[idx].sleeping {
                                resume_tab_webview(&tabs[idx].webview);
                                tabs[idx].sleeping = false;
                            }
                            tabs[idx].last_active_secs = now_epoch_secs();
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
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
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
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
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                        }
                    }
                } else if cmd == "REOPEN_CLOSED_TAB" {
                    if let Some(saved) = closed_tabs.pop() {
                        let host = extract_host(&saved.url);
                        let pol = site_policies.get(&host).cloned().unwrap_or_default();
                        let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                        if let Ok(wv) = create_tab_webview(&window, &mut web_context, tid, &saved.url, &pol, &global_settings, &ipc_token, proxy.clone(), false) {
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
                                sleeping: false,
                                last_active_secs: now_epoch_secs(),
                                zoom: 1.0,
                                webview: wv,
                            });
                            active_tab_idx = tabs.len() - 1;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
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
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
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
                    match gkey {
                        "full_dark" => {
                            global_settings.full_dark_mode = !global_settings.full_dark_mode;
                            let js = format!(
                                "if (window.__shunnoSetDarkMode) window.__shunnoSetDarkMode({});",
                                global_settings.full_dark_mode
                            );
                            for t in &tabs {
                                if t.url != "shunno://newtab" {
                                    let _ = t.webview.evaluate_script(&js);
                                }
                            }
                        }
                        "drop_basket" => {
                            global_settings.drop_basket_enabled = !global_settings.drop_basket_enabled;
                            let js = format!(
                                "if (window.__shunnoSetDropBasket) window.__shunnoSetDropBasket({});",
                                global_settings.drop_basket_enabled
                            );
                            for t in &tabs {
                                if t.url != "shunno://newtab" {
                                    let _ = t.webview.evaluate_script(&js);
                                }
                            }
                        }
                        "bg_audio" => global_settings.background_media_play = !global_settings.background_media_play,
                        "auto_pip" => global_settings.auto_pip_on_blur = !global_settings.auto_pip_on_blur,
                        "auto_clean" => {
                            global_settings.auto_clean_completed = !global_settings.auto_clean_completed;
                            engine.auto_clean_completed.store(global_settings.auto_clean_completed, Ordering::SeqCst);
                        }
                        "strip_tracking" => global_settings.strip_tracking_params = !global_settings.strip_tracking_params,
                        "block_popups" => global_settings.block_popups = !global_settings.block_popups,
                        "save_passwords" | "strict_tracking" => {
                            if gkey == "save_passwords" {
                                global_settings.save_passwords = !global_settings.save_passwords;
                            } else {
                                global_settings.strict_tracking = !global_settings.strict_tracking;
                            }
                            for tab in tabs.iter() {
                                apply_privacy_settings(
                                    &tab.webview,
                                    global_settings.save_passwords && !tab.incognito,
                                    global_settings.strict_tracking,
                                );
                            }
                        }
                        _ => {}
                    }
                    BrowserStorage::save_global_settings(&global_settings);
                } else if cmd == "CLEAR_PASSWORDS" {
                    if let Some(tab) = tabs.iter().find(|t| !t.incognito) {
                        clear_saved_passwords(&tab.webview);
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
                        if let Ok(new_wv) = create_tab_webview(&window, &mut web_context, tid, &current_url, &updated_pol, &global_settings, &ipc_token, proxy.clone(), is_incog) {
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
                                sleeping: false,
                                last_active_secs: now_epoch_secs(),
                                zoom: tab.zoom,
                                webview: new_wv,
                            };
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                        }
                    }
                } else if let Some(spd) = cmd.strip_prefix("SET_PLAYBACK_SPEED:") {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let js = format!("document.querySelectorAll('video,audio').forEach(m => m.playbackRate = {});", spd);
                        let _ = tab.webview.evaluate_script(&js);
                    }
                } else if cmd == "CLEAR_SITE_DATA" {
                    if let Some(tab) = tabs.get_mut(active_tab_idx) {
                        tab.cookie = None;
                        clear_httponly_cookies_for_url(&tab.webview, &tab.url);
                        let _ = tab.webview.evaluate_script("if (window.__shunnoClearSiteStorage) window.__shunnoClearSiteStorage();");
                    }
                } else if cmd == "READER_MODE" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let _ = tab.webview.evaluate_script("if (window.__shunnoToggleReader) window.__shunnoToggleReader();");
                    }
                } else if let Some(em_flag) = cmd.strip_prefix("ENGINE_MENU:") {
                    engine_menu_open = em_flag == "1";
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                } else if let Some(p_flag) = cmd.strip_prefix("PALETTE:") {
                    palette_open = p_flag == "1";
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                } else if cmd == "TOGGLE_SPLIT_VIEW" {
                    if split_tab_idx.is_some() {
                        split_tab_idx = None;
                    } else if tabs.len() >= 2 {
                        let peer = if active_tab_idx + 1 < tabs.len() { active_tab_idx + 1 } else { active_tab_idx - 1 };
                        if tabs[peer].sleeping {
                            resume_tab_webview(&tabs[peer].webview);
                            tabs[peer].sleeping = false;
                        }
                        tabs[peer].last_active_secs = now_epoch_secs();
                        split_tab_idx = Some(peer);
                    }
                    drawer_open = false;
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                } else if cmd == "SLEEP_INACTIVE_TABS" {
                    for (idx, tab) in tabs.iter_mut().enumerate() {
                        let is_visible_pane = idx == active_tab_idx || split_tab_idx == Some(idx);
                        if !is_visible_pane && !tab.pinned && !tab.playing_audio && !tab.sleeping && tab.url != "shunno://newtab" {
                            suspend_tab_webview(&tab.webview);
                            tab.sleeping = true;
                        }
                    }
                } else if cmd == "INSTALL_FFMPEG" {
                    engine.install_ffmpeg_bundle(&rt_handle);
                } else if cmd == "INSTALL_YTDLP" {
                    engine.install_ytdlp_bundle(&rt_handle);
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
                        pending_offline_saves.insert(tab.id);
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
                } else if let Some(snap_payload) = cmd.strip_prefix("SNAP_WIN:") {
                    let parts: Vec<i32> = snap_payload.split('|').filter_map(|s| s.parse().ok()).collect();
                    if parts.len() == 4 {
                        engine_menu_open = false;
                        snap_window_to_fraction(&window, parts[0], parts[1], parts[2], parts[3]);
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                    }
                } else if cmd == "PRINT_TO_PDF" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        if tab.url != "shunno://newtab" {
                            print_tab_to_pdf(&tab.webview, &tab.title);
                        }
                    }
                } else if cmd == "CAPTURE_SCREENSHOT" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        if tab.url != "shunno://newtab" {
                            capture_tab_screenshot_to_file(&tab.webview);
                        }
                    }
                } else if let Some(dir_str) = cmd.strip_prefix("SET_DOWNLOAD_DIR:") {
                    let trimmed = dir_str.trim();
                    if trimmed.is_empty() {
                        global_settings.custom_downloads_dir = None;
                    } else {
                        let _ = fs::create_dir_all(trimmed);
                        global_settings.custom_downloads_dir = Some(trimmed.to_string());
                    }
                    BrowserStorage::save_global_settings(&global_settings);
                } else if cmd == "EXPORT_DL_CSV" {
                    if let Ok(csv_path) = engine.export_history_csv() {
                        let _ = Command::new("explorer.exe")
                            .args(["/select,", &csv_path.to_string_lossy()])
                            .spawn();
                    }
                } else if cmd == "DUPLICATE_TAB" {
                    if let Some(src) = tabs.get(active_tab_idx) {
                        let dup_url = src.url.clone();
                        let dup_title = src.title.clone();
                        let dup_incog = src.incognito;
                        let host = extract_host(&dup_url);
                        let pol = site_policies.get(&host).cloned().unwrap_or_default();
                        let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                        if let Ok(wv) = create_tab_webview(&window, &mut web_context, tid, &dup_url, &pol, &global_settings, &ipc_token, proxy.clone(), dup_incog) {
                            let insert_at = (active_tab_idx + 1).min(tabs.len());
                            tabs.insert(insert_at, BrowserTab {
                                id: tid,
                                title: dup_title,
                                url: dup_url,
                                cookie: None,
                                incognito: dup_incog,
                                pinned: false,
                                playing_audio: false,
                                muted: false,
                                reader_active: false,
                                sleeping: false,
                                last_active_secs: now_epoch_secs(),
                                zoom: pol.zoom,
                                webview: wv,
                            });
                            active_tab_idx = insert_at;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                        }
                    }
                } else if cmd == "CLEAR_COMPLETED_DL" || cmd == "CLEAR_FINISHED_DL" {
                    engine.clear_finished();
                } else if cmd == "DISMISS_DUP" {
                    duplicate_alert = None;
                } else if let Some(ren_url) = cmd.strip_prefix("DUP_RENAME:") {
                    duplicate_alert = None;
                    let (ref_url, cookie_val, wv_opt) = tabs
                        .get(active_tab_idx)
                        .map(|t| (Some(t.url.clone()), t.cookie.clone(), Some(&t.webview)))
                        .unwrap_or((None, None, None));
                    dispatch_download_with_httponly_cookies(
                        wv_opt,
                        ren_url.to_string(),
                        global_settings.default_threads,
                        ref_url,
                        cookie_val,
                        "RENAME",
                        None,
                        false,
                        proxy.clone(),
                    );
                } else if let Some(ov_url) = cmd.strip_prefix("DUP_OVERWRITE:") {
                    duplicate_alert = None;
                    let (ref_url, cookie_val, wv_opt) = tabs
                        .get(active_tab_idx)
                        .map(|t| (Some(t.url.clone()), t.cookie.clone(), Some(&t.webview)))
                        .unwrap_or((None, None, None));
                    dispatch_download_with_httponly_cookies(
                        wv_opt,
                        ov_url.to_string(),
                        global_settings.default_threads,
                        ref_url,
                        cookie_val,
                        "OVERWRITE",
                        None,
                        false,
                        proxy.clone(),
                    );
                } else if let Some(payload) = cmd.strip_prefix("ENQUEUE_SCHED:") {
                    let parts: Vec<&str> = payload.splitn(3, '|').collect();
                    if parts.len() == 3 {
                        let threads = parts[0].parse::<usize>().unwrap_or(16);
                        let delay_s = parts[1].parse::<u64>().ok().filter(|&d| d > 0);
                        let raw_urls = parts[2];
                        let (ref_url, cookie_val, wv_opt) = tabs
                            .get(active_tab_idx)
                            .map(|t| (Some(t.url.clone()), t.cookie.clone(), Some(&t.webview)))
                            .unwrap_or((None, None, None));
                        // The Get box advertises batch patterns like track_[01-12].flac, so expand them here too.
                        let expanded: Vec<String> = raw_urls
                            .lines()
                            .map(|l| l.trim())
                            .filter(|l| !l.is_empty())
                            .flat_map(|l| expand_wildcard_range(l).unwrap_or_else(|| vec![l.to_string()]))
                            .collect();
                        for line in expanded {
                            dispatch_download_with_httponly_cookies(
                                wv_opt,
                                line,
                                threads,
                                ref_url.clone(),
                                cookie_val.clone(),
                                "CHECK",
                                delay_s,
                                false,
                                proxy.clone(),
                            );
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
                        let (ref_url, cookie_val, wv_opt) = tabs
                            .get(active_tab_idx)
                            .map(|t| (Some(t.url.clone()), t.cookie.clone(), Some(&t.webview)))
                            .unwrap_or((None, None, None));
                        for line in raw_urls.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
                            if let Some(expanded_urls) = expand_wildcard_range(line) {
                                for expanded in expanded_urls {
                                    dispatch_download_with_httponly_cookies(
                                        wv_opt,
                                        expanded,
                                        threads,
                                        ref_url.clone(),
                                        cookie_val.clone(),
                                        "CHECK",
                                        None,
                                        false,
                                        proxy.clone(),
                                    );
                                }
                                continue;
                            }
                            dispatch_download_with_httponly_cookies(
                                wv_opt,
                                line.to_string(),
                                threads,
                                ref_url.clone(),
                                cookie_val.clone(),
                                "CHECK",
                                None,
                                false,
                                proxy.clone(),
                            );
                        }
                    }
                } else if let Some(id_str) = cmd.strip_prefix("DL_PAUSE:") {
                    if let Ok(id) = id_str.parse::<u64>() {
                        let is_native = engine.task_kind(id).as_deref() == Some(downloader::KIND_NATIVE);
                        if is_native && native_op_call(id, "pause") {
                            engine.set_native_status(id, "PAUSED", None);
                        } else if !is_native {
                            engine.pause(id);
                        }
                    }
                } else if let Some(id_str) = cmd.strip_prefix("DL_RESUME:") {
                    if let Ok(id) = id_str.parse::<u64>() {
                        let is_native = engine.task_kind(id).as_deref() == Some(downloader::KIND_NATIVE);
                        if is_native && native_op_call(id, "resume") {
                            engine.set_native_status(id, "DOWNLOADING", None);
                        } else {
                            engine.resume_or_restart(&rt_handle, id, None);
                        }
                    }
                } else if let Some(payload) = cmd.strip_prefix("DL_FIX:") {
                    if let Some((id_str, new_url)) = payload.split_once('|') {
                        if let Ok(id) = id_str.parse::<u64>() {
                            native_op_call(id, "cancel");
                            engine.resume_or_restart(&rt_handle, id, Some(new_url.to_string()));
                        }
                    }
                } else if let Some(id_str) = cmd.strip_prefix("DL_DEL:") {
                    if let Ok(id) = id_str.parse::<u64>() {
                        native_op_call(id, "cancel");
                        engine.delete(id);
                    }
                } else if let Some(path) = cmd.strip_prefix("DL_SHOW:") {
                    // Only reveal files that belong to a known download.
                    let known = engine
                        .tasks
                        .lock()
                        .map(|l| l.iter().any(|t| t.saved_path.as_deref() == Some(path)))
                        .unwrap_or(false);
                    if known && std::path::Path::new(path).exists() {
                        let _ = Command::new("explorer.exe").arg(format!("/select,{}", path)).spawn();
                    }
                }

                push_chrome_state(
                    &window,
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    split_tab_idx,
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
                    site_data_cleared,
                    is_fullscreen,
                    &engine,
                );
            }

            _ => {}
        }
    });
}
