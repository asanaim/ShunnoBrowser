//! Browser extensions (WebView2 `AddBrowserExtension`).
//!
//! WebView2 installs unpacked extensions only: no Chrome Web Store. Shunno copies every extension
//! it adds into its own folder (`<profile>/extensions/<slug>`), so the original can be deleted and
//! a `.crx` or `.zip` can be added as well. WebView2 keeps the installed list in the profile; this
//! module adds what the toolbar shows (version, icon, popup and options pages) from manifest.json.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// What the toolbar knows about one extension, beyond WebView2's id, name and enabled state.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Meta {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// Folder Shunno installed it from (inside the profile's extensions folder).
    #[serde(default)]
    pub path: String,
    /// Popup page relative to the extension root (action / browser_action default_popup).
    #[serde(default)]
    pub popup: Option<String>,
    /// Options page relative to the extension root (options_ui.page or options_page).
    #[serde(default)]
    pub options: Option<String>,
    /// Small icon as a data: URL (PNG, at most 48 KB).
    #[serde(default)]
    pub icon: Option<String>,
}

/// One row of the Extensions panel.
#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub version: String,
    pub description: String,
    pub icon: Option<String>,
    pub popup: bool,
    pub options: bool,
}

pub fn root_dir() -> PathBuf {
    crate::state::BrowserStorage::app_data_dir().join("extensions")
}

fn registry_path() -> PathBuf {
    crate::state::BrowserStorage::app_data_dir().join("extensions.json")
}

pub fn load_registry() -> BTreeMap<String, Meta> {
    let mut reg: BTreeMap<String, Meta> = fs::read_to_string(registry_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    // Entries saved by older versions (or with an icon too large then) get their icon now.
    let mut changed = false;
    for m in reg.values_mut() {
        if m.icon.is_none() && !m.path.is_empty() {
            if let Ok(fresh) = read_manifest(Path::new(&m.path)) {
                if fresh.icon.is_some() {
                    m.icon = fresh.icon;
                    changed = true;
                }
            }
        }
    }
    if changed {
        save_registry(&reg);
    }
    reg
}

pub fn save_registry(r: &BTreeMap<String, Meta>) {
    if let Ok(t) = serde_json::to_string_pretty(r) {
        let _ = crate::state::write_atomic(&registry_path(), t.as_bytes());
    }
}

/// Join WebView2's list (id, name, enabled) with what Shunno read from the manifests. Only the
/// extensions the user added are listed: WebView2 also reports its own built-in components (the
/// PDF viewer, the clipboard helper), which are part of the runtime, not something to manage.
pub fn entries(installed: &[(String, String, bool)], reg: &BTreeMap<String, Meta>) -> Vec<Entry> {
    let mut v: Vec<Entry> = installed
        .iter()
        .filter(|(id, _, _)| reg.contains_key(id))
        .map(|(id, name, enabled)| {
            let m = reg.get(id).cloned().unwrap_or_default();
            Entry {
                id: id.clone(),
                name: if m.name.is_empty() { name.clone() } else { m.name },
                enabled: *enabled,
                version: m.version,
                description: m.description,
                icon: m.icon,
                popup: m.popup.is_some(),
                options: m.options.is_some(),
            }
        })
        .collect();
    v.sort_by_key(|e| e.name.to_lowercase());
    v
}

/// `chrome-extension://<id>/<page>` for a popup or options page, when the extension has one.
pub fn page_url(id: &str, meta: &Meta, which: &str) -> Option<String> {
    let page = match which {
        "popup" => meta.popup.as_deref(),
        "options" => meta.options.as_deref(),
        _ => None,
    }?;
    let ok_id = !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric());
    if !ok_id || page.contains("..") || page.contains("://") {
        return None;
    }
    Some(format!("chrome-extension://{id}/{}", page.trim_start_matches('/')))
}

// ------------------------------------------------------------------ manifest.json

/// Resolve "__MSG_key__" names from _locales/<default_locale>/messages.json.
fn localize(dir: &Path, default_locale: Option<&str>, s: &str) -> String {
    let Some(key) = s.strip_prefix("__MSG_").and_then(|k| k.strip_suffix("__")) else {
        return s.to_string();
    };
    let locale = default_locale.unwrap_or("en");
    let file = dir.join("_locales").join(locale).join("messages.json");
    let msgs: serde_json::Value = fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    // Message keys are case-insensitive.
    msgs.as_object()
        .and_then(|o| o.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)))
        .and_then(|(_, v)| v.get("message"))
        .and_then(|m| m.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| key.to_string())
}

