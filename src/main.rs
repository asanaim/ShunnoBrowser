#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod blocker;
mod downloader;
mod extensions;
mod https_guard;
mod importer;
mod passwords;
mod permissions;
mod little;
mod updater;
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
/// Smallest browser window. Below this the toolbar can no longer show its buttons.
const MIN_WINDOW_W: f64 = 420.0;
const MIN_WINDOW_H: f64 = 320.0;

/// Actual toolbar height reported by the chrome UI (`CHROME_H:`), as f64 bits. The bookmarks bar is
/// shown only on the New Tab page, and the maximized tab strip is taller, so the height varies.
static CHROME_BAR_H_BITS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x405C_0000_0000_0000); // 112.0

/// Arc-style Peek: one floating preview web view above the active tab. Lives on the UI thread.
struct Peek {
    id: usize,
    webview: WebView,
    incognito: bool,
}
thread_local! {
    static CHROME_LAST_H: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    static PEEK: std::cell::RefCell<Option<Peek>> = const { std::cell::RefCell::new(None) };
    /// Last laid-out Peek rectangle (x, y, w, h) in logical px, for the chrome UI's frame.
    static PEEK_RECT: std::cell::Cell<[f64; 4]> = const { std::cell::Cell::new([0.0; 4]) };
}
fn peek_open() -> bool {
    PEEK.with(|p| p.borrow().is_some())
}
fn peek_id() -> Option<usize> {
    PEEK.with(|p| p.borrow().as_ref().map(|k| k.id))
}
fn close_peek() -> bool {
    PEEK.with(|p| p.borrow_mut().take()).is_some()
}

#[derive(Serialize)]
struct PeekView {
    url: String,
    rect: [f64; 4],
}

/// Active search engine name, for the right-click menu label (updated with every chrome state push).
fn search_engine_label() -> &'static std::sync::Mutex<String> {
    static L: std::sync::OnceLock<std::sync::Mutex<String>> = std::sync::OnceLock::new();
    L.get_or_init(|| std::sync::Mutex::new(String::from("the web")))
}

/// Run `js` in the content web view with this id (a tab or the Peek).
fn eval_in_tab(tabs: &[BrowserTab], tab_id: usize, js: &str) {
    if let Some(t) = tabs.iter().find(|t| t.id == tab_id) {
        let _ = t.webview.evaluate_script(js);
    } else {
        PEEK.with(|p| {
            if let Some(k) = p.borrow().as_ref().filter(|k| k.id == tab_id) {
                let _ = k.webview.evaluate_script(js);
            }
        });
    }
}

/// Current origin of a content web view (from the engine, not from page-supplied data), and whether it is private.
fn tab_origin(tabs: &[BrowserTab], tab_id: usize) -> Option<(String, bool)> {
    let (url, private) = match tabs.iter().find(|t| t.id == tab_id) {
        Some(t) => (t.webview.url().ok()?, t.incognito),
        None => PEEK.with(|p| p.borrow().as_ref().filter(|k| k.id == tab_id).and_then(|k| Some((k.webview.url().ok()?, k.incognito))))?,
    };
    Some((passwords::origin_of(&url)?, private))
}

thread_local! {
    /// Each tab's site icon (PNG data URL) for the tab strip, by tab id.
    static FAVICONS: std::cell::RefCell<HashMap<usize, String>> = std::cell::RefCell::new(HashMap::new());
}
thread_local! {
    /// A question sheet (Add Bookmark, Rename, Delete?) is open in the focused window's toolbar.
    static SHEET_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
thread_local! {
    /// One-line note shown at the top of the Menu (e.g. after "Set as Default Browser").
    static MENU_NOTE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

// ---------------------------------------------------------------- Tab groups
thread_local! {
    static GROUPS: std::cell::RefCell<Vec<state::TabGroup>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Tab id → group id.
    static TAB_GROUP: std::cell::RefCell<HashMap<usize, u32>> = std::cell::RefCell::new(HashMap::new());
}
fn tab_group(tab_id: usize) -> Option<u32> {
    TAB_GROUP.with(|m| m.borrow().get(&tab_id).copied())
}
fn set_tab_group(tab_id: usize, g: Option<u32>) {
    TAB_GROUP.with(|m| {
        let mut m = m.borrow_mut();
        match g {
            Some(g) => m.insert(tab_id, g),
            None => m.remove(&tab_id),
        };
    });
}
fn group_mut<R>(gid: u32, f: impl FnOnce(&mut state::TabGroup) -> R) -> Option<R> {
    GROUPS.with(|g| g.borrow_mut().iter_mut().find(|x| x.id == gid).map(f))
}
/// Groups that still have tabs (others are dropped), saved when that changes anything.
fn live_groups(tabs: &[BrowserTab]) -> Vec<state::TabGroup> {
    GROUPS.with(|g| {
        let mut g = g.borrow_mut();
        let before = g.len();
        let others = OTHER_TAB_IDS.with(|o| o.borrow().clone());
        let alive = |x: &state::TabGroup| tabs.iter().map(|t| t.id).chain(others.iter().copied()).any(|id| tab_group(id) == Some(x.id));
        if RESTORES_PENDING.with(|r| r.get()) > 0 {
            // Windows still being rebuilt hold tabs of these groups: show, but do not forget, them.
            return g.iter().filter(|x| alive(x)).cloned().collect();
        }
        g.retain(alive);
        if g.len() != before {
            BrowserStorage::save_tab_groups(&g);
        }
        g.clone()
    })
}
fn save_groups() {
    GROUPS.with(|g| BrowserStorage::save_tab_groups(&g.borrow()));
}
/// Make every group's tabs contiguous, keeping the active and split tabs selected.
fn regroup_tabs(tabs: &mut Vec<BrowserTab>, active: &mut usize, split: &mut Option<usize>) {
    let order = state::group_contiguous_order(&tabs.iter().map(|t| tab_group(t.id)).collect::<Vec<_>>());
    if order.iter().enumerate().all(|(i, &j)| i == j) {
        return;
    }
    let active_id = tabs.get(*active).map(|t| t.id);
    let split_id = split.and_then(|i| tabs.get(i)).map(|t| t.id);
    let mut slots: Vec<Option<BrowserTab>> = std::mem::take(tabs).into_iter().map(Some).collect();
    *tabs = order.into_iter().filter_map(|j| slots[j].take()).collect();
    if let Some(id) = active_id {
        *active = tabs.iter().position(|t| t.id == id).unwrap_or(0);
    }
    *split = split_id.and_then(|id| tabs.iter().position(|t| t.id == id));
}
/// Put tab `i` into group `gid` right after the group's last tab.
fn move_into_group(tabs: &mut Vec<BrowserTab>, i: usize, gid: u32, active: &mut usize, split: &mut Option<usize>) {
    if i >= tabs.len() {
        return;
    }
    set_tab_group(tabs[i].id, Some(gid));
    let active_id = tabs.get(*active).map(|t| t.id);
    let split_id = split.and_then(|k| tabs.get(k)).map(|t| t.id);
    let t = tabs.remove(i);
    let at = tabs.iter().rposition(|x| tab_group(x.id) == Some(gid)).map(|k| k + 1).unwrap_or(i.min(tabs.len()));
    tabs.insert(at, t);
    if let Some(aid) = active_id {
        *active = tabs.iter().position(|t| t.id == aid).unwrap_or(0);
    }
    *split = split_id.and_then(|sid| tabs.iter().position(|t| t.id == sid));
    regroup_tabs(tabs, active, split);
}

/// Reader (Safari-style): Mozilla Readability + Shunno's reader UI, injected only when opened.
const READABILITY_JS: &str = include_str!("../assets/js/readability.min.js");
const READER_JS: &str = include_str!("../assets/js/reader.js");

fn build_reader_script(prefs_json: &str) -> String {
    let prefs = state::valid_reader_prefs(prefs_json).unwrap_or_else(|| "{}".to_string());
    // One closure: Readability and the reader stay private to it; only window.__shunnoReader is set.
    format!(
        "(function(){{\"use strict\";var module;\n{}\n{}\n}})();",
        READABILITY_JS,
        READER_JS.replace("__SHUNNO_READER_PREFS__", &prefs)
    )
}

thread_local! {
    /// Tabs whose current page looks like an article (shows the Reader button).
    static READER_AVAILABLE: std::cell::RefCell<std::collections::HashSet<usize>> = std::cell::RefCell::new(std::collections::HashSet::new());
}

/// Permission requests held open (WebView2 deferral) until the user or a saved decision answers.
#[cfg(windows)]
struct PendingPermission {
    id: u64,
    tab_id: usize,
    origin: String,
    kind: &'static str,
    private: bool,
    args: webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2PermissionRequestedEventArgs,
    deferral: webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Deferral,
}
#[cfg(windows)]
thread_local! {
    static PERM_PENDING: std::cell::RefCell<Vec<PendingPermission>> = const { std::cell::RefCell::new(Vec::new()) };
}
static PERM_NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[cfg(windows)]
fn permission_key(kind: webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_PERMISSION_KIND) -> &'static str {
    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    match kind {
        COREWEBVIEW2_PERMISSION_KIND_CAMERA => "camera",
        COREWEBVIEW2_PERMISSION_KIND_MICROPHONE => "microphone",
        COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION => "location",
        COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS => "notifications",
        COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ => "clipboard",
        COREWEBVIEW2_PERMISSION_KIND_OTHER_SENSORS => "sensors",
        COREWEBVIEW2_PERMISSION_KIND_MULTIPLE_AUTOMATIC_DOWNLOADS => "downloads",
        COREWEBVIEW2_PERMISSION_KIND_FILE_READ_WRITE => "files",
        COREWEBVIEW2_PERMISSION_KIND_LOCAL_FONTS => "fonts",
        COREWEBVIEW2_PERMISSION_KIND_MIDI_SYSTEM_EXCLUSIVE_MESSAGES => "midi",
        COREWEBVIEW2_PERMISSION_KIND_WINDOW_MANAGEMENT => "windows",
        COREWEBVIEW2_PERMISSION_KIND_AUTOPLAY => "autoplay",
        _ => "other",
    }
}

/// Answer a held request. Saves the decision when asked to and the tab is not private.
#[cfg(windows)]
fn answer_permission(id: u64, decision: permissions::Decision, remember: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{COREWEBVIEW2_PERMISSION_STATE_ALLOW, COREWEBVIEW2_PERMISSION_STATE_DENY};
    let Some(p) = PERM_PENDING.with(|q| {
        let mut q = q.borrow_mut();
        q.iter().position(|p| p.id == id).map(|i| q.remove(i))
    }) else {
        return;
    };
    unsafe {
        let _ = p.args.SetState(if decision == permissions::Decision::Allow { COREWEBVIEW2_PERMISSION_STATE_ALLOW } else { COREWEBVIEW2_PERMISSION_STATE_DENY });
        let _ = p.deferral.Complete();
    }
    if remember && !p.private && p.kind != "other" {
        if let Ok(mut st) = permissions::global().lock() {
            st.set(&p.origin, p.kind, decision);
            permissions::save(&st);
        }
    }
}
#[cfg(not(windows))]
fn answer_permission(_id: u64, _decision: permissions::Decision, _remember: bool) {}

/// Refuse requests whose page is gone: tab closed, or navigated to another site.
#[cfg(windows)]
fn sweep_permissions(tabs: &[BrowserTab], extra_live: &[usize]) {
    let stale: Vec<u64> = PERM_PENDING.with(|q| {
        q.borrow()
            .iter()
            .filter(|p| match tabs.iter().find(|t| t.id == p.tab_id) {
                Some(t) => t.webview.url().ok().and_then(|u| passwords::origin_of(&u)).as_deref() != Some(p.origin.as_str()),
                None => {
                    !extra_live.contains(&p.tab_id)
                        && peek_id() != Some(p.tab_id)
                        && !OTHER_TAB_IDS.with(|o| o.borrow().contains(&p.tab_id))
                }
            })
            .map(|p| p.id)
            .collect()
    });
    for id in stale {
        answer_permission(id, permissions::Decision::Block, false);
    }
}
#[cfg(not(windows))]
fn sweep_permissions(_tabs: &[BrowserTab], _extra_live: &[usize]) {}

/// The oldest request waiting from one web view.
#[cfg(windows)]
fn pending_prompt(tab_id: usize) -> Option<permissions::PromptView> {
    PERM_PENDING.with(|q| {
        q.borrow().iter().find(|p| p.tab_id == tab_id).map(|p| {
            let (label, icon) = permissions::label(p.kind);
            permissions::PromptView {
                id: p.id,
                origin: p.origin.clone(),
                host: url::Url::parse(&p.origin).ok().and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string())).unwrap_or_default(),
                kind: p.kind.to_string(),
                label,
                icon,
                private: p.private,
            }
        })
    })
}
#[cfg(not(windows))]
fn pending_prompt(_tab_id: usize) -> Option<permissions::PromptView> {
    None
}

/// The request to show in the main toolbar: from an open Peek (it sits in front), else the visible tab.
fn permission_prompt(active_tab_id: Option<usize>) -> Option<permissions::PromptView> {
    peek_id().or(active_tab_id).and_then(pending_prompt)
}

thread_local! {
    /// A group just made from the tab menu: the toolbar opens its name field once.
    static NEW_GROUP_TO_NAME: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
}
/// "Shunno Browser" for the default profile, "Shunno Browser · Work" for others.
fn window_title_for_profile(list: &[state::Profile]) -> String {
    let me = state::profile_id();
    match list.iter().find(|p| p.id == me) {
        Some(p) if me != state::DEFAULT_PROFILE => format!("Shunno Browser \u{00b7} {}", p.name),
        _ => "Shunno Browser".to_string(),
    }
}

// ---------------------------------------------------------------- Extensions (WebView2 profile)
thread_local! {
    static EXTENSIONS: std::cell::RefCell<Vec<extensions::Entry>> = const { std::cell::RefCell::new(Vec::new()) };
    static EXT_NOTICE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

#[cfg(windows)]
fn ext_profile(wv: &WebView) -> Option<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Profile7> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2Profile7, ICoreWebView2_13};
    use windows::core::Interface;
    use wry::WebViewExtWindows;
    unsafe {
        let core = wv.controller().CoreWebView2().ok()?;
        let c13: ICoreWebView2_13 = core.cast().ok()?;
        c13.Profile().ok()?.cast::<ICoreWebView2Profile7>().ok()
    }
}

const EXT_UNSUPPORTED: &str = "Extensions need a newer Microsoft Edge WebView2 Runtime. Update it from Windows Update or microsoft.com/edge/webview2, then restart Shunno.";

/// Ask WebView2 for the installed extensions; the answer arrives as ExtensionsListed.
#[cfg(windows)]
fn ext_refresh(wv: &WebView, proxy: EventLoopProxy<AppEvent>) {
    use webview2_com::ProfileGetBrowserExtensionsCompletedHandler;
    let Some(prof) = ext_profile(wv) else {
        let _ = proxy.send_event(AppEvent::ExtensionsListed(Vec::new()));
        return;
    };
    let p2 = proxy.clone();
    let h = ProfileGetBrowserExtensionsCompletedHandler::create(Box::new(move |res, list| {
        let mut out = Vec::new();
        if let (Ok(()), Some(list)) = (res, list) {
            unsafe {
                let mut n = 0u32;
                let _ = list.Count(&mut n);
                for i in 0..n {
                    if let Ok(e) = list.GetValueAtIndex(i) {
                        let mut id = windows::core::PWSTR::null();
                        let mut name = windows::core::PWSTR::null();
                        let mut on = windows::Win32::Foundation::BOOL::default();
                        let _ = e.Id(&mut id);
                        let _ = e.Name(&mut name);
                        let _ = e.IsEnabled(&mut on);
                        out.push((webview2_com::take_pwstr(id), webview2_com::take_pwstr(name), on.as_bool()));
                    }
                }
            }
        }
        let _ = p2.send_event(AppEvent::ExtensionsListed(out));
        Ok(())
    }));
    unsafe {
        let _ = prof.GetBrowserExtensions(&h);
    }
}
#[cfg(not(windows))]
fn ext_refresh(_wv: &WebView, proxy: EventLoopProxy<AppEvent>) {
    let _ = proxy.send_event(AppEvent::ExtensionsListed(Vec::new()));
}

/// Install Shunno's copy at `dir`. On failure the copy is deleted.
#[cfg(windows)]
fn ext_install(wv: &WebView, proxy: EventLoopProxy<AppEvent>, dir: std::path::PathBuf, meta: extensions::Meta) {
    use webview2_com::ProfileAddBrowserExtensionCompletedHandler;
    let Some(prof) = ext_profile(wv) else {
        extensions::delete_copy(&dir.to_string_lossy());
        let _ = proxy.send_event(AppEvent::ExtensionNotice(EXT_UNSUPPORTED.into()));
        return;
    };
    let p2 = proxy.clone();
    let d2 = dir.clone();
    let h = ProfileAddBrowserExtensionCompletedHandler::create(Box::new(move |res, ext| {
        match (res, ext) {
            (Ok(()), Some(e)) => {
                let mut id = windows::core::PWSTR::null();
                unsafe {
                    let _ = e.Id(&mut id);
                }
                let id = webview2_com::take_pwstr(id);
                let mut reg = extensions::load_registry();
                let name = meta.name.clone();
                let new_path = meta.path.clone();
                // Re-adding an installed extension: the previous copy is no longer used.
                if let Some(old) = reg.insert(id, meta) {
                    if old.path != new_path {
                        extensions::delete_copy(&old.path);
                    }
                }
                extensions::save_registry(&reg);
                let _ = p2.send_event(AppEvent::ExtensionNotice(format!("Added {name}.")));
            }
            (r, _) => {
                extensions::delete_copy(&d2.to_string_lossy());
                let why = r.err().map(|e| e.message().to_string()).filter(|m| !m.is_empty()).unwrap_or_else(|| "unknown error".into());
                let _ = p2.send_event(AppEvent::ExtensionNotice(format!("WebView2 could not install it: {why}")));
            }
        }
        let _ = p2.send_event(AppEvent::ChromeIpc("EXT_REFRESH".into()));
        Ok(())
    }));
    let path = windows::core::HSTRING::from(dir.as_os_str());
    unsafe {
        if prof.AddBrowserExtension(&path, &h).is_err() {
            extensions::delete_copy(&dir.to_string_lossy());
            let _ = proxy.send_event(AppEvent::ExtensionNotice(EXT_UNSUPPORTED.into()));
        }
    }
}
#[cfg(not(windows))]
fn ext_install(_wv: &WebView, _proxy: EventLoopProxy<AppEvent>, _dir: std::path::PathBuf, _meta: extensions::Meta) {}

