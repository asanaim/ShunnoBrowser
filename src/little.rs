//! Little Shunno (Arc's "Little Arc") and the Windows integration it needs:
//! * a small quick window for links opened from other apps and for Ctrl+Alt+N anywhere;
//! * single instance: a second launch hands its link to the running browser (WM_COPYDATA);
//! * registration as a browser, so Windows can make Shunno the default for web links.

/// WM_COPYDATA tag for "open this" messages between Shunno processes.
pub const COPYDATA_TAG: usize = 0x5348_4E4F; // "SHNO"
/// Global hotkey id for Ctrl+Alt+N.
pub const HOTKEY_ID: i32 = 0x534E;

/// What a command-line argument asks for: a web/file URL to open, or nothing usable.
pub fn url_from_arg(arg: &str) -> Option<String> {
    let a = arg.trim().trim_matches('"');
    if a.is_empty() || a.starts_with('-') {
        return None;
    }
    let lower = a.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return url::Url::parse(a).ok().map(|u| u.to_string());
    }
    if lower.starts_with("file:///") {
        return Some(a.to_string());
    }
    // A local .htm/.html/.pdf/.svg path handed over by Explorer.
    let p = std::path::Path::new(a);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if p.is_absolute() && matches!(ext.as_str(), "htm" | "html" | "xhtml" | "pdf" | "svg") {
        return url::Url::from_file_path(p).ok().map(|u| u.to_string());
    }
    None
}

/// First URL among the process arguments (skipping the executable).
pub fn url_from_args<I: IntoIterator<Item = String>>(args: I) -> Option<String> {
    args.into_iter().skip(1).find_map(|a| url_from_arg(&a))
}

/// Named mutex that marks a running profile (the default keeps the original name).
pub fn instance_mutex_name(profile: &str) -> String {
    if profile == crate::state::DEFAULT_PROFILE {
        "Local\\ShunnoBrowser.SingleInstance".to_string()
    } else {
        format!("Local\\ShunnoBrowser.SingleInstance.{profile}")
    }
}

/// Is a profile open in some Shunno process right now?
#[cfg(windows)]
pub fn profile_running(profile: &str) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
    let name: Vec<u16> = instance_mutex_name(profile).encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        match OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, windows::core::PCWSTR(name.as_ptr())) {
            Ok(h) => {
                let _ = CloseHandle(h);
                true
            }
            Err(_) => false,
        }
    }
}
#[cfg(not(windows))]
pub fn profile_running(_profile: &str) -> bool {
    false
}

/// Start (or bring forward) a profile's own Shunno window.
pub fn launch_profile(profile: &str) -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    std::process::Command::new(exe).arg(format!("--profile={profile}")).spawn().is_ok()
}

fn hwnd_file() -> std::path::PathBuf {
    crate::state::BrowserStorage::app_data_dir().join("instance.hwnd")
}

/// Record the main window so a later launch can find it.
pub fn publish_main_window(hwnd: isize) {
    let _ = std::fs::write(hwnd_file(), hwnd.to_string());
}

/// If Shunno is already running, give it `url` (or ask it to come forward) and return true:
/// the caller should exit. Holds a named mutex for the life of the first instance.
#[cfg(windows)]
pub fn hand_off_to_running_instance(url: Option<&str>) -> bool {
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, WPARAM};
    use windows::Win32::System::DataExchange::COPYDATASTRUCT;
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, IsWindow, SendMessageTimeoutW, ASFW_ANY, SMTO_ABORTIFHUNG, WM_COPYDATA};
    unsafe {
        // One instance per profile: each profile is its own process and window.
        let name: Vec<u16> = instance_mutex_name(crate::state::profile_id()).encode_utf16().chain(std::iter::once(0)).collect();
        let m = CreateMutexW(None, true, windows::core::PCWSTR(name.as_ptr()));
        if m.is_err() || GetLastError() != ERROR_ALREADY_EXISTS {
            // First instance: keep the handle open (leaked on purpose) so the name stays taken.
            std::mem::forget(m);
            return false;
        }
        let Some(hwnd) = std::fs::read_to_string(hwnd_file()).ok().and_then(|s| s.trim().parse::<isize>().ok()) else {
            return false;
        };
        let target = HWND(hwnd);
        if !IsWindow(target).as_bool() {
            return false;
        }
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        let payload: Vec<u16> = url.unwrap_or("").encode_utf16().collect();
        let cds = COPYDATASTRUCT {
            dwData: COPYDATA_TAG,
            cbData: (payload.len() * 2) as u32,
            lpData: payload.as_ptr() as *mut core::ffi::c_void,
        };
        let mut result = 0usize;
        SendMessageTimeoutW(target, WM_COPYDATA, WPARAM(0), LPARAM(&cds as *const _ as isize), SMTO_ABORTIFHUNG, 3000, Some(&mut result))
            .0
            != 0
    }
}
#[cfg(not(windows))]
pub fn hand_off_to_running_instance(_url: Option<&str>) -> bool {
    false
}