/// Read what the panel needs from an extension folder's manifest.json.
pub fn read_manifest(dir: &Path) -> Result<Meta, String> {
    let text = fs::read_to_string(dir.join("manifest.json")).map_err(|_| "This folder has no manifest.json, so it is not an extension.".to_string())?;
    let text = text.trim_start_matches('\u{feff}');
    let m: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("manifest.json could not be read: {e}"))?;
    let locale = m.get("default_locale").and_then(|v| v.as_str());
    let s = |k: &str| m.get(k).and_then(|v| v.as_str()).map(|v| localize(dir, locale, v)).unwrap_or_default();
    let name = s("name");
    if name.is_empty() {
        return Err("manifest.json has no name.".into());
    }
    let action = m.get("action").or_else(|| m.get("browser_action")).or_else(|| m.get("page_action"));
    let popup = action.and_then(|a| a.get("default_popup")).and_then(|v| v.as_str()).filter(|p| !p.is_empty()).map(str::to_string);
    let options = m
        .get("options_ui")
        .and_then(|o| o.get("page"))
        .or_else(|| m.get("options_page"))
        .and_then(|v| v.as_str())
        .filter(|p| !p.is_empty())
        .map(str::to_string);
    let icon = icon_candidates(&m, action).into_iter().find_map(|rel| icon_data_url(&dir.join(rel.trim_start_matches('/'))));
    Ok(Meta {
        name: name.chars().take(60).collect(),
        version: s("version").chars().take(20).collect(),
        description: s("description").chars().take(140).collect(),
        path: dir.to_string_lossy().into_owned(),
        popup,
        options,
        icon,
    })
}

/// PNG icons from "icons" and the action's default_icon, closest to 48 px first (larger wins a tie):
/// sharp at the toolbar's 20 px on high-DPI screens and at 36 px in the panel.
fn icon_candidates(m: &serde_json::Value, action: Option<&serde_json::Value>) -> Vec<String> {
    let mut cands: Vec<(u32, String)> = Vec::new();
    for src in [m.get("icons"), action.and_then(|a| a.get("default_icon"))].into_iter().flatten() {
        if let Some(o) = src.as_object() {
            for (k, v) in o {
                if let (Ok(n), Some(p)) = (k.parse::<u32>(), v.as_str()) {
                    cands.push((n, p.to_string()));
                }
            }
        } else if let Some(p) = src.as_str() {
            cands.push((32, p.to_string()));
        }
    }
    cands.sort_by_key(|(n, _)| ((*n as i64 - 48).abs(), -(*n as i64)));
    cands.into_iter().map(|(_, p)| p).filter(|p| p.to_ascii_lowercase().ends_with(".png") && !p.contains("..")).collect()
}

fn icon_data_url(file: &Path) -> Option<String> {
    let bytes = fs::read(file).ok()?;
    if bytes.len() > 128 * 1024 || !bytes.starts_with(b"\x89PNG") {
        return None;
    }
    Some(format!("data:image/png;base64,{}", base64(&bytes)))
}

pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

// ------------------------------------------------------------------ installing files

/// Folder name for an extension copy: its name as a slug plus a number to keep it unique.
pub fn slug(name: &str) -> String {
    let s: String = name.to_ascii_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    let s: String = s.chars().take(40).collect();
    if s.is_empty() {
        "extension".into()
    } else {
        s
    }
}

fn fresh_dir(name: &str) -> PathBuf {
    let root = root_dir();
    let base = slug(name);
    let mut n = 1;
    loop {
        let d = root.join(if n == 1 { base.clone() } else { format!("{base}-{n}") });
        if !d.exists() {
            return d;
        }
        n += 1;
    }
}

