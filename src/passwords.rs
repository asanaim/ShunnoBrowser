//! Shunno password vault.
//!
//! * Storage: one file, `passwords.vault`, encrypted with Windows DPAPI (CryptProtectData, current
//!   user scope, app-specific entropy). Only this Windows account can decrypt it.
//! * Matching: exact scheme + host + port; `www.` is ignored. No guessing across registrable domains
//!   (a two-label heuristic would treat every `*.com.bd` site as one site).
//! * Pages never receive a password unless the user picks an account in the fill menu.
//! * Reveal / copy in the Passwords panel are gated by Windows Hello when it is set up.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Credential {
    pub id: u64,
    /// `scheme://host[:port]`, lowercase.
    pub origin: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub updated: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VaultData {
    #[serde(default)]
    pub creds: Vec<Credential>,
    /// Origins the user chose "Never" for.
    #[serde(default)]
    pub never: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum OfferKind {
    Save,
    Update,
}

/// What the Passwords panel shows (no secrets).
#[derive(Clone, Debug, Serialize)]
pub struct CredentialView {
    pub id: u64,
    pub origin: String,
    pub username: String,
}

// ---------------------------------------------------------------- origins

/// `scheme://host[:port]` for http(s) URLs, lowercase; None for anything else.
pub fn origin_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url.trim()).ok()?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return None;
    }
    let host = u.host_str()?.to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    Some(match u.port() {
        Some(p) => format!("{}://{}:{}", u.scheme(), host, p),
        None => format!("{}://{}", u.scheme(), host),
    })
}

fn strip_www(origin: &str) -> String {
    origin.replacen("://www.", "://", 1)
}

/// Same site for credential purposes: identical origin, ignoring a leading `www.`.
pub fn same_site(a: &str, b: &str) -> bool {
    strip_www(a) == strip_www(b)
}

// ---------------------------------------------------------------- vault logic

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl VaultData {
    fn next_id(&self) -> u64 {
        self.creds.iter().map(|c| c.id).max().unwrap_or(0) + 1
    }

    pub fn accounts_for(&self, origin: &str) -> Vec<(u64, String)> {
        let mut v: Vec<&Credential> = self.creds.iter().filter(|c| same_site(&c.origin, origin)).collect();
        v.sort_by(|a, b| b.updated.cmp(&a.updated));
        v.into_iter().map(|c| (c.id, c.username.clone())).collect()
    }

    pub fn get(&self, id: u64) -> Option<&Credential> {
        self.creds.iter().find(|c| c.id == id)
    }

    pub fn is_never(&self, origin: &str) -> bool {
        self.never.iter().any(|n| same_site(n, origin))
    }

    /// Whether a submitted login should prompt Save, Update, or nothing.
    pub fn offer_kind(&self, origin: &str, username: &str, password: &str) -> Option<OfferKind> {
        if password.is_empty() || self.is_never(origin) {
            return None;
        }
        match self.creds.iter().find(|c| same_site(&c.origin, origin) && c.username == username) {
            Some(c) if c.password == password => None,
            Some(_) => Some(OfferKind::Update),
            None => Some(OfferKind::Save),
        }
    }

    /// Insert or update by (site, username). Returns the credential id.
    pub fn upsert(&mut self, origin: &str, username: &str, password: &str) -> u64 {
        let t = now();
        if let Some(c) = self.creds.iter_mut().find(|c| same_site(&c.origin, origin) && c.username == username) {
            c.password = password.to_string();
            c.updated = t;
            return c.id;
        }
        let id = self.next_id();
        self.creds.push(Credential {
            id,
            origin: origin.to_string(),
            username: username.to_string(),
            password: password.to_string(),
            created: t,
            updated: t,
        });
        id
    }

    pub fn edit(&mut self, id: u64, username: &str, password: &str) -> bool {
        if password.is_empty() {
            return false;
        }
        let clash = self.get(id).map(|c| c.origin.clone()).map(|o| {
            self.creds.iter().any(|x| x.id != id && same_site(&x.origin, &o) && x.username == username)
        });
        if clash != Some(false) {
            return false;
        }
        if let Some(c) = self.creds.iter_mut().find(|c| c.id == id) {
            c.username = username.to_string();
            c.password = password.to_string();
            c.updated = now();
            return true;
        }
        false
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let n = self.creds.len();
        self.creds.retain(|c| c.id != id);
        n != self.creds.len()
    }

    pub fn add_never(&mut self, origin: &str) {
        if !self.is_never(origin) {
            self.never.push(origin.to_string());
        }
    }

    pub fn views(&self) -> Vec<CredentialView> {
        let mut v: Vec<CredentialView> = self
            .creds
            .iter()
            .map(|c| CredentialView { id: c.id, origin: c.origin.clone(), username: c.username.clone() })
            .collect();
        v.sort_by(|a, b| strip_www(&a.origin).cmp(&strip_www(&b.origin)).then(a.username.cmp(&b.username)));
        v
    }

    /// Import rows from a browser CSV export. Returns (added, updated, skipped).
    pub fn import_csv(&mut self, text: &str) -> Result<(usize, usize, usize), String> {
        let rows = parse_password_csv(text)?;
        let (mut added, mut updated, mut skipped) = (0, 0, 0);
        for (url, user, pass) in rows {
            let Some(origin) = origin_of(&url) else {
                skipped += 1;
                continue;
            };
            if pass.is_empty() {
                skipped += 1;
                continue;
            }
            match self.offer_kind_ignoring_never(&origin, &user, &pass) {
                Some(OfferKind::Save) => added += 1,
                Some(OfferKind::Update) => updated += 1,
                None => {
                    skipped += 1;
                    continue;
                }
            }
            self.upsert(&origin, &user, &pass);
        }
        Ok((added, updated, skipped))
    }

    fn offer_kind_ignoring_never(&self, origin: &str, username: &str, password: &str) -> Option<OfferKind> {
        match self.creds.iter().find(|c| same_site(&c.origin, origin) && c.username == username) {
            Some(c) if c.password == password => None,
            Some(_) => Some(OfferKind::Update),
            None => Some(OfferKind::Save),
        }
    }
}

