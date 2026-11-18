#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod downloader;
mod state;
mod ui_chrome;

use downloader::{ShunnoGetDesktop, StreamProbeResult};
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use wry::{Rect, WebView, WebViewBuilder};

use state::{
    extract_host, sanitize_url, BookmarkEntry, BrowserStorage, GlobalSettings, HistoryEntry,
    SavedTab, SearchEngine, SitePolicy, APP_VERSION,
};

const CHROME_BAR_HEIGHT: f64 = 86.0;

enum AppEvent {
    ChromeIpc(String),
    ContentIpc(usize, String),
    DownloadIntercepted(String),
    NewWindowBlockedOrOpened(String),
    NativeStreamCaptured(usize, String),
    StreamSizeResolved(usize, StreamProbeResult),
    RefreshChrome,
}

struct BrowserTab {
    id: usize,
    title: String,
    url: String,
    incognito: bool,
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
}

#[derive(Serialize)]
struct ChromeSnapshot {
    version: &'static str,
    tabs: Vec<TabSummary>,
    active_tab: usize,
    active_url: String,
    active_host: String,
    site: SitePolicy,
    global: GlobalSettings,
    blocked_count: usize,
    streams: Vec<SniffedStreamItem>,
    bookmarks: Vec<BookmarkEntry>,
    history: Vec<HistoryEntry>,
    downloads: Vec<downloader::DownloadTask>,
}