/// Byte offset of the ZIP inside a .crx (CRX2 or CRX3), or 0 for a plain .zip.
pub fn crx_zip_offset(b: &[u8]) -> Result<usize, String> {
    if b.starts_with(b"PK\x03\x04") {
        return Ok(0);
    }
    if !b.starts_with(b"Cr24") || b.len() < 16 {
        return Err("This is not a .crx or .zip extension file.".into());
    }
    let u = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
    let off = match u(4) {
        2 => 16usize.checked_add(u(8)).and_then(|x| x.checked_add(u(12))),
        3 => 12usize.checked_add(u(8)),
        v => return Err(format!("Unsupported .crx version {v}.")),
    }
    .ok_or("Damaged .crx header.")?;
    if off + 4 > b.len() || &b[off..off + 4] != b"PK\x03\x04" {
        return Err("Damaged .crx file.".into());
    }
    Ok(off)
}

/// Unpack a .crx or .zip into a new folder under the extensions folder.
pub fn unpack_archive(file: &Path) -> Result<PathBuf, String> {
    let mut bytes = Vec::new();
    fs::File::open(file).and_then(|mut f| f.read_to_end(&mut bytes)).map_err(|e| format!("Could not read the file: {e}"))?;
    if bytes.len() > 200 * 1024 * 1024 {
        return Err("The extension file is larger than 200 MB.".into());
    }
    let off = crx_zip_offset(&bytes)?;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes[off..])).map_err(|e| format!("The archive could not be opened: {e}"))?;
    let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let dest = fresh_dir(&stem);
    const LIMIT: u64 = 500 * 1024 * 1024;
    let extract = |zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>| -> Result<(), String> {
        let mut total: u64 = 0;
        for i in 0..zip.len() {
            let f = zip.by_index(i).map_err(|e| e.to_string())?;
            // enclosed_name() refuses absolute paths and "..": nothing can land outside `dest`.
            let Some(rel) = f.enclosed_name() else { continue };
            let out = dest.join(rel);
            if f.is_dir() {
                fs::create_dir_all(&out).map_err(|e| e.to_string())?;
                continue;
            }
            if let Some(p) = out.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut w = fs::File::create(&out).map_err(|e| e.to_string())?;
            // Count what is really written (the sizes in the archive can lie).
            let room = LIMIT - total;
            let n = std::io::copy(&mut f.take(room + 1), &mut w).map_err(|e| e.to_string())?;
            total += n;
            if total > LIMIT {
                return Err("The extension unpacks to more than 500 MB.".into());
            }
        }
        Ok(())
    };
    if let Err(e) = extract(&mut zip) {
        let _ = fs::remove_dir_all(&dest);
        return Err(e);
    }
    // Some archives wrap everything in one top folder.
    let dest = if dest.join("manifest.json").exists() {
        dest
    } else {
        let only: Vec<PathBuf> = fs::read_dir(&dest).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        match only.as_slice() {
            [one] if one.join("manifest.json").exists() => one.clone(),
            _ => dest,
        }
    };
    Ok(dest)
}

/// Web stores whose extensions WebView2 can run (Chromium extensions): the Chrome Web Store (which
/// Brave also uses) and Microsoft Edge Add-ons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    Chrome,
    Edge,
}

fn is_extension_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| (b'a'..=b'p').contains(&b))
}

/// The store and extension id of a store detail page, e.g.
/// https://chromewebstore.google.com/detail/ublock-origin-lite/ddkjiahejlhfcafbddmgiahcphecmpfh
pub fn store_extension(url: &str) -> Option<(Store, String)> {
    let u = url::Url::parse(url).ok()?;
    if u.scheme() != "https" {
        return None;
    }
    let host = u.host_str()?.to_ascii_lowercase();
    let segs: Vec<&str> = u.path_segments()?.filter(|s| !s.is_empty()).collect();
    let (store, rest): (Store, &[&str]) = match host.as_str() {
        "chromewebstore.google.com" if segs.first() == Some(&"detail") => (Store::Chrome, &segs[1..]),
        "chrome.google.com" if segs.len() > 2 && segs[0] == "webstore" && segs[1] == "detail" => (Store::Chrome, &segs[2..]),
        "microsoftedge.microsoft.com" if segs.len() > 2 && segs[0] == "addons" && segs[1] == "detail" => (Store::Edge, &segs[2..]),
        _ => return None,
    };
    // The id is the last 32-letter (a to p) segment: /detail/<name>/<id> or /detail/<id>.
    rest.iter().rev().find(|s| is_extension_id(s)).map(|id| (store, id.to_string()))
}

