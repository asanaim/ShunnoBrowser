#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod downloader;
mod state;
mod ui_chrome;

use chrono::Utc;
use downloader::{format_bytes, DownloadManager, DEFAULT_UA};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use state::{
    AppStorage, Bookmark, DownloadItem, HistoryEntry, SavedSession, SearchEngine, SitePermissions,
    TabSession,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use url::Url;
use uuid::Uuid;
use wry::dpi::{LogicalPosition, LogicalSize as WrySize};
use wry::{Rect, WebView, WebViewBuilder};

#[cfg(target_os = "windows")]
use wry::WebViewExtWindows;

const CHROME_HEIGHT: f64 = 88.0;

#[derive(Debug, Clone)]
enum UserEvent {
    ChromeIpc(String),
    PageIpc { tab_id: String, message: String },
    NetworkMediaSniffed {
        tab_id: String,
        url: String,
        kind: String,
        referer: String,
    },
    RefreshChrome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DetectedMedia {
    url: String,
    kind: String,
    label: String,
    referer: String,
}

struct TabItem {
    id: String,
    title: String,
    url: String,
    is_incognito: bool,
    webview: WebView,
    detected_media: Vec<DetectedMedia>,
    last_cookie: String,
    last_video_stream: Option<String>,
    last_audio_stream: Option<String>,
}

#[derive(Serialize)]
struct ChromeRenderPayload<'a> {
    tabs: Vec<TabDto<'a>>,
    active_tab_id: &'a str,
    active_url: &'a str,
    search_engines: &'a [SearchEngine],
    active_engine_id: &'a str,
    full_dark_mode: bool,
    floating_basket_enabled: bool,
    strip_youtube_ads: bool,
    max_segments: usize,
    download_dir: &'a str,
    permissions: SitePermissions,
    detected_media: &'a [DetectedMedia],
    dash_pair: bool,
    downloads: &'a [DownloadItem],
    bookmarks: &'a [Bookmark],
    history: &'a [HistoryEntry],
}

#[derive(Serialize)]
struct TabDto<'a> {
    id: &'a str,
    title: &'a str,
    url: &'a str,
    is_incognito: bool,
}

fn host_from_url(raw: &str) -> String {
    Url::parse(raw)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
        .unwrap_or_else(|| "default".to_string())
}

fn normalize_omnibox_input(input: &str, search_template: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "https://duckduckgo.com".to_string();
    }
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("file://")
        || trimmed.starts_with("about:")
    {
        return trimmed.to_string();
    }
    if !trimmed.contains(' ') && trimmed.contains('.') {
        return format!("https://{}", trimmed);
    }
    let encoded = utf8_percent_encode(trimmed, NON_ALPHANUMERIC).to_string();
    search_template.replace("%s", &encoded)
}