fn build_content_init_script(policy: &SitePolicy, global: &GlobalSettings, user_js: &str) -> String {
    let mut script = String::new();

    // 1. Background Media Playback (Page Visibility API lock)
    if global.background_media_play {
        script.push_str(
            r#"
            (function() {
                try {
                    Object.defineProperty(document, 'hidden', { get: () => false, configurable: true });
                    Object.defineProperty(document, 'visibilityState', { get: () => 'visible', configurable: true });
                    window.addEventListener('visibilitychange', e => e.stopImmediatePropagation(), true);
                    document.addEventListener('visibilitychange', e => e.stopImmediatePropagation(), true);
                } catch(e) {}
            })();
            "#,
        );
    }

    // 2. Early Pre-Roll YouTube Ad Stripper (intercepts ytInitialPlayerResponse & JSON.parse BEFORE video starts)
    if policy.adblock || policy.sponsorblock {
        script.push_str(
            r#"
            (function() {
                function stripAdFields(obj) {
                    if (!obj || typeof obj !== 'object') return obj;
                    try {
                        if ('adPlacements' in obj) delete obj.adPlacements;
                        if ('playerAds' in obj) delete obj.playerAds;
                        if ('adSlots' in obj) delete obj.adSlots;
                        if ('adBreakHeartbeatParams' in obj) delete obj.adBreakHeartbeatParams;
                        if (obj.auxiliaryUi && obj.auxiliaryUi.messageRenderers) {
                            delete obj.auxiliaryUi.messageRenderers;
                        }
                    } catch(e) {}
                    return obj;
                }

                // Hook ytInitialPlayerResponse assignment immediately
                let _ytPlayerResp = undefined;
                try {
                    Object.defineProperty(window, 'ytInitialPlayerResponse', {
                        configurable: true,
                        get() { return _ytPlayerResp; },
                        set(val) { _ytPlayerResp = stripAdFields(val); }
                    });
                } catch(e) {}

                // Hook JSON.parse for inline YouTube player payloads
                const origParse = JSON.parse;
                JSON.parse = function(text, reviver) {
                    const res = origParse.apply(this, arguments);
                    if (res && typeof res === 'object' && ('adPlacements' in res || 'playerAds' in res || 'adSlots' in res)) {
                        stripAdFields(res);
                    }
                    return res;
                };

                // Hook Response.prototype.json for /youtubei/v1/player API calls
                const origRespJson = Response.prototype.json;
                Response.prototype.json = function() {
                    return origRespJson.apply(this, arguments).then(data => stripAdFields(data));
                };
            })();
            "#,
        );
    }

    // 3. Full Dark Mode Injection when toggled ON
    if global.full_dark_mode {
        script.push_str(
            r#"
            (function() {
                const st = document.createElement('style');
                st.id = 'shunno-full-dark';
                st.textContent = `
                    html { filter: invert(0.92) hue-rotate(180deg) !important; background: #111 !important; }
                    img, video, picture, canvas, svg, [style*="background-image"] {
                        filter: invert(1) hue-rotate(180deg) !important;
                    }
                `;
                document.documentElement.appendChild(st);
            })();
            "#,
        );
    }

    // 4. Network Sniffer + AdBlocker + Floating Download Menu with Live File Size Probing
    let adblock_flag = if policy.adblock { "true" } else { "false" };
    script.push_str(&format!(
        r#"
        (function() {{
            const ADBLOCK_ON = {adblock_flag};
            const AD_HOSTS = [
                'doubleclick.net', 'googlesyndication.com', 'googleadservices.com',
                'adnxs.com', 'adsrvr.org', 'taboola.com', 'outbrain.com',
                'popads.net', 'popcash.net', 'propellerads.com', 'exoclick.com',
                'scorecardresearch.com', 'criteo.com', 'pubmatic.com', 'rubiconproject.com',
                'youtube.com/pagead/', 'youtube.com/api/stats/ads', 'youtube.com/ptracking'
            ];
            const MEDIA_EXTS = ['.mp4', '.mkv', '.webm', '.mov', '.m3u8', '.mpd', '.flac', '.alac', '.m4a', '.wav', '.mp3', '.ogg', '.opus'];
            const FILE_EXTS = ['.zip', '.7z', '.rar', '.tar', '.gz', '.iso', '.exe', '.msi', '.apk', '.pdf', '.dmg', '.deb'];
            const ALL_EXTS = MEDIA_EXTS.concat(FILE_EXTS);
            const capturedStreams = [];
            const probedSizes = new Map();

            function requestSizeProbe(targetUrl) {{
                if (!targetUrl || probedSizes.has(targetUrl) || !window.ipc) return;
                probedSizes.set(targetUrl, 'Probing...');
                window.ipc.postMessage('PROBE_STREAM:' + JSON.stringify({{
                    url: targetUrl,
                    referer: location.href,
                    cookie: document.cookie || ''
                }}));
            }}

            function addCapturedStream(rawUrl, kindHint) {{
                if (!rawUrl || !String(rawUrl).startsWith('http')) return;
                const urlStr = String(rawUrl);
                if (urlStr.startsWith('blob:')) return;
                if (capturedStreams.some(s => s.url === urlStr)) return;
                const cleanPath = urlStr.split('?')[0].toLowerCase();
                const ext = ALL_EXTS.find(e => cleanPath.endsWith(e)) || kindHint || '.mp4';
                let labelName = 'Stream ' + ext.toUpperCase();
                try {{
                    const seg = cleanPath.split('/').pop();
                    if (seg && seg.length > 2) labelName = decodeURIComponent(seg);
                }} catch(e) {{}}
                capturedStreams.unshift({{ url: urlStr, ext: ext.toUpperCase(), name: labelName }});
                if (capturedStreams.length > 20) capturedStreams.pop();
                requestSizeProbe(urlStr);
                if (window.ipc) window.ipc.postMessage('STREAM:' + urlStr);
            }}

            window.__shunnoOnStreamProbed = function(info) {{
                if (!info || !info.url) return;
                probedSizes.set(info.url, info.size_label || 'Stream');
                const item = capturedStreams.find(s => s.url === info.url);
                if (item && info.resolved_name) {{
                    item.name = info.resolved_name;
                }}
                if (window.__shunnoRefreshFloatingPill) {{
                    window.__shunnoRefreshFloatingPill(info.url, info.size_label);
                }}
            }};

            function inspectUrl(raw) {{
                if (!raw) return false;
                const str = String(raw);
                const lower = str.toLowerCase();
                const cleanLower = lower.split('?')[0];
                if (ADBLOCK_ON && AD_HOSTS.some(h => lower.includes(h))) {{
                    if (window.ipc) window.ipc.postMessage('AD_BLOCKED');
                    return true;
                }}
                if (ALL_EXTS.some(ext => cleanLower.endsWith(ext)) && str.startsWith('http')) {{
                    addCapturedStream(str);
                }}
                return false;
            }}

            const origFetch = window.fetch;
            window.fetch = function(input, init) {{
                const u = (typeof input === 'string') ? input : (input && input.url);
                if (inspectUrl(u)) return Promise.reject(new Error('Blocked by Shunno AdBlock'));
                return origFetch.apply(this, arguments);
            }};

            const origOpen = XMLHttpRequest.prototype.open;
            XMLHttpRequest.prototype.open = function(method, url) {{
                if (inspectUrl(url)) {{
                    this._shunnoBlocked = true;
                    return;
                }}
                return origOpen.apply(this, arguments);
            }};
            const origSend = XMLHttpRequest.prototype.send;
            XMLHttpRequest.prototype.send = function() {{
                if (this._shunnoBlocked) return;
                return origSend.apply(this, arguments);
            }};

            function triggerShunnoGet(targetUrl, threads) {{
                if (!window.ipc || !targetUrl) return;
                const payload = JSON.stringify({{
                    url: targetUrl,
                    threads: threads || 16,
                    referer: location.href,
                    cookie: document.cookie || ''
                }});
                window.ipc.postMessage('FLOAT_DL:' + payload);
            }}

            function installFloatingMenu() {{
                if (!document.documentElement || document.getElementById('shunno-float-host')) return;

                const host = document.createElement('div');
                host.id = 'shunno-float-host';
                host.style.cssText = 'position:fixed;top:0;left:0;width:0;height:0;z-index:2147483647;pointer-events:none;';
                const shadow = host.attachShadow({{ mode: 'open' }});

                shadow.innerHTML = `
                    <style>
                        .pill {{
                            position: fixed;
                            pointer-events: auto;
                            display: none;
                            align-items: center;
                            gap: 6px;
                            background: rgba(24, 24, 27, 0.94);
                            backdrop-filter: blur(14px);
                            color: #FFFFFF;
                            border: 1px solid rgba(255, 255, 255, 0.24);
                            border-radius: 10px;
                            padding: 6px 11px;
                            font-family: "Segoe UI", sans-serif;
                            font-size: 12px;
                            font-weight: 600;
                            box-shadow: 0 8px 24px rgba(0, 0, 0, 0.38);
                            cursor: pointer;
                            transition: transform 0.12s ease, background 0.12s ease;
                        }}
                        .pill:hover {{
                            background: #0A84FF;
                            transform: translateY(-1px);
                        }}
                        .badge {{
                            background: #30D158;
                            color: #000;
                            font-size: 10px;
                            font-weight: 800;
                            padding: 1px 6px;
                            border-radius: 5px;
                        }}
                        .menu {{
                            position: fixed;
                            pointer-events: auto;
                            display: none;
                            flex-direction: column;
                            gap: 6px;
                            min-width: 290px;
                            max-width: 390px;
                            background: rgba(24, 24, 27, 0.97);
                            backdrop-filter: blur(18px);
                            color: #fff;
                            border: 1px solid rgba(255, 255, 255, 0.24);
                            border-radius: 12px;
                            padding: 10px;
                            font-family: "Segoe UI", sans-serif;
                            font-size: 12px;
                            box-shadow: 0 14px 36px rgba(0, 0, 0, 0.48);
                        }}
                        .menu-hdr {{
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            font-weight: 700;
                            font-size: 12px;
                            color: #30D158;
                            border-bottom: 1px solid rgba(255,255,255,0.14);
                            padding-bottom: 6px;
                        }}
                        .row {{
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            gap: 8px;
                            padding: 6px 7px;
                            border-radius: 8px;
                            background: rgba(255,255,255,0.08);
                        }}
                        .row:hover {{ background: rgba(10,132,255,0.35); }}
                        .name {{
                            overflow: hidden;
                            text-overflow: ellipsis;
                            white-space: nowrap;
                            flex: 1;
                            font-size: 11.5px;
                        }}
                        .go {{
                            background: #0A84FF;
                            color: #fff;
                            border: none;
                            border-radius: 6px;
                            padding: 4px 9px;
                            font-size: 11px;
                            font-weight: 700;
                            cursor: pointer;
                            flex-shrink: 0;
                        }}
                        .go:hover {{ background: #30D158; color: #000; }}
                    </style>
                    <div class="pill" id="pill">
                        <span class="badge">0↓</span>
                        <span id="pill-text">Download</span>
                        <span style="opacity:0.75">▾</span>
                    </div>
                    <div class="menu" id="menu"></div>
                `;
                document.documentElement.appendChild(host);

                const pill = shadow.getElementById('pill');
                const pillText = shadow.getElementById('pill-text');
                const menu = shadow.getElementById('menu');
                let activeItems = [];
                let activeBaseLabel = 'Download';
                let hideTimer = null;

                window.__shunnoRefreshFloatingPill = function(url, sizeLabel) {{
                    if (activeItems.length === 1 && activeItems[0].url === url && pill.style.display !== 'none') {{
                        pillText.textContent = `${{activeBaseLabel}} (${{sizeLabel}})`;
                    }}
                }};

                function scheduleHide() {{
                    clearTimeout(hideTimer);
                    hideTimer = setTimeout(() => {{
                        if (!pill.matches(':hover') && !menu.matches(':hover')) {{
                            pill.style.display = 'none';
                            menu.style.display = 'none';
                        }}
                    }}, 650);
                }}

                pill.addEventListener('mouseenter', () => clearTimeout(hideTimer));
                pill.addEventListener('mouseleave', scheduleHide);
                menu.addEventListener('mouseenter', () => clearTimeout(hideTimer));
                menu.addEventListener('mouseleave', scheduleHide);

                function showPillAt(x, y, baseLabel, items) {{
                    if (!items || items.length === 0) return;
                    clearTimeout(hideTimer);
                    activeItems = items;
                    activeBaseLabel = baseLabel;
                    items.forEach(it => requestSizeProbe(it.url));

                    if (items.length === 1) {{
                        const sz = probedSizes.get(items[0].url);
                        pillText.textContent = sz ? `${{baseLabel}} (${{sz}})` : baseLabel;
                    }} else {{
                        pillText.textContent = `${{baseLabel}} (${{items.length}})`;
                    }}

                    const left = Math.min(window.innerWidth - 230, Math.max(8, x));
                    const top = Math.min(window.innerHeight - 48, Math.max(8, y));
                    pill.style.left = left + 'px';
                    pill.style.top = top + 'px';
                    pill.style.display = 'inline-flex';
                }}

                pill.addEventListener('click', (e) => {{
                    e.stopPropagation();
                    if (activeItems.length === 1) {{
                        triggerShunnoGet(activeItems[0].url, 16);
                        pill.style.display = 'none';
                        return;
                    }}
                    menu.innerHTML = `<div class="menu-hdr"><span>⚡ Shunno Get (${{activeItems.length}} streams)</span><span style="cursor:pointer;color:#aaa" id="m-close">✕</span></div>` +
                        activeItems.map((it, i) => `
                            <div class="row">
                                <span class="badge">${{it.ext.replace('.', '')}} • ${{probedSizes.get(it.url) || '...'}}</span>
                                <span class="name" title="${{it.url}}">${{it.name}}</span>
                                <button class="go" data-idx="${{i}}">16T</button>
                            </div>
                        `).join('');
                    menu.style.left = pill.style.left;
                    menu.style.top = (parseFloat(pill.style.top) + 34) + 'px';
                    menu.style.display = 'flex';
                    shadow.getElementById('m-close').onclick = () => {{ menu.style.display = 'none'; }};
                    menu.querySelectorAll('.go').forEach(btn => {{
                        btn.onclick = () => {{
                            const idx = parseInt(btn.getAttribute('data-idx'), 10);
                            if (activeItems[idx]) triggerShunnoGet(activeItems[idx].url, 16);
                            menu.style.display = 'none';
                            pill.style.display = 'none';
                        }};
                    }});
                }});

                // Detect hover over video containers (including YouTube player wrappers), <video>, <audio>, and downloadable <a> links
                document.addEventListener('mouseover', (e) => {{
                    const target = e.target;
                    if (!target || !target.closest) return;

                    const playerContainer = target.closest('video, audio, .html5-video-player, #movie_player');
                    if (playerContainer) {{
                        const videoEl = playerContainer.matches('video, audio')
                            ? playerContainer
                            : playerContainer.querySelector('video, audio');
                        const rect = playerContainer.getBoundingClientRect();
                        if (rect.width >= 140 && rect.height >= 50) {{
                            const items = [];
                            if (videoEl) {{
                                const directSrc = videoEl.currentSrc || videoEl.src;
                                if (directSrc && directSrc.startsWith('http')) {{
                                    addCapturedStream(directSrc, videoEl.tagName === 'AUDIO' ? '.m4a' : '.mp4');
                                }}
                                videoEl.querySelectorAll('source').forEach(s => {{
                                    if (s.src && s.src.startsWith('http')) addCapturedStream(s.src);
                                }});
                            }}
                            capturedStreams.forEach(s => items.push(s));
                            // On YouTube watch pages, always offer current video page stream option if MSE blob is active
                            if (items.length === 0 && location.hostname.includes('youtube.com') && location.search.includes('v=')) {{
                                items.push({{
                                    url: location.href,
                                    ext: '.MP4',
                                    name: (document.title || 'YouTube Video').replace(' - YouTube', '')
                                }});
                            }}
                            if (items.length > 0) {{
                                const kind = (videoEl && videoEl.tagName === 'AUDIO') ? 'Download audio' : 'Download video';
                                showPillAt(rect.right - 205, rect.top + 12, kind, items);
                                return;
                            }}
                        }}
                    }}

                    const linkEl = target.closest('a[href]');
                    if (linkEl && linkEl.href && linkEl.href.startsWith('http')) {{
                        const cleanHref = linkEl.href.split('?')[0].toLowerCase();
                        const matchedExt = ALL_EXTS.find(ext => cleanHref.endsWith(ext));
                        if (matchedExt) {{
                            const rect = linkEl.getBoundingClientRect();
                            let fname = 'File ' + matchedExt;
                            try {{
                                fname = decodeURIComponent(cleanHref.split('/').pop() || fname);
                            }} catch(e) {{}}
                            showPillAt(
                                rect.left,
                                rect.top - 34,
                                'Download ' + matchedExt.toUpperCase(),
                                [{{ url: linkEl.href, ext: matchedExt.toUpperCase(), name: fname }}]
                            );
                        }}
                    }}
                }}, true);

                document.addEventListener('mouseout', (e) => {{
                    if (e.target && e.target.closest && e.target.closest('video, audio, .html5-video-player, #movie_player, a[href]')) {{
                        scheduleHide();
                    }}
                }}, true);
            }}

            if (document.documentElement) installFloatingMenu();
            window.addEventListener('DOMContentLoaded', () => {{
                if (window.ipc) window.ipc.postMessage('META:' + JSON.stringify({{ title: document.title, url: location.href }}));
                document.querySelectorAll('video source, audio source, video[src], audio[src]').forEach(el => {{
                    if (el.src) inspectUrl(el.src);
                }});
                installFloatingMenu();
            }});

            document.addEventListener('keydown', (e) => {{
                if (e.ctrlKey && !e.shiftKey && !e.altKey) {{
                    const k = e.key.toLowerCase();
                    if (k === 't' || k === 'w' || k === 'j') {{
                        e.preventDefault();
                        if (window.ipc) window.ipc.postMessage('KEY:' + k);
                    }}
                }}
            }});
        }})();
        "#
    ));

    // 5. Per-Site Controls: JavaScript Gate, Image Blocker, Desktop UA, & Fast YouTube Ad Skipper
    if !policy.js_enabled {
        script.push_str(
            r#"
            (function() {
                window.addEventListener('DOMContentLoaded', () => {
                    document.querySelectorAll('script').forEach(s => s.remove());
                });
            })();
            "#,
        );
    }

    if !policy.images_enabled {
        script.push_str(
            r#"
            (function() {
                const st = document.createElement('style');
                st.textContent = 'img, picture, svg, video[poster], [style*="background-image"] { display: none !important; background-image: none !important; }';
                document.documentElement.appendChild(st);
            })();
            "#,
        );
    }

    if policy.desktop_ua {
        script.push_str(
            r#"
            (function() {
                try {
                    Object.defineProperty(navigator, 'userAgent', {
                        get: () => 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36'
                    });
                } catch(e) {}
            })();
            "#,
        );
    }

    if policy.adblock || policy.sponsorblock || policy.auto_reject_cookies {
        let hide_ads = if policy.adblock || policy.sponsorblock {
            "ytd-reel-shelf-renderer, ytd-rich-section-renderer, ytd-ad-slot-renderer, ytd-banner-promo-renderer, #masthead-ad, .ad-showing .ytp-ad-player-overlay, .ytp-ad-module, ins.adsbygoogle, [id^=\"google_ads_\"], [class*=\"taboola\"], [class*=\"outbrain\"] { display: none !important; }"
        } else {
            ""
        };
        let auto_skip = if policy.sponsorblock { "true" } else { "false" };
        let reject_cookies = if policy.auto_reject_cookies { "true" } else { "false" };

        script.push_str(&format!(
            r#"
            (function() {{
                const st = document.createElement('style');
                st.textContent = `{hide_ads}`;
                document.documentElement.appendChild(st);

                setInterval(() => {{
                    if ({auto_skip}) {{
                        const sk = document.querySelector('.ytp-ad-skip-button, .ytp-ad-skip-button-modern, .ytp-skip-ad-button');
                        if (sk) sk.click();
                        const adShowing = document.querySelector('.ad-showing, .ad-interrupting');
                        const v = document.querySelector('video');
                        if (v && adShowing) {{
                            v.muted = true;
                            if (isFinite(v.duration) && v.duration > 0) {{
                                v.currentTime = v.duration;
                            }} else {{
                                v.playbackRate = 16.0;
                            }}
                        }}
                    }}
                    if ({reject_cookies}) {{
                        document.querySelectorAll('[id*="reject"], [class*="reject"], #onetrust-reject-all-handler, .fc-cta-do-not-consent').forEach(b => b.click());
                    }}
                }}, 150);
            }})();
            "#
        ));
    }

    if !user_js.is_empty() {
        script.push_str(user_js);
    }

    script
}

