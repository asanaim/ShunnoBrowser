#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod downloader;
mod sanitizer;
mod state;
mod ui_chrome;
mod userscripts;

use downloader::{DownloadTask, EnqueueOutcome, ShunnoGetDesktop, StreamProbeResult};
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
    RefreshChrome,
}

struct BrowserTab {
    id: usize,
    title: String,
    url: String,
    cookie: Option<String>,
    incognito: bool,
    playing_audio: bool,
    muted: bool,
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
    playing_audio: bool,
    muted: bool,
}

#[derive(Serialize)]
struct ChromeSnapshot {
    version: &'static str,
    ffmpeg_available: bool,
    tabs: Vec<TabSummary>,
    active_tab: usize,
    active_url: String,
    active_host: String,
    active_zoom: f64,
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
            let handler = WebResourceRequestedEventHandler::create(Box::new(move |_sender, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let empty_resp: ICoreWebView2WebResourceResponse = env.CreateWebResourceResponse(
                    None::<&IStream>,
                    403,
                    w!("Blocked by Shunno AdBlock"),
                    PCWSTR::null(),
                )?;
                let _ = args.SetResponse(&empty_resp);
                let _ = proxy.send_event(AppEvent::ContentIpc(tab_id, "INTERNAL|AD_BLOCKED".to_string()));
                Ok(())
            }));

            let _ = core.add_WebResourceRequested(&handler, &mut token);
        }

        // 3. Native WebView2 DownloadStarting Interceptor (routes all browser downloads to Shunno Get)
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
fn load_embedded_window_icon() -> Option<Icon> {
    // Decode the 32x32 uncompressed BGRA entry from assets/app.ico for native taskbar & Alt+Tab icon
    const ICO_BYTES: &[u8] = include_bytes!("../assets/app.ico");
    if ICO_BYTES.len() < 22 {
        return None;
    }
    let count = u16::from_le_bytes([ICO_BYTES[4], ICO_BYTES[5]]) as usize;
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

        if (w == 32 || w == 48) && img_offset + 40 + (w * h * 4) as usize <= ICO_BYTES.len() && bytes_in_res >= 40 {
            let pixel_start = img_offset + 40;
            let row_bytes = (w * 4) as usize;
            let mut rgba = vec![0u8; (w * h * 4) as usize];
            // BMP inside ICO is stored bottom-up in BGRA order; convert to top-down RGBA
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
    None
}

#[cfg(windows)]
fn capture_tab_preview_to_chrome(tab_wv: &WebView, chrome_wv: &WebView) {
    use webview2_com::CapturePreviewCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG;
    use windows::Win32::System::Com::CreateStreamOnHGlobal;
    use windows::Win32::System::Com::StructuredStorage::GetHGlobalFromStream;
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use wry::WebViewExtWindows;

    let controller = tab_wv.controller();
    unsafe {
        let Ok(core) = controller.CoreWebView2() else { return; };
        let Ok(stream) = CreateStreamOnHGlobal(None, true) else { return; };
        let stream_clone = stream.clone();
        // Evaluate via raw pointer sendable wrapper on UI thread
        let chrome_ptr = chrome_wv as *const WebView as usize;
        let handler = CapturePreviewCompletedHandler::create(Box::new(move |res| {
            if res.is_ok() {
                if let Ok(hglobal) = GetHGlobalFromStream(&stream_clone) {
                    let size = GlobalSize(hglobal);
                    let ptr = GlobalLock(hglobal);
                    if !ptr.is_null() && size > 0 {
                        let slice = std::slice::from_raw_parts(ptr as *const u8, size);
                        let b64 = base64_encode(slice);
                        let _ = GlobalUnlock(hglobal);
                        let wv_ref = &*(chrome_ptr as *const WebView);
                        let js = format!(
                            "if (window.setOverlayBackdropImage) window.setOverlayBackdropImage('data:image/png;base64,{}');",
                            b64
                        );
                        let _ = wv_ref.evaluate_script(&js);
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
fn capture_tab_preview_to_chrome(_tab_wv: &WebView, _chrome_wv: &WebView) {}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(((data.len() + 2) / 3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn layout_webviews(window: &Window, chrome_wv: &WebView, tabs: &[BrowserTab], active_idx: usize, drawer_open: bool) {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let w = size.width.max(400.0);
    let h = size.height.max(300.0);
    let content_h = (h - CHROME_BAR_HEIGHT).max(100.0);

    let is_home = tabs
        .get(active_idx)
        .map(|t| t.url == "shunno://newtab")
        .unwrap_or(true);

    let chrome_h = if drawer_open || is_home { h } else { CHROME_BAR_HEIGHT };
    let _ = chrome_wv.set_bounds(Rect {
        position: LogicalPosition::new(0.0, 0.0).into(),
        size: LogicalSize::new(w, chrome_h).into(),
    });

    for (idx, tab) in tabs.iter().enumerate() {
        let _ = tab.webview.set_bounds(Rect {
            position: LogicalPosition::new(0.0, CHROME_BAR_HEIGHT).into(),
            size: LogicalSize::new(w, content_h).into(),
        });
        let show_web = !drawer_open && idx == active_idx && tab.url != "shunno://newtab";
        let _ = tab.webview.set_visible(show_web);
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
    let engine = Arc::new(ShunnoGetDesktop::new(
        global_settings.speed_limit_bps,
        global_settings.auto_clean_completed,
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
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: LogicalSize::new(1240.0, CHROME_BAR_HEIGHT).into(),
        })
        .with_html(ui_chrome::CHROME_HTML)
        .with_ipc_handler(move |req| {
            let _ = chrome_proxy.send_event(AppEvent::ChromeIpc(req.body().clone()));
        })
        .build()?;

    let next_tab_id = Arc::new(AtomicUsize::new(1));
    let mut tabs: Vec<BrowserTab> = Vec::new();
    let mut active_tab_idx: usize = 0;
    let mut drawer_open = false;
    let mut blocked_count: usize = 0;
    let mut sniffed_streams: Vec<SniffedStreamItem> = Vec::new();
    let mut duplicate_alert: Option<DownloadTask> = None;

    // Start local HTTP IDM/ShunnoGet listener on http://127.0.0.1:45780/add?url=...
    let hook_proxy = proxy.clone();
    rt_handle.spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;
        if let Ok(listener) = TcpListener::bind("127.0.0.1:45780").await {
            while let Ok((mut stream, _)) = listener.accept().await {
                let px = hook_proxy.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    if let Ok(n) = stream.read(&mut buf).await {
                        let req_str = String::from_utf8_lossy(&buf[..n]);
                        if let Some(first_line) = req_str.lines().next() {
                            if let Some(path) = first_line.split_whitespace().nth(1) {
                                if let Ok(parsed) = url::Url::parse(&format!("http://127.0.0.1{}", path)) {
                                    if let Some((_, dl_url)) = parsed.query_pairs().find(|(k, _)| k == "url") {
                                        let u = dl_url.into_owned();
                                        if u.starts_with("http") {
                                            let _ = px.send_event(AppEvent::DownloadIntercepted(u));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let resp = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 2\r\n\r\nOK";
                    let _ = stream.write_all(resp.as_bytes()).await;
                });
            }
        }
    });

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
                playing_audio: false,
                muted: false,
                zoom: 1.0,
                webview: wv,
            });
        }
    }

    // Lightweight periodic refresh ONLY when a download is actively running
    let ticker_proxy = proxy.clone();
    let ticker_engine = engine.clone();
    rt_handle.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(750)).await;
            let has_active = ticker_engine
                .tasks
                .lock()
                .map(|l| l.iter().any(|t| t.status == "DOWNLOADING" || t.status == "MUXING"))
                .unwrap_or(false);
            if has_active {
                let _ = ticker_proxy.send_event(AppEvent::RefreshChrome);
            }
        }
    });

    let push_chrome_state = |chrome_wv: &WebView,
                             tabs: &[BrowserTab],
                             active_idx: usize,
                             site_policies: &HashMap<String, SitePolicy>,
                             global_settings: &GlobalSettings,
                             blocked_count: usize,
                             sniffed_streams: &[SniffedStreamItem],
                             bookmarks: &[BookmarkEntry],
                             history: &[HistoryEntry],
                             duplicate_alert: &Option<DownloadTask>,
                             engine: &ShunnoGetDesktop| {
        let active_url = tabs.get(active_idx).map(|t| t.url.clone()).unwrap_or_default();
        let active_zoom = tabs.get(active_idx).map(|t| t.zoom).unwrap_or(1.0);
        let active_host = extract_host(&active_url);
        let site = site_policies.get(&active_host).cloned().unwrap_or_default();
        let downloads = engine.tasks.lock().map(|l| l.clone()).unwrap_or_default();
        let scripts = list_user_scripts(&global_settings.approved_script_hashes);

        let snap = ChromeSnapshot {
            version: APP_VERSION,
            ffmpeg_available: BrowserStorage::find_ffmpeg().is_some(),
            tabs: tabs
                .iter()
                .map(|t| TabSummary {
                    id: t.id,
                    title: t.title.clone(),
                    url: t.url.clone(),
                    incognito: t.incognito,
                    playing_audio: t.playing_audio,
                    muted: t.muted,
                })
                .collect(),
            active_tab: active_idx,
            active_url,
            active_host,
            active_zoom,
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

    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
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
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
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

            Event::UserEvent(AppEvent::TabAudioChanged(tab_id, playing)) => {
                if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.playing_audio = playing;
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
                match engine.enqueue(&rt_handle, dl_url, global_settings.default_threads, ref_url, cookie_val, false) {
                    EnqueueOutcome::Started(_) => { duplicate_alert = None; }
                    EnqueueOutcome::DuplicateDetected(existing) => { duplicate_alert = Some(existing); }
                }
                drawer_open = true;
                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                let _ = chrome_wv.evaluate_script("toggleDrawer('downloads');");
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
                            playing_audio: false,
                            muted: false,
                            zoom: 1.0,
                            webview: wv,
                        });
                        active_tab_idx = tabs.len() - 1;
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                    }
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
                            match engine.enqueue(&rt_handle, dl_url, threads, referer, cookie, false) {
                                EnqueueOutcome::Started(_) => { duplicate_alert = None; }
                                EnqueueOutcome::DuplicateDetected(existing) => { duplicate_alert = Some(existing); }
                            }
                            drawer_open = true;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                            let _ = chrome_wv.evaluate_script("toggleDrawer('downloads');");
                        }
                    }
                } else if let Some(key_cmd) = msg.strip_prefix("KEY:") {
                    match key_cmd {
                        "zoom_in" => {
                            if let Some(tab) = tabs.get_mut(active_tab_idx) {
                                tab.zoom = (tab.zoom + 0.1).min(3.0);
                                let _ = tab.webview.zoom(tab.zoom);
                            }
                        }
                        "zoom_out" => {
                            if let Some(tab) = tabs.get_mut(active_tab_idx) {
                                tab.zoom = (tab.zoom - 0.1).max(0.4);
                                let _ = tab.webview.zoom(tab.zoom);
                            }
                        }
                        "zoom_reset" => {
                            if let Some(tab) = tabs.get_mut(active_tab_idx) {
                                tab.zoom = 1.0;
                                let _ = tab.webview.zoom(tab.zoom);
                            }
                        }
                        "f11" => {
                            let is_fs = window.fullscreen().is_some();
                            window.set_fullscreen(if is_fs { None } else { Some(Fullscreen::Borderless(None)) });
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                        }
                        "t" => {
                            let start = "shunno://newtab".to_string();
                            let pol = SitePolicy::default();
                            let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                            if let Ok(wv) = create_tab_webview(&window, tid, &start, &pol, &global_settings, &ipc_token, proxy.clone()) {
                                tabs.push(BrowserTab { id: tid, title: "New Tab".to_string(), url: start, cookie: None, incognito: false, playing_audio: false, muted: false, zoom: 1.0, webview: wv });
                                active_tab_idx = tabs.len() - 1;
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
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
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                            }
                        }
                        "j" => {
                            drawer_open = true;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
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
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
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
                        let _ = tab.webview.zoom(tab.zoom);
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
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                } else if cmd.starts_with("DRAWER_OPEN:") {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        if tab.url != "shunno://newtab" {
                            capture_tab_preview_to_chrome(&tab.webview, &chrome_wv);
                        } else {
                            let _ = chrome_wv.evaluate_script("if (window.setOverlayBackdropImage) window.setOverlayBackdropImage('');");
                        }
                    }
                    drawer_open = true;
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                } else if cmd == "DRAWER_CLOSE" {
                    drawer_open = false;
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
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
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
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
                            playing_audio: false,
                            muted: false,
                            zoom: 1.0,
                            webview: wv,
                        });
                        active_tab_idx = tabs.len() - 1;
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("SWITCH_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < tabs.len() {
                            active_tab_idx = idx;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
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
                        tabs.remove(active_tab_idx);
                        if active_tab_idx >= tabs.len() {
                            active_tab_idx = tabs.len() - 1;
                        }
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("CLOSE_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if tabs.len() <= 1 {
                            BrowserStorage::save_session(&[]);
                            *control_flow = ControlFlow::Exit;
                            return;
                        } else if idx < tabs.len() {
                            tabs.remove(idx);
                            if active_tab_idx >= tabs.len() {
                                active_tab_idx = tabs.len() - 1;
                            }
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
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
                                    playing_audio: false,
                                    muted: is_muted,
                                    zoom: tab.zoom,
                                    webview: new_wv,
                                };
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
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
                            if (tab.zoom - 1.0).abs() > f64::EPSILON { let _ = new_wv.zoom(tab.zoom); }
                            tabs[active_tab_idx] = BrowserTab {
                                id: tid,
                                title: tab.title.clone(),
                                url: current_url,
                                cookie: prev_cookie,
                                incognito: is_incog,
                                playing_audio: false,
                                muted: is_muted,
                                zoom: tab.zoom,
                                webview: new_wv,
                            };
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                        }
                    }
                } else if let Some(spd) = cmd.strip_prefix("SET_PLAYBACK_SPEED:") {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let js = format!("document.querySelectorAll('video,audio').forEach(m => m.playbackRate = {});", spd);
                        let _ = tab.webview.evaluate_script(&js);
                    }
                } else if cmd == "READER_MODE" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let js = r#"
                            (function() {
                                const text = (document.querySelector('article') || document.body).innerText;
                                document.body.innerHTML = '<div style="max-width:720px;margin:36px auto;padding:20px;font:19px/1.7 system-ui,Segoe UI,sans-serif;color:#1C1C1E;background:#F2F2F7;">' +
                                    text.split('\n').filter(p => p.trim().length > 0).map(p => '<p style="margin-bottom:16px;">' + p + '</p>').join('') + '</div>';
                            })();
                        "#;
                        let _ = tab.webview.evaluate_script(js);
                    }
                } else if let Some(query) = cmd.strip_prefix("FIND:") {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let escaped = query.replace('\\', "\\\\").replace('\'', "\\'");
                        let js = format!("if ('{}') window.find('{}', false, false, true);", escaped, escaped);
                        let _ = tab.webview.evaluate_script(&js);
                    }
                } else if cmd == "FIND_NEXT" || cmd == "FIND_PREV" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let backwards = if cmd == "FIND_PREV" { "true" } else { "false" };
                        let js = format!("window.find('', false, {}, true);", backwards);
                        let _ = tab.webview.evaluate_script(&js);
                    }
                } else if cmd == "ADD_BOOKMARK" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        if !bookmarks.iter().any(|b| b.url == tab.url) {
                            let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                            bookmarks.insert(0, BookmarkEntry {
                                title: tab.title.clone(),
                                url: tab.url.clone(),
                                created_at: now,
                            });
                            BrowserStorage::save_bookmarks(&bookmarks);
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
                    engine.clear_completed();
                } else if cmd == "DISMISS_DUP" {
                    duplicate_alert = None;
                } else if let Some(force_url) = cmd.strip_prefix("FORCE_DL:") {
                    duplicate_alert = None;
                    let (ref_url, cookie_val) = tabs
                        .get(active_tab_idx)
                        .map(|t| (Some(t.url.clone()), t.cookie.clone()))
                        .unwrap_or((None, None));
                    let _ = engine.enqueue(&rt_handle, force_url.to_string(), global_settings.default_threads, ref_url, cookie_val, true);
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
                                        match engine.enqueue(&rt_handle, expanded, threads, ref_url.clone(), cookie_val.clone(), false) {
                                            EnqueueOutcome::Started(_) => {}
                                            EnqueueOutcome::DuplicateDetected(ex) => { duplicate_alert = Some(ex); }
                                        }
                                    }
                                    continue;
                                }
                            }
                            match engine.enqueue(&rt_handle, line.to_string(), threads, ref_url.clone(), cookie_val.clone(), false) {
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

            _ => {}
        }
    });
}