/// Enable, disable or remove one installed extension by id.
#[cfg(windows)]
fn ext_change(wv: &WebView, proxy: EventLoopProxy<AppEvent>, id: String, action: &'static str) {
    use webview2_com::{BrowserExtensionEnableCompletedHandler, BrowserExtensionRemoveCompletedHandler, ProfileGetBrowserExtensionsCompletedHandler};
    let Some(prof) = ext_profile(wv) else { return };
    let p2 = proxy.clone();
    let h = ProfileGetBrowserExtensionsCompletedHandler::create(Box::new(move |res, list| {
        let (Ok(()), Some(list)) = (res, list) else { return Ok(()) };
        unsafe {
            let mut n = 0u32;
            let _ = list.Count(&mut n);
            for i in 0..n {
                let Ok(e) = list.GetValueAtIndex(i) else { continue };
                let mut eid = windows::core::PWSTR::null();
                let _ = e.Id(&mut eid);
                if webview2_com::take_pwstr(eid) != id {
                    continue;
                }
                let p3 = p2.clone();
                match action {
                    "remove" => {
                        let id2 = id.clone();
                        let done = BrowserExtensionRemoveCompletedHandler::create(Box::new(move |r| {
                            if r.is_ok() {
                                let mut reg = extensions::load_registry();
                                if let Some(m) = reg.remove(&id2) {
                                    extensions::delete_copy(&m.path);
                                    let _ = p3.send_event(AppEvent::ExtensionNotice(format!("Removed {}.", m.name)));
                                }
                                extensions::save_registry(&reg);
                            }
                            let _ = p3.send_event(AppEvent::ChromeIpc("EXT_REFRESH".into()));
                            Ok(())
                        }));
                        let _ = e.Remove(&done);
                    }
                    on => {
                        let done = BrowserExtensionEnableCompletedHandler::create(Box::new(move |_r| {
                            let _ = p3.send_event(AppEvent::ChromeIpc("EXT_REFRESH".into()));
                            Ok(())
                        }));
                        let _ = e.Enable(windows::Win32::Foundation::BOOL::from(on == "enable"), &done);
                    }
                }
                break;
            }
        }
        Ok(())
    }));
    unsafe {
        let _ = prof.GetBrowserExtensions(&h);
    }
}
#[cfg(not(windows))]
fn ext_change(_wv: &WebView, _proxy: EventLoopProxy<AppEvent>, _id: String, _action: &'static str) {}

fn chrome_bar_height() -> f64 {
    f64::from_bits(CHROME_BAR_H_BITS.load(std::sync::atomic::Ordering::Relaxed))
}
const MOBILE_UA: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36";
const CONTENT_GUARD_JS: &str = include_str!("../assets/js/content_guard.min.js");

enum AppEvent {
    /// A site asked for a permission (id in PERM_PENDING).
    PermissionRequest(u64),
    /// Another launch of Shunno handed over a link ("" = just bring the window forward).
    ExternalOpen(String),
    /// Open (or focus) Little Shunno, optionally at a URL.
    LittleOpen(Option<String>),
    /// Message from Little Shunno's toolbar.
    LittleIpc(String),
    /// A page started a download in WebView2; Shunno may take it over (see probe_takeover).
    TakeoverCandidate { native_id: u64, tab_id: usize, url: String, file_name: String, total: Option<u64> },
    /// Cookies collected for a takeover candidate; probe next.
    TakeoverCookies { native_id: u64, url: String, referer: Option<String>, cookie: Option<String>, file_name: String, total: Option<u64> },
    /// Probe answered: Some(total) means Shunno's engine can fetch it with the same result.
    TakeoverDecision { native_id: u64, url: String, referer: Option<String>, cookie: Option<String>, file_name: String, ok: bool },
    /// A browser shortcut pressed while a page had focus (see page_shortcut_id).
    PageShortcut(&'static str),
    /// Windows switched between light and dark app mode.
    SystemThemeChanged,
    /// Windows Hello answer for a Passwords panel action: (action, credential id, verified, hello unavailable).
    PwAuthorized(&'static str, u64, bool, bool),
    /// Right-click → Search <engine> for the selected text (source tab id, raw selection).
    SearchSelection(usize, String),
    /// Right-click → Hide Element on Page (tab id): start the element picker at the clicked element.
    StartPicker(usize),
    /// A tab shown under the previous one has had time to draw: put it on top.
    RaiseTab(usize),
    ImportDone(Result<(String, Vec<BookmarkEntry>, Vec<HistoryEntry>), String>),
    ChromeIpc(String),
    /// Toolbar message from a specific browser window (its key); handled as ChromeIpc there.
    ChromeIpcW(u64, String),
    /// WebView2's installed extensions: (id, name, enabled).
    ExtensionsListed(Vec<(String, String, bool)>),
    /// A line for the Extensions panel (added, removed, or why it failed).
    ExtensionNotice(String),
    /// A web store extension was downloaded and unpacked: install it (UI thread).
    ExtStoreReady(Result<(std::path::PathBuf, extensions::Meta), String>),
    /// Session restore: open another window with these tabs (url, group).
    RestoreWindow(Vec<(String, Option<u32>)>),
    ContentIpc(usize, String),
    /// "Download ... with Shunno" from a page's context menu: (tab id, url).
    DownloadIntercepted(usize, String),
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
    /// HTTPS-Only: load this (upgraded) URL in the tab.
    NavigateTab(usize, String),
    /// HTTPS-Only: show the "not secure" warning for this http URL.
    HttpsInterstitial(usize, String),
    /// A top-level navigation finished: (tab, success, cancelled).
    NavigationDone(usize, bool, bool),
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
    /// The page's icon changed: a PNG data URL, or "" when the page has none.
    TabFavicon(usize, String),
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

/// A browser window that is not the one the event loop is working on right now. The loop works on
/// one window at a time (its locals: `window`, `chrome_wv`, `tabs`, ...); an event for another
/// window swaps that window's state in first.
struct ParkedWindow {
    key: u64,
    // Fields drop in this order: web views before the window that hosts them.
    peek: Option<Peek>,
    tabs: Vec<BrowserTab>,
    chrome_wv: WebView,
    window: Window,
    active_tab_idx: usize,
    split_tab_idx: Option<usize>,
    drawer_open: bool,
    engine_menu_open: bool,
    palette_open: bool,
    is_fullscreen: bool,
    find_current: usize,
    find_total: usize,
    site_data_cleared: bool,
    chrome_last_h: f64,
    peek_rect: [f64; 4],
    tab_flow: TabFlow,
}

/// Which tab an event is about, if any (to route it to that tab's window).
fn event_tab_id(e: &AppEvent) -> Option<usize> {
    match e {
        AppEvent::ContentIpc(t, _)
        | AppEvent::NavigateTab(t, _)
        | AppEvent::HttpsInterstitial(t, _)
        | AppEvent::NewWindowBlockedOrOpened(t, _)
        | AppEvent::NativeStreamCaptured(t, _, _, _)
        | AppEvent::StreamSizeResolved(t, _)
        | AppEvent::TabAudioChanged(t, _)
        | AppEvent::TabFavicon(t, _)
        | AppEvent::TabTitleChanged(t, _)
        | AppEvent::TabUrlChanged(t, _)
        | AppEvent::TabZoomChanged(t, _)
        | AppEvent::SearchSelection(t, _)
        | AppEvent::StartPicker(t)
        | AppEvent::RaiseTab(t)
        | AppEvent::DownloadIntercepted(t, _) => Some(*t),
        AppEvent::NavigationDone(t, _, _) => Some(*t),
        AppEvent::TakeoverCandidate { tab_id, .. } => Some(*tab_id),
        #[cfg(windows)]
        AppEvent::PermissionRequest(id) => PERM_PENDING.with(|q| q.borrow().iter().find(|p| p.id == *id).map(|p| p.tab_id)),
        _ => None,
    }
}

/// The other open windows, for the toolbar's "Move to Window" menu: (key, title).
#[derive(Clone, Serialize)]
struct WindowView {
    key: u64,
    title: String,
}
thread_local! {
    static OTHER_WINDOWS: std::cell::RefCell<Vec<WindowView>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Tab ids living in the other windows (so groups and permission requests there stay alive).
    static OTHER_TAB_IDS: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
    static WINDOW_KEY: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
    /// Saved windows not yet rebuilt at startup.
    static RESTORES_PENDING: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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
    group: Option<u32>,
    /// PNG data URL of the site's icon; empty when there is none (the strip draws a letter tile).
    favicon: String,
}

#[derive(Serialize)]
struct ChromeSnapshot {
    version: &'static str,
    ffmpeg_available: bool,
    ffmpeg_status: String,
    ytdlp_status: String,
    adblock_status: String,
    filters: blocker::FiltersView,
    /// "", "uptodate", "available", "ready", "failed"
    update_kind: String,
    update_label: String,
    is_fullscreen: bool,
    is_maximized: bool,
    import_sources: Vec<importer::ImportSource>,
    menu_note: String,
    peek: Option<PeekView>,
    pw_offer: Option<passwords::Offer>,
    passwords: Vec<passwords::CredentialView>,
    pw_status: String,
    pw_csv_pending: bool,
    force_dark_active: bool,
    snap_native: bool,
    perm_prompt: Option<permissions::PromptView>,
    tab_groups: Vec<state::TabGroup>,
    window_key: u64,
    extensions: Vec<extensions::Entry>,
    ext_notice: Option<String>,
    other_windows: Vec<WindowView>,
    name_group: Option<u32>,
    permissions: Vec<permissions::Entry>,
    active_reader_available: bool,
    import_status: String,
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

/// Open a file the way double-clicking it in Explorer does.
#[cfg(windows)]
fn open_with_default_app(path: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let dir = std::path::Path::new(path).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
    unsafe {
        ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from(path), None, &HSTRING::from(dir), SW_SHOWNORMAL);
    }
}
#[cfg(not(windows))]
fn open_with_default_app(_path: &str) {}

/// Save history so that the newest snapshot always wins: background saves can finish out of order,
/// and an older one landing after "Clear history" would restore what the user deleted.
fn save_history_ordered(rt: Option<&tokio::runtime::Handle>, snapshot: Vec<HistoryEntry>) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    static WRITTEN: std::sync::Mutex<u64> = std::sync::Mutex::new(0);
    let gen = NEXT.fetch_add(1, Ordering::SeqCst);
    let write = move || {
        let mut last = WRITTEN.lock().unwrap_or_else(|e| e.into_inner());
        if gen > *last {
            BrowserStorage::save_history(&snapshot);
            *last = gen;
        }
    };
    match rt {
        Some(rt) => {
            rt.spawn_blocking(write);
        }
        None => write(),
    }
}

fn generate_ipc_token() -> String {
    let mut rng = rand::thread_rng();
    let a: u64 = rng.gen();
    let b: u64 = rng.gen();
    format!("{:016x}{:016x}", a, b)
}

/// Per-launch secret mixed into fingerprint farbling: values are stable per site within a session
/// and change on every restart.
fn fp_session_key() -> &'static str {
    static KEY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    KEY.get_or_init(|| format!("{:016x}", rand::random::<u64>()))
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

thread_local! {
    /// Each tab's current main-frame navigation target (fragment removed), from NavigationStarting.
    static MAIN_NAV: std::cell::RefCell<HashMap<usize, String>> = std::cell::RefCell::new(HashMap::new());
    /// Each tab's network policy for the site it is on: (ad blocking, images blocked). Set when the
    /// tab is made and again at every top-level navigation, so it follows the tab across sites.
    static TAB_NET_POLICY: std::cell::RefCell<HashMap<usize, (bool, bool)>> = std::cell::RefCell::new(HashMap::new());
}
/// The engine's own desktop User-Agent (read from the toolbar web view at start), restored when a
/// tab leaves a site set to "mobile site".
static DESKTOP_UA: std::sync::OnceLock<String> = std::sync::OnceLock::new();

// Live WebView2 download operations, keyed by Shunno task id. WebView2 raises all of its
// events on the UI thread, which is also the event loop thread, so a thread_local is sufficient.
#[cfg(windows)]
thread_local! {
    static NATIVE_OPS: std::cell::RefCell<HashMap<u64, webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2DownloadOperation>> =
        std::cell::RefCell::new(HashMap::new());
}

#[cfg(windows)]
thread_local! {
    /// Destinations (lower case) promised to WebView2 downloads still running.
    static NATIVE_DESTS: std::cell::RefCell<std::collections::HashSet<String>> = std::cell::RefCell::new(std::collections::HashSet::new());
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
    let _ = policy; // site flags now come from the per-host table, looked up in the page
    let default_flags = serde_json::to_string(&state::page_flags(&SitePolicy::default())).unwrap_or_else(|_| "[true,true,true,false,true]".into());
    let mut script = CONTENT_GUARD_JS
        .replace("__SHUNNO_IPC_TOKEN__", ipc_token)
        .replace("__SHUNNO_POLICY_TABLE__", &state::page_policy_table_json())
        .replace("__SHUNNO_POLICY_DEFAULT__", &default_flags)
        .replace("__SHUNNO_BG_MEDIA__", if global.background_media_play { "true" } else { "false" })
        .replace("__SHUNNO_FULL_DARK__", if global.full_dark_mode { "true" } else { "false" })
        .replace("__SHUNNO_PEEK__", if global.peek_links { "true" } else { "false" })
        .replace("__SHUNNO_FP_SESSION__", fp_session_key());

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

            if w == target_size && h == target_size && img_offset + 40 + (w * h * 4) as usize <= ICO_BYTES.len() && bytes_in_res >= 40
                // Uncompressed 32-bit DIB only (a PNG-compressed entry would decode as noise).
                && u16::from_le_bytes([ICO_BYTES[img_offset + 14], ICO_BYTES[img_offset + 15]]) == 32
            {
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

/// Replaces the built in error page when there is no connection (no Edge logo, no game).
#[cfg(windows)]
const OFFLINE_PAGE_JS: &str = include_str!("../assets/js/offline_page.js");

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

/// Windows in full screen, with the placement to restore (also tells WM_NCCALCSIZE not to inset).
#[cfg(windows)]
static FULLSCREEN_WP: std::sync::Mutex<Vec<(isize, windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT)>> = std::sync::Mutex::new(Vec::new());
#[cfg(windows)]
fn is_fullscreen_hwnd(h: isize) -> bool {
    FULLSCREEN_WP.lock().map(|g| g.iter().any(|(x, _)| *x == h)).unwrap_or(false)
}

#[cfg(windows)]
fn apply_native_fullscreen(window: &Window, enable: bool) {
    use tao::platform::windows::WindowExtWindows;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowPlacement, SetWindowPlacement, SetWindowPos, HWND_TOP, SWP_FRAMECHANGED,
        SWP_NOOWNERZORDER, WINDOWPLACEMENT,
    };

    let hwnd = HWND(window.hwnd() as isize);

    unsafe {
        if enable {
            let mut wp = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            // Per window: several windows can be full screen and each restores its own placement.
            // fullscreen_motion() may have saved it already, before the window grew.
            let have = is_fullscreen_hwnd(hwnd.0);
            if !have && GetWindowPlacement(hwnd, &mut wp).is_ok() {
                let mut g = FULLSCREEN_WP.lock().unwrap_or_else(|e| e.into_inner());
                g.push((hwnd.0, wp));
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
            let saved = {
                let mut g = FULLSCREEN_WP.lock().unwrap_or_else(|e| e.into_inner());
                let at = g.iter().position(|(h, _)| *h == hwnd.0);
                at.map(|i| g.remove(i).1)
            };
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

/// The window's on-screen rectangle now and where full screen (or leaving it) puts it, for the
/// transition. Entering saves the placement to restore. None when Windows animations are off
/// (Settings > Accessibility > Animation effects) or there is nothing to animate.
#[cfg(windows)]
fn fullscreen_motion(window: &Window, enable: bool) -> Option<(windows::Win32::Foundation::RECT, windows::Win32::Foundation::RECT)> {
    use tao::platform::windows::WindowExtWindows;
    use windows::Win32::Foundation::{BOOL, HWND, RECT};
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowPlacement, GetWindowRect, SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SW_SHOWMAXIMIZED,
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WINDOWPLACEMENT,
    };
    let hwnd = HWND(window.hwnd() as isize);
    unsafe {
        let mut wp = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
        if enable && !is_fullscreen_hwnd(hwnd.0) && GetWindowPlacement(hwnd, &mut wp).is_ok() {
            FULLSCREEN_WP.lock().unwrap_or_else(|e| e.into_inner()).push((hwnd.0, wp));
        }
        let mut anim = BOOL(1);
        let _ = SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some(&mut anim as *mut BOOL as *mut core::ffi::c_void), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
        if !anim.as_bool() {
            return None;
        }
        let mut from = RECT::default();
        GetWindowRect(hwnd, &mut from).ok()?;
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut mi).as_bool() {
            return None;
        }
        let to = if enable {
            mi.rcMonitor
        } else {
            let saved = FULLSCREEN_WP.lock().ok()?.iter().find(|(h, _)| *h == hwnd.0).map(|(_, w)| *w)?;
            if saved.showCmd == SW_SHOWMAXIMIZED.0 as u32 {
                mi.rcWork
            } else {
                // rcNormalPosition is in work-area coordinates.
                let r = saved.rcNormalPosition;
                let (dx, dy) = (mi.rcWork.left - mi.rcMonitor.left, mi.rcWork.top - mi.rcMonitor.top);
                RECT { left: r.left + dx, top: r.top + dy, right: r.right + dx, bottom: r.bottom + dy }
            }
        };
        (from != to && to.right > to.left && to.bottom > to.top).then_some((from, to))
    }
}

/// One frame of the transition: the window at the eased point between `from` and `to`.
/// Curve: fast start, long soft landing, no overshoot (the toolbar's --ease-fluid feel).
#[cfg(windows)]
fn fullscreen_frame(window: &Window, from: windows::Win32::Foundation::RECT, to: windows::Win32::Foundation::RECT, t: f64) {
    use tao::platform::windows::WindowExtWindows;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOOWNERZORDER};
    let e = 1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(4);
    let lerp = |a: i32, b: i32| (a as f64 + (b - a) as f64 * e).round() as i32;
    let (l, tp, r, b) = (lerp(from.left, to.left), lerp(from.top, to.top), lerp(from.right, to.right), lerp(from.bottom, to.bottom));
    unsafe {
        let _ = SetWindowPos(HWND(window.hwnd() as isize), HWND_TOP, l, tp, r - l, b - tp, SWP_NOACTIVATE | SWP_NOOWNERZORDER);
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
                        // Two shots in the same second must not overwrite each other.
                        let Ok(out_path) = downloader::reserve_unique_path(&shots_dir, &format!("shunno_shot_{}.png", now_epoch_secs())) else {
                            return Ok(());
                        };
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

/// Collect the cookies WebView2 would send to `url` (HttpOnly included) and deliver them as one
/// Cookie header through `done`, on the UI thread.
#[cfg(windows)]
fn collect_cookie_header(webview: &WebView, url: &str, done: Box<dyn FnOnce(Option<String>) + 'static>) {
    use webview2_com::GetCookiesCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_2;
    use windows::core::{Interface, PCWSTR};
    use wry::WebViewExtWindows;
    let done = std::rc::Rc::new(std::cell::RefCell::new(Some(done)));
    let fire = {
        let done = done.clone();
        move |v: Option<String>| {
            if let Some(f) = done.borrow_mut().take() {
                f(v);
            }
        }
    };
    unsafe {
        let mgr = webview
            .controller()
            .CoreWebView2()
            .ok()
            .and_then(|c| c.cast::<ICoreWebView2_2>().ok())
            .and_then(|c2| c2.CookieManager().ok());
        let Some(mgr) = mgr else { return fire(None) };
        let wide: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        let fire_cb = fire.clone();
        let handler = GetCookiesCompletedHandler::create(Box::new(move |res, list| {
            let mut pairs = Vec::new();
            if res.is_ok() {
                if let Some(list) = list {
                    let mut n = 0u32;
                    if list.Count(&mut n).is_ok() {
                        for i in 0..n {
                            if let Ok(c) = list.GetValueAtIndex(i) {
                                let (mut np, mut vp) = (windows::core::PWSTR::null(), windows::core::PWSTR::null());
                                if c.Name(&mut np).is_ok() && c.Value(&mut vp).is_ok() {
                                    let (name, val) = (webview2_com::take_pwstr(np), webview2_com::take_pwstr(vp));
                                    if !name.is_empty() {
                                        pairs.push(format!("{name}={val}"));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            fire_cb(if pairs.is_empty() { None } else { Some(pairs.join("; ")) });
            Ok(())
        }));
        if mgr.GetCookies(PCWSTR::from_raw(wide.as_ptr()), &handler).is_err() {
            fire(None);
        }
    }
}
#[cfg(not(windows))]
fn collect_cookie_header(_webview: &WebView, _url: &str, done: Box<dyn FnOnce(Option<String>) + 'static>) {
    done(None);
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
                        let cookie_uri = downloader::split_route_prefix(&url).1;
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


async fn check_for_updates() {
    let client = reqwest::Client::builder()
        .user_agent(format!("ShunnoBrowser/{}", APP_VERSION))
        .connect_timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_default();
    updater::global().check(&client).await;
}

fn update_kind_label() -> (&'static str, String) {
    match updater::global().state() {
        updater::UpdateState::Idle => ("", String::new()),
        updater::UpdateState::UpToDate => ("uptodate", "Up to date".to_string()),
        updater::UpdateState::Available { version, reason } => ("available", format!("v{} available ({})", version, reason)),
        updater::UpdateState::Ready { version } => ("ready", format!("v{} installed, restart to use it", version)),
        updater::UpdateState::Failed(e) => ("failed", format!("Update check failed: {}", e)),
    }
}

/// Downloads and compiles the filter lists (all enabled lists, or only those not on disk yet); a
/// list that fails keeps its previous text and the status says which failed.
async fn refresh_filter_lists(only_missing: bool) {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_default();
    let _ = blocker::global().refresh(&client, only_missing).await;
}

/// After a change to the chosen lists or to "My filters": download any list not on disk yet,
/// then recompile. Pages keep the old engine until the new one is ready.
fn apply_filter_change(rt: &tokio::runtime::Handle, proxy: &EventLoopProxy<AppEvent>) {
    blocker::global().mark_busy();
    let _ = proxy.send_event(AppEvent::RefreshChrome);
    let px = proxy.clone();
    rt.spawn(async move {
        refresh_filter_lists(true).await;
        let _ = px.send_event(AppEvent::RefreshChrome);
    });
}

/// Native Open or Save dialog for a filter text file ("My filters" import and export).
#[cfg(windows)]
fn filter_file_dialog(hwnd: isize, save: bool) -> Option<std::path::PathBuf> {
    use windows::core::{w, Interface};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{FileOpenDialog, FileSaveDialog, IFileDialog, IFileOpenDialog, IFileSaveDialog, SIGDN_FILESYSPATH};
    unsafe {
        let dlg: IFileDialog = if save {
            let d: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
            d.cast().ok()?
        } else {
            let d: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
            d.cast().ok()?
        };
        let filters = [
            COMDLG_FILTERSPEC { pszName: w!("Filter rules (*.txt)"), pszSpec: w!("*.txt") },
            COMDLG_FILTERSPEC { pszName: w!("All files"), pszSpec: w!("*.*") },
        ];
        let _ = dlg.SetFileTypes(&filters);
        if save {
            let _ = dlg.SetTitle(w!("Export My Filters"));
            let _ = dlg.SetFileName(w!("shunno-my-filters.txt"));
            let _ = dlg.SetDefaultExtension(w!("txt"));
        } else {
            let _ = dlg.SetTitle(w!("Import Filter Rules"));
        }
        dlg.Show(HWND(hwnd)).ok()?;
        let item = dlg.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let out = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
        out.map(std::path::PathBuf::from)
    }
}
#[cfg(not(windows))]
fn filter_file_dialog(_hwnd: isize, _save: bool) -> Option<std::path::PathBuf> {
    None
}

/// Replaces the tab's document with locally generated HTML (warning pages).
#[cfg(windows)]
fn show_html_in_tab(webview: &WebView, html: &str) {
    use windows::core::HSTRING;
    use wry::WebViewExtWindows;
    unsafe {
        if let Ok(core) = webview.controller().CoreWebView2() {
            let _ = core.NavigateToString(&HSTRING::from(html));
        }
    }
}

#[cfg(not(windows))]
fn show_html_in_tab(_webview: &WebView, _html: &str) {}

/// Per-site engine settings that must be real, not cosmetic: JavaScript on/off.
#[cfg(windows)]
fn apply_site_settings(webview: &WebView, policy: &SitePolicy) {
    use wry::WebViewExtWindows;
    unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else { return };
        if let Ok(settings) = core.Settings() {
            // Page scripts only; Shunno's injected scripts keep running.
            let _ = settings.SetIsScriptEnabled(policy.js_enabled);
        }
    }
}

#[cfg(not(windows))]
fn apply_site_settings(_webview: &WebView, _policy: &SitePolicy) {}

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
                // Passwords are handled by the Shunno vault (src/passwords.rs); WebView2's own saver
                // would double-prompt and keep a second, unmanageable store.
                let _ = s4.SetIsPasswordAutosaveEnabled(false);
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
/// Browser shortcut pressed inside a page, as the id the toolbar's shortcut handler understands.
/// Pages get their keystrokes first; WebView2 reports these here so they work whichever web view
/// has focus. (In the toolbar web view itself, its own keydown handler deals with them.)
fn page_shortcut_id(vk: u32, ctrl: bool, shift: bool, alt: bool) -> Option<&'static str> {
    if !ctrl || alt {
        return None;
    }
    Some(match (vk, shift) {
        (0x54, false) => "new-tab",       // Ctrl+T
        (0x54, true) => "reopen-tab",     // Ctrl+Shift+T
        (0x57, false) => "close-tab",     // Ctrl+W
        (0x09, false) => "next-tab",      // Ctrl+Tab
        (0x09, true) => "prev-tab",       // Ctrl+Shift+Tab
        (0x4C, false) => "focus-url",     // Ctrl+L
        (0x4B, false) => "palette",       // Ctrl+K
        (0x4A, false) => "downloads",     // Ctrl+J
        (0x46, false) => "find",          // Ctrl+F
        (0x4E, false) => "new-window",    // Ctrl+N
        (0x44, false) => "bookmark",      // Ctrl+D (Add Bookmark)
        _ => return None,
    })
}

fn attach_f11_accelerator_hook(webview: &WebView, proxy: EventLoopProxy<AppEvent>) {
    attach_accelerator_hook(webview, proxy, false);
}

fn attach_page_accelerator_hook(webview: &WebView, proxy: EventLoopProxy<AppEvent>) {
    attach_accelerator_hook(webview, proxy, true);
}

fn attach_accelerator_hook(webview: &WebView, proxy: EventLoopProxy<AppEvent>, page_shortcuts: bool) {
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
                    let down = kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN || kind == COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN;
                    if vk == 0x7A && down {
                        let _ = args.SetHandled(true);
                        let _ = proxy.send_event(AppEvent::ToggleFullscreen);
                    } else if page_shortcuts && vk == 0x78 && kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN {
                        // F9: Reader (as in Edge).
                        let _ = args.SetHandled(true);
                        let _ = proxy.send_event(AppEvent::PageShortcut("reader"));
                    } else if page_shortcuts && kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN {
                        use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
                        let pressed = |k: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| GetKeyState(k.0 as i32) < 0;
                        if let Some(id) = page_shortcut_id(vk, pressed(VK_CONTROL), pressed(VK_SHIFT), pressed(VK_MENU)) {
                            // Ignore auto-repeat so holding Ctrl+T opens one tab.
                            let mut status = Default::default();
                            let repeat = args.PhysicalKeyStatus(&mut status).is_ok() && status.WasKeyDown.as_bool();
                            let _ = args.SetHandled(true);
                            if !repeat {
                                let _ = proxy.send_event(AppEvent::PageShortcut(id));
                            }
                        }
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
#[cfg(not(windows))]
fn attach_page_accelerator_hook(_webview: &WebView, _proxy: EventLoopProxy<AppEvent>) {}

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

/// Reads up to `cap` + 1 bytes from a COM stream (a longer stream comes back longer than `cap`).
#[cfg(windows)]
fn read_stream_capped(stream: &windows::Win32::System::Com::IStream, cap: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let mut n = 0u32;
        let hr = unsafe { stream.Read(buf.as_mut_ptr() as *mut core::ffi::c_void, buf.len() as u32, Some(&mut n)) };
        if hr.is_err() || n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n as usize]);
        if out.len() > cap {
            break;
        }
    }
    out
}

#[cfg(windows)]
fn attach_native_webview2_hooks(
    webview: &WebView,
    tab_id: usize,
    adblock_enabled: bool,
    block_images: bool,
    proxy: EventLoopProxy<AppEvent>,
) {
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE;
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

        // 0a. Navigation outcome (HTTPS-Only needs to know when an upgraded load failed)
        use webview2_com::NavigationCompletedEventHandler;
        use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED;
        let mut nav_done_token = Default::default();
        let nav_done_proxy = proxy.clone();
        let nav_done_handler = NavigationCompletedEventHandler::create(Box::new(move |_sender, args| {
            if let Some(args) = args {
                let mut ok = BOOL::default();
                let _ = args.IsSuccess(&mut ok);
                let mut status = Default::default();
                let _ = args.WebErrorStatus(&mut status);
                let cancelled = status == COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED;
                let _ = nav_done_proxy.send_event(AppEvent::NavigationDone(tab_id, ok.as_bool(), cancelled));
            }
            Ok(())
        }));
        let _ = core.add_NavigationCompleted(&nav_done_handler, &mut nav_done_token);

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

        // 1b. Site icon for the tab strip (ICoreWebView2_15): fetched as PNG whenever it changes.
        {
            use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2_15, COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG};
            use webview2_com::{FaviconChangedEventHandler, GetFaviconCompletedHandler};
            if let Ok(core15) = core.cast::<ICoreWebView2_15>() {
                let mut fav_token = Default::default();
                let fav_proxy = proxy.clone();
                let fav_handler = FaviconChangedEventHandler::create(Box::new(move |sender, _| {
                    let Some(wv) = sender else { return Ok(()) };
                    let Ok(wv15) = wv.cast::<ICoreWebView2_15>() else { return Ok(()) };
                    let px = fav_proxy.clone();
                    let done = GetFaviconCompletedHandler::create(Box::new(move |hr, stream| {
                        let mut url = String::new();
                        if let (true, Some(stream)) = (hr.is_ok(), stream) {
                            let bytes = read_stream_capped(&stream, 64 * 1024);
                            if !bytes.is_empty() && bytes.len() <= 64 * 1024 && bytes.starts_with(b"\x89PNG") {
                                url = format!("data:image/png;base64,{}", extensions::base64(&bytes));
                            }
                        }
                        let _ = px.send_event(AppEvent::TabFavicon(tab_id, url));
                        Ok(())
                    }));
                    let _ = wv15.GetFavicon(COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG, &done);
                    Ok(())
                }));
                let _ = core15.add_FaviconChanged(&fav_handler, &mut fav_token);
            }
        }

        // 2. Native AdBlocker & Stream Sniffer (ICoreWebView2_2)
        let Ok(core2) = core.cast::<ICoreWebView2_2>() else {
            return;
        };
        let Ok(env) = core2.Environment() else {
            return;
        };

        // Only intercept known ad/tracker URL patterns so 99% of page traffic runs at full native Chromium speed
        // Request policy: Brave's adblock engine for every request (all frames and workers), plus
        // image blocking when images are off for this site. One handler re-checks each request.
        TAB_NET_POLICY.with(|m| m.borrow_mut().insert(tab_id, (adblock_enabled, block_images)));
        // Site settings that WebView2 applies per navigation: set them for the site being opened.
        // Changes made inside NavigationStarting apply to that navigation (WebView2 settings docs).
        {
            use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings2;
            use webview2_com::NavigationStartingEventHandler;
            let mut ns_token = Default::default();
            let ns_handler = NavigationStartingEventHandler::create(Box::new(move |sender, args| {
                let (Some(sender), Some(args)) = (sender, args) else { return Ok(()) };
                let mut p = windows::core::PWSTR::null();
                if args.Uri(&mut p).is_err() {
                    return Ok(());
                }
                let uri = webview2_com::take_pwstr(p);
                let pol = state::site_policy_for(&extract_host(&uri));
                TAB_NET_POLICY.with(|m| m.borrow_mut().insert(tab_id, (pol.adblock, !pol.images_enabled)));
                if let Ok(settings) = sender.Settings() {
                    let _ = settings.SetIsScriptEnabled(pol.js_enabled);
                    if let (Some(desktop), Ok(s2)) = (DESKTOP_UA.get(), settings.cast::<ICoreWebView2Settings2>()) {
                        let want = if pol.mobile_ua { MOBILE_UA } else { desktop.as_str() };
                        let _ = s2.SetUserAgent(&windows::core::HSTRING::from(want));
                    }
                }
                Ok(())
            }));
            let _ = core.add_NavigationStarting(&ns_handler, &mut ns_token);
        }
        {
            use webview2_com::Microsoft::Web::WebView2::Win32::{
                ICoreWebView2_22, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH,
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA,
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT,
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_WEBSOCKET,
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST, COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
            };
            // Every request: the tab may move to a site where ad blocking or image blocking applies.
            let filter_ctx = COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL;
            // ICoreWebView2_22 also delivers requests from iframes and workers, where most ads load.
            let added = match core.cast::<ICoreWebView2_22>() {
                Ok(c22) => c22
                    .AddWebResourceRequestedFilterWithRequestSourceKinds(w!("*"), filter_ctx, COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL)
                    .is_ok(),
                Err(_) => false,
            };
            if !added {
                let _ = core.AddWebResourceRequestedFilter(w!("*"), filter_ctx);
            }

            let mut token = Default::default();
            let env_for_ad = env.clone();
            let ad_proxy = proxy.clone();
            let handler = WebResourceRequestedEventHandler::create(Box::new(move |sender, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut uri = String::new();
                if let Ok(req) = args.Request() {
                    let mut p = windows::core::PWSTR::null();
                    if req.Uri(&mut p).is_ok() {
                        uri = webview2_com::take_pwstr(p);
                    }
                }
                let mut ctx = Default::default();
                let _ = args.ResourceContext(&mut ctx);
                let (adblock_enabled, block_images) = TAB_NET_POLICY.with(|m| m.borrow().get(&tab_id).copied()).unwrap_or((true, false));
                let is_blocked_image = block_images && ctx == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE;

                let mut is_ad = false;
                if adblock_enabled && !is_blocked_image && uri.starts_with("http") {
                    let mut page = String::new();
                    if let Some(wv) = sender.as_ref() {
                        let mut p = windows::core::PWSTR::null();
                        if wv.Source(&mut p).is_ok() {
                            page = webview2_com::take_pwstr(p);
                        }
                    }
                    let req_type = match ctx {
                        c if c == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT => "subdocument",
                        c if c == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT => "script",
                        c if c == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE => "image",
                        c if c == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET => "stylesheet",
                        c if c == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT => "font",
                        c if c == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA => "media",
                        c if c == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST
                            || c == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH => "xmlhttprequest",
                        c if c == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_WEBSOCKET => "websocket",
                        c if c == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING => "ping",
                        _ => "other",
                    };
                    // Never block the page itself (a main-frame document); Brave shows an
                    // interstitial there, a blank tab would just look broken.
                    // Source still names the previous page while the next one is requested, so the
                    // main-frame target recorded at NavigationStarting is checked too.
                    let is_main_document = ctx == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT
                        && (page.is_empty() || page == uri || MAIN_NAV.with(|m| m.borrow().get(&tab_id).is_some_and(|n| *n == uri)));
                    if !is_main_document {
                        let source = if page.starts_with("http") { page.as_str() } else { uri.as_str() };
                        is_ad = blocker::global().should_block(&uri, source, req_type);
                    }
                }
                if !is_ad && !is_blocked_image {
                    return Ok(()); // no response set: the request continues normally
                }
                let empty_resp: ICoreWebView2WebResourceResponse = env_for_ad.CreateWebResourceResponse(
                    None::<&IStream>,
                    403,
                    w!("Blocked by Shunno"),
                    PCWSTR::null(),
                )?;
                let _ = args.SetResponse(&empty_resp);
                if is_ad {
                    let _ = ad_proxy.send_event(AppEvent::AdBlocked);
                }
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


        // 2b. Permissions: hold every request (deferral) and let Shunno answer from saved decisions
        //     or by asking in the toolbar. WebView2 must not keep its own copy of the decision.
        {
            use webview2_com::Microsoft::Web::WebView2::Win32::{
                ICoreWebView2PermissionRequestedEventArgs3, COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_STATE_DENY,
            };
            use webview2_com::PermissionRequestedEventHandler;
            let perm_proxy = proxy.clone();
            let perm_handler = PermissionRequestedEventHandler::create(Box::new(move |_sender, args| {
                let Some(args) = args else { return Ok(()) };
                let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                let _ = args.PermissionKind(&mut kind);
                let mut uri_ptr = windows::core::PWSTR::null();
                let uri = if args.Uri(&mut uri_ptr).is_ok() { webview2_com::take_pwstr(uri_ptr) } else { String::new() };
                if let Ok(a3) = args.cast::<ICoreWebView2PermissionRequestedEventArgs3>() {
                    let _ = a3.SetSavesInProfile(false);
                }
                let Some(origin) = passwords::origin_of(&uri) else {
                    let _ = args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY);
                    return Ok(());
                };
                let Ok(deferral) = args.GetDeferral() else { return Ok(()) };
                let id = PERM_NEXT_ID.fetch_add(1, Ordering::Relaxed);
                PERM_PENDING.with(|q| {
                    q.borrow_mut().push(PendingPermission { id, tab_id, origin, kind: permission_key(kind), private: false, args: args.clone(), deferral })
                });
                let _ = perm_proxy.send_event(AppEvent::PermissionRequest(id));
                Ok(())
            }));
            let mut perm_token = Default::default();
            let _ = core.add_PermissionRequested(&perm_handler, &mut perm_token);
        }

        // 2c. No connection: WebView2's own error page carries the Edge logo. Replace it, once the
        //     navigation has failed for a network reason, with a plain page. Certificate and other
        //     errors keep the built in page (it is the warning for those).
        {
            use webview2_com::Microsoft::Web::WebView2::Win32::*;
            use webview2_com::{ExecuteScriptCompletedHandler, NavigationCompletedEventHandler};
            let nav_handler = NavigationCompletedEventHandler::create(Box::new(move |sender, args| {
                let (Some(sender), Some(args)) = (sender, args) else { return Ok(()) };
                let mut ok = windows::Win32::Foundation::BOOL(1);
                let mut status = COREWEBVIEW2_WEB_ERROR_STATUS::default();
                let _ = args.IsSuccess(&mut ok);
                if ok.as_bool() || args.WebErrorStatus(&mut status).is_err() {
                    return Ok(());
                }
                let offline = status == COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED
                    || status == COREWEBVIEW2_WEB_ERROR_STATUS_DISCONNECTED
                    || status == COREWEBVIEW2_WEB_ERROR_STATUS_CANNOT_CONNECT
                    || status == COREWEBVIEW2_WEB_ERROR_STATUS_SERVER_UNREACHABLE
                    || status == COREWEBVIEW2_WEB_ERROR_STATUS_TIMEOUT
                    || status == COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_RESET;
                if offline {
                    let js = windows::core::HSTRING::from(OFFLINE_PAGE_JS);
                    let done = ExecuteScriptCompletedHandler::create(Box::new(|_, _| Ok(())));
                    let _ = sender.ExecuteScript(&js, &done);
                }
                Ok(())
            }));
            let mut nav_token = Default::default();
            let _ = core.add_NavigationCompleted(&nav_handler, &mut nav_token);
        }

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
                // WebView2 writes to a temporary file until the end, so a name can be free on disk yet
                // already promised to another running download: both are checked.
                let stem = std::path::Path::new(&safe_name).file_stem().and_then(|s| s.to_str()).unwrap_or("file").to_string();
                let ext = std::path::Path::new(&safe_name).extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
                let mut dest_path = target_dir.join(&safe_name);
                let mut n = 1;
                while dest_path.exists() || NATIVE_DESTS.with(|d| d.borrow().contains(&dest_path.to_string_lossy().to_lowercase())) {
                    dest_path = target_dir.join(format!("{stem} ({n}){ext}"));
                    n += 1;
                }
                let dest_str = dest_path.to_string_lossy().to_string();
                let dest_key = dest_str.to_lowercase();
                NATIVE_DESTS.with(|d| d.borrow_mut().insert(dest_key.clone()));
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

                if uri.starts_with("https://") || uri.starts_with("http://") {
                    let _ = dl_proxy.send_event(AppEvent::TakeoverCandidate {
                        native_id: dl_id,
                        tab_id,
                        url: uri.clone(),
                        file_name: safe_name.clone(),
                        total: u64::try_from(total_bytes).ok().filter(|&t| t > 0),
                    });
                }
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
                                NATIVE_DESTS.with(|d| d.borrow_mut().remove(&dest_key));
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
                                    NATIVE_DESTS.with(|d| d.borrow_mut().remove(&dest_key));
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

                // Selected text: "Search <engine> for “…”" at the top of the menu.
                let mut has_sel = windows::Win32::Foundation::BOOL::default();
                let _ = target.HasSelection(&mut has_sel);
                if has_sel.as_bool() {
                    let mut sel_ptr = windows::core::PWSTR::null();
                    if target.SelectionText(&mut sel_ptr).is_ok() {
                        let query = sanitizer::selection_query(&webview2_com::take_pwstr(sel_ptr));
                        if !query.is_empty() {
                            let engine = search_engine_label().lock().map(|g| g.clone()).unwrap_or_default();
                            let shown: String = if query.chars().count() > 28 {
                                format!("{}\u{2026}", query.chars().take(27).collect::<String>().trim_end())
                            } else {
                                query.clone()
                            };
                            let label = windows::core::HSTRING::from(format!("Search {engine} for \u{201C}{shown}\u{201D}"));
                            if let Ok(search_item) = env9.CreateContextMenuItem(&label, None::<&IStream>, COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND) {
                                let mut s_token = Default::default();
                                let s_proxy = ctx_proxy.clone();
                                let s_handler = CustomItemSelectedEventHandler::create(Box::new(move |_s, _| {
                                    let _ = s_proxy.send_event(AppEvent::SearchSelection(tab_id, query.clone()));
                                    Ok(())
                                }));
                                let _ = search_item.add_CustomItemSelected(&s_handler, &mut s_token);
                                let _ = items.InsertValueAtIndex(insert_pos, &search_item);
                                insert_pos += 1;
                            }
                        }
                    }
                }

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
                                    let _ = dl_sel_proxy.send_event(AppEvent::DownloadIntercepted(tab_id, dl_sel_uri.clone()));
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
                                    let _ = m_proxy.send_event(AppEvent::DownloadIntercepted(tab_id, m_uri.clone()));
                                    Ok(())
                                }));
                                let _ = dl_media_item.add_CustomItemSelected(&m_handler, &mut m_token);
                                let _ = items.InsertValueAtIndex(insert_pos, &dl_media_item);
                            }
                        }
                    }
                }

                // Element picker (uBlock style), last in the menu, starting at the clicked element.
                if let Ok(pick_item) = env9.CreateContextMenuItem(w!("Hide Element on Page\u{2026}"), None::<&IStream>, COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND) {
                    let mut p_token = Default::default();
                    let p_proxy = ctx_proxy.clone();
                    let p_handler = CustomItemSelectedEventHandler::create(Box::new(move |_s, _| {
                        let _ = p_proxy.send_event(AppEvent::StartPicker(tab_id));
                        Ok(())
                    }));
                    let _ = pick_item.add_CustomItemSelected(&p_handler, &mut p_token);
                    let mut count = 0u32;
                    let _ = items.Count(&mut count);
                    let _ = items.InsertValueAtIndex(count, &pick_item);
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
    _block_images: bool,
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
    let user_js = load_approved_user_scripts(&global.approved_script_hashes);
    let init_script = build_content_init_script(policy, global, ipc_token, &user_js);

    let ipc_proxy = proxy.clone();
    let win_proxy = proxy.clone();
    let title_proxy = proxy.clone();
    let nav_proxy = proxy.clone();

    let mut builder = WebViewBuilder::new_as_child(window)
        // Painted in the theme colour until the page draws (no white flash on new tabs in dark mode),
        // and hidden until layout_webviews places it.
        .with_background_color(page_background(global.full_dark_mode))
        .with_visible(false);
    #[cfg(windows)]
    {
        use wry::WebViewBuilderExtWindows;
        builder = builder
            .with_additional_browser_args(state::browser_args(global.force_dark_pages))
            .with_theme(page_theme(global.full_dark_mode));
    }
    if policy.mobile_ua {
        // Real HTTP User-Agent (and navigator.userAgent), so servers send their mobile site.
        builder = builder.with_user_agent(MOBILE_UA);
    }
    let wv = builder
        .with_web_context(web_context)
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, CHROME_BAR_HEIGHT).into(),
            size: LogicalSize::new(size.width.max(1.0), content_h).into(),
        })
        // No URL here: the page script is installed first (below), then the first page is opened.
        // Private tabs get a separate in-memory WebView2 InPrivate session: no shared cookies, storage, cache or passwords.
        .with_incognito(incognito)
        .with_document_title_changed_handler(move |new_title| {
            let _ = title_proxy.send_event(AppEvent::TabTitleChanged(tab_id, new_title));
        })
        .with_navigation_handler(move |nav_url| {
            match https_guard::global().decide(tab_id, &nav_url) {
                https_guard::Decision::Upgrade(secure) => {
                    let _ = nav_proxy.send_event(AppEvent::NavigateTab(tab_id, secure));
                    return false;
                }
                https_guard::Decision::Interstitial(insecure) => {
                    let _ = nav_proxy.send_event(AppEvent::HttpsInterstitial(tab_id, insecure));
                    return false;
                }
                https_guard::Decision::Allow => {}
            }
            let target = nav_url.split('#').next().unwrap_or("").to_string();
            MAIN_NAV.with(|m| m.borrow_mut().insert(tab_id, target));
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
            // Sign-in popups must stay real popups (window.opener). Judged on host and path only:
            // a query string such as "?next=oauth" must not let an ordinary popup through.
            let sign_in = url::Url::parse(&lower).ok().is_some_and(|u| {
                let host = u.host_str().unwrap_or("");
                host == "accounts.google.com" || host.contains("oauth") || u.path().contains("oauth")
            });
            if popup_url.is_empty() || lower.starts_with("about:blank") || lower.starts_with("javascript:") || sign_in {
                return true;
            }
            let _ = win_proxy.send_event(AppEvent::NewWindowBlockedOrOpened(tab_id, popup_url));
            false
        })
        .build()?;

    attach_page_accelerator_hook(&wv, proxy.clone());
    attach_native_webview2_hooks(&wv, tab_id, policy.adblock, !policy.images_enabled, proxy);
    apply_privacy_settings(&wv, global.save_passwords && !incognito, global.strict_tracking);
    apply_site_settings(&wv, policy);

    if (policy.zoom - 1.0).abs() > f64::EPSILON {
        apply_tab_zoom(&wv, policy.zoom);
    }
    let first = if url == "shunno://newtab" { "about:blank" } else { url };
    if url.starts_with("chrome-extension://") {
        // An extension's own page is not a web page: no page script (it carries the IPC token and
        // hooks that would break the extension's interface).
        let _ = wv.load_url(url);
    } else {
        install_content_guard(&wv, tab_id, &init_script, Some(first.to_string()));
    }
    let _ = wv.set_visible(true);
    Ok(wv)
}

thread_local! {
    /// Per tab: (install sequence, id of the page script currently installed in it).
    static GUARD_IDS: std::cell::RefCell<HashMap<usize, (u64, Option<String>)>> = std::cell::RefCell::new(HashMap::new());
}

/// Install (or replace) the content script that runs at the start of every document of a tab.
/// Installed by Shunno rather than by wry so it can be removed and re-added: a change of a site
/// setting or of a browser setting then reaches tabs that are already open, from their next page
/// load on. `then_open`: the tab's first page, opened once the script is registered (so that page
/// has the script too), unless the tab has navigated meanwhile.
#[cfg(windows)]
fn install_content_guard(wv: &WebView, tab_id: usize, script: &str, then_open: Option<String>) {
    use webview2_com::AddScriptToExecuteOnDocumentCreatedCompletedHandler;
    use wry::WebViewExtWindows;
    let Ok(core) = (unsafe { wv.controller().CoreWebView2() }) else { return };
    let seq = GUARD_IDS.with(|g| {
        let mut g = g.borrow_mut();
        let e = g.entry(tab_id).or_insert((0, None));
        if let Some(old) = e.1.take() {
            unsafe {
                let _ = core.RemoveScriptToExecuteOnDocumentCreated(&windows::core::HSTRING::from(old));
            }
        }
        e.0 += 1;
        e.0
    });
    let core2 = core.clone();
    let fallback = then_open.clone();
    // Asynchronous on purpose: a sleeping (suspended) tab answers when it wakes.
    let handler = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(move |res, id| {
        if res.is_err() {
            // Without the script the page still has to open (it just lacks Shunno's page features).
            if let Some(u) = then_open {
                if MAIN_NAV.with(|m| !m.borrow().contains_key(&tab_id)) {
                    unsafe {
                        let _ = core2.Navigate(&windows::core::HSTRING::from(u));
                    }
                }
            }
            return Ok(());
        }
        let current = GUARD_IDS.with(|g| {
            let mut g = g.borrow_mut();
            match g.get_mut(&tab_id) {
                Some(e) if e.0 == seq => {
                    e.1 = Some(id.clone());
                    true
                }
                _ => false,
            }
        });
        unsafe {
            if !current {
                // A newer script was asked for meanwhile: this one must not run as well.
                let _ = core2.RemoveScriptToExecuteOnDocumentCreated(&windows::core::HSTRING::from(id));
                return Ok(());
            }
            if let Some(u) = then_open {
                if MAIN_NAV.with(|m| !m.borrow().contains_key(&tab_id)) {
                    let _ = core2.Navigate(&windows::core::HSTRING::from(u));
                }
            }
        }
        Ok(())
    }));
    unsafe {
        if core.AddScriptToExecuteOnDocumentCreated(&windows::core::HSTRING::from(script), &handler).is_err() {
            if let Some(u) = fallback {
                let _ = core.Navigate(&windows::core::HSTRING::from(u));
            }
        }
    }
}

#[cfg(not(windows))]
fn install_content_guard(wv: &WebView, _tab_id: usize, _script: &str, then_open: Option<String>) {
    if let Some(u) = then_open {
        let _ = wv.load_url(&u);
    }
}

/// Non-Overlapping Split Layout:
/// When a drawer/popover is open, `chrome_wv` occupies the top bar + right docked panel `(w - panel_w, CHROME_BAR_HEIGHT, panel_w, content_h)`
/// while the active tab WebView occupies the left viewport `(0, CHROME_BAR_HEIGHT, w - panel_w, content_h)`.
/// Because WebView2 HWNDs never overlap, every button in the menu/downloads panel is 100% clickable AND the live webpage remains visible!
#[cfg(windows)]
/// COLORREF (0x00BBGGRR) used to paint the window's own client area: the resize grip strips
/// around the web views. Unpainted, they show as black lines.
#[cfg(windows)]
static FRAME_COLOR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x00FF_FFFF);

#[cfg(windows)]
fn set_frame_theme(window: &Window, dark: bool) {
    use std::sync::atomic::Ordering;
    use tao::platform::windows::WindowExtWindows;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::InvalidateRect;
    // Matches --surface in ui_chrome.rs: #FFFFFF light, #1C1C1E dark.
    let c: u32 = if dark { 0x001E_1C1C } else { 0x00FF_FFFF };
    if FRAME_COLOR.swap(c, Ordering::Relaxed) != c {
        unsafe {
            let _ = InvalidateRect(HWND(window.hwnd() as isize), None, true);
        }
    }
}
#[cfg(not(windows))]
fn set_frame_theme(_window: &Window, _dark: bool) {}

/// Windows "app mode" (Settings > Personalization > Colors): true when apps should be dark.
#[cfg(windows)]
fn system_prefers_dark() -> bool {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut v: u32 = 1;
    let mut sz = std::mem::size_of::<u32>() as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            windows::core::w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut v as *mut u32 as *mut core::ffi::c_void),
            Some(&mut sz),
        )
    };
    r.is_ok() && v == 0
}
#[cfg(not(windows))]
fn system_prefers_dark() -> bool {
    false
}