#[cfg(windows)]
fn attach_native_webview2_interceptor(
    webview: &WebView,
    tab_id: usize,
    adblock_enabled: bool,
    proxy: EventLoopProxy<AppEvent>,
) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL, ICoreWebView2WebResourceResponse,
    };
    use webview2_com::{take_pwstr, WebResourceRequestedEventHandler};
    use windows::core::{w, HSTRING, PCWSTR};
    use wry::WebViewExtWindows;

    let controller = webview.controller();
    unsafe {
        let Ok(core) = controller.CoreWebView2() else {
            return;
        };
        let Ok(env) = core.Environment() else {
            return;
        };

        let _ = core.AddWebResourceRequestedFilter(
            w!("*"),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
        );

        let mut token = std::mem::zeroed();
        let handler = WebResourceRequestedEventHandler::create(Box::new(move |_sender, args| {
            let Some(args) = args else {
                return Ok(());
            };
            let Ok(request) = args.Request() else {
                return Ok(());
            };
            let mut uri_ptr = windows::core::PWSTR::null();
            if request.Uri(&mut uri_ptr).is_ok() {
                let uri = take_pwstr(uri_ptr);
                let lower = uri.to_ascii_lowercase();
                let clean_lower = lower.split('?').next().unwrap_or("");

                const AD_DOMAINS: &[&str] = &[
                    "doubleclick.net",
                    "googlesyndication.com",
                    "googleadservices.com",
                    "adnxs.com",
                    "adsrvr.org",
                    "taboola.com",
                    "outbrain.com",
                    "popads.net",
                    "popcash.net",
                    "propellerads.com",
                    "exoclick.com",
                    "youtube.com/pagead/",
                    "youtube.com/api/stats/ads",
                    "youtube.com/ptracking",
                ];

                if adblock_enabled && AD_DOMAINS.iter().any(|d| lower.contains(d)) {
                    let empty_resp: ICoreWebView2WebResourceResponse = env.CreateWebResourceResponse(
                        None,
                        403,
                        &HSTRING::from("Blocked by Shunno AdBlock"),
                        PCWSTR::null(),
                    )?;
                    let _ = args.SetResponse(&empty_resp);
                    let _ = proxy.send_event(AppEvent::ContentIpc(tab_id, "AD_BLOCKED".to_string()));
                    return Ok(());
                }

                const MEDIA_EXTS: &[&str] = &[
                    ".mp4", ".mkv", ".webm", ".m3u8", ".mpd", ".flac", ".alac", ".m4a", ".wav",
                    ".mp3", ".zip", ".7z", ".rar", ".iso", ".exe", ".msi", ".apk", ".pdf",
                ];

                if uri.starts_with("http") && MEDIA_EXTS.iter().any(|ext| clean_lower.ends_with(ext)) {
                    let _ = proxy.send_event(AppEvent::NativeStreamCaptured(tab_id, uri));
                }
            }
            Ok(())
        }));

        let _ = core.add_WebResourceRequested(&handler, &mut token);
    }
}

