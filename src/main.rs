#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod downloader;

use downloader::ShunnoGetDesktop;
use std::sync::Arc;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

enum UserEvent {
    Navigate(String),
    TriggerDownload(String),
}

const SHUNNO_INJECT_SCRIPT: &str = r#"(function(){try{Object.defineProperty(document,'hidden',{get:()=>false,configurable:true});Object.defineProperty(document,'visibilityState',{get:()=>'visible',configurable:true});window.addEventListener('visibilitychange',e=>e.stopImmediatePropagation(),true);document.addEventListener('visibilitychange',e=>e.stopImmediatePropagation(),true);}catch(e){}const st=document.createElement('style');st.textContent='ytd-reel-shelf-renderer,ytd-rich-section-renderer,.ad-showing,.ytp-ad-module,ins.adsbygoogle,[id^="google_ads_"],[class*="taboola"],[class*="outbrain"]{display:none!important;}#shunno-hud{position:fixed;bottom:14px;right:14px;z-index:2147483647;display:flex;gap:6px;align-items:center;background:rgba(28,28,30,0.88);backdrop-filter:blur(14px);padding:7px 12px;border-radius:980px;border:1px solid rgba(255,255,255,0.18);font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;box-shadow:0 8px 24px rgba(0,0,0,0.28);}#shunno-hud button{background:rgba(255,255,255,0.12);color:#fff;border:none;border-radius:980px;padding:4px 10px;font-size:12px;font-weight:600;cursor:pointer;}#shunno-hud button:hover{background:#0A84FF;}#shunno-hud input{background:rgba(255,255,255,0.14);color:#fff;border:none;border-radius:980px;padding:4px 10px;font-size:12px;width:210px;outline:none;}';document.documentElement.appendChild(st);setInterval(()=>{const sk=document.querySelector('.ytp-ad-skip-button,.ytp-ad-skip-button-modern');if(sk)sk.click();document.querySelectorAll('[id*="reject"],#onetrust-reject-all-handler').forEach(b=>b.click());},900);window.addEventListener('DOMContentLoaded',()=>{if(document.getElementById('shunno-hud'))return;const h=document.createElement('div');h.id='shunno-hud';h.innerHTML='<button onclick="history.back()">‹</button><button onclick="history.forward()">›</button><input id="shunno-url-box" value="'+location.href+'" onkeydown="if(event.key===\'Enter\'){window.ipc.postMessage(\'NAV:\'+this.value);}" /><button onclick="document.querySelectorAll(\'video,audio\').forEach(v=>v.playbackRate=(v.playbackRate===2?1:2))">2×</button><button onclick="document.body.innerHTML=\'<div style=\\\'max-width:720px;margin:40px auto;font:19px/1.7 -apple-system,sans-serif;color:#1C1C1E\\\'>\'+document.body.innerText.split(\'\\n\').map(p=>\'<p>\'+p+\'</p>\').join(\'\')+\'</div>\'">📖</button><button style="background:#30D158" onclick="const u=prompt(\'Paste direct file/stream URL for 16-thread Shunno Get:\');if(u)window.ipc.postMessage(\'DL:\'+u);">⚡ Get</button>';document.body.appendChild(h);});})();"#;

fn main() -> wry::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Tokio runtime failed");
    let rt_handle = rt.handle().clone();
    let engine = Arc::new(ShunnoGetDesktop::new());

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Shunno Browser (Rust) — NBPro + Brave Background Audio + 16-Thread Shunno Get")
        .with_inner_size(tao::dpi::LogicalSize::new(1180.0, 760.0))
        .build(&event_loop)
        .unwrap();

    let dl_proxy = proxy.clone();
    let ipc_proxy = proxy.clone();

    let webview = WebViewBuilder::new(&window)
        .with_url("https://duckduckgo.com")
        .with_initialization_script(SHUNNO_INJECT_SCRIPT)
        .with_ipc_handler(move |req| {
            let msg = req.body();
            if let Some(target) = msg.strip_prefix("NAV:") {
                let url = if target.starts_with("http") {
                    target.to_string()
                } else {
                    format!("https://duckduckgo.com/?q={}", target.replace(' ', "+"))
                };
                let _ = ipc_proxy.send_event(UserEvent::Navigate(url));
            } else if let Some(dl_url) = msg.strip_prefix("DL:") {
                let _ = ipc_proxy.send_event(UserEvent::TriggerDownload(dl_url.to_string()));
            }
        })
        .with_download_started_handler(move |url, _dest| {
            let _ = dl_proxy.send_event(UserEvent::TriggerDownload(url));
            false
        })
        .build()?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::Navigate(url)) => {
                let _ = webview.load_url(&url);
            }
            Event::UserEvent(UserEvent::TriggerDownload(url)) => {
                engine.enqueue(&rt_handle, url, None, None);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}