// ---------------------------------------------------------------- CSV

/// RFC 4180 CSV (quoted fields, doubled quotes, newlines inside quotes, CRLF).
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_q = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_q {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => in_q = false,
                _ => field.push(c),
            }
        } else {
            match c {
                '"' => in_q = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\r' => {}
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    if !(row.len() == 1 && row[0].is_empty()) {
                        rows.push(std::mem::take(&mut row));
                    } else {
                        row.clear();
                    }
                }
                _ => field.push(c),
            }
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// (url, username, password) rows from Chrome / Edge / Brave / Opera / Vivaldi (`name,url,username,password,note`),
/// Firefox (`url,username,password,httpRealm,...`) and Bitwarden (`login_uri,login_username,login_password`) exports.
pub fn parse_password_csv(text: &str) -> Result<Vec<(String, String, String)>, String> {
    let rows = parse_csv(text);
    let header = rows.first().ok_or("The file is empty.")?;
    let col = |names: &[&str]| header.iter().position(|h| names.iter().any(|n| h.trim().eq_ignore_ascii_case(n)));
    let iu = col(&["url", "login_uri", "website", "origin", "web site"]).ok_or("No URL column; is this a password export?")?;
    let iname = col(&["username", "login_username", "user name", "login"]).ok_or("No username column.")?;
    let ipass = col(&["password", "login_password"]).ok_or("No password column.")?;
    Ok(rows
        .iter()
        .skip(1)
        .filter_map(|r| Some((r.get(iu)?.trim().to_string(), r.get(iname).cloned().unwrap_or_default(), r.get(ipass)?.clone())))
        .collect())
}

// ---------------------------------------------------------------- pending save offer

#[derive(Clone, Debug, Serialize)]
pub struct Offer {
    pub origin: String,
    pub username: String,
    #[serde(skip)]
    pub password: String,
    pub kind: OfferKind,
}

// ---------------------------------------------------------------- storage (Windows DPAPI)

const MAGIC: &[u8] = b"SHPV1\n";
const ENTROPY: &[u8] = b"ShunnoBrowser.PasswordVault.v1";

fn vault_path() -> std::path::PathBuf {
    crate::state::BrowserStorage::app_data_dir().join("passwords.vault")
}

#[cfg(windows)]
fn dpapi(data: &[u8], protect: bool) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let entropy = CRYPT_INTEGER_BLOB { cbData: ENTROPY.len() as u32, pbData: ENTROPY.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        let r = if protect {
            CryptProtectData(&input, windows::core::w!("Shunno passwords"), Some(&entropy), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        } else {
            CryptUnprotectData(&input, None, Some(&entropy), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        };
        r.map_err(|e| e.to_string())?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(out.pbData as *mut core::ffi::c_void));
        Ok(v)
    }
}
#[cfg(not(windows))]
fn dpapi(_data: &[u8], _protect: bool) -> Result<Vec<u8>, String> {
    Err("password vault requires Windows".into())
}

fn load() -> VaultData {
    let Ok(raw) = std::fs::read(vault_path()) else { return VaultData::default() };
    let Some(blob) = raw.strip_prefix(MAGIC) else { return VaultData::default() };
    dpapi(blob, false).ok().and_then(|plain| serde_json::from_slice(&plain).ok()).unwrap_or_default()
}

fn save(v: &VaultData) -> Result<(), String> {
    let plain = serde_json::to_vec(v).map_err(|e| e.to_string())?;
    let blob = dpapi(&plain, true)?;
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&blob);
    let path = vault_path();
    let tmp = path.with_extension("vault.tmp");
    std::fs::write(&tmp, &out).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub struct Vault {
    data: Mutex<Option<VaultData>>,
    pub offer: Mutex<Option<Offer>>,
    pub status: Mutex<String>,
    /// Unix seconds until which reveal/copy is allowed without asking Windows Hello again.
    pub unlocked_until: std::sync::atomic::AtomicU64,
    /// A plaintext CSV was imported and not yet deleted.
    pub csv_pending: std::sync::atomic::AtomicBool,
}