#[cfg(not(windows))]
fn attach_native_webview2_interceptor(
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
    proxy: EventLoopProxy<AppEvent>,
    drawer_open: bool,
) -> wry::Result<WebView> {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let content_h = (size.height - CHROME_BAR_HEIGHT).max(100.0);
    let host = extract_host(url);
    let user_js = BrowserStorage::load_matching_user_scripts(&host);
    let init_script = build_content_init_script(policy, global, &user_js);

    let ipc_proxy = proxy.clone();
    let dl_proxy = proxy.clone();
    let win_proxy = proxy.clone();

    let wv = WebViewBuilder::new_as_child(window)
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, CHROME_BAR_HEIGHT).into(),
            size: LogicalSize::new(size.width.max(400.0), content_h).into(),
        })
        .with_url(url)
        .with_initialization_script(&init_script)
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

    attach_native_webview2_interceptor(&wv, tab_id, policy.adblock, proxy);

    let _ = wv.set_visible(!drawer_open);
    Ok(wv)
}

fn layout_webviews(window: &Window, chrome_wv: &WebView, tabs: &[BrowserTab], active_idx: usize, drawer_open: bool) {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let w = size.width.max(400.0);
    let h = size.height.max(300.0);

    let chrome_h = if drawer_open { h } else { CHROME_BAR_HEIGHT };
    let _ = chrome_wv.set_bounds(Rect {
        position: LogicalPosition::new(0.0, 0.0).into(),
        size: LogicalSize::new(w, chrome_h).into(),
    });

    let content_h = (h - CHROME_BAR_HEIGHT).max(100.0);
    for (idx, tab) in tabs.iter().enumerate() {
        let _ = tab.webview.set_bounds(Rect {
            position: LogicalPosition::new(0.0, CHROME_BAR_HEIGHT).into(),
            size: LogicalSize::new(w, content_h).into(),
        });
        let _ = tab.webview.set_visible(!drawer_open && idx == active_idx);
    }
}