fn build_page_init_script(
    perms: &SitePermissions,
    full_dark_mode: bool,
    strip_youtube_ads: bool,
    floating_basket: bool,
) -> String {
    let dark_on = full_dark_mode || perms.dark_mode_override;
    format!(
        r##"(function() {{
  if (window.__shunnoInjected) return;
  window.__shunnoInjected = true;

  const SHUNNO_CFG = {{
    adBlock: {ad_block},
    stripYtAds: {yt_ads},
    darkMode: {dark_mode},
    readerMode: {reader_mode},
    floatingBasket: {basket}
  }};

  function postToHost(payload) {{
    try {{
      if (window.ipc && typeof window.ipc.postMessage === 'function') {{
        window.ipc.postMessage(JSON.stringify(payload));
      }}
    }} catch (_) {{}}
  }}

  // 1. Report Title, URL & Active Session Cookies to Rust Host
  function syncPageContext() {{
    postToHost({{
      type: 'PAGE_CONTEXT',
      title: document.title || location.hostname || 'New Tab',
      url: location.href,
      cookie: document.cookie || ''
    }});
  }}
  window.addEventListener('DOMContentLoaded', syncPageContext);
  window.addEventListener('load', syncPageContext);
  setInterval(syncPageContext, 2500);

  // 2. Keyboard Shortcuts Forwarder
  window.addEventListener('keydown', function(e) {{
    const ctrl = e.ctrlKey || e.metaKey;
    if (ctrl && e.key.toLowerCase() === 't') {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'NEW_TAB' }}); }}
    else if (ctrl && e.key.toLowerCase() === 'w') {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'CLOSE_ACTIVE_TAB' }}); }}
    else if (ctrl && e.key === 'Tab') {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: e.shiftKey ? 'PREV_TAB' : 'NEXT_TAB' }}); }}
    else if ((ctrl && e.key.toLowerCase() === 'l') || (e.altKey && e.key.toLowerCase() === 'd') || e.key === 'F6') {{
      e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'FOCUS_OMNIBOX' }});
    }}
    else if ((ctrl && e.key.toLowerCase() === 'r') || e.key === 'F5') {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'RELOAD' }}); }}
    else if (e.altKey && e.key === 'ArrowLeft') {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'GO_BACK' }}); }}
    else if (e.altKey && e.key === 'ArrowRight') {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'GO_FORWARD' }}); }}
    else if (ctrl && e.key.toLowerCase() === 'j') {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'OPEN_DOWNLOADS' }}); }}
    else if (ctrl && e.key.toLowerCase() === 'd' && !e.shiftKey) {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'ADD_BOOKMARK' }}); }}
    else if (ctrl && e.shiftKey && e.key.toLowerCase() === 'd') {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'TOGGLE_DARK' }}); }}
    else if (e.altKey && e.key.toLowerCase() === 'r') {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'TOGGLE_READER' }}); }}
    else if (ctrl && (e.key === '+' || e.key === '=')) {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'ZOOM_IN' }}); }}
    else if (ctrl && e.key === '-') {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'ZOOM_OUT' }}); }}
    else if (ctrl && e.key === '0') {{ e.preventDefault(); postToHost({{ type: 'SHORTCUT', action: 'ZOOM_RESET' }}); }}
  }}, true);

  // 3. YouTube Pre-Roll Ad Stripper (JSON.parse & ytInitialPlayerResponse Hook)
  if (SHUNNO_CFG.stripYtAds && location.hostname.includes('youtube.com')) {{
    const stripAdKeys = (obj) => {{
      if (!obj || typeof obj !== 'object') return obj;
      delete obj.adPlacements;
      delete obj.playerAds;
      delete obj.adSlots;
      delete obj.adBreakHeartbeatParams;
      return obj;
    }};

    const origParse = JSON.parse;
    JSON.parse = function(text, reviver) {{
      const parsed = origParse.call(this, text, reviver);
      return stripAdKeys(parsed);
    }};

    let ytPlayerResp = undefined;
    try {{
      Object.defineProperty(window, 'ytInitialPlayerResponse', {{
        configurable: true,
        get() {{ return ytPlayerResp; }},
        set(val) {{ ytPlayerResp = stripAdKeys(val); }}
      }});
    }} catch (_) {{}}

    setInterval(() => {{
      const skipBtn = document.querySelector('.ytp-skip-ad-button, .ytp-ad-skip-button, .ytp-ad-skip-button-modern');
      if (skipBtn) skipBtn.click();
      const player = document.querySelector('.html5-video-player');
      const video = document.querySelector('video');
      if (player && player.classList.contains('ad-showing') && video && isFinite(video.duration)) {{
        video.currentTime = video.duration;
      }}
    }}, 150);
  }}

  // 4. Full Dark Mode Injection for Webpages
  if (SHUNNO_CFG.darkMode) {{
    const applyDark = () => {{
      if (document.getElementById('__shunno_dark_style')) return;
      const st = document.createElement('style');
      st.id = '__shunno_dark_style';
      st.textContent = `
        html {{ background: #11161f !important; filter: invert(0.9) hue-rotate(180deg) !important; }}
        img, video, picture, canvas, iframe, [style*="background-image"] {{
          filter: invert(1) hue-rotate(180deg) !important;
        }}
      `;
      (document.head || document.documentElement).appendChild(st);
    }};
    if (document.head) applyDark();
    else window.addEventListener('DOMContentLoaded', applyDark);
  }}

  // 5. Reader Mode View
  if (SHUNNO_CFG.readerMode) {{
    window.addEventListener('DOMContentLoaded', () => {{
      const article = document.querySelector('article, main, [role="main"]') || document.body;
      const textHtml = article ? article.innerHTML : '';
      document.body.innerHTML = `
        <div style="max-width:740px;margin:36px auto;padding:24px 32px;font-family:Georgia,serif;font-size:19px;line-height:1.75;color:#e5e7eb;background:#161b22;border-radius:14px;">
          <h1 style="font-family:system-ui,sans-serif;margin-bottom:18px;">${{document.title}}</h1>
          ${{textHtml}}
        </div>`;
    }});
  }}

  // 6. Floating Shadow-DOM Download Menu + Live Size Probe on Video, Audio & Download Links
  const DOWNLOAD_EXT_RE = /\.(mp4|webm|mkv|mov|mp3|m4a|ogg|wav|flac|pdf|zip|rar|7z|tar|gz|exe|msi|iso|apk|epub|docx?|xlsx?|pptx?)(\?|$)/i;
  let probeCache = {{}};
  let activeFloatTarget = null;
  let floatHost = null;
  let floatShadow = null;

  function ensureFloatPill() {{
    if (floatHost && document.body.contains(floatHost)) return;
    floatHost = document.createElement('div');
    floatHost.id = '__shunno_float_dl_host';
    floatHost.style.cssText = 'position:fixed;z-index:2147483647;display:none;pointer-events:auto;';
    floatShadow = floatHost.attachShadow({{ mode: 'closed' }});
    floatShadow.innerHTML = `
      <style>
        .pill {{
          display:inline-flex;align-items:center;gap:7px;padding:7px 12px;border-radius:999px;
          background:linear-gradient(135deg,#0f172a,#1e293b);color:#f8fafc;
          border:1px solid rgba(59,130,246,0.65);box-shadow:0 10px 25px rgba(0,0,0,0.45);
          font:600 12px/1 "Segoe UI",system-ui,sans-serif;cursor:pointer;transition:transform .12s ease;
        }}
        .pill:hover {{ transform:scale(1.04); border-color:#10b981; }}
        .badge {{
          background:#10b981;color:#052e16;padding:2px 6px;border-radius:999px;font-size:10.5px;font-weight:800;
        }}
      </style>
      <button class="pill" id="dl-btn">
        <span>0↓ Download</span>
        <span class="badge" id="size-badge">Probing...</span>
      </button>
    `;
    document.body.appendChild(floatHost);
  }}

  window.__shunnoReceiveProbe = function(url, sizeStr, filename) {{
    probeCache[url] = {{ sizeStr, filename }};
    if (floatShadow && activeFloatTarget && activeFloatTarget.url === url) {{
      const badge = floatShadow.getElementById('size-badge');
      if (badge) badge.textContent = sizeStr;
    }}
  }};

  function showFloatMenuFor(el, mediaUrl, kindLabel) {{
    ensureFloatPill();
    const rect = el.getBoundingClientRect();
    if (rect.width < 20 || rect.height < 16) return;
    activeFloatTarget = {{ el, url: mediaUrl, kind: kindLabel }};
    floatHost.style.top = Math.max(8, rect.top + 10) + 'px';
    floatHost.style.left = Math.max(8, rect.left + 10) + 'px';
    floatHost.style.display = 'block';

    const btn = floatShadow.getElementById('dl-btn');
    const badge = floatShadow.getElementById('size-badge');
    btn.firstElementChild.textContent = '0↓ Download ' + kindLabel;

    if (probeCache[mediaUrl]) {{
      badge.textContent = probeCache[mediaUrl].sizeStr;
    }} else {{
      badge.textContent = 'Probing...';
      postToHost({{
        type: 'PROBE_MEDIA',
        url: mediaUrl,
        title: document.title || kindLabel,
        cookie: document.cookie || ''
      }});
    }}

    btn.onclick = (ev) => {{
      ev.preventDefault();
      ev.stopPropagation();
      postToHost({{
        type: 'START_DOWNLOAD',
        url: mediaUrl,
        title: document.title || kindLabel,
        cookie: document.cookie || ''
      }});
    }};
  }}

  document.addEventListener('mouseover', (e) => {{
    const target = e.target;
    if (!target || (floatHost && floatHost.contains(target))) return;

    const video = target.closest('video, #movie_player, .html5-video-player');
    if (video) {{
      const vEl = video.tagName === 'VIDEO' ? video : video.querySelector('video');
      const src = (vEl && (vEl.currentSrc || vEl.src)) || location.href;
      showFloatMenuFor(video, src, 'Video');
      return;
    }}

    const audio = target.closest('audio');
    if (audio) {{
      const src = audio.currentSrc || audio.src;
      if (src) showFloatMenuFor(audio, src, 'Audio');
      return;
    }}

    const link = target.closest('a[href]');
    if (link && link.href && (link.hasAttribute('download') || DOWNLOAD_EXT_RE.test(link.href))) {{
      showFloatMenuFor(link, link.href, link.textContent.trim().slice(0, 24) || 'File');
    }}
  }}, true);

  // 7. Draggable Floating Download Basket
  if (SHUNNO_CFG.floatingBasket) {{
    window.addEventListener('DOMContentLoaded', () => {{
      if (document.getElementById('__shunno_basket')) return;
      const basket = document.createElement('div');
      basket.id = '__shunno_basket';
      basket.style.cssText = 'position:fixed;bottom:22px;right:22px;width:64px;height:64px;border-radius:18px;background:linear-gradient(135deg,#2563eb,#10b981);color:#fff;display:flex;flex-direction:column;align-items:center;justify-content:center;font:700 11px/1.2 "Segoe UI",sans-serif;box-shadow:0 12px 30px rgba(0,0,0,0.45);z-index:2147483646;cursor:grab;user-select:none;border:2px solid rgba(255,255,255,0.35);';
      basket.innerHTML = '<span style="font-size:20px;">🧺</span><span>Drop 0↓</span>';
      basket.title = 'Drag & Drop any link, media, or multi-line URLs here to download with Shunno';

      basket.addEventListener('dragover', (e) => {{
        e.preventDefault();
        basket.style.transform = 'scale(1.12)';
      }});
      basket.addEventListener('dragleave', () => {{
        basket.style.transform = 'scale(1)';
      }});
      basket.addEventListener('drop', (e) => {{
        e.preventDefault();
        basket.style.transform = 'scale(1)';
        const dropped = e.dataTransfer.getData('text/uri-list') || e.dataTransfer.getData('text/plain') || '';
        if (dropped.trim()) {{
          postToHost({{
            type: 'BASKET_DROP',
            payload: dropped.trim(),
            title: document.title || 'Dropped_Item',
            cookie: document.cookie || ''
          }});
        }}
      }});

      let dragging = false, offX = 0, offY = 0;
      basket.addEventListener('mousedown', (e) => {{
        dragging = true;
        offX = e.clientX - basket.getBoundingClientRect().left;
        offY = e.clientY - basket.getBoundingClientRect().top;
      }});
      window.addEventListener('mousemove', (e) => {{
        if (!dragging) return;
        basket.style.left = (e.clientX - offX) + 'px';
        basket.style.top = (e.clientY - offY) + 'px';
        basket.style.right = 'auto';
        basket.style.bottom = 'auto';
      }});
      window.addEventListener('mouseup', () => {{ dragging = false; }});

      document.body.appendChild(basket);
    }});
  }}
}})();"##,
        ad_block = perms.ad_block,
        yt_ads = strip_youtube_ads,
        dark_mode = dark_on,
        reader_mode = perms.reader_mode,
        basket = floating_basket,
    )
}