/// Read a COPYDATASTRUCT sent by `hand_off_to_running_instance`.
#[cfg(windows)]
pub unsafe fn read_copydata(lparam: isize) -> Option<String> {
    use windows::Win32::System::DataExchange::COPYDATASTRUCT;
    let cds = (lparam as *const COPYDATASTRUCT).as_ref()?;
    if cds.dwData != COPYDATA_TAG || cds.cbData % 2 != 0 || cds.cbData > 64 * 1024 {
        return None;
    }
    if cds.cbData == 0 {
        return Some(String::new());
    }
    let s = std::slice::from_raw_parts(cds.lpData as *const u16, (cds.cbData / 2) as usize);
    Some(String::from_utf16_lossy(s))
}

/// Register Shunno as a browser for this user (no admin rights needed) and open the Windows
/// "Default apps" page on it. Windows requires the user to confirm the choice there.
#[cfg(windows)]
pub fn register_as_browser() -> Result<(), String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
    use windows::Win32::UI::Shell::{SHChangeNotify, ShellExecuteW, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy().to_string();
    let open_cmd = format!("\"{exe}\" \"%1\"");
    let icon = format!("\"{exe}\",0");
    let set = |key: &str, name: Option<&str>, val: &str| -> Result<(), String> {
        let wide: Vec<u16> = val.encode_utf16().chain(Some(0)).collect();
        let k = HSTRING::from(key);
        let n = name.map(HSTRING::from);
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &k,
                n.as_ref().map(|h| PCWSTR(h.as_ptr())).unwrap_or(PCWSTR::null()),
                REG_SZ.0,
                Some(wide.as_ptr() as *const core::ffi::c_void),
                (wide.len() * 2) as u32,
            )
            .ok()
            .map_err(|e| format!("{key}: {e}"))
        }
    };
    let cap = r"Software\Clients\StartMenuInternet\ShunnoBrowser\Capabilities";
    set(r"Software\Classes\ShunnoURL", None, "Shunno Browser Document")?;
    set(r"Software\Classes\ShunnoURL\DefaultIcon", None, &icon)?;
    set(r"Software\Classes\ShunnoURL\shell\open\command", None, &open_cmd)?;
    set(r"Software\Clients\StartMenuInternet\ShunnoBrowser", None, "Shunno Browser")?;
    set(r"Software\Clients\StartMenuInternet\ShunnoBrowser\DefaultIcon", None, &icon)?;
    set(r"Software\Clients\StartMenuInternet\ShunnoBrowser\shell\open\command", None, &format!("\"{exe}\""))?;
    set(cap, Some("ApplicationName"), "Shunno Browser")?;
    set(cap, Some("ApplicationDescription"), "Lightweight, private browser with a built-in download manager.")?;
    set(cap, Some("ApplicationIcon"), &icon)?;
    for scheme in ["http", "https"] {
        set(&format!(r"{cap}\URLAssociations"), Some(scheme), "ShunnoURL")?;
    }
    for ext in [".htm", ".html", ".xhtml", ".svg", ".pdf"] {
        set(&format!(r"{cap}\FileAssociations"), Some(ext), "ShunnoURL")?;
    }
    set(r"Software\RegisteredApplications", Some("Shunno Browser"), cap)?;
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
        ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from("ms-settings:defaultapps?registeredAppUser=Shunno%20Browser"), None, None, SW_SHOWNORMAL);
    }
    Ok(())
}
#[cfg(not(windows))]
pub fn register_as_browser() -> Result<(), String> {
    Err("Windows only".into())
}

/// Register Ctrl+Alt+N for Little Shunno on `hwnd` (delivered as WM_HOTKEY). False if taken.
#[cfg(windows)]
pub fn register_hotkey(hwnd: isize) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT};
    unsafe { RegisterHotKey(HWND(hwnd), HOTKEY_ID, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x4E).is_ok() }
}
#[cfg(not(windows))]
pub fn register_hotkey(_hwnd: isize) -> bool {
    false
}