/// Windows build number (22000+ is Windows 11, which has Snap Layouts).
#[cfg(windows)]
fn windows_build() -> u32 {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let mut buf = [0u16; 32];
    let mut sz = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            windows::core::w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
            windows::core::w!("CurrentBuildNumber"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut sz),
        )
    };
    if r.is_err() {
        return 0;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n]).trim().parse().unwrap_or(0)
}
#[cfg(not(windows))]
fn windows_build() -> u32 {
    0
}

/// Windows 11: the maximize button is a real caption button (HTMAXBUTTON), so hovering it shows the
/// system Snap Layouts flyout exactly as in other apps. Windows 10 keeps the web-drawn button.
static SNAP_NATIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Native maximize button: a hole cut in the toolbar web view, hit-tested and painted by the window.
#[derive(Clone, Copy, Default)]
struct MaxButton {
    /// Rectangle in the toolbar web view's host window, physical px.
    rect: (i32, i32, i32, i32),
    bg: u32,
    fg: u32,
    hover_bg: u32,
    radius: i32,
    glyph_px: i32,
    hover: bool,
    pressed: bool,
    active: bool,
    /// Theme cross-fade: colours (bg, fg, hover) fade from `fade_from` to bg/fg/hover_bg over
    /// `fade_ms` from `fade_start` (ms on the system tick), on the curve of the page's fade.
    fade_from: (u32, u32, u32),
    fade_start: u64,
    fade_ms: u32,
    /// Height of the toolbar web view that is visible, physical px (0 = all of it). The view itself
    /// always keeps its full size, so showing or hiding the New Tab page and the popovers never
    /// resizes it (a resize draws an empty frame); a window region clips it instead.
    clip_h: i32,
}
/// Timer that steps the maximize button's theme fade (one per window, on the window itself).
const MAXBTN_FADE_TIMER: usize = 0x5B17;

fn tick_ms() -> u64 {
    #[cfg(windows)]
    unsafe {
        windows::Win32::System::SystemInformation::GetTickCount64()
    }
    #[cfg(not(windows))]
    0
}

/// cubic-bezier(0.32, 0.72, 0, 1): the toolbar's `--ease-fluid` and the theme cross-fade curve.
fn ease_fluid(x: f64) -> f64 {
    let bz = |t: f64, a: f64, b: f64| 3.0 * a * t * (1.0 - t) * (1.0 - t) + 3.0 * b * t * t * (1.0 - t) + t * t * t;
    let x = x.clamp(0.0, 1.0);
    let (mut lo, mut hi, mut t) = (0.0f64, 1.0f64, x);
    for _ in 0..30 {
        t = (lo + hi) / 2.0;
        if bz(t, 0.32, 0.0) < x {
            lo = t;
        } else {
            hi = t;
        }
    }
    bz(t, 0.72, 1.0)
}

/// Blend two COLORREFs: k = 0 gives `a`, 1 gives `b`.
fn mix_colorref(a: u32, b: u32, k: f64) -> u32 {
    let ch = |c: u32, s: u32| ((c >> s) & 0xFF) as f64;
    let m = |s: u32| ((ch(a, s) + (ch(b, s) - ch(a, s)) * k).round().clamp(0.0, 255.0) as u32) << s;
    m(0) | m(8) | m(16)
}

impl MaxButton {
    /// Fade progress 0..1 (1 when no fade is running).
    fn fade_progress(&self, now: u64) -> f64 {
        if self.fade_ms == 0 {
            return 1.0;
        }
        ((now.saturating_sub(self.fade_start)) as f64 / self.fade_ms as f64).min(1.0)
    }
    /// The colours to paint now: (bg, fg, hover).
    fn colors_at(&self, now: u64) -> (u32, u32, u32) {
        let p = self.fade_progress(now);
        if p >= 1.0 {
            return (self.bg, self.fg, self.hover_bg);
        }
        let k = ease_fluid(p);
        (
            mix_colorref(self.fade_from.0, self.bg, k),
            mix_colorref(self.fade_from.1, self.fg, k),
            mix_colorref(self.fade_from.2, self.hover_bg, k),
        )
    }
}
/// Each subclassed window's previous window procedure (HWND, procedure).
static ORIG_WNDPROCS: std::sync::Mutex<Vec<(isize, isize)>> = std::sync::Mutex::new(Vec::new());

/// Per browser window (keyed by its HWND): the native maximize button and the toolbar web view's
/// host window (wry's container HWND).
static MAX_BUTTONS: std::sync::Mutex<Vec<(isize, MaxButton, isize)>> = std::sync::Mutex::new(Vec::new());
fn with_max_button<R>(main: isize, f: impl FnOnce(&mut MaxButton) -> R) -> R {
    let mut v = MAX_BUTTONS.lock().unwrap_or_else(|e| e.into_inner());
    if !v.iter().any(|e| e.0 == main) {
        v.push((main, MaxButton { glyph_px: 10, ..Default::default() }, 0));
    }
    let e = v.iter_mut().find(|e| e.0 == main).expect("just inserted");
    f(&mut e.1)
}
fn chrome_host_of(main: isize) -> isize {
    MAX_BUTTONS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|e| e.0 == main).map(|e| e.2).unwrap_or(0)
}
fn set_chrome_host(main: isize, host: isize) {
    with_max_button(main, |_| ());
    if let Some(e) = MAX_BUTTONS.lock().unwrap_or_else(|e| e.into_inner()).iter_mut().find(|e| e.0 == main) {
        e.2 = host;
    }
    apply_max_button_hole(main);
}

/// Shows only the top `h` logical px of the toolbar web view (see `MaxButton::clip_h`).
fn clip_chrome_view(window: &Window, h: f64) {
    #[cfg(windows)]
    {
        use tao::platform::windows::WindowExtWindows;
        let main = window.hwnd() as isize;
        let px = (h * window.scale_factor()).round() as i32;
        let changed = with_max_button(main, |b| {
            let c = b.clip_h != px;
            b.clip_h = px;
            c
        });
        if changed {
            apply_max_button_hole(main);
        }
    }
    #[cfg(not(windows))]
    let _ = (window, h);
}
fn forget_window_native(main: isize) {
    MAX_BUTTONS.lock().unwrap_or_else(|e| e.into_inner()).retain(|e| e.0 != main);
    #[cfg(windows)]
    FULLSCREEN_WP.lock().unwrap_or_else(|e| e.into_inner()).retain(|(h, _)| *h != main);
    // A later window may get the same handle value: it must be subclassed afresh.
    ORIG_WNDPROCS.lock().unwrap_or_else(|e| e.into_inner()).retain(|(h, _)| *h != main);
}

/// Parse "#RRGGBB" to COLORREF (0x00BBGGRR).
fn colorref(hex: &str) -> Option<u32> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(((v & 0xFF) << 16) | (v & 0xFF00) | ((v >> 16) & 0xFF))
}

/// Cut the maximize button out of the toolbar web view so the window receives the mouse there.
/// The native maximize button's rectangle in the main window's client coordinates, if active.
#[cfg(windows)]
fn max_button_client_rect(main: windows::Win32::Foundation::HWND) -> Option<windows::Win32::Foundation::RECT> {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::Graphics::Gdi::MapWindowPoints;
    let b = with_max_button(main.0, |b| *b);
    let host = chrome_host_of(main.0);
    if !b.active || host == 0 || !SNAP_NATIVE.load(Ordering::Relaxed) {
        return None;
    }
    let mut pts = [POINT { x: b.rect.0, y: b.rect.1 }, POINT { x: b.rect.2, y: b.rect.3 }];
    unsafe {
        MapWindowPoints(HWND(host), main, &mut pts);
    }
    Some(RECT { left: pts[0].x, top: pts[0].y, right: pts[1].x, bottom: pts[1].y })
}

#[cfg(windows)]
fn invalidate_max_button(main: windows::Win32::Foundation::HWND) {
    if let Some(r) = max_button_client_rect(main) {
        unsafe {
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(main, Some(&r), false);
        }
    }
}