#[cfg(target_os = "windows")]
fn attach_native_webview2_hook(
    wv: &WebView,
    tab_id: String,
    proxy: tao::event_loop::EventLoopProxy<UserEvent>,
    block_ads_flag: Arc<StdMutex<bool>>,
) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL, ICoreWebView2WebResourceRequestedEventArgs,
    };
    use webview2_com::{take_pwstr, WebResourceRequestedEventHandler};
    use windows_core::{w, PWSTR};

    let controller = wv.controller();
    unsafe {
        let Ok(core) = controller.CoreWebView2() else {
            return;
        };
        let _ = core.AddWebResourceRequestedFilter(
            w!("*"),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
        );

        let handler = WebResourceRequestedEventHandler::create(Box::new(
            move |_webview, args: Option<ICoreWebView2WebResourceRequestedEventArgs>| {
                let Some(args) = args else {
                    return Ok(());
                };
                let Ok(request) = args.Request() else {
                    return Ok(());
                };
                let mut uri_ptr = PWSTR::null();
                if request.Uri(&mut uri_ptr).is_ok() {
                    let uri = take_pwstr(uri_ptr);
                    let lower = uri.to_lowercase();

                    // Block known ad/tracker hosts at network layer when shield is enabled
                    if *block_ads_flag.lock().unwrap() {
                        const AD_PATTERNS: &[&str] = &[
                            "doubleclick.net",
                            "googlesyndication.com",
                            "googleadservices.com",
                            "/pagead/",
                            "/ptracking",
                            "/api/stats/ads",
                            "adservice.google.",
                            "amazon-adsystem.com",
                            "taboola.com",
                            "outbrain.com",
                        ];
                        if AD_PATTERNS.iter().any(|p| lower.contains(p)) {
                            if let Ok(env_webview) = _webview.as_ref().ok_or(()) {
                                // Cancel request by clearing headers or returning empty 204 if environment available
                                let _ = env_webview;
                            }
                            let _ = request.SetUri(w!("about:blank"));
                            return Ok(());
                        }
                    }

                    // Sniff direct media & DASH streams
                    let is_video = lower.contains(".mp4")
                        || lower.contains(".webm")
                        || lower.contains(".m4s")
                        || lower.contains(".m3u8")
                        || lower.contains(".mpd")
                        || (lower.contains("videoplayback") && lower.contains("mime=video"));
                    let is_audio = lower.contains(".mp3")
                        || lower.contains(".m4a")
                        || lower.contains(".ogg")
                        || lower.contains(".flac")
                        || (lower.contains("videoplayback") && lower.contains("mime=audio"));

                    if is_video || is_audio {
                        let kind = if is_video { "Video" } else { "Audio" }.to_string();
                        let _ = proxy.send_event(UserEvent::NetworkMediaSniffed {
                            tab_id: tab_id.clone(),
                            url: uri,
                            kind,
                            referer: String::new(),
                        });
                    }
                }
                Ok(())
            },
        ));

        let mut token = 0i64;
        let _ = core.add_WebResourceRequested(&handler, &mut token);
    }
}