/// The Little window's own toolbar: address/search field, "Open in Shunno", theme-aware.
pub const BAR_HTML: &str = r##"<!DOCTYPE html>
<html><head><meta charset="utf-8"><style>
  :root { --bg:#F2F2F7; --field:#FFFFFF; --text:#1D1D1F; --muted:#6E6E73; --blue:#0A84FF; --border:rgba(0,0,0,.10); }
  body.dark { --bg:#1C1C1E; --field:#2C2C2E; --text:#F5F5F7; --muted:#98989D; --border:rgba(255,255,255,.12); }
  html,body { margin:0; height:100%; background:var(--bg); color:var(--text);
    font:13px -apple-system,"Segoe UI Variable Text","Segoe UI",system-ui,sans-serif; overflow:hidden; user-select:none; }
  .bar { display:flex; align-items:center; gap:8px; height:100%; padding:0 10px; box-sizing:border-box; border-bottom:1px solid var(--border); }
  input { flex:1; height:28px; border:1px solid var(--border); border-radius:8px; background:var(--field); color:var(--text);
    padding:0 10px; font:inherit; outline:none; user-select:text; }
  input:focus { border-color:var(--blue); box-shadow:0 0 0 3px rgba(10,132,255,.18); }
  button { height:28px; border:none; border-radius:8px; padding:0 12px; font:600 12px inherit; font-family:inherit; cursor:pointer; }
  .open { background:var(--blue); color:#fff; }
  .ghost { background:transparent; color:var(--muted); padding:0 10px; font-size:20px; font-weight:400; line-height:1; }
  .ghost:hover { background:var(--border); color:var(--text); }
  /* Permission request: a pill that slides in beside the address, like Safari's site prompt. */
  .perm { display:none; align-items:center; gap:6px; height:30px; padding:0 4px 0 10px; border-radius:15px;
    background:rgba(10,132,255,.14); font-size:12px; white-space:nowrap; max-width:60%; animation:pin .22s cubic-bezier(.2,.8,.2,1) both; }
  .perm.on { display:flex; }
  .perm .t { overflow:hidden; text-overflow:ellipsis; }
  .perm button { height:22px; border-radius:11px; padding:0 10px; font-size:12px; }
  .perm .deny { background:rgba(118,118,128,.18); color:var(--text); }
  .perm .allow { background:var(--blue); color:#fff; }
  @keyframes pin { from { opacity:0; transform:translateY(-4px) scale(.97); } to { opacity:1; transform:none; } }
</style></head><body>
<div class="bar">
  <button class="ghost" title="Back" onclick="send('BACK')">&#8249;</button>
  <input id="u" placeholder="Search or enter address" spellcheck="false"
    onkeydown="if(event.key==='Enter'){send('NAV:'+this.value);this.blur();} if(event.key==='Escape'){send('CLOSE');}">
  <div class="perm" id="perm"></div>
  <button class="open" title="Move this page into the main Shunno window" onclick="send('OPEN')">Open in Shunno</button>
</div>
<script>
  function send(m){ window.ipc.postMessage(m); }
  window.__setUrl = function(u){ const i=document.getElementById('u'); if (document.activeElement!==i) i.value = (u==='about:blank') ? '' : u; };
  window.__setDark = function(d){ document.body.classList.toggle('dark', !!d); };
  window.__perm = function(o){
    const el = document.getElementById('perm');
    if (!o) { el.className = 'perm'; el.dataset.k = ''; el.textContent = ''; return; }
    if (el.dataset.k === String(o.id)) return;
    el.dataset.k = String(o.id);
    el.textContent = '';
    const t = document.createElement('span'); t.className = 't';
    const b = document.createElement('b'); b.textContent = o.host;
    t.append(o.icon + ' ', b, ' wants to ' + o.label);
    t.title = o.origin;
    const mk = (cls, text, d) => { const x = document.createElement('button'); x.className = cls; x.textContent = text; x.onclick = () => send('PERM:' + o.id + '|' + d); return x; };
    el.append(t, mk('deny', 'Block', 'block'), mk('allow', 'Allow', 'allow'));
    el.className = 'perm on';
  };
  window.__focus = function(){ const i=document.getElementById('u'); i.focus(); i.select(); };
</script></body></html>"##;