/// Where the store serves the packaged extension (.crx) for `id`. `chrome_version` is the engine's
/// Chromium version; the Chrome Web Store needs it to pick a compatible package.
pub fn store_crx_url(store: Store, id: &str, chrome_version: &str) -> String {
    match store {
        Store::Chrome => format!(
            "https://clients2.google.com/service/update2/crx?response=redirect&prodversion={chrome_version}&acceptformat=crx2,crx3&x=id%3D{id}%26uc"
        ),
        Store::Edge => format!("https://edge.microsoft.com/extensionwebstorebase/v1/crx?response=redirect&x=id%3D{id}%26installsource%3Dondemand%26uc"),
    }
}

/// The Chromium version in a User-Agent string ("Chrome/131.0.6778.86" gives "131.0.6778.86").
pub fn chrome_version_from_ua(ua: &str) -> Option<String> {
    let v = ua.split("Chrome/").nth(1)?.split_whitespace().next()?;
    (!v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.')).then(|| v.to_string())
}

/// Copy an unpacked extension folder (skipping .git and node_modules) into the extensions folder.
pub fn copy_folder(src: &Path) -> Result<PathBuf, String> {
    let meta = read_manifest(src)?;
    let dest = fresh_dir(&meta.name);
    fn walk(from: &Path, to: &Path, budget: &mut u64) -> std::io::Result<()> {
        fs::create_dir_all(to)?;
        for e in fs::read_dir(from)?.flatten() {
            let name = e.file_name();
            let ft = e.file_type()?;
            if ft.is_symlink() || name == ".git" || name == "node_modules" {
                continue;
            }
            if ft.is_dir() {
                walk(&e.path(), &to.join(&name), budget)?;
            } else {
                let len = e.metadata()?.len();
                if len > *budget {
                    return Err(std::io::Error::other("too large"));
                }
                *budget -= len;
                fs::copy(e.path(), to.join(&name))?;
            }
        }
        Ok(())
    }
    let mut budget = 500 * 1024 * 1024;
    if let Err(e) = walk(src, &dest, &mut budget) {
        let _ = fs::remove_dir_all(&dest);
        return Err(format!("Could not copy the extension: {e}"));
    }
    Ok(dest)
}

/// Delete Shunno's copy of an extension (only ever inside the extensions folder).
pub fn delete_copy(path: &str) {
    let p = PathBuf::from(path);
    if let (Ok(p), Ok(root)) = (p.canonicalize(), root_dir().canonicalize()) {
        if p.starts_with(&root) && p != root {
            let _ = fs::remove_dir_all(&p);
            // An archive that wrapped everything in one folder left an empty parent: remove it too.
            let mut up = p.parent();
            while let Some(d) = up.filter(|d| *d != root && d.starts_with(&root)) {
                if fs::remove_dir(d).is_err() {
                    break; // not empty
                }
                up = d.parent();
            }
        }
    }
}

// ------------------------------------------------------------------ pickers

/// Native picker: a folder (unpacked extension) or a .crx/.zip file.
#[cfg(windows)]
pub fn pick(hwnd: isize, folder: bool) -> Option<PathBuf> {
    use windows::core::w;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{FileOpenDialog, IFileOpenDialog, FOS_PICKFOLDERS, SIGDN_FILESYSPATH};
    unsafe {
        let dlg: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        if folder {
            let opts = dlg.GetOptions().ok()?;
            let _ = dlg.SetOptions(opts | FOS_PICKFOLDERS);
            let _ = dlg.SetTitle(w!("Choose an unpacked extension folder (with manifest.json)"));
        } else {
            let filters = [
                COMDLG_FILTERSPEC { pszName: w!("Extension (*.crx, *.zip)"), pszSpec: w!("*.crx;*.zip") },
                COMDLG_FILTERSPEC { pszName: w!("All files"), pszSpec: w!("*.*") },
            ];
            let _ = dlg.SetFileTypes(&filters);
            let _ = dlg.SetTitle(w!("Add an extension file"));
        }
        dlg.Show(HWND(hwnd)).ok()?;
        let item = dlg.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
        s.map(PathBuf::from)
    }
}
#[cfg(not(windows))]
pub fn pick(_hwnd: isize, _folder: bool) -> Option<PathBuf> {
    None
}