fn create_tab_webview(
    window: &tao::window::Window,
    tab_id: &str,
    initial_url: &str,
    storage: &AppStorage,
    proxy: tao::event_loop::EventLoopProxy<UserEvent>,
    block_ads_flag: Arc<StdMutex<bool>>,
) -> WebView {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let host = host_from_url(initial_url);
    let perms = storage.permissions_for_host(&host);
    let init_js = build_page_init_script(
        &perms,
        storage.settings.full_dark_mode,
        storage.settings.strip_youtube_ads,
        storage.settings.floating_basket_enabled,
    );

    let tab_id_ipc = tab_id.to_string();
    let proxy_ipc = proxy.clone();
    let proxy_nav = proxy.clone();
    let tab_id_nav = tab_id.to_string();

    let mut builder = WebViewBuilder::new()
        .with_url(initial_url)
        .with_user_agent(DEFAULT_UA)
        .with_initialization_script(&init_js)
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
            size: WrySize::new(size.width.max(200.0), (size.height - CHROME_HEIGHT).max(200.0)).into(),
        })
        .with_ipc_handler(move |req| {
            let _ = proxy_ipc.send_event(UserEvent::PageIpc {
                tab_id: tab_id_ipc.clone(),
                message: req.body().to_string(),
            });
        })
        .with_navigation_handler(move |_url| {
            let _ = proxy_nav.send_event(UserEvent::RefreshChrome);
            let _ = &tab_id_nav;
            true
        });

    if !perms.javascript {
        // Handled dynamically; keep IPC operational while scripts are restricted
        builder = builder.with_devtools(false);
    }

    let wv = builder.build_as_child(window).expect("Failed to create tab WebView");

    #[cfg(target_os = "windows")]
    attach_native_webview2_hook(&wv, tab_id.to_string(), proxy, block_ads_flag);

    wv
}