/// Paint the native maximize/restore button with Windows' own caption glyphs (Segoe Fluent Icons).
#[cfg(windows)]
fn paint_max_button(main: windows::Win32::Foundation::HWND, hdc: windows::Win32::Graphics::Gdi::HDC) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::Graphics::Gdi::{
        CreateFontW, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, DrawTextW, FillRect, FillRgn, SelectObject, SetBkMode,
        SetTextColor, ANTIALIASED_QUALITY, DT_CENTER, DT_NOCLIP, DT_SINGLELINE, DT_VCENTER, FW_NORMAL, TRANSPARENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::IsZoomed;
    let Some(mut r) = max_button_client_rect(main) else { return };
    let b0 = with_max_button(main.0, |b| *b);
    let (cbg, cfg, chov) = b0.colors_at(tick_ms());
    let b = MaxButton { bg: cbg, fg: cfg, hover_bg: chov, ..b0 };
    unsafe {
        let bg = CreateSolidBrush(COLORREF(b.bg));
        FillRect(hdc, &r, bg);
        let _ = DeleteObject(bg);
        if b.hover || b.pressed {
            let c = if b.pressed {
                // A little darker/lighter than hover.
                let mix = |a: u32, b: u32| (a * 3 + b) / 4;
                let (h, f) = (b.hover_bg, b.fg);
                mix(h & 0xFF, f & 0xFF) | (mix((h >> 8) & 0xFF, (f >> 8) & 0xFF) << 8) | (mix((h >> 16) & 0xFF, (f >> 16) & 0xFF) << 16)
            } else {
                b.hover_bg
            };
            let hb = CreateSolidBrush(COLORREF(c));
            let rgn = CreateRoundRectRgn(r.left, r.top, r.right + 1, r.bottom + 1, b.radius * 2, b.radius * 2);
            let _ = FillRgn(hdc, rgn, hb);
            let _ = DeleteObject(rgn);
            let _ = DeleteObject(hb);
        }
        let mut face: Vec<u16> = "Segoe Fluent Icons".encode_utf16().chain(Some(0)).collect();
        let mut font = CreateFontW(-b.glyph_px, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, ANTIALIASED_QUALITY.0 as u32, 0, windows::core::PCWSTR(face.as_ptr()));
        if font.is_invalid() {
            face = "Segoe MDL2 Assets".encode_utf16().chain(Some(0)).collect();
            font = CreateFontW(-b.glyph_px, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, ANTIALIASED_QUALITY.0 as u32, 0, windows::core::PCWSTR(face.as_ptr()));
        }
        let old = SelectObject(hdc, font);
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, COLORREF(b.fg));
        // E922 = Maximize, E923 = Restore (the glyphs Windows uses on caption buttons).
        let mut glyph: Vec<u16> = vec![if IsZoomed(main).as_bool() { 0xE923 } else { 0xE922 }];
        DrawTextW(hdc, &mut glyph, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP);
        SelectObject(hdc, old);
        let _ = DeleteObject(font);
    }
}

#[cfg(windows)]
fn apply_max_button_hole(main_hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, DeleteObject, InvalidateRect, SetWindowRgn, RGN_DIFF};
    let host = chrome_host_of(main_hwnd);
    if host == 0 {
        return;
    }
    let b = with_max_button(main_hwnd, |b| *b);
    unsafe {
        let all = CreateRectRgn(0, 0, 32000, if b.clip_h > 0 { b.clip_h } else { 32000 });
        if b.active && b.rect.2 > b.rect.0 && b.rect.3 > b.rect.1 {
            let hole = CreateRectRgn(b.rect.0, b.rect.1, b.rect.2, b.rect.3);
            CombineRgn(all, all, hole, RGN_DIFF);
            let _ = DeleteObject(hole);
        }
        // The system owns the region after this call. No repaint of the exposed area is requested: a
        // popover closing exposes the page view, which draws itself, and a forced repaint of the
        // parent first filled that area with the frame colour (the flash when a menu closed).
        SetWindowRgn(HWND(host), all, false);
        if let Some(r) = max_button_client_rect(HWND(main_hwnd)) {
            let _ = InvalidateRect(HWND(main_hwnd), Some(&r), false);
        }
    }
}
#[cfg(not(windows))]
fn apply_max_button_hole(_main_hwnd: isize) {}

/// Proxy for the window procedure to report WM_SETTINGCHANGE("ImmersiveColorSet").
/// "Darken All Sites" as it was when the browser started (the setting applies at next start).
static FORCE_DARK_AT_START: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

static THEME_PROXY: std::sync::OnceLock<std::sync::Mutex<EventLoopProxy<AppEvent>>> = std::sync::OnceLock::new();

fn page_theme(dark: bool) -> wry::Theme {
    if dark { wry::Theme::Dark } else { wry::Theme::Light }
}

/// Apply light/dark everywhere: window frame, pages' preferred colour scheme, pre-paint background.
fn apply_theme(window: &Window, tabs: &[BrowserTab], dark: bool) {
    set_frame_theme(window, dark);
    #[cfg(windows)]
    {
        use wry::WebViewExtWindows;
        let each = |wv: &WebView| {
            let _ = wv.set_theme(page_theme(dark));
            let _ = wv.set_background_color(page_background(dark));
        };
        for t in tabs {
            each(&t.webview);
        }
        PEEK.with(|p| {
            if let Some(k) = p.borrow().as_ref() {
                each(&k.webview);
            }
        });
    }
}

/// Default background shown before a page paints (no white flash in dark mode).
fn page_background(dark: bool) -> (u8, u8, u8, u8) {
    if dark { (0x1C, 0x1C, 0x1E, 255) } else { (255, 255, 255, 255) }
}

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
        HTBOTTOMRIGHT, HTCLIENT, HTMAXBUTTON, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, ICON_BIG,
        ICON_SMALL, LR_DEFAULTCOLOR, SWP_FRAMECHANGED, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, WM_NCCALCSIZE, WM_NCHITTEST, WM_SETICON, WNDPROC,
        WS_EX_APPWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
    };
    use wry::WebViewExtWindows;

    static ORIG_WNDPROC: AtomicIsize = AtomicIsize::new(0);
    fn orig_wndproc_for(h: isize) -> Option<isize> {
        ORIG_WNDPROCS.lock().ok()?.iter().find(|(x, _)| *x == h).map(|(_, p)| *p)
    }
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

        // 2. Window/taskbar icons at the exact sizes Windows asks for at this DPI (32 px at 100%, 40 at
        //    125%, 48 at 150%...). Handing Windows a 48 px icon at 100% made it downscale to a blurry 32.
        {
            use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
            use windows::Win32::UI::WindowsAndMessaging::{SM_CXICON, SM_CXSMICON};
            const ICO_BYTES: &[u8] = include_bytes!("../assets/app.ico");
            let dpi = GetDpiForWindow(hwnd).max(96);
            let entries: Vec<(i32, &[u8])> = (|| {
                let count = u16::from_le_bytes([*ICO_BYTES.get(4)?, *ICO_BYTES.get(5)?]) as usize;
                let mut v = Vec::new();
                for i in 0..count {
                    let e = ICO_BYTES.get(6 + i * 16..6 + i * 16 + 16)?;
                    let w = if e[0] == 0 { 256 } else { e[0] as i32 };
                    let sz = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
                    let off = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
                    v.push((w, ICO_BYTES.get(off..off + sz)?));
                }
                Some(v)
            })()
            .unwrap_or_default();
            // Exact size if present, else the smallest larger one, else the largest.
            let pick = |want: i32| -> Option<(i32, &[u8])> {
                entries.iter().filter(|(w, _)| *w >= want).min_by_key(|(w, _)| *w)
                    .or_else(|| entries.iter().max_by_key(|(w, _)| *w))
                    .map(|(w, b)| (*w, *b))
            };
            for (metric, class_slot, which) in [(SM_CXICON, GCLP_HICON, ICON_BIG), (SM_CXSMICON, GCLP_HICONSM, ICON_SMALL)] {
                let want = GetSystemMetricsForDpi(metric, dpi).max(16);
                if let Some((_, bytes)) = pick(want) {
                    // One icon per size for the whole process: every window (Ctrl+N) reuses it, so
                    // no HICON is created per window and none leaks.
                    static ICONS: std::sync::Mutex<Vec<(i32, isize)>> = std::sync::Mutex::new(Vec::new());
                    let mut cache = ICONS.lock().unwrap_or_else(|e| e.into_inner());
                    let hicon = match cache.iter().find(|(w, _)| *w == want) {
                        Some((_, h)) => Some(*h),
                        None => CreateIconFromResourceEx(bytes, true, 0x00030000, want, want, LR_DEFAULTCOLOR).ok().map(|h| {
                            cache.push((want, h.0));
                            h.0
                        }),
                    };
                    if let Some(h) = hicon {
                        SetClassLongPtrW(hwnd, class_slot, h);
                        let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(which as usize), LPARAM(h));
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
                // Paint the grip strips in the theme colour (the window class has no background brush).
                0x0014 /* WM_ERASEBKGND */ => return LRESULT(1),
                0x004A /* WM_COPYDATA */ => {
                    if let Some(url) = little::read_copydata(lparam.0) {
                        if let Some(p) = THEME_PROXY.get().and_then(|m| m.lock().ok()) {
                            let _ = p.send_event(AppEvent::ExternalOpen(url));
                        }
                        return LRESULT(1);
                    }
                }
                0x0312 /* WM_HOTKEY */ if wparam.0 as i32 == little::HOTKEY_ID => {
                    // Ctrl+Alt+N opens Little Shunno in the profile used last, whichever process owns the key.
                    let last = BrowserStorage::load_last_profile().unwrap_or_else(|| state::profile_id().to_string());
                    if last != state::profile_id() && little::profile_running(&last) && little::send_to_profile(&last, little::LITTLE_PAYLOAD) {
                        return LRESULT(0);
                    }
                    if let Some(p) = THEME_PROXY.get().and_then(|m| m.lock().ok()) {
                        let _ = p.send_event(AppEvent::LittleOpen(None));
                    }
                    return LRESULT(0);
                }
                0x001A /* WM_SETTINGCHANGE */ => {
                    if lparam.0 != 0 {
                        let s = windows::core::PCWSTR(lparam.0 as *const u16).to_string().unwrap_or_default();
                        if s == "ImmersiveColorSet" {
                            if let Some(p) = THEME_PROXY.get().and_then(|m| m.lock().ok()) {
                                let _ = p.send_event(AppEvent::SystemThemeChanged);
                            }
                        }
                    }
                }
                0x000F /* WM_PAINT */ => {
                    use windows::Win32::Graphics::Gdi::{BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, PAINTSTRUCT};
                    use windows::Win32::Foundation::COLORREF;
                    use windows::Win32::Graphics::Gdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, SelectObject, SetViewportOrgEx, SRCCOPY};
                    let mut ps = PAINTSTRUCT::default();
                    let hdc = BeginPaint(hwnd, &mut ps);
                    // Drawn off screen and copied in one step: the button never shows a half-painted
                    // state (frame colour, then background, then glyph), which flickered on repaints.
                    let rc = ps.rcPaint;
                    let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
                    let mem = CreateCompatibleDC(hdc);
                    let bmp = if w > 0 && h > 0 { CreateCompatibleBitmap(hdc, w, h) } else { Default::default() };
                    let buffered = !mem.is_invalid() && !bmp.is_invalid();
                    let target = if buffered { mem } else { hdc };
                    let old_bmp = if buffered { SelectObject(mem, bmp) } else { Default::default() };
                    if buffered {
                        let _ = SetViewportOrgEx(mem, -rc.left, -rc.top, None);
                    }
                    let brush = CreateSolidBrush(COLORREF(FRAME_COLOR.load(Ordering::Relaxed)));
                    FillRect(target, &rc, brush);
                    let _ = DeleteObject(brush);
                    paint_max_button(hwnd, target);
                    if buffered {
                        let _ = SetViewportOrgEx(mem, 0, 0, None);
                        let _ = BitBlt(hdc, rc.left, rc.top, w, h, mem, 0, 0, SRCCOPY);
                        SelectObject(mem, old_bmp);
                        let _ = DeleteObject(bmp);
                    }
                    if !mem.is_invalid() {
                        let _ = DeleteDC(mem);
                    }
                    let _ = EndPaint(hwnd, &ps);
                    return LRESULT(0);
                }
                0x0113 /* WM_TIMER */ if wparam.0 == MAXBTN_FADE_TIMER => {
                    let done = with_max_button(hwnd.0, |b| {
                        let done = b.fade_progress(tick_ms()) >= 1.0;
                        if done {
                            b.fade_ms = 0;
                        }
                        done
                    });
                    invalidate_max_button(hwnd);
                    if done {
                        let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(hwnd, MAXBTN_FADE_TIMER);
                    }
                    return LRESULT(0);
                }
                // Native maximize button (Windows 11): hover tracking and clicks. Mouse-move still goes
                // to the default handler so Windows can show Snap Layouts.
                0x00A0 /* WM_NCMOUSEMOVE */ => {
                    let over = wparam.0 == HTMAXBUTTON as usize;
                    let changed = with_max_button(hwnd.0, |b| {
                        let c = b.hover != over;
                        b.hover = over;
                        if !over {
                            b.pressed = false;
                        }
                        c
                    });
                    if over {
                        use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT};
                        let mut t = TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE | TME_NONCLIENT,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        let _ = TrackMouseEvent(&mut t);
                    }
                    if changed {
                        invalidate_max_button(hwnd);
                    }
                }
                0x02A2 /* WM_NCMOUSELEAVE */ => {
                    with_max_button(hwnd.0, |b| {
                        b.hover = false;
                        b.pressed = false;
                    });
                    invalidate_max_button(hwnd);
                }
                0x00A1 /* WM_NCLBUTTONDOWN */ | 0x00A3 /* WM_NCLBUTTONDBLCLK */ if wparam.0 == HTMAXBUTTON as usize => {
                    with_max_button(hwnd.0, |b| b.pressed = true);
                    invalidate_max_button(hwnd);
                    return LRESULT(0);
                }
                0x00A2 /* WM_NCLBUTTONUP */ if wparam.0 == HTMAXBUTTON as usize => {
                    let was_pressed = with_max_button(hwnd.0, |b| std::mem::replace(&mut b.pressed, false));
                    if was_pressed {
                        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_MAXIMIZE, SW_RESTORE};
                        let _ = ShowWindow(hwnd, if IsZoomed(hwnd).as_bool() { SW_RESTORE } else { SW_MAXIMIZE });
                    }
                    invalidate_max_button(hwnd);
                    return LRESULT(0);
                }
                WM_NCCALCSIZE => {
                    if wparam.0 != 0 {
                        // Client area = whole window (custom frame). When maximized, Windows places a
                        // WS_THICKFRAME window ~8 px beyond every monitor edge; inset by the real frame size
                        // for this window's DPI, or the top of the tab strip and the window buttons are clipped.
                        // Not in full screen: there the window must cover the monitor exactly, even if it
                        // was maximized when F11 was pressed.
                        if IsZoomed(hwnd).as_bool() && !is_fullscreen_hwnd(hwnd.0) {
                            use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
                            use windows::Win32::UI::WindowsAndMessaging::{
                                NCCALCSIZE_PARAMS, SM_CXFRAME, SM_CXPADDEDBORDER, SM_CYFRAME,
                            };
                            let dpi = GetDpiForWindow(hwnd).max(96);
                            let pad = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
                            let fx = GetSystemMetricsForDpi(SM_CXFRAME, dpi) + pad;
                            let fy = GetSystemMetricsForDpi(SM_CYFRAME, dpi) + pad;
                            let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
                            if !params.is_null() {
                                let r = &mut (*params).rgrc[0];
                                r.left += fx;
                                r.right -= fx;
                                r.top += fy;
                                r.bottom -= fy;
                            }
                        }
                        return LRESULT(0);
                    }
                }
                WM_NCHITTEST => {
                    if let Some(r) = max_button_client_rect(hwnd) {
                        use windows::Win32::Graphics::Gdi::ScreenToClient;
                        let mut pt = windows::Win32::Foundation::POINT {
                            x: (lparam.0 & 0xFFFF) as i16 as i32,
                            y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
                        };
                        let _ = ScreenToClient(hwnd, &mut pt);
                        if pt.x >= r.left && pt.x < r.right && pt.y >= r.top && pt.y < r.bottom {
                            return LRESULT(HTMAXBUTTON as isize);
                        }
                    }
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
            let prev_ptr = orig_wndproc_for(hwnd.0).unwrap_or_else(|| ORIG_WNDPROC.load(Ordering::Relaxed));
            if prev_ptr != 0 && prev_ptr != borderless_resize_wndproc as *const () as usize as isize {
                let prev_fn: WNDPROC = std::mem::transmute(prev_ptr);
                CallWindowProcW(prev_fn, hwnd, msg, wparam, lparam)
            } else {
                LRESULT(0)
            }
        }

        // Subclass once per window, and remember each window's own previous procedure (never our
        // own: chaining to ourselves would recurse forever).
        let ours = borderless_resize_wndproc as *const () as usize as isize;
        if orig_wndproc_for(hwnd.0).is_none() {
            let prev = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, ours);
            if prev != 0 && prev != ours {
                ORIG_WNDPROC.store(prev, Ordering::Relaxed);
                ORIG_WNDPROCS.lock().unwrap_or_else(|e| e.into_inner()).push((hwnd.0, prev));
            }
        }
    }
}

#[cfg(not(windows))]
fn install_native_win32_window_features(_window: &Window, _chrome_wv: &WebView) {}

/// Window order of the pages and the toolbar view: `pages` from the front to the back, and the
/// toolbar view above all of them. The toolbar view is always on top and shows only the part
/// `clip_chrome_view` leaves, so a popover or the New Tab page is only a change of that part.
#[cfg(windows)]
fn stack_views(chrome_wv: &WebView, pages: &[&WebView]) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE};
    use wry::WebViewExtWindows;
    let hwnd_of = |wv: &WebView| -> Option<HWND> {
        let mut h = HWND::default();
        unsafe { wv.controller().ParentWindow(&mut h).ok().filter(|_| h.0 != 0).map(|_| h) }
    };
    let Some(chrome) = hwnd_of(chrome_wv) else { return };
    unsafe {
        for wv in pages.iter().rev() {
            if let Some(h) = hwnd_of(wv) {
                if h.0 != chrome.0 {
                    let _ = SetWindowPos(h, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
            }
        }
        let _ = SetWindowPos(chrome, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}
#[cfg(not(windows))]
fn stack_views(_chrome_wv: &WebView, _pages: &[&WebView]) {}

thread_local! {
    /// What layout_webviews last applied to each tab's web view: (visible, x, y, w, h). Every call
    /// used to resize and hide every tab, tens of window calls per click with many tabs open.
    static TAB_APPLIED: std::cell::RefCell<std::collections::HashMap<usize, (bool, [f64; 4])>> = std::cell::RefCell::new(std::collections::HashMap::new());
    static LAST_SCALE: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    /// Tab ids whose web views layout_webviews left visible.
    static TAB_SHOWN: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The current tab, and the one before it (kept on screen under the current one).
    static TAB_LAST_ACTIVE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static TAB_WARM_PREV: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    /// A newly shown tab waiting to be raised over the previous one.
    static TAB_RAISE_PENDING: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    /// The current tab has been brought to the front after its wait (no further wait for it).
    static TAB_RAISE_DONE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    /// The New Tab page (the grown toolbar view) is what is on screen.
    static HOME_SHOWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The tab switching state of one window. The six thread locals above hold the current window's;
/// they are swapped with the window (a second window must not see or disturb the first one's).
#[derive(Default)]
struct TabFlow {
    shown: Vec<usize>,
    last_active: Option<usize>,
    warm_prev: Option<usize>,
    raise_pending: Option<usize>,
    raise_done: Option<usize>,
    home_shown: bool,
}
/// Installs `new` as the current window's tab switching state and returns what it replaces.
fn swap_tab_flow(new: TabFlow) -> TabFlow {
    TabFlow {
        shown: TAB_SHOWN.with(|c| std::mem::replace(&mut *c.borrow_mut(), new.shown)),
        last_active: TAB_LAST_ACTIVE.with(|c| c.replace(new.last_active)),
        warm_prev: TAB_WARM_PREV.with(|c| c.replace(new.warm_prev)),
        raise_pending: TAB_RAISE_PENDING.with(|c| c.replace(new.raise_pending)),
        raise_done: TAB_RAISE_DONE.with(|c| c.replace(new.raise_done)),
        home_shown: HOME_SHOWN.with(|c| c.replace(new.home_shown)),
    }
}



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
    // The real window size, never a larger one: a floor here pushed the toolbar view past the
    // window edge (controls cut off, right edge unreachable). The window's own minimum is
    // MIN_WINDOW_W by MIN_WINDOW_H.
    let w = size.width.max(1.0);
    let h = size.height.max(1.0);
    let is_max = window.is_maximized() || is_fs;
    let grip = if is_max { 0.0 } else { 3.0 };

    let top_bar_h = if is_fs { 0.0 } else { chrome_bar_height() };
    let inner_w = (w - grip * 2.0).max(1.0);
    let inner_h = (h - grip).max(200.0);
    let content_h = (inner_h - top_bar_h).max(80.0);

    let valid_split = split_idx.filter(|&s| s < tabs.len() && s != active_idx);
    let is_home = valid_split.is_none()
        && tabs
            .get(active_idx)
            .map(|t| t.url == "shunno://newtab")
            .unwrap_or(true);

    let overlay_active = drawer_open || engine_menu_open || palette_open || SHEET_OPEN.with(|c| c.get()) || (peek_open() && !is_fs);

    // No blank frames between pages (as Chrome does). A web view that has just been shown, or just
    // grown, draws an empty frame first; that frame was the flash when switching tabs and opening a
    // New Tab. So whatever is on screen stays on top until what replaces it has had time to draw
    // (70 ms), then the order changes. The tab you came from also stays drawn under the current one,
    // so switching back is only a change of order.
    let active_id = tabs.get(active_idx).map(|t| t.id);
    let shown_before = TAB_SHOWN.with(|v| v.borrow().clone());
    let home_was_shown = HOME_SHOWN.with(|c| c.get());
    let last = TAB_LAST_ACTIVE.with(|c| c.get());
    if let Some(aid) = active_id {
        if last != Some(aid) {
            TAB_WARM_PREV.with(|c| c.set(last));
            TAB_LAST_ACTIVE.with(|c| c.set(Some(aid)));
            TAB_RAISE_DONE.with(|c| c.set(None));
        }
    }
    let raised = active_id.is_some() && TAB_RAISE_DONE.with(|c| c.get()) == active_id;
    let page_tab = |i: usize| tabs.get(i).is_some_and(|t| t.url != "shunno://newtab");
    // The page that was on top before this change (if it is still open).
    let prev_idx = TAB_WARM_PREV
        .with(|c| c.get())
        .and_then(|pid| tabs.iter().position(|t| t.id == pid))
        .filter(|&pi| pi != active_idx && page_tab(pi));
    let single = valid_split.is_none() && !is_fs;
    // A page to New Tab: the New Tab page (the toolbar view, grown) draws under the page first.
    let wait_home = single
        && is_home
        && !raised
        && !home_was_shown
        && prev_idx.is_some_and(|pi| shown_before.contains(&tabs[pi].id));
    // New Tab to a page: the page draws under the New Tab page first.
    let wait_leave_home = single
        && !is_home
        && !raised
        && home_was_shown
        && page_tab(active_idx)
        && active_id.is_some_and(|aid| !shown_before.contains(&aid));
    // A page to another page.
    let wait_page = single
        && !is_home
        && !raised
        && !home_was_shown
        && page_tab(active_idx)
        && active_id.is_some_and(|aid| !shown_before.contains(&aid) || TAB_RAISE_PENDING.with(|c| c.get()) == Some(aid))
        && prev_idx.is_some_and(|pi| shown_before.contains(&tabs[pi].id));
    let waiting = wait_home || wait_leave_home || wait_page;

    let chrome_h = if is_fs {
        if is_home { inner_h } else { 0.0 }
    } else if overlay_active || (is_home && !wait_home) || wait_leave_home {
        inner_h
    } else {
        top_bar_h
    };

    // Order matters to avoid a flash of the bare window: when the chrome grows (New Tab page, popovers),
    // cover the page first and hide it after; when it shrinks, show the page first and shrink after.
    let apply_chrome = || {
        let _ = chrome_wv.set_visible(!is_fs || is_home);
        // The toolbar view never changes size; only the part of it that shows does.
        let _ = chrome_wv.set_bounds(Rect {
            position: LogicalPosition::new(grip, 0.0).into(),
            size: LogicalSize::new(inner_w, inner_h).into(),
        });
        clip_chrome_view(window, chrome_h);
    };
    let last_h = CHROME_LAST_H.with(|c| c.replace(chrome_h));
    let chrome_grows = chrome_h >= last_h;
    if chrome_grows {
        apply_chrome();
    }

    // Full-width webpage (zero splitting): Menu & Downloads float over the live webpage at 10% transparency.
    let web_w = inner_w;

    let mut placements: Vec<(usize, f64, f64)> = Vec::new(); // (tab index, x, width) of visible tabs
    if let Some(s_idx) = valid_split {
        let half_w = ((web_w - 2.0) / 2.0).max(100.0);
        if page_tab(active_idx) {
            placements.push((active_idx, grip, half_w));
        }
        if page_tab(s_idx) {
            placements.push((s_idx, grip + half_w + 2.0, half_w));
        }
    } else {
        if page_tab(active_idx) {
            placements.push((active_idx, grip, web_w));
        }
        // The previous page stays drawn underneath (only one page wide, never in full screen).
        if let Some(pi) = prev_idx {
            if !is_fs && (!is_home || wait_home) {
                placements.push((pi, grip, web_w));
            }
        }
    }
    // Bounds are logical, so a move to a monitor with another scale factor keeps them equal while
    // the physical size changes: the cache is only valid for the scale factor it was filled at.
    let scale = window.scale_factor();
    if LAST_SCALE.with(|c| c.replace(scale)) != scale {
        TAB_APPLIED.with(|m| m.borrow_mut().clear());
    }
    TAB_APPLIED.with(|m| {
        let mut m = m.borrow_mut();
        m.retain(|id, _| tabs.iter().any(|t| t.id == *id));
        for &(idx, x, pw) in &placements {
            let tab = &tabs[idx];
            let want = (true, [x, top_bar_h, pw, content_h]);
            if m.get(&tab.id) != Some(&want) {
                let _ = tab.webview.set_bounds(Rect {
                    position: LogicalPosition::new(x, top_bar_h).into(),
                    size: LogicalSize::new(pw, content_h).into(),
                });
                let _ = tab.webview.set_visible(true);
                m.insert(tab.id, want);
            }
        }
        // A hidden tab is only hidden, once; it gets its size when it is shown.
        for (idx, tab) in tabs.iter().enumerate() {
            if placements.iter().any(|p| p.0 == idx) {
                continue;
            }
            if m.get(&tab.id).map(|e| e.0) != Some(false) {
                let _ = tab.webview.set_visible(false);
                m.insert(tab.id, (false, [0.0; 4]));
            }
        }
    });
    TAB_SHOWN.with(|v| *v.borrow_mut() = placements.iter().map(|p| tabs[p.0].id).collect());
    if !chrome_grows {
        apply_chrome();
    }

    if waiting {
        if let Some(aid) = active_id {
            if TAB_RAISE_PENDING.with(|c| c.replace(Some(aid))) != Some(aid) {
                if wait_leave_home {
                    // The toolbar keeps drawing the New Tab page over the page meanwhile.
                    let _ = chrome_wv.evaluate_script("window.__holdHome && window.__holdHome(90)");
                }
                if let Some(px) = THEME_PROXY.get().and_then(|m| m.lock().ok().map(|p| p.clone())) {
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(70));
                        let _ = px.send_event(AppEvent::RaiseTab(aid));
                    });
                }
            }
        }
    } else {
        TAB_RAISE_PENDING.with(|c| c.set(None));
    }
    HOME_SHOWN.with(|c| c.set(if waiting { home_was_shown } else { is_home && !is_fs }));

    // Stacking: the toolbar view on top; the page the user should see in front of the one it replaces.
    if !is_fs {
        let cur = tabs.get(active_idx).filter(|_| page_tab(active_idx)).map(|t| &t.webview);
        let prev = prev_idx.map(|pi| &tabs[pi].webview);
        let mut order: Vec<&WebView> = Vec::new();
        if wait_home || wait_page {
            // The page on screen stays in front until its replacement has drawn.
            order.extend(prev);
            order.extend(cur);
        } else {
            order.extend(cur);
            order.extend(prev);
        }
        stack_views(chrome_wv, &order);
    }

    // Peek sits above everything, inset in the content area; the chrome draws the frame and backdrop.
    PEEK.with(|p| {
        if let Some(peek) = p.borrow().as_ref() {
            if is_fs {
                let _ = peek.webview.set_visible(false);
                return;
            }
            let side = (inner_w * 0.07).clamp(24.0, 140.0);
            let header = 46.0;
            let x = grip + side;
            let y = top_bar_h + 14.0 + header;
            let pw = (inner_w - side * 2.0).max(200.0);
            let ph = (inner_h - y - 18.0).max(120.0);
            PEEK_RECT.with(|r| r.set([x, y, pw, ph]));
            let _ = peek.webview.set_bounds(Rect {
                position: LogicalPosition::new(x, y).into(),
                size: LogicalSize::new(pw, ph).into(),
            });
            let _ = peek.webview.set_visible(true);
            bring_webview_to_top(&peek.webview);
        }
    });
}