pub fn global() -> &'static Vault {
    static V: std::sync::OnceLock<Vault> = std::sync::OnceLock::new();
    V.get_or_init(|| Vault {
        data: Mutex::new(None),
        offer: Mutex::new(None),
        status: Mutex::new(String::new()),
        unlocked_until: std::sync::atomic::AtomicU64::new(0),
        csv_pending: std::sync::atomic::AtomicBool::new(false),
    })
}

impl Vault {
    /// Run `f` on the (lazily loaded) vault; persist if it returns `true` as the second value.
    pub fn with<R>(&self, f: impl FnOnce(&mut VaultData) -> (R, bool)) -> R {
        let mut g = self.data.lock().unwrap_or_else(|e| e.into_inner());
        let v = g.get_or_insert_with(load);
        let (r, dirty) = f(v);
        if dirty {
            if let Err(e) = save(v) {
                *self.status.lock().unwrap_or_else(|e| e.into_inner()) = format!("Could not save passwords: {e}");
            }
        }
        r
    }

    pub fn set_status(&self, s: impl Into<String>) {
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) = s.into();
    }

    pub fn is_unlocked(&self) -> bool {
        self.unlocked_until.load(std::sync::atomic::Ordering::Relaxed) > now()
    }

    pub fn unlock_for(&self, secs: u64) {
        self.unlocked_until.store(now() + secs, std::sync::atomic::Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------- Windows Hello

pub enum HelloResult {
    Verified,
    /// Hello is not set up on this PC/account: allowed, and the UI suggests setting it up.
    Unavailable,
    Denied,
}

/// Ask Windows Hello (PIN, face, fingerprint) to confirm the user. Blocks: call off the UI thread.
#[cfg(windows)]
pub fn verify_user(hwnd: isize, message: &str) -> HelloResult {
    use windows::core::{factory, HSTRING};
    use windows::Foundation::IAsyncOperation;
    use windows::Security::Credentials::UI::{UserConsentVerificationResult, UserConsentVerifier};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::WinRT::IUserConsentVerifierInterop;
    let run = || -> windows::core::Result<UserConsentVerificationResult> {
        let interop = factory::<UserConsentVerifier, IUserConsentVerifierInterop>()?;
        let op: IAsyncOperation<UserConsentVerificationResult> =
            unsafe { interop.RequestVerificationForWindowAsync(HWND(hwnd), &HSTRING::from(message))? };
        op.get()
    };
    match run() {
        Ok(UserConsentVerificationResult::Verified) => HelloResult::Verified,
        Ok(UserConsentVerificationResult::DeviceNotPresent)
        | Ok(UserConsentVerificationResult::NotConfiguredForUser)
        | Ok(UserConsentVerificationResult::DisabledByPolicy) => HelloResult::Unavailable,
        Ok(_) => HelloResult::Denied,
        Err(_) => HelloResult::Unavailable,
    }
}
#[cfg(not(windows))]
pub fn verify_user(_hwnd: isize, _message: &str) -> HelloResult {
    HelloResult::Unavailable
}

// ---------------------------------------------------------------- clipboard

/// Put text on the clipboard (Unicode).
#[cfg(windows)]
pub fn set_clipboard(text: &str) -> bool {
    use windows::Win32::Foundation::{HANDLE, HWND};
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    const CF_UNICODETEXT: u32 = 13;
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        if OpenClipboard(HWND(0)).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let ok = (|| {
            let h = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2).ok()?;
            let p = GlobalLock(h) as *mut u16;
            if p.is_null() {
                return None;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
            let _ = GlobalUnlock(h);
            SetClipboardData(CF_UNICODETEXT, HANDLE(h.0 as isize)).ok()
        })()
        .is_some();
        let _ = CloseClipboard();
        ok
    }
}
#[cfg(not(windows))]
pub fn set_clipboard(_text: &str) -> bool {
    false
}

// ---------------------------------------------------------------- file picker

/// Native "Open" dialog for a CSV file. Returns the chosen path.
#[cfg(windows)]
pub fn pick_csv(hwnd: isize) -> Option<std::path::PathBuf> {
    use windows::core::w;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH};
    unsafe {
        let dlg: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let filters = [
            COMDLG_FILTERSPEC { pszName: w!("Password export (*.csv)"), pszSpec: w!("*.csv") },
            COMDLG_FILTERSPEC { pszName: w!("All files"), pszSpec: w!("*.*") },
        ];
        let _ = dlg.SetFileTypes(&filters);
        let _ = dlg.SetTitle(w!("Import passwords from a CSV export"));
        dlg.Show(HWND(hwnd)).ok()?;
        let item = dlg.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
        s.map(std::path::PathBuf::from)
    }
}
#[cfg(not(windows))]
pub fn pick_csv(_hwnd: isize) -> Option<std::path::PathBuf> {
    None
}