fn main() {
    let rt = tokio::runtime::Runtime::new().expect("Failed to start Tokio runtime");
    let _rt_guard = rt.enter();

    let mut storage = AppStorage::load();
    let dl_manager = DownloadManager::new(storage.downloads.clone(), storage.data_dir.clone());
    let block_ads_flag = Arc::new(StdMutex::new(storage.settings.block_trackers));

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Shunno Browser v1.0.1")
        .with_inner_size(LogicalSize::new(1280.0, 820.0))
        .build(&event_loop)
        .expect("Failed to build main window");

    let win_size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let chrome_proxy = proxy.clone();
    let chrome_webview = WebViewBuilder::new()
        .with_html(ui_chrome::build_chrome_html())
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: WrySize::new(win_size.width.max(200.0), CHROME_HEIGHT).into(),
        })
        .with_ipc_handler(move |req| {
            let _ = chrome_proxy.send_event(UserEvent::ChromeIpc(req.body().to_string()));
        })
        .build_as_child(&window)
        .expect("Failed to build Chrome toolbar WebView");

    let mut tabs: Vec<TabItem> = Vec::new();
    let mut active_tab_id = String::new();
    let mut drawer_open = false;

    if let Some(saved) = storage.load_session() {
        for ts in saved.tabs {
            let wv = create_tab_webview(
                &window,
                &ts.id,
                &ts.url,
                &storage,
                proxy.clone(),
                block_ads_flag.clone(),
            );
            tabs.push(TabItem {
                id: ts.id,
                title: ts.title,
                url: ts.url,
                is_incognito: ts.is_incognito,
                webview: wv,
                detected_media: Vec::new(),
                last_cookie: String::new(),
                last_video_stream: None,
                last_audio_stream: None,
            });
        }
        active_tab_id = saved.active_tab_id;
    }

    if tabs.is_empty() {
        let id = Uuid::new_v4().to_string();
        let home = storage.settings.home_url.clone();
        let wv = create_tab_webview(
            &window,
            &id,
            &home,
            &storage,
            proxy.clone(),
            block_ads_flag.clone(),
        );
        tabs.push(TabItem {
            id: id.clone(),
            title: "Shunno Browser".to_string(),
            url: home,
            is_incognito: false,
            webview: wv,
            detected_media: Vec::new(),
            last_cookie: String::new(),
            last_video_stream: None,
            last_audio_stream: None,
        });
        active_tab_id = id;
    }

    if !tabs.iter().any(|t| t.id == active_tab_id) {
        active_tab_id = tabs[0].id.clone();
    }

    for t in &tabs {
        let _ = t.webview.set_visible(t.id == active_tab_id);
    }

    // Periodic UI sync for live download speeds & progress bars
    let ticker_proxy = proxy.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(750));
        let _ = ticker_proxy.send_event(UserEvent::RefreshChrome);
    });

    let sync_chrome_ui = |chrome_wv: &WebView,
                          tabs: &[TabItem],
                          active_id: &str,
                          storage: &AppStorage,
                          dl_items: &[DownloadItem]| {
        let active_tab = tabs.iter().find(|t| t.id == active_id).unwrap_or(&tabs[0]);
        let host = host_from_url(&active_tab.url);
        let perms = storage.permissions_for_host(&host);
        let dash_pair = active_tab.last_video_stream.is_some() && active_tab.last_audio_stream.is_some();

        let payload = ChromeRenderPayload {
            tabs: tabs
                .iter()
                .map(|t| TabDto {
                    id: &t.id,
                    title: &t.title,
                    url: &t.url,
                    is_incognito: t.is_incognito,
                })
                .collect(),
            active_tab_id: active_id,
            active_url: &active_tab.url,
            search_engines: &storage.settings.search_engines,
            active_engine_id: &storage.settings.active_engine_id,
            full_dark_mode: storage.settings.full_dark_mode,
            floating_basket_enabled: storage.settings.floating_basket_enabled,
            strip_youtube_ads: storage.settings.strip_youtube_ads,
            max_segments: storage.settings.max_segments,
            download_dir: &storage.settings.download_dir,
            permissions: perms,
            detected_media: &active_tab.detected_media,
            dash_pair,
            downloads: dl_items,
            bookmarks: &storage.bookmarks,
            history: &storage.history,
        };

        if let Ok(json) = serde_json::to_string(&payload) {
            let js = format!("if(window.renderChromeState) window.renderChromeState({});", json);
            let _ = chrome_wv.evaluate_script(&js);
        }
    };

    let persist_session = |tabs: &[TabItem], active_id: &str, storage: &AppStorage| {
        let session = SavedSession {
            active_tab_id: active_id.to_string(),
            tabs: tabs
                .iter()
                .filter(|t| !t.is_incognito)
                .map(|t| TabSession {
                    id: t.id.clone(),
                    title: t.title.clone(),
                    url: t.url.clone(),
                    is_incognito: false,
                })
                .collect(),
        };
        storage.save_session(&session);
    };

    let resize_views = |window: &tao::window::Window,
                        chrome_wv: &WebView,
                        tabs: &[TabItem],
                        drawer_is_open: bool| {
        let sz = window.inner_size().to_logical::<f64>(window.scale_factor());
        let w = sz.width.max(200.0);
        let h = sz.height.max(200.0);
        let chrome_h = if drawer_is_open { h } else { CHROME_HEIGHT };
        let _ = chrome_wv.set_bounds(Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: WrySize::new(w, chrome_h).into(),
        });
        for t in tabs {
            let _ = t.webview.set_bounds(Rect {
                position: LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
                size: WrySize::new(w, (h - CHROME_HEIGHT).max(100.0)).into(),
            });
        }
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                persist_session(&tabs, &active_tab_id, &storage);
                *control_flow = ControlFlow::Exit;
            }

            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                resize_views(&window, &chrome_webview, &tabs, drawer_open);
            }

            Event::UserEvent(UserEvent::RefreshChrome) => {
                let dl_snapshot = rt.block_on(async { dl_manager.items.lock().await.clone() });
                sync_chrome_ui(&chrome_webview, &tabs, &active_tab_id, &storage, &dl_snapshot);
            }

            Event::UserEvent(UserEvent::NetworkMediaSniffed {
                tab_id,
                url,
                kind,
                referer,
            }) => {
                if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                    if kind == "Video" {
                        tab.last_video_stream = Some(url.clone());
                    } else if kind == "Audio" {
                        tab.last_audio_stream = Some(url.clone());
                    }
                    if !tab.detected_media.iter().any(|m| m.url == url) {
                        let short_label = if tab.title.is_empty() {
                            format!("{} Stream", kind)
                        } else {
                            format!("{} — {}", tab.title, kind)
                        };
                        tab.detected_media.insert(
                            0,
                            DetectedMedia {
                                url,
                                kind,
                                label: short_label,
                                referer: if referer.is_empty() {
                                    tab.url.clone()
                                } else {
                                    referer
                                },
                            },
                        );
                        if tab.detected_media.len() > 25 {
                            tab.detected_media.truncate(25);
                        }
                    }
                }
            }

            Event::UserEvent(UserEvent::PageIpc { tab_id, message }) => {
                let Ok(val) = serde_json::from_str::<serde_json::Value>(&message) else {
                    return;
                };
                let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");

                match msg_type {
                    "PAGE_CONTEXT" => {
                        let title = val.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let url = val.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let cookie = val.get("cookie").and_then(|v| v.as_str()).unwrap_or("").to_string();

                        if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                            let url_changed = !url.is_empty() && tab.url != url;
                            if !title.is_empty() {
                                tab.title = title.clone();
                            }
                            if !url.is_empty() {
                                tab.url = url.clone();
                            }
                            tab.last_cookie = cookie;

                            if url_changed && !tab.is_incognito && url.starts_with("http") {
                                storage.history.insert(
                                    0,
                                    HistoryEntry {
                                        title: if title.is_empty() { url.clone() } else { title },
                                        url,
                                        visited_at: Utc::now(),
                                    },
                                );
                                if storage.history.len() > 300 {
                                    storage.history.truncate(300);
                                }
                                storage.save_history();
                            }
                        }
                        let _ = proxy.send_event(UserEvent::RefreshChrome);
                    }

                    "PROBE_MEDIA" => {
                        let media_url = val.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let title = val.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let cookie = val.get("cookie").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let referer = tabs
                            .iter()
                            .find(|t| t.id == tab_id)
                            .map(|t| t.url.clone());

                        let mgr = dl_manager.clone();
                        let probe_res = rt.block_on(async move {
                            mgr.probe_url(
                                &media_url,
                                referer.as_deref(),
                                cookie.as_deref(),
                                title.as_deref(),
                            )
                            .await
                        });

                        if let Some(tab) = tabs.iter().find(|t| t.id == tab_id) {
                            if let (Ok(u_json), Ok(s_json), Ok(f_json)) = (
                                serde_json::to_string(&probe_res.url),
                                serde_json::to_string(&probe_res.formatted_size),
                                serde_json::to_string(&probe_res.suggested_filename),
                            ) {
                                let js = format!(
                                    "if(window.__shunnoReceiveProbe) window.__shunnoReceiveProbe({}, {}, {});",
                                    u_json, s_json, f_json
                                );
                                let _ = tab.webview.evaluate_script(&js);
                            }
                        }
                    }

                    "START_DOWNLOAD" => {
                        let media_url = val.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let title = val.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let cookie = val.get("cookie").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let referer = tabs.iter().find(|t| t.id == tab_id).map(|t| t.url.clone());

                        if !media_url.is_empty() {
                            let mgr = dl_manager.clone();
                            let dl_dir = storage.settings.download_dir.clone();
                            let segs = storage.settings.max_segments;
                            rt.block_on(async move {
                                mgr.enqueue_download(media_url, dl_dir, segs, referer, cookie, title)
                                    .await;
                            });
                            let _ = proxy.send_event(UserEvent::RefreshChrome);
                        }
                    }

                    "BASKET_DROP" => {
                        let raw = val.get("payload").and_then(|v| v.as_str()).unwrap_or("");
                        let title = val.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let cookie = val.get("cookie").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let referer = tabs.iter().find(|t| t.id == tab_id).map(|t| t.url.clone());

                        let urls: Vec<String> = raw
                            .split(|c: char| c.is_whitespace() || c == ',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
                            .collect();

                        for u in urls {
                            let mgr = dl_manager.clone();
                            let dl_dir = storage.settings.download_dir.clone();
                            let segs = storage.settings.max_segments;
                            let ref_c = referer.clone();
                            let ck_c = cookie.clone();
                            let tt_c = title.clone();
                            rt.block_on(async move {
                                mgr.enqueue_download(u, dl_dir, segs, ref_c, ck_c, tt_c).await;
                            });
                        }
                        let _ = proxy.send_event(UserEvent::RefreshChrome);
                    }

                    "OFFLINE_HTML_READY" => {
                        let html = val.get("html").and_then(|v| v.as_str()).unwrap_or("");
                        let title = val
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Saved_Page");
                        if !html.is_empty() {
                            let clean = format!("{}.html", downloader::sanitize_filename(title));
                            let dest = std::path::PathBuf::from(&storage.settings.download_dir).join(clean);
                            let _ = std::fs::create_dir_all(&storage.settings.download_dir);
                            let _ = std::fs::write(&dest, html);
                        }
                    }

                    "SHORTCUT" => {
                        let action = val.get("action").and_then(|v| v.as_str()).unwrap_or("");
                        let _ = proxy.send_event(UserEvent::ChromeIpc(format!("SHORTCUT:{}", action)));
                    }

                    _ => {}
                }
            }

            Event::UserEvent(UserEvent::ChromeIpc(cmd)) => {
                if cmd == "DRAWER_OPENED" {
                    drawer_open = true;
                    resize_views(&window, &chrome_webview, &tabs, true);
                } else if cmd == "DRAWER_CLOSED" {
                    drawer_open = false;
                    resize_views(&window, &chrome_webview, &tabs, false);
                } else if cmd == "NEW_TAB" || cmd == "SHORTCUT:NEW_TAB" || cmd == "NEW_INCOGNITO_TAB" {
                    let is_incog = cmd == "NEW_INCOGNITO_TAB";
                    let id = Uuid::new_v4().to_string();
                    let home = storage.settings.home_url.clone();
                    let wv = create_tab_webview(
                        &window,
                        &id,
                        &home,
                        &storage,
                        proxy.clone(),
                        block_ads_flag.clone(),
                    );
                    tabs.push(TabItem {
                        id: id.clone(),
                        title: if is_incog { "Private Tab" } else { "New Tab" }.to_string(),
                        url: home,
                        is_incognito: is_incog,
                        webview: wv,
                        detected_media: Vec::new(),
                        last_cookie: String::new(),
                        last_video_stream: None,
                        last_audio_stream: None,
                    });
                    active_tab_id = id;
                    for t in &tabs {
                        let _ = t.webview.set_visible(t.id == active_tab_id);
                    }
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if let Some(target_id) = cmd.strip_prefix("SWITCH_TAB:") {
                    if tabs.iter().any(|t| t.id == target_id) {
                        active_tab_id = target_id.to_string();
                        for t in &tabs {
                            let _ = t.webview.set_visible(t.id == active_tab_id);
                        }
                        let _ = proxy.send_event(UserEvent::RefreshChrome);
                    }
                } else if cmd == "SHORTCUT:NEXT_TAB" || cmd == "SHORTCUT:PREV_TAB" {
                    if tabs.len() > 1 {
                        if let Some(idx) = tabs.iter().position(|t| t.id == active_tab_id) {
                            let next = if cmd == "SHORTCUT:NEXT_TAB" {
                                (idx + 1) % tabs.len()
                            } else {
                                (idx + tabs.len() - 1) % tabs.len()
                            };
                            active_tab_id = tabs[next].id.clone();
                            for t in &tabs {
                                let _ = t.webview.set_visible(t.id == active_tab_id);
                            }
                            let _ = proxy.send_event(UserEvent::RefreshChrome);
                        }
                    }
                } else if cmd == "SHORTCUT:CLOSE_ACTIVE_TAB" || cmd.starts_with("CLOSE_TAB:") {
                    let close_id = if cmd == "SHORTCUT:CLOSE_ACTIVE_TAB" {
                        active_tab_id.clone()
                    } else {
                        cmd.trim_start_matches("CLOSE_TAB:").to_string()
                    };

                    // Closing the last remaining tab closes the browser cleanly
                    if tabs.len() <= 1 {
                        persist_session(&tabs, &active_tab_id, &storage);
                        *control_flow = ControlFlow::Exit;
                        return;
                    }

                    if let Some(pos) = tabs.iter().position(|t| t.id == close_id) {
                        let removed = tabs.remove(pos);
                        let _ = removed.webview.set_visible(false);
                        if active_tab_id == close_id {
                            let next_idx = pos.min(tabs.len() - 1);
                            active_tab_id = tabs[next_idx].id.clone();
                        }
                        for t in &tabs {
                            let _ = t.webview.set_visible(t.id == active_tab_id);
                        }
                        let _ = proxy.send_event(UserEvent::RefreshChrome);
                    }
                } else if let Some(raw_nav) = cmd.strip_prefix("NAVIGATE:") {
                    let target_url =
                        normalize_omnibox_input(raw_nav, &storage.settings.active_search_template());
                    if let Some(tab) = tabs.iter_mut().find(|t| t.id == active_tab_id) {
                        tab.url = target_url.clone();
                        tab.detected_media.clear();
                        tab.last_video_stream = None;
                        tab.last_audio_stream = None;
                        let _ = tab.webview.load_url(&target_url);
                    }
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if cmd == "GO_BACK" || cmd == "SHORTCUT:GO_BACK" {
                    if let Some(tab) = tabs.iter().find(|t| t.id == active_tab_id) {
                        let _ = tab.webview.evaluate_script("history.back()");
                    }
                } else if cmd == "GO_FORWARD" || cmd == "SHORTCUT:GO_FORWARD" {
                    if let Some(tab) = tabs.iter().find(|t| t.id == active_tab_id) {
                        let _ = tab.webview.evaluate_script("history.forward()");
                    }
                } else if cmd == "RELOAD" || cmd == "SHORTCUT:RELOAD" {
                    if let Some(tab) = tabs.iter().find(|t| t.id == active_tab_id) {
                        let _ = tab.webview.evaluate_script("location.reload()");
                    }
                } else if cmd == "SHORTCUT:FOCUS_OMNIBOX" {
                    let _ = chrome_webview.evaluate_script("if(window.focusOmnibox) window.focusOmnibox();");
                } else if cmd == "SHORTCUT:OPEN_DOWNLOADS" {
                    let _ = chrome_webview.evaluate_script("if(window.openDrawerById) window.openDrawerById('downloads-drawer');");
                } else if let Some(eng_id) = cmd.strip_prefix("SET_SEARCH_ENGINE:") {
                    storage.settings.active_engine_id = eng_id.to_string();
                    storage.save_settings();
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if let Some(json_str) = cmd.strip_prefix("ADD_SEARCH_ENGINE:") {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                        if let (Some(name), Some(url)) = (
                            v.get("name").and_then(|s| s.as_str()),
                            v.get("url").and_then(|s| s.as_str()),
                        ) {
                            let id = Uuid::new_v4().to_string();
                            storage.settings.search_engines.push(SearchEngine {
                                id: id.clone(),
                                name: name.to_string(),
                                url_template: url.to_string(),
                            });
                            storage.settings.active_engine_id = id;
                            storage.save_settings();
                            let _ = proxy.send_event(UserEvent::RefreshChrome);
                        }
                    }
                } else if let Some(rem_id) = cmd.strip_prefix("REMOVE_SEARCH_ENGINE:") {
                    if storage.settings.search_engines.len() > 1 {
                        storage.settings.search_engines.retain(|e| e.id != rem_id);
                        if storage.settings.active_engine_id == rem_id {
                            storage.settings.active_engine_id =
                                storage.settings.search_engines[0].id.clone();
                        }
                        storage.save_settings();
                        let _ = proxy.send_event(UserEvent::RefreshChrome);
                    }
                } else if cmd == "TOGGLE_FULL_DARK_MODE" || cmd == "SHORTCUT:TOGGLE_DARK" {
                    storage.settings.full_dark_mode = !storage.settings.full_dark_mode;
                    storage.save_settings();
                    if let Some(tab) = tabs.iter().find(|t| t.id == active_tab_id) {
                        let _ = tab.webview.evaluate_script("location.reload()");
                    }
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if cmd == "TOGGLE_FLOATING_BASKET" {
                    storage.settings.floating_basket_enabled = !storage.settings.floating_basket_enabled;
                    storage.save_settings();
                    if let Some(tab) = tabs.iter().find(|t| t.id == active_tab_id) {
                        let _ = tab.webview.evaluate_script("location.reload()");
                    }
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if cmd == "TOGGLE_YT_AD_STRIPPER" {
                    storage.settings.strip_youtube_ads = !storage.settings.strip_youtube_ads;
                    storage.save_settings();
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if cmd == "TOGGLE_READER_MODE" || cmd == "SHORTCUT:TOGGLE_READER" {
                    if let Some(tab) = tabs.iter().find(|t| t.id == active_tab_id) {
                        let host = host_from_url(&tab.url);
                        let mut perms = storage.permissions_for_host(&host);
                        perms.reader_mode = !perms.reader_mode;
                        storage.site_permissions.insert(host, perms);
                        storage.save_permissions();
                        let _ = tab.webview.evaluate_script("location.reload()");
                    }
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if cmd == "SAVE_OFFLINE_HTML" {
                    if let Some(tab) = tabs.iter().find(|t| t.id == active_tab_id) {
                        let js = r#"
                          (function(){
                            const clone = document.documentElement.clone();
                            const base = document.createElement('base');
                            base.href = location.href;
                            const head = clone.querySelector('head');
                            if (head) head.insertBefore(base, head.firstChild);
                            window.ipc.postMessage(JSON.stringify({
                              type: 'OFFLINE_HTML_READY',
                              title: document.title || 'Saved_Page',
                              html: '<!DOCTYPE html>\n' + clone.outerHTML
                            }));
                          })();
                        "#;
                        let _ = tab.webview.evaluate_script(js);
                    }
                } else if let Some(perm_key) = cmd.strip_prefix("TOGGLE_SITE_PERM:") {
                    if let Some(tab) = tabs.iter().find(|t| t.id == active_tab_id) {
                        let host = host_from_url(&tab.url);
                        let mut perms = storage.permissions_for_host(&host);
                        match perm_key {
                            "ad_block" => {
                                perms.ad_block = !perms.ad_block;
                                *block_ads_flag.lock().unwrap() = perms.ad_block;
                            }
                            "javascript" => perms.javascript = !perms.javascript,
                            "cookies" => perms.cookies = !perms.cookies,
                            "images" => perms.images = !perms.images,
                            "desktop_ua" => perms.desktop_ua = !perms.desktop_ua,
                            _ => {}
                        }
                        storage.site_permissions.insert(host, perms);
                        storage.save_permissions();
                        let _ = tab.webview.evaluate_script("location.reload()");
                    }
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if cmd == "ZOOM_IN"
                    || cmd == "ZOOM_OUT"
                    || cmd == "ZOOM_RESET"
                    || cmd == "SHORTCUT:ZOOM_IN"
                    || cmd == "SHORTCUT:ZOOM_OUT"
                    || cmd == "SHORTCUT:ZOOM_RESET"
                {
                    if let Some(tab) = tabs.iter().find(|t| t.id == active_tab_id) {
                        let host = host_from_url(&tab.url);
                        let mut perms = storage.permissions_for_host(&host);
                        if cmd.ends_with("ZOOM_IN") {
                            perms.zoom_level = (perms.zoom_level + 0.1).min(2.5);
                        } else if cmd.ends_with("ZOOM_OUT") {
                            perms.zoom_level = (perms.zoom_level - 0.1).max(0.5);
                        } else {
                            perms.zoom_level = 1.0;
                        }
                        let _ = tab.webview.zoom(perms.zoom_level);
                        storage.site_permissions.insert(host, perms);
                        storage.save_permissions();
                    }
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if cmd == "NUKE_SITE_DATA" {
                    if let Some(tab) = tabs.iter().find(|t| t.id == active_tab_id) {
                        let _ = tab.webview.evaluate_script(
                            "localStorage.clear(); sessionStorage.clear(); document.cookie.split(';').forEach(c => document.cookie = c.replace(/^ +/, '').replace(/=.*/, '=;expires=' + new Date().toUTCString() + ';path=/')); location.reload();",
                        );
                    }
                } else if cmd == "ADD_BOOKMARK" || cmd == "SHORTCUT:ADD_BOOKMARK" {
                    if let Some(tab) = tabs.iter().find(|t| t.id == active_tab_id) {
                        if !storage.bookmarks.iter().any(|b| b.url == tab.url) {
                            storage.bookmarks.insert(
                                0,
                                Bookmark {
                                    id: Uuid::new_v4().to_string(),
                                    title: tab.title.clone(),
                                    url: tab.url.clone(),
                                    created_at: Utc::now(),
                                },
                            );
                            storage.save_bookmarks();
                        }
                    }
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if let Some(bm_id) = cmd.strip_prefix("REMOVE_BOOKMARK:") {
                    storage.bookmarks.retain(|b| b.id != bm_id);
                    storage.save_bookmarks();
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if cmd == "CLEAR_HISTORY" {
                    storage.history.clear();
                    storage.save_history();
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if let Some(seg_str) = cmd.strip_prefix("SET_MAX_SEGMENTS:") {
                    if let Ok(n) = seg_str.parse::<usize>() {
                        storage.settings.max_segments = n.clamp(1, 16);
                        storage.save_settings();
                    }
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if cmd == "CHOOSE_DOWNLOAD_DIR" {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        storage.settings.download_dir = folder.to_string_lossy().to_string();
                        storage.save_settings();
                        let _ = proxy.send_event(UserEvent::RefreshChrome);
                    }
                } else if cmd == "OPEN_DOWNLOAD_FOLDER" {
                    let _ = std::fs::create_dir_all(&storage.settings.download_dir);
                    #[cfg(target_os = "windows")]
                    let _ = std::process::Command::new("explorer")
                        .arg(&storage.settings.download_dir)
                        .spawn();
                } else if let Some(json_str) = cmd.strip_prefix("DOWNLOAD_URL:") {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                        let url = v.get("url").and_then(|s| s.as_str()).unwrap_or("").to_string();
                        let title = v.get("title").and_then(|s| s.as_str()).map(|s| s.to_string());
                        let (referer, cookie) = tabs
                            .iter()
                            .find(|t| t.id == active_tab_id)
                            .map(|t| (Some(t.url.clone()), Some(t.last_cookie.clone())))
                            .unwrap_or((None, None));
                        if !url.is_empty() {
                            let mgr = dl_manager.clone();
                            let dir = storage.settings.download_dir.clone();
                            let segs = storage.settings.max_segments;
                            rt.block_on(async move {
                                mgr.enqueue_download(url, dir, segs, referer, cookie, title).await;
                            });
                            let _ = proxy.send_event(UserEvent::RefreshChrome);
                        }
                    }
                } else if let Some(raw_batch) = cmd.strip_prefix("BATCH_DOWNLOAD:") {
                    let (referer, cookie) = tabs
                        .iter()
                        .find(|t| t.id == active_tab_id)
                        .map(|t| (Some(t.url.clone()), Some(t.last_cookie.clone())))
                        .unwrap_or((None, None));

                    let mut urls: Vec<String> = raw_batch
                        .split(|c: char| c == '\n' || c == '\r' || c == ',' || c == ' ' || c == '\t')
                        .map(|s| s.trim().to_string())
                        .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
                        .collect();
                    urls.sort();
                    urls.dedup();

                    for u in urls {
                        let mgr = dl_manager.clone();
                        let dir = storage.settings.download_dir.clone();
                        let segs = storage.settings.max_segments;
                        let r_c = referer.clone();
                        let c_c = cookie.clone();
                        rt.block_on(async move {
                            mgr.enqueue_download(u, dir, segs, r_c, c_c, None).await;
                        });
                    }
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if cmd == "MUX_DASH_PAIR" {
                    if let Some(tab) = tabs.iter().find(|t| t.id == active_tab_id) {
                        if let (Some(v_url), Some(a_url)) =
                            (tab.last_video_stream.clone(), tab.last_audio_stream.clone())
                        {
                            let mgr = dl_manager.clone();
                            let dir = storage.settings.download_dir.clone();
                            let segs = storage.settings.max_segments;
                            let referer = Some(tab.url.clone());
                            let cookie = Some(tab.last_cookie.clone());
                            let title = Some(tab.title.clone());
                            rt.block_on(async move {
                                mgr.enqueue_dash_mux_download(
                                    v_url, a_url, dir, segs, referer, cookie, title,
                                )
                                .await;
                            });
                            let _ = proxy.send_event(UserEvent::RefreshChrome);
                        }
                    }
                } else if let Some(dl_id) = cmd.strip_prefix("PAUSE_DOWNLOAD:") {
                    let mgr = dl_manager.clone();
                    let id = dl_id.to_string();
                    rt.block_on(async move { mgr.pause(&id).await });
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if let Some(dl_id) = cmd.strip_prefix("RESUME_DOWNLOAD:") {
                    let mgr = dl_manager.clone();
                    let id = dl_id.to_string();
                    let segs = storage.settings.max_segments;
                    rt.block_on(async move { mgr.resume(&id, segs).await });
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if let Some(dl_id) = cmd.strip_prefix("REMOVE_DOWNLOAD:") {
                    let mgr = dl_manager.clone();
                    let id = dl_id.to_string();
                    rt.block_on(async move { mgr.remove(&id, false).await });
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                } else if cmd == "CLEAR_COMPLETED_DOWNLOADS" {
                    let mgr = dl_manager.clone();
                    rt.block_on(async move { mgr.clear_completed().await });
                    let _ = proxy.send_event(UserEvent::RefreshChrome);
                }
            }

            _ => {}
        }
    });
}