#[cfg(windows)]
fn bring_webview_to_top(wv: &WebView) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE};
    use wry::WebViewExtWindows;
    unsafe {
        let mut h = HWND::default();
        if wv.controller().ParentWindow(&mut h).is_ok() && h.0 != 0 {
            let _ = SetWindowPos(h, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
}
#[cfg(not(windows))]
fn bring_webview_to_top(_wv: &WebView) {}

fn main() -> wry::Result<()> {
    set_explicit_app_user_model_id();

    // Extensions: every WebView2 environment sharing the profile folder must use the same options,
    // so this is set once, before the first web view.
    #[cfg(windows)]
    webview2_com::DEFAULT_BROWSER_EXTENSIONS_ENABLED.store(true, Ordering::Relaxed);

    // Profile first: every data path below depends on it.
    // One Shunno per profile: a second launch (e.g. a link clicked in another app) hands its link over.
    let start_url = little::url_from_args(std::env::args());
    {
        let explicit = std::env::args().any(|a| a.starts_with("--profile="));
        let arg = explicit.then(|| state::profile_from_args(std::env::args()));
        state::set_profile(&state::choose_launch_profile(
            arg.as_deref(),
            start_url.is_some(),
            &BrowserStorage::load_links_profile(),
            BrowserStorage::load_last_profile().as_deref(),
            &BrowserStorage::load_profiles(),
        ));
    }
    if little::hand_off_to_running_instance(start_url.as_deref()) {
        return Ok(());
    }

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
    // full_dark_mode is the effective theme for this run (System follows Windows).
    global_settings.full_dark_mode = state::effective_dark(&global_settings.theme_mode, system_prefers_dark());
    https_guard::global().set_enabled(global_settings.https_only);
    let mut site_policies = BrowserStorage::load_site_policies();
    let mut bookmarks = BrowserStorage::load_bookmarks();
    if !global_settings.favorites_migrated {
        state::migrate_to_favorites(&mut bookmarks, &global_settings.quick_shortcuts, now_epoch_secs());
        BrowserStorage::save_bookmarks(&bookmarks);
        global_settings.favorites_migrated = true;
        BrowserStorage::save_global_settings(&global_settings);
    }
    let mut history = BrowserStorage::load_history();
    let engine = Arc::new(NativeDownloadEngine::new(
        global_settings.speed_limit_bps,
        global_settings.auto_clean_completed,
        global_settings.max_concurrent_downloads,
    ));

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let mut window = WindowBuilder::new()
        .with_title(window_title_for_profile(&BrowserStorage::load_profiles()))
        .with_decorations(false)
        .with_window_icon(load_embedded_window_icon())
        .with_inner_size(LogicalSize::new(1240.0, 780.0))
        .with_min_inner_size(LogicalSize::new(MIN_WINDOW_W, MIN_WINDOW_H))
        .build(&event_loop)
        .unwrap();

    let chrome_proxy = proxy.clone();
    #[allow(unused_mut)]
    let mut chrome_builder = WebViewBuilder::new_as_child(&window);
    #[cfg(windows)]
    {
        // Must match the tab web views' arguments: they share one WebView2 environment.
        use wry::WebViewBuilderExtWindows;
        chrome_builder = chrome_builder.with_additional_browser_args(state::browser_args(global_settings.force_dark_pages));
    }
    let mut chrome_wv = chrome_builder
        .with_web_context(&mut web_context)
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: LogicalSize::new(1240.0, CHROME_BAR_HEIGHT).into(),
        })
        .with_transparent(true)
        .with_html(ui_chrome::CHROME_HTML)
        .with_ipc_handler(move |req| {
            let _ = chrome_proxy.send_event(AppEvent::ChromeIpcW(1, req.body().clone()));
        })
        .build()?;

    install_native_win32_window_features(&window, &chrome_wv);
    #[cfg(windows)]
    unsafe {
        // The engine's default User-Agent, for tabs leaving a "mobile site" host.
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings2;
        use windows::core::Interface;
        use wry::WebViewExtWindows;
        if let Ok(s2) = chrome_wv.controller().CoreWebView2().and_then(|c| c.Settings()).and_then(|s| s.cast::<ICoreWebView2Settings2>()) {
            let mut p = windows::core::PWSTR::null();
            if s2.UserAgent(&mut p).is_ok() {
                let ua = webview2_com::take_pwstr(p);
                if !ua.is_empty() {
                    let _ = DESKTOP_UA.set(ua);
                }
            }
        }
    }
    #[cfg(windows)]
    {
        use tao::platform::windows::WindowExtWindows;
        little::publish_main_window(window.hwnd() as isize);
        little::register_hotkey(window.hwnd() as isize);
    }
    set_frame_theme(&window, global_settings.full_dark_mode);
    let _ = THEME_PROXY.set(std::sync::Mutex::new(proxy.clone()));
    FORCE_DARK_AT_START.store(global_settings.force_dark_pages, Ordering::Relaxed);
    SNAP_NATIVE.store(windows_build() >= 22000, Ordering::Relaxed);
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_STYLE, WS_MAXIMIZEBOX};
        use wry::WebViewExtWindows;
        let mut host = windows::Win32::Foundation::HWND::default();
        if chrome_wv.controller().ParentWindow(&mut host).is_ok() {
            use tao::platform::windows::WindowExtWindows;
            set_chrome_host(window.hwnd() as isize, host.0);
        }
        if SNAP_NATIVE.load(Ordering::Relaxed) {
            // Windows only offers Snap Layouts for windows that have a maximize box.
            use tao::platform::windows::WindowExtWindows;
            let h = windows::Win32::Foundation::HWND(window.hwnd() as isize);
            let st = GetWindowLongPtrW(h, GWL_STYLE);
            SetWindowLongPtrW(h, GWL_STYLE, st | WS_MAXIMIZEBOX.0 as isize);
        }
    }
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
    // Last imported password CSV, so the Passwords panel can offer to delete it.
    let mut pw_last_csv: Option<std::path::PathBuf> = None;
    let mut find_current: usize = 0;
    let mut find_total: usize = 0;
    let mut had_active_downloads = false;
    let mut site_data_cleared = false;
    // Tabs for which the user asked "Save page"; OFFLINE_HTML from any other tab is ignored.
    let mut pending_offline_saves: std::collections::HashSet<usize> = std::collections::HashSet::new();
    // Tabs for which the user pressed "Grab"; GRAB_RESULTS from any other tab is ignored.
    let mut pending_grabs: std::collections::HashSet<usize> = std::collections::HashSet::new();
    // Tabs where the user started the element picker; PICK_FILTER from any other tab is ignored.
    let mut pending_picks: std::collections::HashSet<usize> = std::collections::HashSet::new();

    let saved_session = BrowserStorage::load_session();
    GROUPS.with(|g| *g.borrow_mut() = BrowserStorage::load_tab_groups());
    let known = |g: u32| GROUPS.with(|x| x.borrow().iter().any(|y| y.id == g));
    let mut saved_windows: Vec<Vec<(String, Option<u32>)>> = state::session_windows(saved_session)
        .into_iter()
        .map(|w| w.into_iter().map(|t| (t.url, t.group.filter(|&g| known(g)))).collect())
        .collect();
    let initial: Vec<(String, Option<u32>)> = if saved_windows.is_empty() {
        vec![("shunno://newtab".to_string(), None)]
    } else {
        saved_windows.remove(0)
    };
    // The other windows are rebuilt once the event loop runs.
    for w in saved_windows {
        RESTORES_PENDING.with(|r| r.set(r.get() + 1));
        let _ = proxy.send_event(AppEvent::RestoreWindow(w));
    }

    for (u, restored_group) in initial {
        let host = extract_host(&u);
        let pol = site_policies.get(&host).cloned().unwrap_or_default();
        let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
        set_tab_group(tid, restored_group);
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
                zoom: pol.zoom,
                webview: wv,
            });
        }
    }

    if let Some(u) = start_url.clone() {
        let _ = proxy.send_event(AppEvent::NewWindowBlockedOrOpened(0, u));
    }
    // Little Shunno (one at a time).
    struct Little {
        // Fields drop in this order: web views before the window that hosts them.
        bar: WebView,
        content: WebView,
        window: Window,
        id: usize,
    }
    let mut little: Option<Little> = None;
    const LITTLE_BAR_H: f64 = 44.0;
    /// Little Shunno asks in its own bar (it has no main toolbar).
    fn little_sync_permission(l: &Little) {
        let v = pending_prompt(l.id);
        let js = serde_json::to_string(&v).unwrap_or_else(|_| "null".into());
        let _ = l.bar.evaluate_script(&format!("window.__perm && window.__perm({js});"));
    }
    fn layout_little(l: &Little) {
        let size = l.window.inner_size().to_logical::<f64>(l.window.scale_factor());
        let _ = l.bar.set_bounds(Rect { position: LogicalPosition::new(0.0, 0.0).into(), size: LogicalSize::new(size.width, LITTLE_BAR_H).into() });
        let _ = l.content.set_bounds(Rect {
            position: LogicalPosition::new(0.0, LITTLE_BAR_H).into(),
            size: LogicalSize::new(size.width, (size.height - LITTLE_BAR_H).max(50.0)).into(),
        });
        let _ = l.content.set_visible(true);
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

    // Updates: remove the EXE left by a previous update, then check shortly after start and daily.
    updater::global().cleanup_previous();
    if global_settings.auto_update {
        let px = proxy.clone();
        rt_handle.spawn(async move {
            tokio::time::sleep(Duration::from_secs(20)).await;
            loop {
                check_for_updates().await;
                let _ = px.send_event(AppEvent::RefreshChrome);
                tokio::time::sleep(Duration::from_secs(24 * 3600)).await;
            }
        });
    }

    // Ad blocking: load the cached engine instantly, refresh the lists in the background when a week old.
    {
        let px = proxy.clone();
        rt_handle.spawn(async move {
            let _ = tokio::task::spawn_blocking(|| blocker::global().load_cache()).await;
            if blocker::global().is_stale() {
                refresh_filter_lists(false).await;
            }
            let _ = px.send_event(AppEvent::RefreshChrome);
        });
    }

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
    let push_chrome_state = move |window: &Window,
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
            adblock_status: blocker::global().status(),
            filters: blocker::global().view(),
            update_kind: update_kind_label().0.to_string(),
            update_label: update_kind_label().1,
            is_fullscreen,
            is_maximized: window.is_maximized(),
            import_sources: {
                if let (Ok(mut l), Some(e)) = (search_engine_label().lock(), global_settings.search_engines.get(global_settings.active_search_engine)) {
                    if *l != e.name {
                        *l = e.name.clone();
                    }
                }
                importer::ui_snapshot().0
            },
            pw_offer: passwords::global().offer.lock().map(|o| o.clone()).unwrap_or(None),
            passwords: passwords::global().with(|v| (v.views(), false)),
            pw_status: passwords::global().status.lock().map(|s| s.clone()).unwrap_or_default(),
            pw_csv_pending: passwords::global().csv_pending.load(std::sync::atomic::Ordering::Relaxed),
            force_dark_active: FORCE_DARK_AT_START.load(Ordering::Relaxed),
            snap_native: SNAP_NATIVE.load(Ordering::Relaxed),
            perm_prompt: permission_prompt(tabs.get(active_idx).map(|t| t.id)),
            permissions: permissions::global().lock().map(|s| s.entries()).unwrap_or_default(),
            tab_groups: live_groups(tabs),
            window_key: WINDOW_KEY.with(|k| k.get()),
            extensions: EXTENSIONS.with(|e| e.borrow().clone()),
            ext_notice: EXT_NOTICE.with(|n| n.borrow_mut().take()),
            other_windows: OTHER_WINDOWS.with(|w| w.borrow().clone()),
            name_group: NEW_GROUP_TO_NAME.with(|n| n.take()),
            active_reader_available: tabs.get(active_idx).map(|t| READER_AVAILABLE.with(|r| r.borrow().contains(&t.id))).unwrap_or(false),
            peek: PEEK.with(|p| {
                p.borrow().as_ref().map(|k| PeekView {
                    url: k.webview.url().unwrap_or_default(),
                    rect: PEEK_RECT.with(|r| r.get()),
                })
            }),
            import_status: importer::ui_snapshot().1,
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
                    group: tab_group(t.id),
                    favicon: FAVICONS.with(|f| f.borrow().get(&t.id).cloned().unwrap_or_default()),
                })
                .collect(),
            menu_note: MENU_NOTE.with(|n| n.borrow().clone()),
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

    // Fill the toolbar's extension buttons.
    let _ = proxy.send_event(AppEvent::ChromeIpc("EXT_REFRESH".into()));

    // ------------------------------------------------------------ Multiple windows
    let mut parked: Vec<ParkedWindow> = Vec::new();
    let mut current_key: u64 = 1;
    let mut focused_key: u64 = 1;
    let mut next_window_key: u64 = 2;

    let mut seen_guard_gen = state::guard_generation();
    event_loop.run(move |event, elwt, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::LoopDestroyed = event {
            // Shunno is quitting: FFmpeg / yt-dlp runs end now, not after the process is gone.
            downloader::end_all_tools();
            return;
        }

        // A site or browser setting changed: give every open tab (all windows, Little Shunno, Peek)
        // the new page script. It applies from each tab's next page load.
        if state::guard_generation() != seen_guard_gen {
            seen_guard_gen = state::guard_generation();
            let user_js = load_approved_user_scripts(&global_settings.approved_script_hashes);
            let script = build_content_init_script(&SitePolicy::default(), &global_settings, &ipc_token, &user_js);
            let mut views: Vec<(&WebView, usize)> = Vec::new();
            for t in tabs.iter().chain(parked.iter().flat_map(|p| p.tabs.iter())) {
                views.push((&t.webview, t.id));
            }
            for p in parked.iter().filter_map(|p| p.peek.as_ref()) {
                views.push((&p.webview, p.id));
            }
            if let Some(l) = little.as_ref() {
                views.push((&l.content, l.id));
            }
            for (wv, id) in views {
                install_content_guard(wv, id, &script, None);
            }
            PEEK.with(|p| {
                if let Some(p) = p.borrow().as_ref() {
                    install_content_guard(&p.webview, p.id, &script, None);
                }
            });
        }

        // Swap another window's state into the loop's locals (and the current one out).
        macro_rules! swap_window {
            ($p:expr) => {{
                let p: &mut ParkedWindow = $p;
                std::mem::swap(&mut window, &mut p.window);
                std::mem::swap(&mut chrome_wv, &mut p.chrome_wv);
                std::mem::swap(&mut tabs, &mut p.tabs);
                std::mem::swap(&mut active_tab_idx, &mut p.active_tab_idx);
                std::mem::swap(&mut split_tab_idx, &mut p.split_tab_idx);
                std::mem::swap(&mut drawer_open, &mut p.drawer_open);
                std::mem::swap(&mut engine_menu_open, &mut p.engine_menu_open);
                std::mem::swap(&mut palette_open, &mut p.palette_open);
                std::mem::swap(&mut is_fullscreen, &mut p.is_fullscreen);
                std::mem::swap(&mut find_current, &mut p.find_current);
                std::mem::swap(&mut find_total, &mut p.find_total);
                std::mem::swap(&mut site_data_cleared, &mut p.site_data_cleared);
                std::mem::swap(&mut current_key, &mut p.key);
                p.chrome_last_h = CHROME_LAST_H.with(|c| c.replace(p.chrome_last_h));
                p.tab_flow = swap_tab_flow(std::mem::take(&mut p.tab_flow));
                PEEK.with(|k| std::mem::swap(&mut *k.borrow_mut(), &mut p.peek));
                p.peek_rect = PEEK_RECT.with(|c| c.replace(p.peek_rect));
            }};
        }
        macro_rules! switch_to {
            ($k:expr) => {{
                let k: u64 = $k;
                if k != current_key {
                    if let Some(i) = parked.iter().position(|p| p.key == k) {
                        swap_window!(&mut parked[i]);
                    }
                }
            }};
        }
        // Every non-incognito tab of every window, for the saved session (this window first).
        macro_rules! session_tabs {
            () => {{
                let mut v: Vec<SavedTab> = Vec::new();
                let all = std::iter::once(&tabs).chain(parked.iter().map(|p| &p.tabs));
                for (w, ts) in all.enumerate() {
                    for t in ts.iter().filter(|t| !t.incognito) {
                        v.push(SavedTab { title: t.title.clone(), url: t.url.clone(), group: tab_group(t.id), window: w as u32 });
                    }
                }
                v
            }};
        }
        // A new, empty browser window (made current; the caller adds tabs).
        macro_rules! open_window {
            ($pos:expr) => {{
                let key = next_window_key;
                next_window_key += 1;
                let size = window.inner_size().to_logical::<f64>(window.scale_factor());
                let mut wb = WindowBuilder::new()
                    .with_title(window_title_for_profile(&BrowserStorage::load_profiles()))
                    .with_decorations(false)
                    .with_window_icon(load_embedded_window_icon())
                    .with_inner_size(LogicalSize::new(size.width.max(640.0), size.height.max(420.0)))
                    .with_min_inner_size(LogicalSize::new(MIN_WINDOW_W, MIN_WINDOW_H));
                let pos: Option<(f64, f64)> = $pos;
                match pos {
                    Some((x, y)) => wb = wb.with_position(tao::dpi::PhysicalPosition::new(x, y)),
                    None => {
                        if let Ok(p) = window.outer_position() {
                            wb = wb.with_position(tao::dpi::PhysicalPosition::new(p.x + 36, p.y + 36));
                        }
                    }
                }
                match wb.build(elwt) {
                    Ok(nw) => {
                        let cp = proxy.clone();
                        #[allow(unused_mut)]
                        let mut cb = WebViewBuilder::new_as_child(&nw);
                        #[cfg(windows)]
                        {
                            use wry::WebViewBuilderExtWindows;
                            cb = cb.with_additional_browser_args(state::browser_args(FORCE_DARK_AT_START.load(Ordering::Relaxed)));
                        }
                        let built = cb
                            .with_web_context(&mut web_context)
                            .with_bounds(Rect { position: LogicalPosition::new(0.0, 0.0).into(), size: LogicalSize::new(size.width, CHROME_BAR_HEIGHT).into() })
                            .with_transparent(true)
                            .with_html(ui_chrome::CHROME_HTML)
                            .with_ipc_handler(move |req| {
                                let _ = cp.send_event(AppEvent::ChromeIpcW(key, req.body().clone()));
                            })
                            .build();
                        match built {
                            Ok(nc) => {
                                install_native_win32_window_features(&nw, &nc);
                                set_frame_theme(&nw, global_settings.full_dark_mode);
                                #[cfg(windows)]
                                unsafe {
                                    use tao::platform::windows::WindowExtWindows;
                                    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_STYLE, WS_MAXIMIZEBOX};
                                    use wry::WebViewExtWindows;
                                    let mut host = windows::Win32::Foundation::HWND::default();
                                    if nc.controller().ParentWindow(&mut host).is_ok() {
                                        set_chrome_host(nw.hwnd() as isize, host.0);
                                    }
                                    if SNAP_NATIVE.load(Ordering::Relaxed) {
                                        let h = windows::Win32::Foundation::HWND(nw.hwnd() as isize);
                                        let st = GetWindowLongPtrW(h, GWL_STYLE);
                                        SetWindowLongPtrW(h, GWL_STYLE, st | WS_MAXIMIZEBOX.0 as isize);
                                    }
                                }
                                attach_f11_accelerator_hook(&nc, proxy.clone());
                                parked.push(ParkedWindow {
                                    key,
                                    window: nw,
                                    chrome_wv: nc,
                                    tabs: Vec::new(),
                                    active_tab_idx: 0,
                                    split_tab_idx: None,
                                    drawer_open: false,
                                    engine_menu_open: false,
                                    palette_open: false,
                                    is_fullscreen: false,
                                    find_current: 0,
                                    find_total: 0,
                                    site_data_cleared: false,
                                    chrome_last_h: 0.0,
                                    peek: None,
                                    peek_rect: [0.0; 4],
                                    tab_flow: TabFlow::default(),
                                });
                                switch_to!(key);
                                focused_key = key;
                                true
                            }
                            Err(_) => false,
                        }
                    }
                    Err(_) => false,
                }
            }};
        }
        // Move a live tab's web view under the current window (no reload).
        macro_rules! adopt_webview {
            ($wv:expr) => {{
                #[cfg(windows)]
                unsafe {
                    use tao::platform::windows::WindowExtWindows;
                    use windows::Win32::UI::WindowsAndMessaging::SetParent;
                    use wry::WebViewExtWindows;
                    let mut host = windows::Win32::Foundation::HWND::default();
                    if $wv.controller().ParentWindow(&mut host).is_ok() {
                        let _ = SetParent(host, windows::Win32::Foundation::HWND(window.hwnd() as isize));
                    }
                }
            }};
        }
        // Close the current window (others stay open): its tabs go to Recently Closed.
        macro_rules! close_current_window {
            () => {{
                let _ = close_peek();
                for t in tabs.drain(..) {
                    if !t.incognito && t.url != "shunno://newtab" {
                        closed_tabs.push(SavedTab { title: t.title.clone(), url: t.url.clone(), group: None, window: 0 });
                    }
                    set_tab_group(t.id, None);
                }
                #[cfg(windows)]
                {
                    use tao::platform::windows::WindowExtWindows;
                    forget_window_native(window.hwnd() as isize);
                }
                little::hotkey_owner_closed();
                let mut next = parked.pop().expect("another window");
                swap_window!(&mut next);
                drop(next);
                focused_key = current_key;
                window.set_focus();
                #[cfg(windows)]
                {
                    use tao::platform::windows::WindowExtWindows;
                    little::publish_main_window(window.hwnd() as isize);
                    little::register_hotkey(window.hwnd() as isize);
                }
                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                let _ = proxy.send_event(AppEvent::RefreshChrome);
            }};
        }

        // Route the event to its window.
        if let Event::WindowEvent { window_id, .. } = &event {
            let ours = *window_id == window.id()
                || parked.iter().any(|p| p.window.id() == *window_id)
                || little.as_ref().map(|l| l.window.id()) == Some(*window_id);
            if !ours {
                return;
            }
        }
        let event = match event {
            Event::UserEvent(AppEvent::ChromeIpcW(k, m)) => {
                switch_to!(k);
                Event::UserEvent(AppEvent::ChromeIpc(m))
            }
            e => {
                let target = match &e {
                    Event::WindowEvent { window_id, .. } => {
                        if *window_id == window.id() {
                            Some(current_key)
                        } else {
                            parked.iter().find(|p| p.window.id() == *window_id).map(|p| p.key)
                        }
                    }
                    Event::UserEvent(ue) => Some(
                        event_tab_id(ue)
                            .and_then(|t| {
                                if tabs.iter().any(|x| x.id == t) {
                                    Some(current_key)
                                } else {
                                    parked.iter().find(|p| p.tabs.iter().any(|x| x.id == t)).map(|p| p.key)
                                }
                            })
                            .unwrap_or(focused_key),
                    ),
                    _ => None,
                };
                if let Some(k) = target {
                    switch_to!(k);
                }
                e
            }
        };
        // Per-window values the toolbar snapshot reads; recomputed whenever another window is swapped in.
        macro_rules! refresh_window_locals {
            () => {{
                WINDOW_KEY.with(|k| k.set(current_key));
                OTHER_WINDOWS.with(|w| {
                    *w.borrow_mut() = parked
                        .iter()
                        .map(|p| WindowView { key: p.key, title: p.tabs.get(p.active_tab_idx).map(|t| t.title.clone()).unwrap_or_default() })
                        .collect()
                });
                OTHER_TAB_IDS.with(|o| *o.borrow_mut() = parked.iter().flat_map(|p| p.tabs.iter().map(|t| t.id)).collect());
            }};
        }
        refresh_window_locals!();

        // Little Shunno's own window events never reach the main window's handlers.
        if let Event::WindowEvent { window_id, event: ref wev, .. } = event {
            if little.as_ref().map(|l| l.window.id()) == Some(window_id) {
                match wev {
                    WindowEvent::CloseRequested => {
                        little = None;
                        sweep_permissions(&tabs, &[]);
                    }
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                        if let Some(l) = little.as_ref() {
                            layout_little(l);
                        }
                    }
                    _ => {}
                }
                return;
            }
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                // Keeps the maximize/restore icon and corner radius in sync (state push is de-duplicated).
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

            Event::WindowEvent {
                event: WindowEvent::Focused(true),
                ..
            } => {
                // Remember this profile as the one in use (links and Ctrl+Alt+N follow it),
                // and take Ctrl+Alt+N if the window that held it has closed.
                BrowserStorage::save_last_profile(state::profile_id());
                // This window is now the one shortcuts, links and new tabs go to.
                let changed = focused_key != current_key;
                focused_key = current_key;
                #[cfg(windows)]
                {
                    use tao::platform::windows::WindowExtWindows;
                    little::register_hotkey(window.hwnd() as isize);
                    if changed {
                        little::publish_main_window(window.hwnd() as isize);
                    }
                }
                if changed {
                    let _ = proxy.send_event(AppEvent::RefreshChrome);
                }
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
                if !parked.is_empty() {
                    close_current_window!();
                    return;
                }
                let to_save: Vec<SavedTab> = session_tabs!();
                BrowserStorage::save_session(&to_save);
                save_groups();
                *control_flow = ControlFlow::Exit;
            }

            Event::UserEvent(AppEvent::PermissionRequest(id)) => {
                #[cfg(windows)]
                {
                    // Private? Answer now if a saved decision or a default applies; otherwise ask.
                    let found = PERM_PENDING.with(|q| {
                        let mut q = q.borrow_mut();
                        let p = q.iter_mut().find(|p| p.id == id)?;
                        let main_tab = tabs.iter().find(|t| t.id == p.tab_id);
                        p.private = match main_tab {
                            Some(t) => t.incognito,
                            None => PEEK.with(|k| k.borrow().as_ref().filter(|k| k.id == p.tab_id).map(|k| k.incognito)).unwrap_or(false),
                        };
                        Some((p.origin.clone(), p.kind, p.private))
                    });
                    if let Some((origin, kind, private)) = found {
                        let saved = permissions::global().lock().ok().and_then(|s| s.get(&origin, kind));
                        if let Some(d) = permissions::resolve(saved, kind, private) {
                            answer_permission(id, d, false);
                            return;
                        }
                    }
                }
                if let Some(l) = little.as_ref() {
                    little_sync_permission(l);
                }
                let _ = proxy.send_event(AppEvent::RefreshChrome);
            }

            Event::UserEvent(AppEvent::SleepCheck) => {
                sweep_permissions(&tabs, &little.as_ref().map(|l| vec![l.id]).unwrap_or_default());
                // Every window's tabs sleep and archive, not only the focused window's.
                let keys: Vec<u64> = std::iter::once(current_key).chain(parked.iter().map(|p| p.key)).collect();
                for key in keys {
                switch_to!(key);
                refresh_window_locals!();
                let now = now_epoch_secs();
                let mut changed = false;
                // Visible tabs are "in use": keep their clock fresh so switching away starts the idle timer.
                for (idx, tab) in tabs.iter_mut().enumerate() {
                    if idx == active_tab_idx || split_tab_idx == Some(idx) {
                        tab.last_active_secs = now;
                    }
                }
                // Arc-style auto-archive: close long-unused tabs; they stay reopenable (Ctrl+Shift+T) and in history.
                let probes: Vec<state::ArchiveProbe> = tabs
                    .iter()
                    .enumerate()
                    .map(|(idx, t)| state::ArchiveProbe {
                        last_active_secs: t.last_active_secs,
                        pinned: t.pinned,
                        playing_audio: t.playing_audio,
                        visible: idx == active_tab_idx || split_tab_idx == Some(idx),
                        incognito: t.incognito,
                    })
                    .collect();
                let doomed = state::tabs_to_archive(now, global_settings.auto_archive_hours, &probes);
                if !doomed.is_empty() {
                    let active_id = tabs.get(active_tab_idx).map(|t| t.id);
                    let split_id = split_tab_idx.and_then(|i| tabs.get(i)).map(|t| t.id);
                    for idx in doomed {
                        let removed = tabs.remove(idx);
                        set_tab_group(removed.id, None);
                        if removed.url != "shunno://newtab" {
                            closed_tabs.push(SavedTab { title: removed.title, url: removed.url, group: None, window: 0 });
                        }
                    }
                    active_tab_idx = active_id.and_then(|id| tabs.iter().position(|t| t.id == id)).unwrap_or(0);
                    split_tab_idx = split_id.and_then(|id| tabs.iter().position(|t| t.id == id));
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                    changed = true;
                }
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
                switch_to!(focused_key);
                refresh_window_locals!();
            }

            Event::UserEvent(AppEvent::ToggleFullscreen) => {
                if last_fs_toggle.elapsed() < Duration::from_millis(300) {
                    return;
                }
                last_fs_toggle = std::time::Instant::now();
                is_fullscreen = !is_fullscreen;
                // Smooth transition: the window glides to its new frame in about a quarter second,
                // the pages re-laid out with it every frame (no jump, no blank strip), then the
                // final frame is set exactly. Skipped when Windows animation effects are off.
                #[cfg(windows)]
                if let Some((from, to)) = fullscreen_motion(&window, is_fullscreen) {
                    const DURATION: f64 = 0.26;
                    let started = std::time::Instant::now();
                    loop {
                        let t = (started.elapsed().as_secs_f64() / DURATION).min(1.0);
                        fullscreen_frame(&window, from, to, t);
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                        if t >= 1.0 {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(8));
                    }
                }
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

            Event::UserEvent(AppEvent::TabTitleChanged(tab_id, new_title)) if little.as_ref().map(|l| l.id) == Some(tab_id) => {
                if let Some(l) = little.as_ref() {
                    let t = new_title.trim();
                    l.window.set_title(&if t.is_empty() { "Little Shunno".to_string() } else { format!("{t} \u{2014} Little Shunno") });
                    let u = l.content.url().unwrap_or_default();
                    let _ = l.bar.evaluate_script(&format!("window.__setUrl({})", serde_json::Value::String(u)));
                }
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
                // The picker belongs to the page it was started on.
                pending_picks.remove(&tab_id);
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

            Event::UserEvent(AppEvent::TabFavicon(tab_id, url)) => {
                let changed = FAVICONS.with(|f| {
                    let mut f = f.borrow_mut();
                    if f.len() > 400 {
                        // Long sessions: forget icons of tabs no longer in this window.
                        f.retain(|id, _| tabs.iter().any(|t| t.id == *id));
                    }
                    if url.is_empty() {
                        f.remove(&tab_id).is_some()
                    } else {
                        f.insert(tab_id, url.clone()).as_ref() != Some(&url)
                    }
                });
                if changed {
                    let _ = proxy.send_event(AppEvent::RefreshChrome);
                }
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

            Event::UserEvent(AppEvent::ImportDone(result)) => {
                match result {
                    Ok((browser, b, h)) => {
                        let nb = importer::merge_bookmarks(&mut bookmarks, b);
                        let nh = importer::merge_history(&mut history, h, state::HISTORY_CAP);
                        if nb > 0 {
                            BrowserStorage::save_bookmarks(&bookmarks);
                        }
                        if nh > 0 {
                            save_history_ordered(None, history.clone());
                        }
                        importer::set_status(format!(
                            "Imported {nb} bookmark{} and {nh} history entr{} from {browser}.",
                            if nb == 1 { "" } else { "s" },
                            if nh == 1 { "y" } else { "ies" }
                        ));
                    }
                    Err(e) => importer::set_status(e),
                }
                let _ = proxy.send_event(AppEvent::RefreshChrome);
            }

            Event::UserEvent(AppEvent::RaiseTab(tab_id)) => {
                if TAB_RAISE_PENDING.with(|c| c.get()) == Some(tab_id) {
                    TAB_RAISE_PENDING.with(|c| c.set(None));
                    TAB_RAISE_DONE.with(|c| c.set(Some(tab_id)));
                    if tabs.get(active_tab_idx).map(|t| t.id) == Some(tab_id) {
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                    }
                }
            }

            Event::UserEvent(AppEvent::StartPicker(tab_id)) => {
                if let Some(tab) = tabs.iter().find(|t| t.id == tab_id && t.url.starts_with("http")) {
                    pending_picks.insert(tab_id);
                    let _ = tab.webview.evaluate_script("window.__shunnoPicker && window.__shunnoPicker(true)");
                }
            }

            Event::UserEvent(AppEvent::SearchSelection(source_tab, query)) => {
                // Opens next to the source like a link; a private tab's search stays private.
                let url = sanitizer::search_url(&query, &global_settings);
                let _ = proxy.send_event(AppEvent::NewWindowBlockedOrOpened(source_tab, url));
            }

            Event::UserEvent(AppEvent::PwAuthorized(action, id, ok, unavailable)) => {
                let vault = passwords::global();
                if !ok {
                    vault.set_status("Not verified.");
                } else {
                    vault.unlock_for(120);
                    if let Some(c) = vault.with(|v| (v.get(id).cloned(), false)) {
                        if action == "reveal" {
                            let _ = chrome_wv.evaluate_script(&format!("window.__pwReveal && window.__pwReveal({}, {});", id, serde_json::Value::String(c.password)));
                            vault.set_status(if unavailable { "Tip: set up Windows Hello (Settings > Accounts > Sign-in options) so passwords need your PIN or face to show." } else { "" });
                        } else if passwords::set_clipboard(&c.password) {
                            vault.set_status(if unavailable { "Password copied. Tip: set up Windows Hello to protect this." } else { "Password copied." });
                        }
                    }
                }
                let _ = proxy.send_event(AppEvent::RefreshChrome);
            }

            Event::UserEvent(AppEvent::PageShortcut(id)) => {
                // Run it through the toolbar's one shortcut handler; shortcuts that type into the
                // toolbar (address bar, palette, find) need it to have keyboard focus.
                if matches!(id, "focus-url" | "palette" | "find" | "new-tab" | "bookmark") {
                    let _ = chrome_wv.focus();
                }
                let _ = chrome_wv.evaluate_script(&format!("window.__shunnoShortcut && window.__shunnoShortcut('{id}')"));
            }

            Event::UserEvent(AppEvent::TakeoverCandidate { native_id, tab_id, url, file_name, total }) => {
                // IDM-style takeover: only normal tabs, only files worth accelerating.
                const MIN_TAKEOVER_BYTES: u64 = 2 * 1024 * 1024;
                if !global_settings.download_takeover || total.map(|t| t < MIN_TAKEOVER_BYTES).unwrap_or(false) {
                    return;
                }
                let Some(tab) = tabs.iter().find(|t| t.id == tab_id && !t.incognito) else { return };
                let referer = tab.webview.url().ok().filter(|u| u.starts_with("http"));
                let px = proxy.clone();
                let url_c = url.clone();
                collect_cookie_header(
                    &tab.webview,
                    &url,
                    Box::new(move |cookie| {
                        let _ = px.send_event(AppEvent::TakeoverCookies { native_id, url: url_c, referer, cookie, file_name, total });
                    }),
                );
                return;
            }

            Event::UserEvent(AppEvent::TakeoverCookies { native_id, url, referer, cookie, file_name, total }) => {
                let eng = engine.clone();
                let px = proxy.clone();
                rt_handle.spawn(async move {
                    let ok = eng.probe_takeover(&url, referer.clone(), cookie.clone(), total).await.is_some();
                    let _ = px.send_event(AppEvent::TakeoverDecision { native_id, url, referer, cookie, file_name, ok });
                });
                return;
            }

            Event::UserEvent(AppEvent::TakeoverDecision { native_id, url, referer, cookie, file_name, ok }) => {
                if !ok {
                    return; // WebView2 keeps the download.
                }
                // Still running in WebView2? (A fast download may already be finished: keep it.)
                let Some(op) = NATIVE_OPS.with(|ops| ops.borrow_mut().remove(&native_id)) else { return };
                #[cfg(windows)]
                unsafe {
                    let _ = op.Cancel();
                }
                #[cfg(not(windows))]
                let _ = op;
                engine.delete(native_id);
                // "UNIQUE": no duplicate prompt and no forced rename; a taken name becomes "name (1).ext" at the end.
                let _ = engine.enqueue_named(&rt_handle, url, global_settings.default_threads, referer, cookie, "UNIQUE", None, Some(file_name));
            }

            Event::UserEvent(AppEvent::ExternalOpen(raw)) => {
                // Any program on the desktop can send this message: accept only what a command line
                // may carry (http(s), file:///, a local page path), never javascript: or shunno:.
                let url = if raw.is_empty() || raw == little::LITTLE_PAYLOAD {
                    raw
                } else {
                    match little::url_from_arg(&raw) {
                        Some(u) => u,
                        None => return,
                    }
                };
                if url == little::LITTLE_PAYLOAD {
                    let _ = proxy.send_event(AppEvent::LittleOpen(None));
                } else if url.is_empty() {
                    window.set_minimized(false);
                    window.set_focus();
                } else if global_settings.little_for_links {
                    let _ = proxy.send_event(AppEvent::LittleOpen(Some(url)));
                } else {
                    window.set_minimized(false);
                    window.set_focus();
                    let _ = proxy.send_event(AppEvent::NewWindowBlockedOrOpened(0, url));
                }
                return;
            }

            Event::UserEvent(AppEvent::LittleOpen(url)) => {
                if let Some(l) = little.as_ref() {
                    if let Some(u) = url.as_deref() {
                        let _ = l.content.load_url(u);
                        let _ = l.bar.evaluate_script(&format!("window.__setUrl({})", serde_json::Value::String(u.to_string())));
                    } else {
                        let _ = l.bar.focus();
                        let _ = l.bar.evaluate_script("window.__focus()");
                    }
                    l.window.set_minimized(false);
                    l.window.set_focus();
                    return;
                }
                let lw = match WindowBuilder::new()
                    .with_title("Little Shunno")
                    .with_window_icon(load_embedded_window_icon())
                    .with_inner_size(LogicalSize::new(980.0, 700.0))
                    .with_min_inner_size(LogicalSize::new(480.0, 320.0))
                    .build(elwt)
                {
                    Ok(w) => w,
                    Err(_) => return,
                };
                let lproxy = proxy.clone();
                #[allow(unused_mut)]
                let mut bar_builder = WebViewBuilder::new_as_child(&lw);
                #[cfg(windows)]
                {
                    use wry::WebViewBuilderExtWindows;
                    bar_builder = bar_builder
                        .with_additional_browser_args(state::browser_args(FORCE_DARK_AT_START.load(Ordering::Relaxed)))
                        .with_theme(page_theme(global_settings.full_dark_mode));
                }
                let bar = match bar_builder
                    .with_web_context(&mut web_context)
                    .with_bounds(Rect { position: LogicalPosition::new(0.0, 0.0).into(), size: LogicalSize::new(980.0, LITTLE_BAR_H).into() })
                    .with_html(little::BAR_HTML)
                    .with_ipc_handler(move |req| {
                        let _ = lproxy.send_event(AppEvent::LittleIpc(req.body().clone()));
                    })
                    .build()
                {
                    Ok(b) => b,
                    Err(_) => return,
                };
                let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                let start = url.clone().unwrap_or_else(|| "about:blank".to_string());
                let pol = site_policies.get(&extract_host(&start)).cloned().unwrap_or_default();
                let Ok(content) = create_tab_webview(&lw, &mut web_context, tid, &start, &pol, &global_settings, &ipc_token, proxy.clone(), false) else { return };
                let _ = bar.evaluate_script(&format!("window.__setDark({}); window.__setUrl({});", global_settings.full_dark_mode, serde_json::Value::String(start.clone())));
                let l = Little { window: lw, bar, content, id: tid };
                layout_little(&l);
                if url.is_none() {
                    let _ = l.bar.focus();
                    let _ = l.bar.evaluate_script("window.__focus()");
                }
                l.window.set_focus();
                little = Some(l);
                return;
            }

            Event::UserEvent(AppEvent::LittleIpc(msg)) => {
                if msg == "OPEN" {
                    // Move the live page into the main window as a tab: no reload, so scroll position,
                    // half-filled forms, playing media and sign-ins carry over. Then close Little Shunno.
                    let Some(Little { window: lw, bar, content, id }) = little.take() else { return };
                    let u = content.url().unwrap_or_default();
                    if u.starts_with("http") {
                        #[cfg(windows)]
                        unsafe {
                            use tao::platform::windows::WindowExtWindows;
                            use windows::Win32::UI::WindowsAndMessaging::SetParent;
                            use wry::WebViewExtWindows;
                            let mut host = windows::Win32::Foundation::HWND::default();
                            if content.controller().ParentWindow(&mut host).is_ok() {
                                let _ = SetParent(host, windows::Win32::Foundation::HWND(window.hwnd() as isize));
                            }
                        }
                        tabs.push(BrowserTab {
                            id,
                            title: extract_host(&u),
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
                            webview: content,
                        });
                        active_tab_idx = tabs.len() - 1;
                        drop(bar);
                        drop(lw);
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                    } else {
                        drop(content);
                        drop(bar);
                        drop(lw);
                        let _ = proxy.send_event(AppEvent::ChromeIpc("NEW_TAB".into()));
                    }
                    window.set_minimized(false);
                    window.set_focus();
                    let _ = proxy.send_event(AppEvent::RefreshChrome);
                    return;
                }
                let Some(l) = little.as_ref() else { return };
                if let Some(text) = msg.strip_prefix("NAV:") {
                    if !text.trim().is_empty() {
                        let target = sanitize_url(text, &global_settings);
                        let _ = l.content.load_url(&target);
                        let _ = l.content.focus();
                    }
                } else if msg == "BACK" {
                    let _ = l.content.evaluate_script("history.back()");
                } else if let Some(arg) = msg.strip_prefix("PERM:") {
                    if let Some((id, d)) = arg.split_once('|') {
                        if let Ok(id) = id.parse::<u64>() {
                            answer_permission(id, if d == "allow" { permissions::Decision::Allow } else { permissions::Decision::Block }, true);
                        }
                    }
                    if let Some(l) = little.as_ref() {
                        little_sync_permission(l);
                    }
                } else if msg == "CLOSE" {
                    little = None;
                    sweep_permissions(&tabs, &[]);
                }
                return;
            }

            Event::UserEvent(AppEvent::ExtensionsListed(list)) => {
                let reg = extensions::load_registry();
                EXTENSIONS.with(|e| *e.borrow_mut() = extensions::entries(&list, &reg));
                let _ = proxy.send_event(AppEvent::RefreshChrome);
            }

            Event::UserEvent(AppEvent::ExtStoreReady(res)) => {
                match res {
                    Ok((dir, meta)) => ext_install(&chrome_wv, proxy.clone(), dir, meta),
                    Err(e) => {
                        let _ = proxy.send_event(AppEvent::ExtensionNotice(e));
                    }
                }
            }

            Event::UserEvent(AppEvent::ExtensionNotice(text)) => {
                EXT_NOTICE.with(|n| *n.borrow_mut() = Some(text));
                let _ = proxy.send_event(AppEvent::RefreshChrome);
            }

            Event::UserEvent(AppEvent::RestoreWindow(list)) => {
                RESTORES_PENDING.with(|r| r.set(r.get().saturating_sub(1)));
                if open_window!(None) {
                    for (u, grp) in list {
                        let host = extract_host(&u);
                        let pol = site_policies.get(&host).cloned().unwrap_or_default();
                        let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                        set_tab_group(tid, grp);
                        if let Ok(wv) = create_tab_webview(&window, &mut web_context, tid, &u, &pol, &global_settings, &ipc_token, proxy.clone(), false) {
                            let title = if u == "shunno://newtab" { "New Tab".to_string() } else if host.is_empty() { u.clone() } else { host };
                            tabs.push(BrowserTab {
                                id: tid,
                                title,
                                url: u,
                                cookie: None,
                                incognito: false,
                                pinned: false,
                                playing_audio: false,
                                muted: false,
                                reader_active: false,
                                sleeping: false,
                                last_active_secs: now_epoch_secs(),
                                zoom: pol.zoom,
                                webview: wv,
                            });
                        }
                    }
                    if tabs.is_empty() {
                        let _ = proxy.send_event(AppEvent::ChromeIpc("NEW_TAB".into()));
                    }
                    active_tab_idx = 0;
                    for (i, t) in tabs.iter().enumerate() {
                        if i != active_tab_idx {
                            let _ = t.webview.set_visible(false);
                            TAB_APPLIED.with(|m| { m.borrow_mut().remove(&t.id); });
                        }
                    }
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                    let _ = proxy.send_event(AppEvent::RefreshChrome);
                }
            }

            Event::UserEvent(AppEvent::SystemThemeChanged) => {
                if global_settings.theme_mode == "system" {
                    let dark = system_prefers_dark();
                    if dark != global_settings.full_dark_mode {
                        global_settings.full_dark_mode = dark;
                        apply_theme(&window, &tabs, dark);
                        for p in &parked {
                            apply_theme(&p.window, &p.tabs, dark);
                        }
                        let _ = proxy.send_event(AppEvent::RefreshChrome);
                    }
                }
            }

            Event::UserEvent(AppEvent::AdBlocked) => {
                blocked_count += 1;
            }

            Event::UserEvent(AppEvent::NavigateTab(tab_id, target)) => {
                if let Some(tab) = tabs.iter().find(|t| t.id == tab_id) {
                    let _ = tab.webview.load_url(&target);
                }
            }

            Event::UserEvent(AppEvent::NavigationDone(tab_id, success, cancelled)) => {
                if let Some(insecure) = https_guard::global().on_navigation_done(tab_id, success, cancelled) {
                    let _ = proxy.send_event(AppEvent::HttpsInterstitial(tab_id, insecure));
                }
            }

            Event::UserEvent(AppEvent::HttpsInterstitial(tab_id, insecure)) => {
                if let (Some(tab), Some(allow)) = (
                    tabs.iter().find(|t| t.id == tab_id),
                    https_guard::global().issue_allow_link(&insecure),
                ) {
                    show_html_in_tab(&tab.webview, &https_guard::interstitial_html(&insecure, &allow));
                }
            }

            Event::UserEvent(AppEvent::DownloadIntercepted(src_tab, dl_url)) => {
                // Referer and cookies of the page that was right-clicked (split view, Little Shunno),
                // not of whichever tab happens to be active.
                let little_tab = little.as_ref().filter(|l| l.id == src_tab).map(|l| &l.content);
                let active_tab = tabs.iter().find(|t| t.id == src_tab).or_else(|| if little_tab.is_some() { None } else { tabs.get(active_tab_idx) });
                if let Some(wv) = little_tab {
                    let referer = wv.url().ok().filter(|u| u.starts_with("http"));
                    dispatch_download_with_httponly_cookies(Some(wv), dl_url, global_settings.default_threads, referer, None, "CHECK", None, true, proxy.clone());
                    return;
                }
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
                // A pop-up from a private Peek must stay private too.
                let opener_private = tabs
                    .iter()
                    .find(|t| t.id == opener_id)
                    .map(|t| t.incognito)
                    .or_else(|| PEEK.with(|p| p.borrow().as_ref().filter(|k| k.id == opener_id).map(|k| k.incognito)))
                    .unwrap_or(false);
                // Match ad networks by host only, so a search query or path mentioning them is not blocked.
                let lower = extract_host(&popup_url).to_ascii_lowercase();
                let is_ad_popup = lower.contains("popads")
                    || lower.contains("popcash")
                    || lower.contains("doubleclick")
                    || lower.contains("propellerads");
                if global_settings.block_popups && is_ad_popup {
                    blocked_count += 1;
                } else if popup_url.starts_with("http") || (opener_id == 0 && popup_url.starts_with("file:///")) {
                    // file:/// only from outside (Explorer, the command line), never from a page.
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
                            zoom: pol.zoom,
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

                if let Some(target) = msg.strip_prefix("PEEK:") {
                    let target = target.trim();
                    let from_peek = peek_id() == Some(tab_id);
                    if global_settings.peek_links && (target.starts_with("https://") || target.starts_with("http://")) {
                        let clean = sanitize_url(target, &global_settings);
                        if from_peek || peek_open() {
                            // Shift+click inside a Peek just navigates the Peek.
                            PEEK.with(|p| {
                                if let Some(k) = p.borrow().as_ref() {
                                    let _ = k.webview.load_url(&clean);
                                }
                            });
                        } else if let Some(src) = tabs.iter().find(|t| t.id == tab_id) {
                            let incognito = src.incognito;
                            let pol = site_policies.get(&extract_host(&clean)).cloned().unwrap_or_default();
                            let pid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                            if let Ok(wv) = create_tab_webview(&window, &mut web_context, pid, &clean, &pol, &global_settings, &ipc_token, proxy.clone(), incognito) {
                                drawer_open = false;
                                engine_menu_open = false;
                                palette_open = false;
                                PEEK.with(|p| *p.borrow_mut() = Some(Peek { id: pid, webview: wv, incognito }));
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            }
                        }
                    }
                } else if let Some(json) = msg.strip_prefix("PW_OFFER:") {
                    // Never offer in private tabs or when the manager is off.
                    if let (true, Some((origin, false))) = (global_settings.save_passwords, tab_origin(&tabs, tab_id)) {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
                            let user = v.get("u").and_then(|x| x.as_str()).unwrap_or("").chars().take(256).collect::<String>();
                            let pass = v.get("p").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            if pass.len() <= 1024 {
                                if let Some(kind) = passwords::global().with(|vd| (vd.offer_kind(&origin, &user, &pass), false)) {
                                    if let Ok(mut o) = passwords::global().offer.lock() {
                                        *o = Some(passwords::Offer { origin, username: user, password: pass, kind });
                                    }
                                }
                            }
                        }
                    }
                } else if msg == "PW_QUERY" {
                    if !global_settings.save_passwords {
                        return;
                    }
                    let Some((origin, _)) = tab_origin(&tabs, tab_id) else { return };
                    let list = passwords::global().with(|v| (v.accounts_for(&origin), false));
                    if list.is_empty() {
                        return;
                    }
                    let arr: Vec<serde_json::Value> = list.into_iter().map(|(id, u)| serde_json::json!({ "id": id, "u": u })).collect();
                    eval_in_tab(&tabs, tab_id, &format!("window.__shunnoPwAccounts && window.__shunnoPwAccounts({});", serde_json::Value::Array(arr)));
                    return;
                } else if let Some(id) = msg.strip_prefix("PW_FILL:") {
                    // Re-check the site at fill time: the page may have navigated since the menu opened.
                    let (Ok(id), Some((origin, _))) = (id.parse::<u64>(), tab_origin(&tabs, tab_id)) else { return };
                    let cred = passwords::global().with(|v| (v.get(id).cloned(), false));
                    if let Some(c) = cred.filter(|c| passwords::same_site(&c.origin, &origin)) {
                        let js = format!(
                            "window.__shunnoPwFill && window.__shunnoPwFill({}, {});",
                            serde_json::Value::String(c.username),
                            serde_json::Value::String(c.password)
                        );
                        eval_in_tab(&tabs, tab_id, &js);
                    }
                    return;
                } else if msg == "KEY_ESC" {
                    if peek_id() == Some(tab_id) && close_peek() {
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                    } else {
                        return;
                    }
                } else if msg == "AD_BLOCKED" {
                    blocked_count += 1;
                } else if let Some(fc) = msg.strip_prefix("FIND_COUNT:") {
                    if let Some((cur_s, tot_s)) = fc.split_once('|') {
                        find_current = cur_s.parse().unwrap_or(0);
                        find_total = tot_s.parse().unwrap_or(0);
                    }
                } else if msg.starts_with("SITE_DATA_CLEARED:") {
                    site_data_cleared = true;
                } else if let Some(flag) = msg.strip_prefix("READER_AVAILABLE:") {
                    let changed = READER_AVAILABLE.with(|r| {
                        let mut r = r.borrow_mut();
                        if flag == "1" { r.insert(tab_id) } else { r.remove(&tab_id) }
                    });
                    if !changed || tabs.get(active_tab_idx).map(|t| t.id) != Some(tab_id) {
                        return;
                    }
                } else if let Some(json) = msg.strip_prefix("READER_PREFS:") {
                    if let Some(p) = state::valid_reader_prefs(json) {
                        global_settings.reader_prefs = p;
                        BrowserStorage::save_global_settings(&global_settings);
                    }
                    return;
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
                        // "page-audio" / "page-flac": the same, extracting audio only.
                        let via = val.get("via").and_then(|v| v.as_str()).unwrap_or("");
                        let route_prefix = match via {
                            "page" => Some(downloader::YTDLP_PREFIX),
                            "page-audio" => Some(downloader::YTDLP_AUDIO_PREFIX),
                            "page-flac" => Some(downloader::YTDLP_FLAC_PREFIX),
                            _ => None,
                        };
                        let referer = val.get("referer").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let cookie = val.get("cookie").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
                        if dl_url.starts_with("http") && page_may_target(page_url.as_deref(), &dl_url) {
                            let wv_opt = tabs.iter().find(|t| t.id == tab_id).map(|t| &t.webview);
                            let routed = match route_prefix { Some(p) => format!("{p}{dl_url}"), None => dl_url };
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
                            if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                                tab.zoom = (tab.zoom + 0.1).min(3.0);
                                apply_tab_zoom(&tab.webview, tab.zoom);
                            }
                        }
                        "zoom_out" => {
                            if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                                tab.zoom = (tab.zoom - 0.1).max(0.4);
                                apply_tab_zoom(&tab.webview, tab.zoom);
                            }
                        }
                        "zoom_reset" => {
                            if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                                tab.zoom = 1.0;
                                apply_tab_zoom(&tab.webview, tab.zoom);
                            }
                        }
                        "f11" => {
                            let _ = proxy.send_event(AppEvent::ToggleFullscreen);
                        }
                        // Same path as the native accelerator hook, so groups, split view,
                        // Recently Closed and multiple windows are handled in one place.
                        "t" | "w" | "j" | "k" | "next_tab" | "prev_tab" => {
                            let id: &'static str = match key_cmd {
                                "t" => "new-tab",
                                "w" => "close-tab",
                                "j" => "downloads",
                                "k" => "palette",
                                "next_tab" => "next-tab",
                                _ => "prev-tab",
                            };
                            let _ = proxy.send_event(AppEvent::PageShortcut(id));
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
                        // Never overwrite an existing file; mark it as from the Internet (it is the page's own HTML).
                        if let Ok(out) = downloader::reserve_unique_path(&BrowserStorage::downloads_root_dir(), &safe_name) {
                            if fs::write(&out, html).is_ok() {
                                downloader::mark_of_the_web(&out, page_url.as_deref().unwrap_or(""), None);
                            } else {
                                let _ = fs::remove_file(&out);
                            }
                        }
                    }
                } else if let Some(grab_json) = msg.strip_prefix("GRAB_RESULTS:") {
                    // Only right after the user pressed Grab for this tab (a page could call the grab itself).
                    if !pending_grabs.remove(&tab_id) {
                        return;
                    }
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
                    if stream_url.starts_with("http") && page_may_target(page_url.as_deref(), stream_url) && !sniffed_streams.iter().any(|s| s.url == stream_url) {
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
                } else if let Some(pick_json) = msg.strip_prefix("PICK_FILTER:") {
                    // Only after the user started the picker in this tab, and only for this tab's own site.
                    if !pending_picks.remove(&tab_id) {
                        return;
                    }
                    let host = extract_host(page_url.as_deref().unwrap_or(""));
                    let sel = serde_json::from_str::<serde_json::Value>(pick_json)
                        .ok()
                        .and_then(|v| v.get("selector").and_then(|x| x.as_str()).map(str::to_string))
                        .unwrap_or_default();
                    match blocker::picker_rule(&host, &sel) {
                        Some(rule) => {
                            let added = blocker::global().add_user_rules(&rule);
                            blocker::global().set_notice(if added > 0 {
                                format!("Added to My Filters: {}", rule)
                            } else {
                                format!("Already in My Filters: {}", rule)
                            });
                            apply_filter_change(&rt_handle, &proxy);
                        }
                        None => blocker::global().set_notice("That element cannot be hidden with a rule.".to_string()),
                    }
                    return;
                } else if msg == "PICK_DONE" {
                    pending_picks.remove(&tab_id);
                    return;
                } else if let Some(cos_json) = msg.strip_prefix("COSMETIC:") {
                    // Element hiding for the top frame: page-specific rules plus generic rules for the
                    // classes/ids the page actually uses. Page input only narrows which list rules apply.
                    let tab_url = page_url.clone().unwrap_or_default();
                    let host = extract_host(&tab_url);
                    let adblock_on = site_policies.get(&host).map(|p| p.adblock).unwrap_or(true);
                    if adblock_on && tab_url.starts_with("http") {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(cos_json) {
                            let take = |k: &str| -> Vec<String> {
                                val.get(k)
                                    .and_then(|v| v.as_array())
                                    .map(|a| {
                                        a.iter()
                                            .filter_map(|x| x.as_str())
                                            .filter(|x| x.len() <= 120)
                                            .take(2000)
                                            .map(|x| x.to_string())
                                            .collect()
                                    })
                                    .unwrap_or_default()
                            };
                            let claimed = val.get("url").and_then(|v| v.as_str()).unwrap_or("");
                            let match_url = if extract_host(claimed) == host { claimed.to_string() } else { tab_url.clone() };
                            let css = blocker::global().cosmetic_css(&match_url, &take("classes"), &take("ids"));
                            if !css.is_empty() {
                                if let (Some(tab), Ok(arg)) = (tabs.iter().find(|t| t.id == tab_id), serde_json::to_string(&css)) {
                                    let _ = tab
                                        .webview
                                        .evaluate_script(&format!("window.__shunnoApplyCosmetic && window.__shunnoApplyCosmetic({});", arg));
                                }
                            }
                        }
                    }
                    return;
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
                                    history.truncate(state::HISTORY_CAP);
                                    save_history_ordered(Some(&rt_handle), history.clone());
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
                // Peek is transient: anything that changes the tab or opens a popover dismisses it.
                if peek_open()
                    && ["SWITCH_TAB:", "NAV:", "NAV_NEW_TAB:", "NEW_TAB", "NEW_INCOGNITO", "NEXT_TAB", "PREV_TAB", "CLOSE_TAB:",
                        "CLOSE_ACTIVE_TAB", "DRAWER_OPEN:", "PALETTE:", "TOGGLE_SPLIT_VIEW", "TOGGLE_FULLSCREEN"]
                        .iter()
                        .any(|p| cmd.starts_with(p))
                {
                    close_peek();
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                }
                if cmd == "PEEK_CLOSE" {
                    if close_peek() {
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                    }
                } else if cmd == "PEEK_OPEN" {
                    // Promote the live preview to a real tab (keeps scroll position, logins, form state).
                    if let Some(peek) = PEEK.with(|p| p.borrow_mut().take()) {
                        let url = peek.webview.url().unwrap_or_default();
                        tabs.push(BrowserTab {
                            id: peek.id,
                            title: extract_host(&url),
                            url,
                            cookie: None,
                            incognito: peek.incognito,
                            pinned: false,
                            playing_audio: false,
                            muted: false,
                            reader_active: false,
                            sleeping: false,
                            last_active_secs: now_epoch_secs(),
                            zoom: 1.0,
                            webview: peek.webview,
                        });
                        active_tab_idx = tabs.len() - 1;
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                    }
                } else if cmd == "WIN_DRAG" {
                    let _ = window.drag_window();
                } else if cmd == "WIN_MINIMIZE" {
                    window.set_minimized(true);
                } else if cmd == "WIN_MAXIMIZE" {
                    window.set_maximized(!window.is_maximized());
                } else if cmd == "WIN_CLOSE" {
                    if !parked.is_empty() {
                        close_current_window!();
                        return;
                    }
                    let to_save: Vec<SavedTab> = session_tabs!();
                    BrowserStorage::save_session(&to_save);
                    save_groups();
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
                        // New Tab "Add Favorite": a bookmark in Favorites (typed addresses are completed).
                        if clean_u.starts_with("http") && state::save_bookmark(&mut bookmarks, "", &clean_u, t, state::FAVORITES, now_epoch_secs()) {
                            BrowserStorage::save_bookmarks(&bookmarks);
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
                    if cmd == "DRAWER_OPEN:extensions" {
                        ext_refresh(&chrome_wv, proxy.clone());
                    }
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
                } else if let Some(raw) = cmd.strip_prefix("NAV_NEW_TAB:") {
                    // Bookmarks open in a new tab; an empty New Tab page is reused instead of left behind.
                    drawer_open = false;
                    engine_menu_open = false;
                    palette_open = false;
                    let reuse = tabs.get(active_tab_idx).map(|t| t.url == "shunno://newtab").unwrap_or(false);
                    if reuse {
                        let _ = proxy.send_event(AppEvent::ChromeIpc(format!("NAV:{raw}")));
                    } else {
                        // Opener id 0 belongs to no tab, so the new tab is never private.
                        let _ = proxy.send_event(AppEvent::NewWindowBlockedOrOpened(0, raw.to_string()));
                    }
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                } else if cmd == "NEW_TAB" || cmd == "NEW_INCOGNITO" {
                    let is_incog = cmd == "NEW_INCOGNITO";
                    // Draw the start page before the toolbar grows over the old page.
                    let _ = chrome_wv.evaluate_script("window.__showHomeNow && window.__showHomeNow()");
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
                        // The new (hidden) page must not keep keyboard focus: give it to the start page,
                        // so typing searches and the next Ctrl+T works.
                        let _ = chrome_wv.focus();
                        let _ = chrome_wv.evaluate_script("window.__focusHomeSearch && window.__focusHomeSearch()");
                    }
                } else if cmd == "NEXT_TAB" || cmd == "PREV_TAB" {
                    if !tabs.is_empty() {
                        let n = tabs.len();
                        let idx = if cmd == "NEXT_TAB" { (active_tab_idx + 1) % n } else { (active_tab_idx + n - 1) % n };
                        active_tab_idx = idx.min(tabs.len() - 1);
                        // As SWITCH_TAB: wake a sleeping tab, restart its idle clock, open its group.
                        if split_tab_idx == Some(active_tab_idx) {
                            split_tab_idx = None;
                        }
                        let t = &mut tabs[active_tab_idx];
                        if t.sleeping {
                            resume_tab_webview(&t.webview);
                            t.sleeping = false;
                        }
                        t.last_active_secs = now_epoch_secs();
                        if let Some(g) = tab_group(t.id) {
                            if group_mut(g, |x| std::mem::replace(&mut x.collapsed, false)) == Some(true) {
                                save_groups();
                            }
                        }
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
                                // Tab groups: dropped inside a group joins it; dropped away from its group leaves it.
                                if !tabs[to].pinned {
                                    let gs: Vec<Option<u32>> = tabs.iter().map(|t| tab_group(t.id)).collect();
                                    set_tab_group(tabs[to].id, state::group_after_drop(&gs, to));
                                    regroup_tabs(&mut tabs, &mut active_tab_idx, &mut split_tab_idx);
                                }
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            }
                        }
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("SWITCH_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < tabs.len() {
                            active_tab_idx = idx;
                            // A tab in a collapsed group opens its group.
                            if let Some(g) = tab_group(tabs[idx].id) {
                                if group_mut(g, |x| std::mem::replace(&mut x.collapsed, false)) == Some(true) {
                                    save_groups();
                                }
                            }
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
                    // One code path for closing (split view handled there).
                    let _ = proxy.send_event(AppEvent::ChromeIpc(format!("CLOSE_TAB:{active_tab_idx}")));
                    return;
                } else if let Some(idx_str) = cmd.strip_prefix("CLOSE_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if tabs.len() <= 1 && !parked.is_empty() {
                            // Last tab of one of several windows: close just that window.
                            close_current_window!();
                            return;
                        } else if tabs.len() <= 1 {
                            BrowserStorage::save_session(&[]);
                            *control_flow = ControlFlow::Exit;
                            return;
                        } else if idx < tabs.len() {
                            let removed = tabs.remove(idx);
                            set_tab_group(removed.id, None);
                            if !removed.incognito && removed.url != "shunno://newtab" {
                                closed_tabs.push(SavedTab { title: removed.title, url: removed.url, group: None, window: 0 });
                            }
                            // Keep the same tab active when a tab to its left closes.
                            if idx < active_tab_idx {
                                active_tab_idx -= 1;
                            }
                            split_tab_idx = match split_tab_idx {
                                Some(sp) if sp == idx => None,
                                Some(sp) if sp > idx => Some(sp - 1),
                                other => other,
                            };
                            if active_tab_idx >= tabs.len() {
                                active_tab_idx = tabs.len() - 1;
                            }
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                        }
                    }
                } else if cmd == "CLOSED_TABS_GET" {
                    // Newest first, as the tab search lists them; index n here is what REOPEN_CLOSED_TAB:n takes.
                    let list: Vec<serde_json::Value> = closed_tabs
                        .iter()
                        .rev()
                        .take(15)
                        .map(|t| serde_json::json!({ "title": t.title, "url": t.url }))
                        .collect();
                    if let Ok(arg) = serde_json::to_string(&list) {
                        let _ = chrome_wv.evaluate_script(&format!("window.__setClosedTabs && window.__setClosedTabs({});", arg));
                    }
                } else if let Some(sel) = if cmd == "REOPEN_CLOSED_TAB" { Some(0) } else { cmd.strip_prefix("REOPEN_CLOSED_TAB:").and_then(|n| n.parse::<usize>().ok()) } {
                    let picked = if sel < closed_tabs.len() { Some(closed_tabs.remove(closed_tabs.len() - 1 - sel)) } else { None };
                    if let Some(saved) = picked {
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
                                zoom: pol.zoom,
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
                            if tabs[idx].pinned {
                                set_tab_group(tabs[idx].id, None);
                            }
                            let active_id = tabs.get(active_tab_idx).map(|t| t.id);
                            let split_id = split_tab_idx.and_then(|k| tabs.get(k)).map(|t| t.id);
                            let tab_item = tabs.remove(idx);
                            let target_idx = if tab_item.pinned {
                                tabs.iter().take_while(|t| t.pinned).count()
                            } else {
                                tabs.len()
                            };
                            tabs.insert(target_idx, tab_item);
                            active_tab_idx = active_id.and_then(|id| tabs.iter().position(|t| t.id == id)).unwrap_or(target_idx);
                            split_tab_idx = split_id.and_then(|id| tabs.iter().position(|t| t.id == id));
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
                            // Legacy toggle (command palette): flip to an explicit light/dark.
                            global_settings.theme_mode = if global_settings.full_dark_mode { "light" } else { "dark" }.to_string();
                            global_settings.full_dark_mode = !global_settings.full_dark_mode;
                            apply_theme(&window, &tabs, global_settings.full_dark_mode);
                            for p in &parked {
                                apply_theme(&p.window, &p.tabs, global_settings.full_dark_mode);
                            }
                        }
                        "force_dark_pages" => {
                            global_settings.force_dark_pages = !global_settings.force_dark_pages;
                        }
                        "download_takeover" => global_settings.download_takeover = !global_settings.download_takeover,
                        "little_for_links" => global_settings.little_for_links = !global_settings.little_for_links,
                        "bg_audio" => global_settings.background_media_play = !global_settings.background_media_play,
                        "auto_pip" => global_settings.auto_pip_on_blur = !global_settings.auto_pip_on_blur,
                        "auto_clean" => {
                            global_settings.auto_clean_completed = !global_settings.auto_clean_completed;
                            engine.auto_clean_completed.store(global_settings.auto_clean_completed, Ordering::SeqCst);
                        }
                        "strip_tracking" => global_settings.strip_tracking_params = !global_settings.strip_tracking_params,
                        "block_popups" => global_settings.block_popups = !global_settings.block_popups,
                        "favorites_bar" => global_settings.show_favorites_bar = !global_settings.show_favorites_bar,
                        "peek_links" => {
                            global_settings.peek_links = !global_settings.peek_links;
                            let js = format!("window.__shunnoPeekOn = {};", global_settings.peek_links);
                            for t in &tabs {
                                let _ = t.webview.evaluate_script(&js);
                            }
                        }
                        "auto_update" => global_settings.auto_update = !global_settings.auto_update,
                        "https_only" => {
                            global_settings.https_only = !global_settings.https_only;
                            https_guard::global().set_enabled(global_settings.https_only);
                        }
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
                } else if cmd == "CHECK_UPDATES" {
                    let px = proxy.clone();
                    rt_handle.spawn(async move {
                        check_for_updates().await;
                        let _ = px.send_event(AppEvent::RefreshChrome);
                    });
                } else if cmd == "OPEN_RELEASES" {
                    let opener = tabs.get(active_tab_idx).map(|t| t.id).unwrap_or(0);
                    let _ = proxy.send_event(AppEvent::NewWindowBlockedOrOpened(opener, updater::RELEASES_PAGE.to_string()));
                } else if cmd == "RESTART_FOR_UPDATE" {
                    if matches!(updater::global().state(), updater::UpdateState::Ready { .. }) {
                        let to_save: Vec<SavedTab> = session_tabs!();
                        BrowserStorage::save_session(&to_save);
                        save_groups();
                        engine.save_queue();
                        if let Ok(exe) = std::env::current_exe() {
                            // Come back in the same profile.
                            if Command::new(exe).arg(format!("--profile={}", state::profile_id())).spawn().is_ok() {
                                *control_flow = ControlFlow::Exit;
                                return;
                            }
                        }
                    }
                } else if cmd == "UPDATE_FILTERS" {
                    blocker::global().set_status("Updating filter lists...".to_string());
                    blocker::global().set_notice(String::new());
                    blocker::global().mark_busy();
                    let _ = proxy.send_event(AppEvent::RefreshChrome);
                    let px = proxy.clone();
                    rt_handle.spawn(async move {
                        refresh_filter_lists(false).await;
                        let _ = px.send_event(AppEvent::RefreshChrome);
                    });
                } else if let Some(arg) = cmd.strip_prefix("FILTER_LIST:") {
                    if let Some((id, on)) = arg.split_once('|') {
                        if blocker::global().set_list(id, on == "1") {
                            blocker::global().set_notice(String::new());
                            apply_filter_change(&rt_handle, &proxy);
                        }
                    }
                } else if let Some(on) = cmd.strip_prefix("FILTER_AUTO:") {
                    blocker::global().set_auto_update(on == "1");
                    let _ = proxy.send_event(AppEvent::RefreshChrome);
                } else if let Some(days) = cmd.strip_prefix("FILTER_INTERVAL:") {
                    if let Ok(d) = days.parse::<u32>() {
                        blocker::global().set_interval(d);
                    }
                    let _ = proxy.send_event(AppEvent::RefreshChrome);
                } else if cmd == "FILTERS_USER_GET" {
                    if let Ok(arg) = serde_json::to_string(&blocker::user_rules_text()) {
                        let _ = chrome_wv.evaluate_script(&format!("window.__setUserFilters && window.__setUserFilters({});", arg));
                    }
                } else if let Some(text) = cmd.strip_prefix("FILTERS_USER_SAVE:") {
                    let bad = blocker::global().save_user_rules(text);
                    let n = blocker::rule_lines(&blocker::user_rules_text()).count();
                    blocker::global().set_notice(match bad.first() {
                        None => format!("Saved {} rule{}.", n, if n == 1 { "" } else { "s" }),
                        Some((line, txt)) => format!(
                            "Saved. {} line{} not understood and skipped, first on line {}: {}",
                            bad.len(),
                            if bad.len() == 1 { " was" } else { "s were" },
                            line,
                            txt.chars().take(80).collect::<String>()
                        ),
                    });
                    apply_filter_change(&rt_handle, &proxy);
                } else if cmd == "FILTERS_IMPORT" || cmd == "FILTERS_EXPORT" {
                    #[cfg(windows)]
                    let hwnd = {
                        use tao::platform::windows::WindowExtWindows;
                        window.hwnd() as isize
                    };
                    #[cfg(not(windows))]
                    let hwnd = 0isize;
                    let save = cmd == "FILTERS_EXPORT";
                    if let Some(path) = filter_file_dialog(hwnd, save) {
                        let msg = if save {
                            match fs::write(&path, blocker::user_rules_text()) {
                                Ok(()) => format!("Exported to {}.", path.display()),
                                Err(e) => format!("Could not export: {e}"),
                            }
                        } else {
                            match fs::read(&path) {
                                Ok(bytes) if bytes.len() <= blocker::MAX_USER_BYTES => {
                                    let text = String::from_utf8_lossy(&bytes).to_string();
                                    if text.trim_start().starts_with('<') {
                                        "That file is not a list of filter rules.".to_string()
                                    } else {
                                        let added = blocker::global().add_user_rules(&text);
                                        apply_filter_change(&rt_handle, &proxy);
                                        let bad = blocker::invalid_rules(&blocker::user_rules_text()).len();
                                        let mut m = format!("Imported {} new rule{}.", added, if added == 1 { "" } else { "s" });
                                        if bad > 0 {
                                            m.push_str(&format!(" {} line{} in My Filters cannot be used.", bad, if bad == 1 { "" } else { "s" }));
                                        }
                                        m
                                    }
                                }
                                Ok(_) => "That file is larger than 1 MB; My Filters is for your own rules, not whole lists.".to_string(),
                                Err(e) => format!("Could not read the file: {e}"),
                            }
                        };
                        blocker::global().set_notice(msg);
                        if let Ok(arg) = serde_json::to_string(&blocker::user_rules_text()) {
                            let _ = chrome_wv.evaluate_script(&format!("window.__setUserFilters && window.__setUserFilters({});", arg));
                        }
                        let _ = proxy.send_event(AppEvent::RefreshChrome);
                    }
                } else if cmd == "PICK_ELEMENT" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        if tab.url.starts_with("http") {
                            pending_picks.insert(tab.id);
                            let _ = tab.webview.evaluate_script("window.__shunnoPicker && window.__shunnoPicker(false)");
                        }
                    }
                } else if cmd == "PW_SAVE" || cmd == "PW_NEVER" || cmd == "PW_DISMISS" {
                    let offer = passwords::global().offer.lock().ok().and_then(|mut o| o.take());
                    if let Some(o) = offer {
                        if cmd == "PW_SAVE" {
                            passwords::global().with(|v| {
                                v.upsert(&o.origin, &o.username, &o.password);
                                ((), true)
                            });
                        } else if cmd == "PW_NEVER" {
                            passwords::global().with(|v| {
                                v.add_never(&o.origin);
                                ((), true)
                            });
                        }
                    }
                } else if let Some(id) = cmd.strip_prefix("PW_DELETE:") {
                    if let Ok(id) = id.parse::<u64>() {
                        passwords::global().with(|v| {
                            let changed = v.remove(id);
                            ((), changed)
                        });
                    }
                } else if let Some(arg) = cmd.strip_prefix("PW_EDIT:") {
                    // PW_EDIT:<id>|{"u":..,"p":..}
                    if let Some((id, json)) = arg.split_once('|') {
                        if let (Ok(id), Ok(v)) = (id.parse::<u64>(), serde_json::from_str::<serde_json::Value>(json)) {
                            let u = v.get("u").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            let p = v.get("p").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            let ok = passwords::global().with(|vd| {
                                let ok = vd.edit(id, &u, &p);
                                (ok, ok)
                            });
                            passwords::global().set_status(if ok { "Saved." } else { "Not saved: empty password, or that username already exists for this site." });
                        }
                    }
                } else if let Some(id) = cmd.strip_prefix("PW_COPY_USER:") {
                    if let Some(c) = id.parse::<u64>().ok().and_then(|id| passwords::global().with(|v| (v.get(id).cloned(), false))) {
                        if passwords::set_clipboard(&c.username) {
                            passwords::global().set_status("Username copied.");
                        }
                    }
                } else if let Some((action, id)) = cmd
                    .strip_prefix("PW_REVEAL:")
                    .map(|i| ("reveal", i))
                    .or_else(|| cmd.strip_prefix("PW_COPY:").map(|i| ("copy", i)))
                {
                    if let Ok(id) = id.parse::<u64>() {
                        if passwords::global().is_unlocked() {
                            let _ = proxy.send_event(AppEvent::PwAuthorized(action, id, true, false));
                        } else {
                            // Windows Hello blocks until answered: ask on a worker thread.
                            #[cfg(windows)]
                            let hwnd = {
                                use tao::platform::windows::WindowExtWindows;
                                window.hwnd() as isize
                            };
                            #[cfg(not(windows))]
                            let hwnd = 0isize;
                            let px = proxy.clone();
                            std::thread::spawn(move || {
                                let r = passwords::verify_user(hwnd, "Shunno Browser wants to show a saved password.");
                                let (ok, unavailable) = match r {
                                    passwords::HelloResult::Verified => (true, false),
                                    passwords::HelloResult::Unavailable => (true, true),
                                    passwords::HelloResult::Denied => (false, false),
                                };
                                let _ = px.send_event(AppEvent::PwAuthorized(action, id, ok, unavailable));
                            });
                        }
                    }
                } else if cmd == "PW_IMPORT" {
                    #[cfg(windows)]
                    let hwnd = {
                        use tao::platform::windows::WindowExtWindows;
                        window.hwnd() as isize
                    };
                    #[cfg(not(windows))]
                    let hwnd = 0isize;
                    if let Some(path) = passwords::pick_csv(hwnd) {
                        let msg = match std::fs::read(&path) {
                            Ok(bytes) => match passwords::global().with(|v| {
                                let r = v.import_csv(&String::from_utf8_lossy(&bytes));
                                let dirty = r.as_ref().map(|(a, u, _)| a + u > 0).unwrap_or(false);
                                (r, dirty)
                            }) {
                                Ok((a, u, sk)) => {
                                    pw_last_csv = Some(path.clone());
                                    passwords::global().csv_pending.store(true, Ordering::Relaxed);
                                    format!("Imported {a} new, updated {u}, skipped {sk}. The CSV file holds your passwords in plain text: delete it.")
                                }
                                Err(e) => e,
                            },
                            Err(e) => format!("Could not read the file: {e}"),
                        };
                        passwords::global().set_status(msg);
                    }
                } else if cmd == "PW_DELETE_CSV" {
                    passwords::global().csv_pending.store(false, Ordering::Relaxed);
                    if let Some(path) = pw_last_csv.take() {
                        let msg = match std::fs::remove_file(&path) {
                            Ok(()) => format!("Deleted {}.", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
                            Err(e) => format!("Could not delete the CSV: {e}"),
                        };
                        passwords::global().set_status(msg);
                    }
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
                            "mobile_ua" => pol.mobile_ua = !pol.mobile_ua,
                            "sponsorblock" => pol.sponsorblock = !pol.sponsorblock,
                            "auto_reject_cookies" => pol.auto_reject_cookies = !pol.auto_reject_cookies,
                            "fp_protect" => pol.fp_protect = !pol.fp_protect,
                            _ => {}
                        }
                        let updated_pol = pol.clone();
                        BrowserStorage::save_site_policies(&site_policies);

                        let current_url = tab.url.clone();
                        let prev_cookie = tab.cookie.clone();
                        let is_incog = tab.incognito;
                        let is_muted = tab.muted;
                        let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                        let old_id = tab.id;
                        if let Ok(new_wv) = create_tab_webview(&window, &mut web_context, tid, &current_url, &updated_pol, &global_settings, &ipc_token, proxy.clone(), is_incog) {
                            // The rebuilt tab keeps its group.
                            set_tab_group(tid, tab_group(old_id));
                            set_tab_group(old_id, None);
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
                    // A number only: this text is placed into script run in the page.
                    let Some(spd) = spd.trim().parse::<f64>().ok().filter(|v| v.is_finite() && (0.0625..=16.0).contains(v)) else { return };
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
                        let _ = tab.webview.evaluate_script(&build_reader_script(&global_settings.reader_prefs));
                    }
                } else if let Some(em_flag) = cmd.strip_prefix("ENGINE_MENU:") {
                    engine_menu_open = em_flag == "1";
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                } else if let Some(s_flag) = cmd.strip_prefix("SHEET:") {
                    // A question sheet is open in the toolbar: lay the toolbar over the page.
                    SHEET_OPEN.with(|c| c.set(s_flag == "1"));
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
                } else if let Some(arg) = cmd.strip_prefix("MAXBTN:") {
                    // MAXBTN:x|y|w|h|#bg|#fg|#hover|radius|glyph[|fade ms]  (physical px in the toolbar web view)
                    // With fade ms, the new colours are reached by a native fade timed like the page's
                    // theme cross-fade, instead of jumping (the button is not part of the page).
                    if SNAP_NATIVE.load(Ordering::Relaxed) {
                        let f: Vec<&str> = arg.split('|').collect();
                        if f.len() == 9 || f.len() == 10 {
                            let fade_ms = f.get(9).and_then(|v| v.trim().parse::<u32>().ok()).unwrap_or(0).min(2000);
                            let n = |i: usize| f[i].trim().parse::<f64>().map(|v| v.round() as i32).unwrap_or(0);
                            #[cfg(windows)]
                            let moved = with_max_button({ use tao::platform::windows::WindowExtWindows; window.hwnd() as isize }, |b| {
                                let rect = (n(0), n(1), n(0) + n(2), n(1) + n(3));
                                let active = n(2) > 0 && n(3) > 0;
                                let moved = b.rect != rect || b.active != active;
                                b.rect = rect;
                                let now = tick_ms();
                                let shown = b.colors_at(now);
                                b.bg = colorref(f[4]).unwrap_or(0x00FFFFFF);
                                b.fg = colorref(f[5]).unwrap_or(0);
                                b.hover_bg = colorref(f[6]).unwrap_or(b.bg);
                                if fade_ms > 0 && shown != (b.bg, b.fg, b.hover_bg) {
                                    b.fade_from = shown;
                                    b.fade_start = now;
                                    b.fade_ms = fade_ms;
                                } else if fade_ms == 0 {
                                    b.fade_ms = 0;
                                }
                                b.radius = n(7);
                                b.glyph_px = n(8).max(8);
                                b.active = active;
                                moved
                            });
                            #[cfg(windows)]
                            {
                                use tao::platform::windows::WindowExtWindows;
                                if fade_ms > 0 {
                                    unsafe {
                                        let _ = windows::Win32::UI::WindowsAndMessaging::SetTimer(
                                            windows::Win32::Foundation::HWND(window.hwnd() as isize),
                                            MAXBTN_FADE_TIMER,
                                            15,
                                            None,
                                        );
                                    }
                                }
                                if moved {
                                    apply_max_button_hole(window.hwnd() as isize);
                                } else {
                                    // Colours only (theme cross-fade frames): repaint the button, nothing else.
                                    if let Some(r) = max_button_client_rect(windows::Win32::Foundation::HWND(window.hwnd() as isize)) {
                                        unsafe {
                                            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(windows::Win32::Foundation::HWND(window.hwnd() as isize), Some(&r), false);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return;
                } else if cmd == "SET_DEFAULT_BROWSER" {
                    let note = match little::register_as_browser() {
                        Ok(()) => "Windows Settings opened: choose Shunno Browser for HTTP and HTTPS.".to_string(),
                        Err(e) => format!("Could not register Shunno: {e}"),
                    };
                    MENU_NOTE.with(|n| *n.borrow_mut() = note);
                } else if cmd == "LITTLE_OPEN" {
                    let _ = proxy.send_event(AppEvent::LittleOpen(None));
                } else if cmd == "EXT_REFRESH" {
                    ext_refresh(&chrome_wv, proxy.clone());
                } else if let Some(page) = cmd.strip_prefix("EXT_STORE:") {
                    // "Add to Shunno" on a Chrome Web Store or Edge Add-ons page: fetch the package
                    // the store serves to Chromium browsers, unpack it, install it like a .crx file.
                    if let Some((store, id)) = extensions::store_extension(page) {
                        let ver = DESKTOP_UA.get().and_then(|ua| extensions::chrome_version_from_ua(ua)).unwrap_or_else(|| "131.0.0.0".into());
                        let crx_url = extensions::store_crx_url(store, &id, &ver);
                        let eng = engine.clone();
                        let p = proxy.clone();
                        let _ = proxy.send_event(AppEvent::ExtensionNotice("Downloading the extension from the store…".into()));
                        rt_handle.spawn(async move {
                            let res = async {
                                let bytes = eng.fetch_store_package(&crx_url, 200 * 1024 * 1024).await?;
                                // Disk work and unpacking off the async workers; a folder of its own per
                                // install, so two clicks on "Add" never share (or delete) one file.
                                tokio::task::spawn_blocking(move || -> Result<(std::path::PathBuf, extensions::Meta), String> {
                                    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
                                    let tmp = extensions::root_dir().join(format!(".download-{stamp}"));
                                    fs::create_dir_all(&tmp).map_err(|e| format!("Could not save the package: {e}"))?;
                                    let file = tmp.join(format!("{id}.crx"));
                                    let saved = fs::write(&file, &bytes).map_err(|e| format!("Could not save the package: {e}"));
                                    let dir = saved.and_then(|_| extensions::unpack_archive(&file));
                                    let _ = fs::remove_dir_all(&tmp);
                                    let dir = dir?;
                                    match extensions::read_manifest(&dir) {
                                        Ok(m) => Ok((dir, m)),
                                        Err(e) => {
                                            extensions::delete_copy(&dir.to_string_lossy());
                                            Err(e)
                                        }
                                    }
                                })
                                .await
                                .map_err(|e| e.to_string())?
                            }
                            .await;
                            let _ = p.send_event(AppEvent::ExtStoreReady(res));
                        });
                    }
                } else if cmd == "EXT_ADD_FOLDER" || cmd == "EXT_ADD_FILE" {
                    // Pick, copy into the profile's extensions folder, read the manifest, install.
                    #[cfg(windows)]
                    {
                        use tao::platform::windows::WindowExtWindows;
                        if let Some(src) = extensions::pick(window.hwnd() as isize, cmd == "EXT_ADD_FOLDER") {
                            // Copying or unpacking (up to 500 MB) runs off the window's thread.
                            let is_folder = cmd == "EXT_ADD_FOLDER";
                            let p = proxy.clone();
                            let _ = proxy.send_event(AppEvent::ExtensionNotice("Installing the extension…".into()));
                            rt_handle.spawn_blocking(move || {
                                let res = (if is_folder { extensions::copy_folder(&src) } else { extensions::unpack_archive(&src) }).and_then(|dir| {
                                    match extensions::read_manifest(&dir) {
                                        Ok(m) => Ok((dir, m)),
                                        Err(e) => {
                                            extensions::delete_copy(&dir.to_string_lossy());
                                            Err(e)
                                        }
                                    }
                                });
                                let _ = p.send_event(AppEvent::ExtStoreReady(res));
                            });
                        }
                    }
                } else if let Some(arg) = cmd.strip_prefix("EXT_SET:") {
                    // EXT_SET:<id>|enable|disable|remove
                    if let Some((id, a)) = arg.split_once('|') {
                        let action: Option<&'static str> = match a {
                            "enable" => Some("enable"),
                            "disable" => Some("disable"),
                            "remove" => Some("remove"),
                            _ => None,
                        };
                        if let Some(action) = action {
                            ext_change(&chrome_wv, proxy.clone(), id.to_string(), action);
                        }
                    }
                } else if let Some(arg) = cmd.strip_prefix("EXT_OPEN:") {
                    // EXT_OPEN:<id>|popup|options: the page opens in Peek, like a popover over the tab.
                    if let Some((id, which)) = arg.split_once('|') {
                        let reg = extensions::load_registry();
                        if let Some(url) = reg.get(id).and_then(|m| extensions::page_url(id, m, which)) {
                            let _ = close_peek();
                            let pid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                            if let Ok(wv) = create_tab_webview(&window, &mut web_context, pid, &url, &SitePolicy::default(), &global_settings, &ipc_token, proxy.clone(), false) {
                                drawer_open = false;
                                engine_menu_open = false;
                                palette_open = false;
                                PEEK.with(|p| *p.borrow_mut() = Some(Peek { id: pid, webview: wv, incognito: false }));
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            }
                        }
                    }
                } else if cmd == "NEW_WINDOW" {
                    if open_window!(None) {
                        window.set_focus();
                        let _ = proxy.send_event(AppEvent::ChromeIpc("NEW_TAB".into()));
                    }
                } else if let Some(arg) = cmd.strip_prefix("MOVE_TAB_NEW_WINDOW:") {
                    // MOVE_TAB_NEW_WINDOW:<tab index>[|<screen x>|<screen y>]: the live tab gets its own window.
                    let f: Vec<&str> = arg.split('|').collect();
                    if let Ok(i) = f[0].parse::<usize>() {
                        if i < tabs.len() && tabs.len() > 1 {
                            let pos = if f.len() == 3 {
                                match (f[1].parse::<f64>(), f[2].parse::<f64>()) {
                                    (Ok(x), Ok(y)) => Some((x - 120.0, y - 18.0)),
                                    _ => None,
                                }
                            } else {
                                None
                            };
                            let t = tabs.remove(i);
                            set_tab_group(t.id, None);
                            if split_tab_idx == Some(i) {
                                split_tab_idx = None;
                            }
                            if active_tab_idx >= i && active_tab_idx > 0 {
                                active_tab_idx -= 1;
                            }
                            split_tab_idx = split_tab_idx.map(|k| if k > i { k - 1 } else { k });
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            let _ = proxy.send_event(AppEvent::RefreshChrome);
                            let mut t = Some(t);
                            if open_window!(pos) {
                                let t = t.take().expect("tab");
                                adopt_webview!(t.webview);
                                tabs.push(t);
                                active_tab_idx = 0;
                                let _ = tabs[0].webview.set_visible(true);
                                TAB_APPLIED.with(|m| { m.borrow_mut().remove(&tabs[0].id); });
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                                window.set_focus();
                                let _ = proxy.send_event(AppEvent::RefreshChrome);
                            } else if let Some(t) = t.take() {
                                tabs.insert(i.min(tabs.len()), t);
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            }
                        }
                    }
                } else if let Some(arg) = cmd.strip_prefix("MOVE_TAB_TO_WINDOW:") {
                    // MOVE_TAB_TO_WINDOW:<tab index>|<window key>
                    if let Some((i, k)) = arg.split_once('|') {
                        if let (Ok(i), Ok(k)) = (i.parse::<usize>(), k.parse::<u64>()) {
                            if i < tabs.len() && k != current_key && parked.iter().any(|p| p.key == k) {
                                let from_key = current_key;
                                let t = tabs.remove(i);
                                set_tab_group(t.id, None);
                                split_tab_idx = None;
                                if active_tab_idx >= i && active_tab_idx > 0 {
                                    active_tab_idx -= 1;
                                }
                                let emptied = tabs.is_empty();
                                if !emptied {
                                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                                    let _ = proxy.send_event(AppEvent::RefreshChrome);
                                }
                                switch_to!(k);
                                adopt_webview!(t.webview);
                                tabs.push(t);
                                active_tab_idx = tabs.len() - 1;
                                focused_key = current_key;
                                window.set_focus();
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                                let _ = proxy.send_event(AppEvent::RefreshChrome);
                                if emptied {
                                    // Its last tab left: close the old window.
                                    switch_to!(from_key);
                                    close_current_window!();
                                    switch_to!(k);
                                    focused_key = k;
                                }
                            }
                        }
                    }
                } else if let Some(i) = cmd.strip_prefix("TAB_GROUP_NEW:") {
                    // TAB_GROUP_NEW:<tab index> → a new, unnamed group holding that tab.
                    if let Ok(i) = i.parse::<usize>() {
                        if i < tabs.len() && !tabs[i].pinned && !tabs[i].incognito {
                            let gid = GROUPS.with(|g| {
                                let mut g = g.borrow_mut();
                                let id = g.iter().map(|x| x.id).max().unwrap_or(0) + 1;
                                let color = state::GROUP_COLORS[g.len() % state::GROUP_COLORS.len()].to_string();
                                g.push(state::TabGroup { id, name: String::new(), color, collapsed: false });
                                id
                            });
                            move_into_group(&mut tabs, i, gid, &mut active_tab_idx, &mut split_tab_idx);
                            save_groups();
                            NEW_GROUP_TO_NAME.with(|n| n.set(Some(gid)));
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                        }
                    }
                } else if let Some(arg) = cmd.strip_prefix("TAB_GROUP_ADD:") {
                    // TAB_GROUP_ADD:<tab index>|<group id>
                    if let Some((i, g)) = arg.split_once('|') {
                        if let (Ok(i), Ok(g)) = (i.parse::<usize>(), g.parse::<u32>()) {
                            let exists = GROUPS.with(|x| x.borrow().iter().any(|y| y.id == g));
                            if i < tabs.len() && exists && !tabs[i].pinned && !tabs[i].incognito {
                                move_into_group(&mut tabs, i, g, &mut active_tab_idx, &mut split_tab_idx);
                                save_groups();
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            }
                        }
                    }
                } else if let Some(i) = cmd.strip_prefix("TAB_UNGROUP:") {
                    // Leave the group and sit just after it, so the tab does not jump away.
                    if let Ok(i) = i.parse::<usize>() {
                        if let Some(g) = tabs.get(i).and_then(|t| tab_group(t.id)) {
                            let id = tabs[i].id;
                            let active_id = tabs[active_tab_idx].id;
                            let split_id = split_tab_idx.and_then(|k| tabs.get(k)).map(|t| t.id);
                            set_tab_group(id, None);
                            let t = tabs.remove(i);
                            let at = tabs.iter().rposition(|x| tab_group(x.id) == Some(g)).map(|k| k + 1).unwrap_or(i.min(tabs.len()));
                            tabs.insert(at, t);
                            active_tab_idx = tabs.iter().position(|t| t.id == active_id).unwrap_or(0);
                            split_tab_idx = split_id.and_then(|sid| tabs.iter().position(|t| t.id == sid));
                            save_groups();
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                        }
                    }
                } else if let Some(arg) = cmd.strip_prefix("GROUP_EDIT:") {
                    // GROUP_EDIT:<group id>|{"name":..,"color":..}
                    if let Some((g, json)) = arg.split_once('|') {
                        if let (Ok(g), Ok(v)) = (g.parse::<u32>(), serde_json::from_str::<serde_json::Value>(json)) {
                            let (name, color) = state::clean_group(v["name"].as_str().unwrap_or(""), v["color"].as_str().unwrap_or(""));
                            group_mut(g, |x| {
                                x.name = name;
                                x.color = color;
                            });
                            save_groups();
                        }
                    }
                } else if let Some(g) = cmd.strip_prefix("GROUP_TOGGLE:") {
                    if let Ok(g) = g.parse::<u32>() {
                        if let Some(now_collapsed) = group_mut(g, |x| {
                            x.collapsed = !x.collapsed;
                            x.collapsed
                        }) {
                            save_groups();
                            // Collapsing the group on screen moves to the nearest visible tab.
                            if now_collapsed && tabs.get(active_tab_idx).and_then(|t| tab_group(t.id)) == Some(g) {
                                let hidden = |t: &BrowserTab| {
                                    tab_group(t.id).is_some_and(|gg| GROUPS.with(|x| x.borrow().iter().any(|y| y.id == gg && y.collapsed)))
                                };
                                let pick = (active_tab_idx + 1..tabs.len())
                                    .chain((0..active_tab_idx).rev())
                                    .find(|&k| !hidden(&tabs[k]));
                                split_tab_idx = None;
                                match pick {
                                    Some(k) => active_tab_idx = k,
                                    None => {
                                        let _ = proxy.send_event(AppEvent::ChromeIpc("NEW_TAB".into()));
                                    }
                                }
                                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                            }
                        }
                    }
                } else if let Some(g) = cmd.strip_prefix("GROUP_UNGROUP:") {
                    if let Ok(g) = g.parse::<u32>() {
                        for t in &tabs {
                            if tab_group(t.id) == Some(g) {
                                set_tab_group(t.id, None);
                            }
                        }
                        GROUPS.with(|x| x.borrow_mut().retain(|y| y.id != g));
                        save_groups();
                    }
                } else if let Some(g) = cmd.strip_prefix("GROUP_CLOSE:") {
                    // Close every tab of the group, last first so the earlier indices stay valid.
                    if let Ok(g) = g.parse::<u32>() {
                        let mut idx: Vec<usize> = (0..tabs.len()).filter(|&k| tab_group(tabs[k].id) == Some(g)).collect();
                        idx.reverse();
                        for k in idx {
                            let _ = proxy.send_event(AppEvent::ChromeIpc(format!("CLOSE_TAB:{k}")));
                        }
                    }
                } else if let Some(arg) = cmd.strip_prefix("PERM_DECIDE:") {
                    // PERM_DECIDE:<id>|allow|block
                    if let Some((id, d)) = arg.split_once('|') {
                        if let Ok(id) = id.parse::<u64>() {
                            let d = if d == "allow" { permissions::Decision::Allow } else { permissions::Decision::Block };
                            answer_permission(id, d, true);
                        }
                    }
                } else if let Some(arg) = cmd.strip_prefix("PERM_SET:") {
                    // PERM_SET:<origin>|<kind>|allow|block|reset
                    let f: Vec<&str> = arg.splitn(3, '|').collect();
                    if f.len() == 3 {
                        if let Ok(mut st) = permissions::global().lock() {
                            match f[2] {
                                "allow" => st.set(f[0], f[1], permissions::Decision::Allow),
                                "block" => st.set(f[0], f[1], permissions::Decision::Block),
                                _ => st.reset(f[0], Some(f[1])),
                            }
                            permissions::save(&st);
                        }
                    }
                } else if let Some(origin) = cmd.strip_prefix("PERM_RESET_SITE:") {
                    if let Ok(mut st) = permissions::global().lock() {
                        st.reset(origin, None);
                        permissions::save(&st);
                    }
                } else if let Some(mode) = cmd.strip_prefix("SET_THEME:") {
                    if matches!(mode, "system" | "light" | "dark") {
                        global_settings.theme_mode = mode.to_string();
                        global_settings.full_dark_mode = state::effective_dark(mode, system_prefers_dark());
                        apply_theme(&window, &tabs, global_settings.full_dark_mode);
                            for p in &parked {
                                apply_theme(&p.window, &p.tabs, global_settings.full_dark_mode);
                            }
                        BrowserStorage::save_global_settings(&global_settings);
                    }
                } else if let Some(h) = cmd.strip_prefix("SET_AUTO_ARCHIVE:") {
                    if let Ok(h) = h.trim().parse::<u32>() {
                        global_settings.auto_archive_hours = h.min(24 * 30);
                        BrowserStorage::save_global_settings(&global_settings);
                    }
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
                        // A JSON string is a valid JS string literal for any text (quotes, newlines).
                        let arg = serde_json::Value::String(query.to_string());
                        let js = format!("if (window.__shunnoFindOnPage) window.__shunnoFindOnPage({}, 'next');", arg);
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
                        .unwrap_or(state::FAVORITES)
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
                } else if let Some(json) = cmd.strip_prefix("BOOKMARK_SAVE:") {
                    // {"orig_url","url","title","folder"}: Add Bookmark, Edit Bookmark, Library edit,
                    // New Tab "Add Favorite".
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
                        let g = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
                        if state::save_bookmark(&mut bookmarks, &g("orig_url"), &g("url"), &g("title"), &g("folder"), now_epoch_secs()) {
                            BrowserStorage::save_bookmarks(&bookmarks);
                        }
                    }
                } else if let Some(url) = cmd.strip_prefix("BOOKMARK_REMOVE_URL:") {
                    let before = bookmarks.len();
                    bookmarks.retain(|b| b.url != url);
                    if bookmarks.len() != before {
                        BrowserStorage::save_bookmarks(&bookmarks);
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
                } else if let Some(h) = cmd.strip_prefix("CHROME_H:") {
                    if let Ok(h) = h.trim().parse::<f64>() {
                        let h = h.clamp(40.0, 200.0).round();
                        if (h - chrome_bar_height()).abs() >= 1.0 {
                            CHROME_BAR_H_BITS.store(h.to_bits(), Ordering::Relaxed);
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, split_tab_idx, drawer_open, engine_menu_open, palette_open, is_fullscreen);
                        }
                    }
                    return;
                } else if cmd == "IMPORT_SCAN" {
                    importer::scan();
                } else if let Some(arg) = cmd.strip_prefix("IMPORT_BROWSER:") {
                    // IMPORT_BROWSER:<source index>:<b?><h?>
                    let mut it = arg.splitn(2, ':');
                    let idx = it.next().and_then(|x| x.parse::<usize>().ok());
                    let what = it.next().unwrap_or("");
                    let (wb, wh) = (what.contains('b'), what.contains('h'));
                    if let (Some(src), true) = (idx.and_then(importer::source_at), wb || wh) {
                        importer::set_status(format!("Importing from {}\u{2026}", src.browser));
                        let px = proxy.clone();
                        std::thread::spawn(move || {
                            let r = importer::load(&src, wb, wh, state::HISTORY_CAP)
                                .map(|(b, h)| (src.browser.clone(), b, h));
                            let _ = px.send_event(AppEvent::ImportDone(r));
                        });
                    }
                } else if cmd == "CLEAR_HISTORY" {
                    history.clear();
                    // Ordered with background saves: an older snapshot finishing late must not bring it back.
                    save_history_ordered(None, Vec::new());
                } else if cmd == "SAVE_OFFLINE_HTML" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        pending_offline_saves.insert(tab.id);
                        // content_guard posts the page itself; the IPC token never goes into evaluated script.
                        let _ = tab.webview.evaluate_script("window.__shunnoSaveOffline && window.__shunnoSaveOffline()");
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
                        pending_grabs.insert(tab.id);
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
                        let dup_group = tab_group(src.id);
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
                            // A duplicate sits next to its source, so it joins the source's group.
                            set_tab_group(tid, dup_group);
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
                } else if let Some(path) = cmd.strip_prefix("DL_OPEN:") {
                    // Open a finished download with its default app. Only files of a known, finished
                    // download; Windows still shows its Internet-file warning for marked files.
                    let known = engine
                        .tasks
                        .lock()
                        .map(|l| l.iter().any(|t| t.status == "COMPLETED" && t.saved_path.as_deref() == Some(path)))
                        .unwrap_or(false);
                    if known && std::path::Path::new(path).is_file() {
                        open_with_default_app(path);
                    } else if known {
                        let _ = chrome_wv.evaluate_script("window.askNotice && window.askNotice('File not found', 'That file is no longer where it was saved. It may have been moved or deleted.')");
                    }
                } else if let Some(path) = cmd.strip_prefix("DL_SHOW:") {
                    // Only reveal files that belong to a known download.
                    let known = engine
                        .tasks
                        .lock()
                        .map(|l| l.iter().any(|t| t.saved_path.as_deref() == Some(path)))
                        .unwrap_or(false);
                    if known && std::path::Path::new(path).exists() {
                        let _ = Command::new("explorer.exe").args(["/select,", path]).spawn();
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