fn main() -> wry::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to start Tokio runtime");
    let rt_handle = rt.handle().clone();

    let mut global_settings = BrowserStorage::load_global_settings();
    let mut site_policies = BrowserStorage::load_site_policies();
    let mut bookmarks = BrowserStorage::load_bookmarks();
    let mut history = BrowserStorage::load_history();
    let engine = Arc::new(ShunnoGetDesktop::new(global_settings.speed_limit_bps));

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Shunno Browser")
        .with_inner_size(LogicalSize::new(1220.0, 780.0))
        .build(&event_loop)
        .unwrap();

    let chrome_proxy = proxy.clone();
    let chrome_wv = WebViewBuilder::new_as_child(&window)
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: LogicalSize::new(1220.0, CHROME_BAR_HEIGHT).into(),
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

    let saved_session = BrowserStorage::load_session();
    let initial_urls: Vec<String> = if saved_session.is_empty() {
        vec!["https://duckduckgo.com".to_string()]
    } else {
        saved_session.into_iter().map(|t| t.url).collect()
    };

    for u in initial_urls {
        let host = extract_host(&u);
        let pol = site_policies.get(&host).cloned().unwrap_or_default();
        let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
        if let Ok(wv) = create_tab_webview(&window, tid, &u, &pol, &global_settings, proxy.clone(), false) {
            tabs.push(BrowserTab {
                id: tid,
                title: u.clone(),
                url: u,
                incognito: false,
                webview: wv,
            });
        }
    }

    let ticker_proxy = proxy.clone();
    rt_handle.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(600)).await;
            let _ = ticker_proxy.send_event(AppEvent::RefreshChrome);
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
                             engine: &ShunnoGetDesktop| {
        let active_url = tabs.get(active_idx).map(|t| t.url.clone()).unwrap_or_default();
        let active_host = extract_host(&active_url);
        let site = site_policies.get(&active_host).cloned().unwrap_or_default();
        let downloads = engine.tasks.lock().map(|l| l.clone()).unwrap_or_default();

        let snap = ChromeSnapshot {
            version: APP_VERSION,
            tabs: tabs
                .iter()
                .map(|t| TabSummary {
                    id: t.id,
                    title: t.title.clone(),
                    url: t.url.clone(),
                    incognito: t.incognito,
                })
                .collect(),
            active_tab: active_idx,
            active_url,
            active_host,
            site,
            global: global_settings.clone(),
            blocked_count,
            streams: sniffed_streams.to_vec(),
            bookmarks: bookmarks.to_vec(),
            history: history.to_vec(),
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
                    let ref_url = tabs.iter().find(|t| t.id == tab_id).map(|t| t.url.clone());
                    let eng = engine.clone();
                    let px = proxy.clone();
                    rt_handle.spawn(async move {
                        let res = eng.probe_stream_metadata(stream_url, ref_url, None).await;
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
                let ref_url = tabs.get(active_tab_idx).map(|t| t.url.clone());
                engine.enqueue(&rt_handle, dl_url, global_settings.default_threads, ref_url, None);
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
                    if let Ok(wv) = create_tab_webview(&window, tid, &clean, &pol, &global_settings, proxy.clone(), drawer_open) {
                        tabs.push(BrowserTab {
                            id: tid,
                            title: clean.clone(),
                            url: clean,
                            incognito: false,
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
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::ContentIpc(tab_id, msg)) => {
                if msg == "AD_BLOCKED" {
                    blocked_count += 1;
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
                            engine.enqueue(&rt_handle, dl_url, threads, referer, cookie);
                            drawer_open = true;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                            let _ = chrome_wv.evaluate_script("toggleDrawer('downloads');");
                        }
                    }
                } else if let Some(key_cmd) = msg.strip_prefix("KEY:") {
                    match key_cmd {
                        "t" => {
                            let start = "https://duckduckgo.com".to_string();
                            let pol = SitePolicy::default();
                            let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                            if let Ok(wv) = create_tab_webview(&window, tid, &start, &pol, &global_settings, proxy.clone(), drawer_open) {
                                tabs.push(BrowserTab { id: tid, title: "New Tab".to_string(), url: start, incognito: false, webview: wv });
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
                        let safe_name = title
                            .chars()
                            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
                            .take(40)
                            .collect::<String>()
                            .trim()
                            .replace(' ', "_");
                        let fname = if safe_name.is_empty() { "snapshot.html".to_string() } else { format!("{}.html", safe_name) };
                        let out = BrowserStorage::downloads_root_dir().join(&fname);
                        let _ = fs::write(out, html);
                    }
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
                        if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
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
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::ChromeIpc(cmd)) => {
                if let Some(open_flag) = cmd.strip_prefix("DRAWER_OPEN:") {
                    drawer_open = open_flag == "1";
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
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
                        let _ = tab.webview.load_url(&clean);
                    }
                } else if cmd == "NEW_TAB" || cmd == "NEW_INCOGNITO" {
                    let is_incog = cmd == "NEW_INCOGNITO";
                    let start = "https://duckduckgo.com".to_string();
                    let pol = SitePolicy::default();
                    let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                    if let Ok(wv) = create_tab_webview(&window, tid, &start, &pol, &global_settings, proxy.clone(), drawer_open) {
                        tabs.push(BrowserTab {
                            id: tid,
                            title: if is_incog { "Private Tab".to_string() } else { "New Tab".to_string() },
                            url: start,
                            incognito: is_incog,
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
                } else if let Some(gkey) = cmd.strip_prefix("TOGGLE_GLOBAL:") {
                    match gkey {
                        "full_dark" => {
                            global_settings.full_dark_mode = !global_settings.full_dark_mode;
                            // Reload active tab so Full Dark Mode applies immediately to web content
                            if let Some(tab) = tabs.get(active_tab_idx) {
                                let host = extract_host(&tab.url);
                                let pol = site_policies.get(&host).cloned().unwrap_or_default();
                                let current_url = tab.url.clone();
                                let is_incog = tab.incognito;
                                let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                                if let Ok(new_wv) = create_tab_webview(&window, tid, &current_url, &pol, &global_settings, proxy.clone(), drawer_open) {
                                    tabs[active_tab_idx] = BrowserTab {
                                        id: tid,
                                        title: tab.title.clone(),
                                        url: current_url,
                                        incognito: is_incog,
                                        webview: new_wv,
                                    };
                                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                                }
                            }
                        }
                        "bg_audio" => global_settings.background_media_play = !global_settings.background_media_play,
                        "strip_tracking" => global_settings.strip_tracking_params = !global_settings.strip_tracking_params,
                        "block_popups" => global_settings.block_popups = !global_settings.block_popups,
                        _ => {}
                    }
                    BrowserStorage::save_global_settings(&global_settings);
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
                        let is_incog = tab.incognito;
                        let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                        if let Ok(new_wv) = create_tab_webview(&window, tid, &current_url, &updated_pol, &global_settings, proxy.clone(), drawer_open) {
                            tabs[active_tab_idx] = BrowserTab {
                                id: tid,
                                title: tab.title.clone(),
                                url: current_url,
                                incognito: is_incog,
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
                                document.body.innerHTML = '<div style="max-width:720px;margin:36px auto;padding:20px;font:19px/1.7 BlinkMacSystemFont,Segoe UI,sans-serif;color:#1C1C1E;background:#F2F2F7;">' +
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
                        let js = "if (window.ipc) window.ipc.postMessage('OFFLINE_HTML:' + JSON.stringify({ title: document.title, html: '<!DOCTYPE html>' + document.documentElement.outerHTML }));";
                        let _ = tab.webview.evaluate_script(js);
                    }
                } else if cmd == "OPEN_SCRIPTS_FOLDER" {
                    let dir = BrowserStorage::user_scripts_dir();
                    let _ = Command::new("explorer.exe").arg(dir).spawn();
                } else if cmd == "OPEN_DOWNLOADS_FOLDER" {
                    let dir = BrowserStorage::downloads_root_dir();
                    let _ = Command::new("explorer.exe").arg(dir).spawn();
                } else if let Some(limit_str) = cmd.strip_prefix("SET_SPEED_LIMIT:") {
                    if let Ok(bps) = limit_str.parse::<u64>() {
                        global_settings.speed_limit_bps = bps;
                        engine.speed_limit_bps.store(bps, Ordering::SeqCst);
                        BrowserStorage::save_global_settings(&global_settings);
                    }
                } else if let Some(payload) = cmd.strip_prefix("ENQUEUE_DL:") {
                    if let Some((threads_str, raw_urls)) = payload.split_once('|') {
                        let threads = threads_str.parse::<usize>().unwrap_or(16);
                        let ref_url = tabs.get(active_tab_idx).map(|t| t.url.clone());
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
                                        engine.enqueue(&rt_handle, expanded, threads, ref_url.clone(), None);
                                    }
                                    continue;
                                }
                            }
                            engine.enqueue(&rt_handle, line.to_string(), threads, ref_url.clone(), None);
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
                    &engine,
                );
            }

            _ => {}
        }
    });
}
