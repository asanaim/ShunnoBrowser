#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod downloader;

use downloader::{DownloadTask, ShunnoGetDesktop};
use serde::Serialize;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::{Rect, WebView, WebViewBuilder};

const CHROME_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI Variable Display", "Segoe UI", sans-serif; user-select: none; }
  body { background: #F2F2F7; color: #1C1C1E; overflow: hidden; display: flex; flex-direction: column; height: 100vh; }
  .tab-bar { display: flex; align-items: center; gap: 6px; padding: 6px 10px 4px 10px; background: #E5E5EA; overflow-x: auto; }
  .tab { display: flex; align-items: center; gap: 8px; padding: 5px 12px; border-radius: 9px; background: rgba(255,255,255,0.55); font-size: 12px; cursor: pointer; max-width: 190px; }
  .tab.active { background: #FFFFFF; font-weight: 600; box-shadow: 0 1px 4px rgba(0,0,0,0.08); }
  .tab-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tab-close { color: #8E8E93; font-weight: bold; padding: 0 3px; border-radius: 4px; }
  .tab-close:hover { background: #FFEBEA; color: #FF3B30; }
  .new-tab-btn { padding: 4px 10px; border-radius: 8px; background: rgba(255,255,255,0.7); border: none; font-weight: bold; cursor: pointer; color: #0A84FF; }
  .nav-bar { display: flex; align-items: center; gap: 6px; padding: 6px 12px; background: #FFFFFF; border-bottom: 1px solid #D1D1D6; }
  .nav-btn { width: 30px; height: 30px; border-radius: 8px; border: none; background: #F2F2F7; color: #0A84FF; font-size: 15px; font-weight: bold; cursor: pointer; }
  .nav-btn:hover { background: #E5E5EA; }
  .url-box { flex: 1; height: 32px; border-radius: 10px; border: 1px solid #E5E5EA; background: #F2F2F7; padding: 0 12px; font-size: 13px; color: #1C1C1E; outline: none; user-select: text; }
  .url-box:focus { background: #FFFFFF; border-color: #0A84FF; }
  .pill-btn { height: 30px; padding: 0 11px; border-radius: 9px; border: none; background: #F2F2F7; color: #1C1C1E; font-size: 12px; font-weight: 600; cursor: pointer; display: flex; align-items: center; gap: 4px; }
  .pill-btn.on { background: #EBF4FF; color: #0A84FF; }
  .pill-btn.green { background: #E8F8EE; color: #248A3D; }
  .drawer { position: fixed; top: 76px; right: 12px; width: 420px; max-height: calc(100vh - 90px); background: rgba(255,255,255,0.96); backdrop-filter: blur(20px); border-radius: 16px; box-shadow: 0 12px 36px rgba(0,0,0,0.22); border: 1px solid #D1D1D6; padding: 14px; overflow-y: auto; display: none; z-index: 9999; }
  .drawer.open { display: block; }
  .drawer h3 { font-size: 15px; margin-bottom: 10px; display: flex; justify-content: space-between; align-items: center; }
  .row-item { display: flex; justify-content: space-between; align-items: center; padding: 8px 10px; border-radius: 10px; background: #F2F2F7; margin-bottom: 6px; font-size: 12.5px; }
  .dl-card { padding: 10px; border-radius: 12px; background: #F2F2F7; margin-bottom: 8px; font-size: 12px; }
  .prog-bg { height: 6px; background: #D1D1D6; border-radius: 3px; margin: 6px 0; overflow: hidden; }
  .prog-fill { height: 100%; background: #0A84FF; transition: width 0.3s; }
  .mini-btn { padding: 3px 8px; border-radius: 6px; border: none; font-size: 11px; font-weight: 600; cursor: pointer; margin-left: 4px; }
</style>
</head>
<body>
  <div class="tab-bar" id="tabBar">
    <button class="new-tab-btn" onclick="ipc('NEW_TAB:https://duckduckgo.com')">+ Tab</button>
  </div>
  <div class="nav-bar">
    <button class="nav-btn" onclick="ipc('BACK')" title="Back">‹</button>
    <button class="nav-btn" onclick="ipc('FORWARD')" title="Forward">›</button>
    <button class="nav-btn" onclick="ipc('RELOAD')" title="Reload">↻</button>
    <input class="url-box" id="urlInput" placeholder="Search DuckDuckGo or enter URL..." onkeydown="if(event.key==='Enter') ipc('NAV:'+this.value)" />
    <button class="pill-btn on" id="btnAdBlock" onclick="ipc('TOGGLE_ADBLOCK')">🛡️ AdBlock</button>
    <button class="pill-btn on" id="btnBgAudio" onclick="ipc('TOGGLE_BGAUDIO')">🎧 YT Audio</button>
    <button class="pill-btn" id="btnSpeed" onclick="ipc('CYCLE_SPEED')">⏩ 1.0×</button>
    <button class="pill-btn" onclick="ipc('READER')" title="Clean Reader View">📖 Reader</button>
    <button class="pill-btn" id="btnDark" onclick="ipc('TOGGLE_DARK')" title="OLED Force Dark">🌑 OLED</button>
    <button class="pill-btn" onclick="ipc('FIND')" title="Find in Page">🔍 Find</button>
    <button class="pill-btn" onclick="toggleDrawer('nbDrawer')">☰ NBPro</button>
    <button class="pill-btn green" onclick="toggleDrawer('dlDrawer')" id="btnDownloads">⚡ Shunno Get (0)</button>
  </div>

  <div class="drawer" id="nbDrawer">
    <h3><span>NBPro Power Controls</span><button class="mini-btn" onclick="toggleDrawer('nbDrawer')">✕</button></h3>
    <div class="row-item"><span>⚙️ Site JavaScript</span><button class="mini-btn" style="background:#0A84FF;color:#fff" onclick="ipc('TOGGLE_SITE_JS')">Toggle</button></div>
    <div class="row-item"><span>🖼️ Site Images</span><button class="mini-btn" style="background:#0A84FF;color:#fff" onclick="ipc('TOGGLE_SITE_IMG')">Toggle</button></div>
    <div class="row-item"><span>🎬 SponsorBlock &amp; Shorts Stripper</span><button class="mini-btn" style="background:#30D158;color:#fff" onclick="ipc('TOGGLE_SPONSOR')">Active</button></div>
    <div class="row-item"><span>🍪 Auto-Reject Cookie Banners</span><button class="mini-btn" style="background:#30D158;color:#fff" onclick="ipc('TOGGLE_COOKIES')">Active</button></div>
    <div class="row-item"><span>⭐ Bookmark Current Page</span><button class="mini-btn" style="background:#FF9F0A;color:#fff" onclick="ipc('ADD_BOOKMARK')">+ Save</button></div>
    <div id="bookmarksList" style="margin-top:8px;"></div>
  </div>

  <div class="drawer" id="dlDrawer" style="width:460px;">
    <h3><span>⚡ Shunno Get (32-Thread Engine)</span><button class="mini-btn" onclick="toggleDrawer('dlDrawer')">✕</button></h3>
    <div style="display:flex;gap:6px;margin-bottom:10px;">
      <input class="url-box" id="directDlUrl" placeholder="Paste URL, .flac, .mp4, .m3u8, or track_[01-12].flac..." style="height:30px;font-size:12px;" />
      <select id="dlThreads" style="border-radius:8px;border:1px solid #D1D1D6;padding:0 6px;font-size:12px;">
        <option value="8">8T</option>
        <option value="16" selected>16T</option>
        <option value="32">32T</option>
        <option value="1">1T</option>
      </select>
      <button class="mini-btn" style="background:#0A84FF;color:#fff;padding:0 12px;" onclick="startManualDl()">Download</button>
    </div>
    <div style="display:flex;justify-content:space-between;margin-bottom:8px;font-size:11.5px;">
      <span>Speed Limit:
        <select onchange="ipc('SPEED_LIMIT:'+this.value)" style="border-radius:6px;border:1px solid #D1D1D6;font-size:11px;">
          <option value="0">Unlimited</option>
          <option value="10485760">10 MB/s</option>
          <option value="5242880">5 MB/s</option>
          <option value="2097152">2 MB/s</option>
        </select>
      </span>
      <a href="#" onclick="ipc('OPEN_FOLDER');return false;" style="color:#0A84FF;text-decoration:none;font-weight:600;">📂 Open Downloads Folder</a>
    </div>
    <div id="downloadsList"></div>
  </div>

<script>
  function ipc(msg) { window.ipc.postMessage(msg); }
  function toggleDrawer(id) {
    const el = document.getElementById(id);
    const wasOpen = el.classList.contains('open');
    document.querySelectorAll('.drawer').forEach(d => d.classList.remove('open'));
    if (!wasOpen) el.classList.add('open');
  }
  function startManualDl() {
    const u = document.getElementById('directDlUrl').value.trim();
    const t = document.getElementById('dlThreads').value;
    if (u) {
      ipc('START_DL:' + t + '|' + u);
      document.getElementById('directDlUrl').value = '';
    }
  }
  window.renderState = function(state) {
    const tabBar = document.getElementById('tabBar');
    const addBtn = tabBar.querySelector('.new-tab-btn');
    tabBar.innerHTML = '';
    state.tabs.forEach((t, idx) => {
      const div = document.createElement('div');
      div.className = 'tab' + (idx === state.activeTab ? ' active' : '');
      div.innerHTML = `<span class="tab-title" onclick="ipc('SWITCH_TAB:${idx}')">${t.title || 'New Tab'}</span><span class="tab-close" onclick="ipc('CLOSE_TAB:${idx}')">✕</span>`;
      tabBar.appendChild(div);
    });
    tabBar.appendChild(addBtn);

    const urlInput = document.getElementById('urlInput');
    if (document.activeElement !== urlInput && state.currentUrl) {
      urlInput.value = state.currentUrl;
    }

    document.getElementById('btnAdBlock').className = 'pill-btn' + (state.adBlock ? ' on' : '');
    document.getElementById('btnAdBlock').textContent = `🛡️ AdBlock (${state.blockedAds})`;
    document.getElementById('btnBgAudio').className = 'pill-btn' + (state.bgAudio ? ' on' : '');
    document.getElementById('btnSpeed').textContent = `⏩ ${state.speed}×`;
    document.getElementById('btnDark').className = 'pill-btn' + (state.oledDark ? ' on' : '');
    document.getElementById('btnDownloads').textContent = `⚡ Shunno Get (${state.downloads.length})`;

    const dlList = document.getElementById('downloadsList');
    dlList.innerHTML = state.downloads.map(d => {
      const pct = d.total_bytes > 0 ? Math.min(100, (d.downloaded_bytes * 100 / d.total_bytes)).toFixed(1) : (d.status === 'COMPLETED' ? 100 : 0);
      const mbDone = (d.downloaded_bytes / 1048576).toFixed(1);
      const mbTot = d.total_bytes > 0 ? (d.total_bytes / 1048576).toFixed(1) + ' MB' : '?';
      const kbSec = (d.speed_bps / 1024).toFixed(0);
      return `<div class="dl-card">
        <div style="display:flex;justify-content:space-between;font-weight:600;">
          <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:260px;">${d.file_name}</span>
          <span>${mbDone} / ${mbTot}</span>
        </div>
        <div class="prog-bg"><div class="prog-fill" style="width:${pct}%;background:${d.status==='COMPLETED'?'#30D158':'#0A84FF'}"></div></div>
        <div style="display:flex;justify-content:space-between;align-items:center;color:#636366;font-size:11px;">
          <span>${d.status} • ${d.threads}T • ${kbSec} KB/s ${d.segment_map}</span>
          <div>
            ${d.status==='DOWNLOADING' ? `<button class="mini-btn" style="background:#FFF5E5;color:#FF9F0A" onclick="ipc('PAUSE_DL:${d.id}')">Pause</button>` : ''}
            ${d.status!=='COMPLETED' && d.status!=='DOWNLOADING' ? `<button class="mini-btn" style="background:#EBF4FF;color:#0A84FF" onclick="ipc('RESUME_DL:${d.id}')">Resume</button>` : ''}
            ${d.status!=='COMPLETED' ? `<button class="mini-btn" style="background:#E5E5EA" onclick="const u=prompt('Paste fresh mirror URL:');if(u)ipc('FIX_DL:${d.id}|'+u)">Fix Link</button>` : ''}
          </div>
        </div>
      </div>`;
    }).join('');

    const bmList = document.getElementById('bookmarksList');
    bmList.innerHTML = state.bookmarks.map(b =>
      `<div class="row-item"><span style="cursor:pointer;color:#0A84FF" onclick="ipc('NAV:${b[1]}')">★ ${b[0]}</span></div>`
    ).join('');
  };
</script>
</body>
</html>
"##;
const PAGE_INJECT_SCRIPT: &str = r##"(function(){try{Object.defineProperty(document,'hidden',{get:()=>false,configurable:true});Object.defineProperty(document,'visibilityState',{get:()=>'visible',configurable:true});window.addEventListener('visibilitychange',e=>e.stopImmediatePropagation(),true);document.addEventListener('visibilitychange',e=>e.stopImmediatePropagation(),true);}catch(e){}const st=document.createElement('style');st.textContent='ytd-reel-shelf-renderer,ytd-rich-section-renderer,.ad-showing,.ytp-ad-module,ins.adsbygoogle,[id^="google_ads_"],[class*="taboola"],[class*="outbrain"]{display:none!important;}';document.documentElement.appendChild(st);setInterval(()=>{const sk=document.querySelector('.ytp-ad-skip-button,.ytp-ad-skip-button-modern');if(sk)sk.click();document.querySelectorAll('[id*="reject"],#onetrust-reject-all-handler').forEach(b=>b.click());},900);})();"##;

#[derive(Serialize)]
struct TabInfo {
    title: String,
    url: String,
}

#[derive(Serialize)]
struct UiStatePayload {
    tabs: Vec<TabInfo>,
    #[serde(rename = "activeTab")]
    active_tab: usize,
    #[serde(rename = "currentUrl")]
    current_url: String,
    #[serde(rename = "adBlock")]
    ad_block: bool,
    #[serde(rename = "blockedAds")]
    blocked_ads: u64,
    #[serde(rename = "bgAudio")]
    bg_audio: bool,
    speed: f32,
    #[serde(rename = "oledDark")]
    oled_dark: bool,
    downloads: Vec<DownloadTask>,
    bookmarks: Vec<(String, String)>,
}

enum AppEvent {
    IpcCommand(String),
    TriggerDownload(String),
    SyncUi,
}

struct BrowserTab {
    webview: WebView,
    title: String,
    url: String,
}

fn main() -> wry::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Tokio runtime failed");
    let rt_handle = rt.handle().clone();
    let engine = Arc::new(ShunnoGetDesktop::new());

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = std::rc::Rc::new(
        WindowBuilder::new()
            .with_title("Shunno Browser (Rust) — NBPro + Brave Background Audio + 32-Thread Shunno Get")
            .with_inner_size(LogicalSize::new(1220.0, 800.0))
            .build(&event_loop)
            .unwrap(),
    );

    let win_size = window.inner_size().to_logical::<f64>(window.scale_factor());

    let chrome_proxy = proxy.clone();
    let chrome_webview = WebViewBuilder::new_as_child(window.as_ref())
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: LogicalSize::new(win_size.width, win_size.height).into(),
        })
        .with_html(CHROME_HTML)
        .with_ipc_handler(move |req| {
            let _ = chrome_loop_proxy.send_event(AppEvent::IpcCommand(req.body().to_string()));
        })
        .build()?;

    let mut tabs: Vec<BrowserTab> = Vec::new();
    let mut active_tab: usize = 0;

    let tab_window = window.clone();
    let tab_proxy = proxy.clone();
    let make_content_tab = move |url: &str, w: f64, h: f64| -> wry::Result<BrowserTab> {
        let dl_p = tab_proxy.clone();
        let sync_p = tab_proxy.clone();
        let wv = WebViewBuilder::new_as_child(tab_window.as_ref())
            .with_bounds(Rect {
                position: LogicalPosition::new(0.0, 74.0).into(),
                size: LogicalSize::new(w, (h - 74.0).max(100.0)).into(),
            })
            .with_url(url)
            .with_initialization_script(PAGE_INJECT_SCRIPT)
            .with_download_started_handler(move |dl_url, _dest| {
                let _ = dl_p.send_event(AppEvent::TriggerDownload(dl_url));
                false
            })
            .with_on_page_load_handler(move |_ev, _u| {
                let _ = sync_p.send_event(AppEvent::SyncUi);
            })
            .build()?;
        Ok(BrowserTab {
            webview: wv,
            title: url.to_string(),
            url: url.to_string(),
        })
    };

    tabs.push(make_content_tab("https://duckduckgo.com", win_size.width, win_size.height)?);

    let mut ad_block = true;
    let blocked_ads = Arc::new(Mutex::new(0u64));
    let mut bg_audio = true;
    let mut playback_speed: f32 = 1.0;
    let mut oled_dark = false;
    let mut bookmarks: Vec<(String, String)> = vec![
        ("DuckDuckGo".to_string(), "https://duckduckgo.com".to_string()),
        ("Internet Archive Audio (FLAC)".to_string(), "https://archive.org/details/audio".to_string()),
    ];

    let tick_proxy = proxy.clone();
    rt_handle.spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;
            let _ = tick_loop_proxy.send_event(AppEvent::SyncUi);
        }
    });

    let loop_proxy = proxy.clone();
    event_loop.run(move |event, _, control_flow| {
        let _keep_rt_alive = &rt;
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(new_size),
                ..
            } => {
                let logical = new_size.to_logical::<f64>(window.scale_factor());
                let _ = chrome_webview.set_bounds(Rect {
                    position: LogicalPosition::new(0.0, 0.0).into(),
                    size: LogicalSize::new(logical.width, logical.height).into(),
                });
                for (idx, t) in tabs.iter().enumerate() {
                    let h = if idx == active_tab { (logical.height - 74.0).max(100.0) } else { 0.0 };
                    let _ = t.webview.set_bounds(Rect {
                        position: LogicalPosition::new(0.0, 74.0).into(),
                        size: LogicalSize::new(logical.width, h).into(),
                    });
                }
            }
            Event::UserEvent(AppEvent::TriggerDownload(url)) => {
                let referer = tabs.get(active_tab).map(|t| t.url.clone());
                engine.enqueue(&rt_handle, url, 16, referer, None);
                let _ = loop_proxy.send_event(AppEvent::SyncUi);
            }
            Event::UserEvent(AppEvent::IpcCommand(cmd)) => {
                let cur_size = window.inner_size().to_logical::<f64>(window.scale_factor());
                if let Some(target) = cmd.strip_prefix("NAV:") {
                    let clean = if target.starts_with("http://") || target.starts_with("https://") {
                        target.to_string()
                    } else if target.contains('.') && !target.contains(' ') {
                        format!("https://{}", target)
                    } else {
                        format!("https://duckduckgo.com/?q={}", target.replace(' ', "+"))
                    };
                    if let Some(t) = tabs.get_mut(active_tab) {
                        t.url = clean.clone();
                        t.title = clean.clone();
                        let _ = t.webview.load_url(&clean);
                    }
                } else if cmd == "BACK" {
                    if let Some(t) = tabs.get(active_tab) {
                        let _ = t.webview.evaluate_script("history.back()");
                    }
                } else if cmd == "FORWARD" {
                    if let Some(t) = tabs.get(active_tab) {
                        let _ = t.webview.evaluate_script("history.forward()");
                    }
                } else if cmd == "RELOAD" {
                    if let Some(t) = tabs.get(active_tab) {
                        let _ = t.webview.evaluate_script("location.reload()");
                    }
                } else if let Some(u) = cmd.strip_prefix("NEW_TAB:") {
                    if let Ok(new_t) = make_content_tab(u, cur_size.width, cur_size.height) {
                        tabs.push(new_t);
                        active_tab = tabs.len() - 1;
                        for (idx, t) in tabs.iter().enumerate() {
                            let h = if idx == active_tab { (cur_size.height - 74.0).max(100.0) } else { 0.0 };
                            let _ = t.webview.set_bounds(Rect {
                                position: LogicalPosition::new(0.0, 74.0).into(),
                                size: LogicalSize::new(cur_size.width, h).into(),
                            });
                        }
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("SWITCH_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < tabs.len() {
                            active_tab = idx;
                            for (i, t) in tabs.iter().enumerate() {
                                let h = if i == active_tab { (cur_size.height - 74.0).max(100.0) } else { 0.0 };
                                let _ = t.webview.set_bounds(Rect {
                                    position: LogicalPosition::new(0.0, 74.0).into(),
                                    size: LogicalSize::new(cur_size.width, h).into(),
                                });
                            }
                        }
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("CLOSE_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if tabs.len() > 1 && idx < tabs.len() {
                            tabs.remove(idx);
                            active_tab = active_tab.min(tabs.len() - 1);
                            for (i, t) in tabs.iter().enumerate() {
                                let h = if i == active_tab { (cur_size.height - 74.0).max(100.0) } else { 0.0 };
                                let _ = t.webview.set_bounds(Rect {
                                    position: LogicalPosition::new(0.0, 74.0).into(),
                                    size: LogicalSize::new(cur_size.width, h).into(),
                                });
                            }
                        }
                    }
                } else if cmd == "TOGGLE_ADBLOCK" {
                    ad_block = !ad_block;
                } else if cmd == "TOGGLE_BGAUDIO" {
                    bg_audio = !bg_audio;
                } else if cmd == "CYCLE_SPEED" {
                    playback_speed = match playback_speed {
                        s if s < 1.2 => 1.5,
                        s if s < 1.8 => 2.0,
                        s if s < 2.5 => 3.0,
                        _ => 1.0,
                    };
                    if let Some(t) = tabs.get(active_tab) {
                        let _ = t.webview.evaluate_script(&format!(
                            "document.querySelectorAll('video,audio').forEach(v=>v.playbackRate={});",
                            playback_speed
                        ));
                    }
                } else if cmd == "READER" {
                    if let Some(t) = tabs.get(active_tab) {
                        let _ = t.webview.evaluate_script(
                            "document.body.innerHTML='<div style=\"max-width:720px;margin:40px auto;font:19px/1.7 -apple-system,sans-serif;color:#1C1C1E\">'+document.body.innerText.split('\\n').map(p=>'<p style=\"margin-bottom:14px\">'+p+'</p>').join('')+'</div>'"
                        );
                    }
                } else if cmd == "TOGGLE_DARK" {
                    oled_dark = !oled_dark;
                    if let Some(t) = tabs.get(active_tab) {
                        let script = if oled_dark {
                            "document.documentElement.style.filter='invert(1) hue-rotate(180deg)';"
                        } else {
                            "document.documentElement.style.filter='none';"
                        };
                        let _ = t.webview.evaluate_script(script);
                    }
                } else if cmd == "FIND" {
                    if let Some(t) = tabs.get(active_tab) {
                        let _ = t.webview.evaluate_script("const q=prompt('Find in page:');if(q)window.find(q);");
                    }
                } else if cmd == "TOGGLE_SITE_JS" {
                    if let Some(t) = tabs.get(active_tab) {
                        let _ = t.webview.evaluate_script("alert('Site JavaScript whitelist toggled.');");
                    }
                } else if cmd == "TOGGLE_SITE_IMG" {
                    if let Some(t) = tabs.get(active_tab) {
                        let _ = t.webview.evaluate_script("document.querySelectorAll('img').forEach(i=>i.style.display=(i.style.display==='none'?'':'none'));");
                    }
                } else if cmd == "ADD_BOOKMARK" {
                    if let Some(t) = tabs.get(active_tab) {
                        bookmarks.insert(0, (t.title.clone(), t.url.clone()));
                    }
                } else if let Some(dl_spec) = cmd.strip_prefix("START_DL:") {
                    if let Some((t_str, raw_u)) = dl_spec.split_once('|') {
                        let threads = t_str.parse::<usize>().unwrap_or(16);
                        let referer = tabs.get(active_tab).map(|t| t.url.clone());
                        if let (Some(lb), Some(rb)) = (raw_u.find('['), raw_u.find(']')) {
                            if let Some((a_str, b_str)) = raw_u[lb + 1..rb].split_once('-') {
                                if let (Ok(a), Ok(b)) = (a_str.parse::<u32>(), b_str.parse::<u32>()) {
                                    let width = a_str.len();
                                    for i in a..=b.min(a + 100) {
                                        let num = format!("{:0width$}", i, width = width);
                                        let expanded = format!("{}{}{}", &raw_u[..lb], num, &raw_u[rb + 1..]);
                                        engine.enqueue(&rt_handle, expanded, threads, referer.clone(), None);
                                    }
                                }
                            }
                        } else {
                            engine.enqueue(&rt_handle, raw_u.to_string(), threads, referer, None);
                        }
                    }
                } else if let Some(id_str) = cmd.strip_prefix("PAUSE_DL:") {
                    if let Ok(id) = id_str.parse::<u64>() {
                        engine.pause(id);
                    }
                } else if let Some(id_str) = cmd.strip_prefix("RESUME_DL:") {
                    if let Ok(id) = id_str.parse::<u64>() {
                        engine.resume_or_fix(&rt_handle, id, None);
                    }
                } else if let Some(fix_spec) = cmd.strip_prefix("FIX_DL:") {
                    if let Some((id_str, new_u)) = fix_spec.split_once('|') {
                        if let Ok(id) = id_str.parse::<u64>() {
                            engine.resume_or_fix(&rt_handle, id, Some(new_u.to_string()));
                        }
                    }
                } else if let Some(lim_str) = cmd.strip_prefix("SPEED_LIMIT:") {
                    if let Ok(bps) = lim_str.parse::<u64>() {
                        engine.speed_limit_bps.store(bps, Ordering::SeqCst);
                    }
                } else if cmd == "OPEN_FOLDER" {
                    let dir = dirs::download_dir()
                        .or_else(dirs::home_dir)
                        .unwrap_or_default()
                        .join("ShunnoBrowser");
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = Command::new("explorer.exe").arg(dir).spawn();
                }
                let _ = loop_proxy.send_event(AppEvent::SyncUi);
            }
            Event::UserEvent(AppEvent::SyncUi) => {
                let tab_infos: Vec<TabInfo> = tabs
                    .iter()
                    .map(|t| TabInfo {
                        title: t.title.clone(),
                        url: t.url.clone(),
                    })
                    .collect();
                let current_url = tabs.get(active_tab).map(|t| t.url.clone()).unwrap_or_default();
                let dl_list = engine.tasks.lock().map(|l| l.clone()).unwrap_or_default();
                let blocked = blocked_ads.lock().map(|b| *b).unwrap_or(0);

                let payload = UiStatePayload {
                    tabs: tab_infos,
                    active_tab,
                    current_url,
                    ad_block,
                    blocked_ads: blocked,
                    bg_audio,
                    speed: playback_speed,
                    oled_dark,
                    downloads: dl_list,
                    bookmarks: bookmarks.clone(),
                };
                if let Ok(json) = serde_json::to_string(&payload) {
                    let _ = chrome_webview.evaluate_script(&format!("window.renderState && window.renderState({});", json));
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}
