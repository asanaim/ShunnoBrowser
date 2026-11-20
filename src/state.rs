use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

/// Stored history entries (newest first).
pub const HISTORY_CAP: usize = 250;

pub const APP_VERSION: &str = "v2.14.2";

/// Build a `Command` that never flashes a console window (this is a GUI subsystem app).
pub fn hidden_command<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchEngine {
    pub name: String,
    pub query_url: String,
}

fn default_site_zoom() -> f64 {
    1.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SitePolicy {
    pub adblock: bool,
    pub js_enabled: bool,
    pub images_enabled: bool,
    /// Request the mobile version of the site (real HTTP User-Agent). Replaces the old
    /// desktop_ua switch, which only spoofed navigator.userAgent and did nothing on desktop.
    #[serde(default)]
    pub mobile_ua: bool,
    pub sponsorblock: bool,
    pub auto_reject_cookies: bool,
    /// Fingerprinting protection (per-site farbling of canvas, WebGL, audio, CPU count).
    #[serde(default = "default_true")]
    pub fp_protect: bool,
    #[serde(default = "default_site_zoom")]
    pub zoom: f64,
}

impl Default for SitePolicy {
    fn default() -> Self {
        Self {
            adblock: true,
            js_enabled: true,
            images_enabled: true,
            mobile_ua: false,
            sponsorblock: true,
            auto_reject_cookies: true,
            fp_protect: true,
            zoom: 1.0,
        }
    }
}

fn default_bookmark_folder() -> String {
    "General".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BookmarkEntry {
    pub title: String,
    pub url: String,
    #[serde(default = "default_bookmark_folder")]
    pub folder: String,
    pub created_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub title: String,
    pub url: String,
    pub visited_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedTab {
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuickShortcut {
    pub title: String,
    pub url: String,
}

fn default_max_concurrent() -> usize { 3 }
fn default_on_complete() -> String { "NONE".to_string() }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub full_dark_mode: bool,
    pub background_media_play: bool,
    pub strip_tracking_params: bool,
    pub block_popups: bool,
    pub drop_basket_enabled: bool,
    #[serde(default)]
    pub auto_pip_on_blur: bool,
    #[serde(default)]
    pub auto_clean_completed: bool,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_downloads: usize,
    #[serde(default = "default_on_complete")]
    pub on_queue_complete: String,
    #[serde(default)]
    pub custom_downloads_dir: Option<String>,
    /// WebView2 built-in password manager (DPAPI-encrypted in the profile). Never active in private tabs.
    #[serde(default = "default_true")]
    pub save_passwords: bool,
    /// Check for signed updates daily.
    #[serde(default = "default_true")]
    pub auto_update: bool,
    /// Upgrade http:// navigations to https:// and warn when a site has no secure version.
    #[serde(default = "default_true")]
    pub https_only: bool,
    /// Edge tracking prevention: Strict when true, Balanced when false.
    #[serde(default)]
    pub strict_tracking: bool,
    #[serde(default)]
    pub approved_script_hashes: Vec<String>,
    #[serde(default)]
    pub quick_shortcuts: Vec<QuickShortcut>,
    pub default_threads: usize,
    pub speed_limit_bps: u64,
    pub active_search_engine: usize,
    pub search_engines: Vec<SearchEngine>,
}

fn default_true() -> bool {
    true
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            full_dark_mode: false,
            background_media_play: true,
            strip_tracking_params: true,
            block_popups: true,
            drop_basket_enabled: true,
            auto_pip_on_blur: false,
            auto_clean_completed: false,
            max_concurrent_downloads: 3,
            on_queue_complete: "NONE".to_string(),
            custom_downloads_dir: None,
            save_passwords: true,
            https_only: true,
            auto_update: true,
            strict_tracking: false,
            approved_script_hashes: Vec::new(),
            quick_shortcuts: vec![
                QuickShortcut { title: "DuckDuckGo".to_string(), url: "https://duckduckgo.com".to_string() },
                QuickShortcut { title: "YouTube".to_string(), url: "https://www.youtube.com".to_string() },
                QuickShortcut { title: "GitHub".to_string(), url: "https://github.com".to_string() },
                QuickShortcut { title: "Wikipedia".to_string(), url: "https://www.wikipedia.org".to_string() },
                QuickShortcut { title: "Internet Archive".to_string(), url: "https://archive.org".to_string() },
                QuickShortcut { title: "Krishi Bank".to_string(), url: "https://krishibank.org.bd".to_string() },
            ],
            default_threads: 16,
            speed_limit_bps: 0,
            active_search_engine: 0,
            search_engines: vec![
                SearchEngine {
                    name: "DuckDuckGo".to_string(),
                    query_url: "https://duckduckgo.com/?q=%s".to_string(),
                },
                SearchEngine {
                    name: "Google".to_string(),
                    query_url: "https://www.google.com/search?q=%s".to_string(),
                },
                SearchEngine {
                    name: "Startpage".to_string(),
                    query_url: "https://www.startpage.com/sp/search?query=%s".to_string(),
                },
                SearchEngine {
                    name: "Bing".to_string(),
                    query_url: "https://www.bing.com/search?q=%s".to_string(),
                },
                SearchEngine {
                    name: "YouTube".to_string(),
                    query_url: "https://www.youtube.com/results?search_query=%s".to_string(),
                },
            ],
        }
    }
}

pub struct BrowserStorage;

impl BrowserStorage {
    pub fn app_data_dir() -> PathBuf {
        let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        let dir = base.join("ShunnoBrowser");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn custom_dl_lock() -> &'static Mutex<Option<String>> {
        static CUSTOM_DL_DIR: OnceLock<Mutex<Option<String>>> = OnceLock::new();
        CUSTOM_DL_DIR.get_or_init(|| Mutex::new(None))
    }

    pub fn set_custom_downloads_dir(dir: Option<String>) {
        let cleaned = dir
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if let Ok(mut guard) = Self::custom_dl_lock().lock() {
            *guard = cleaned;
        }
    }

    pub fn downloads_root_dir() -> PathBuf {
        if let Ok(guard) = Self::custom_dl_lock().lock() {
            if let Some(ref custom) = *guard {
                let p = PathBuf::from(custom);
                if fs::create_dir_all(&p).is_ok() {
                    return p;
                }
            }
        }
        let base = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let dir = base.join("ShunnoBrowser");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    /// Helper tools (ffmpeg.exe, yt-dlp.exe) live in app data, so changing the
    /// downloads folder never "loses" them.
    pub fn ffmpeg_bin_dir() -> PathBuf {
        let dir = Self::app_data_dir().join("bin");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn legacy_bin_dir() -> PathBuf {
        // Versions <= 2.9.6 stored tools under <Downloads>\ShunnoBrowser\bin
        Self::downloads_root_dir().join("bin")
    }

    fn find_tool(exe_name: &str, probe_arg: &str) -> Option<PathBuf> {
        let local_exe = Self::ffmpeg_bin_dir().join(exe_name);
        if local_exe.is_file() {
            return Some(local_exe);
        }
        let legacy = Self::legacy_bin_dir().join(exe_name);
        if legacy.is_file() {
            return Some(legacy);
        }
        let bare = exe_name.trim_end_matches(".exe");
        if let Ok(out) = hidden_command(bare).arg(probe_arg).output() {
            if out.status.success() {
                return Some(PathBuf::from(bare));
            }
        }
        None
    }

    pub fn find_ffmpeg() -> Option<PathBuf> {
        Self::find_tool("ffmpeg.exe", "-version")
    }

    pub fn find_ytdlp() -> Option<PathBuf> {
        Self::find_tool("yt-dlp.exe", "--version")
    }

    pub fn load_global_settings() -> GlobalSettings {
        let path = Self::app_data_dir().join("global_settings.json");
        let mut cfg: GlobalSettings = fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if cfg.search_engines.is_empty() {
            cfg.search_engines = GlobalSettings::default().search_engines;
        }
        if cfg.active_search_engine >= cfg.search_engines.len() {
            cfg.active_search_engine = 0;
        }
        if cfg.quick_shortcuts.is_empty() {
            cfg.quick_shortcuts = GlobalSettings::default().quick_shortcuts;
        }
        Self::set_custom_downloads_dir(cfg.custom_downloads_dir.clone());
        cfg
    }

    pub fn save_global_settings(settings: &GlobalSettings) {
        Self::set_custom_downloads_dir(settings.custom_downloads_dir.clone());
        let path = Self::app_data_dir().join("global_settings.json");
        if let Ok(json) = serde_json::to_string_pretty(settings) {
            let _ = fs::write(path, json);
        }
    }

    pub fn load_site_policies() -> HashMap<String, SitePolicy> {
        let path = Self::app_data_dir().join("site_policies.json");
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_site_policies(policies: &HashMap<String, SitePolicy>) {
        let path = Self::app_data_dir().join("site_policies.json");
        if let Ok(json) = serde_json::to_string_pretty(policies) {
            let _ = fs::write(path, json);
        }
    }

    pub fn load_bookmarks() -> Vec<BookmarkEntry> {
        let path = Self::app_data_dir().join("bookmarks.json");
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| {
                vec![
                    BookmarkEntry {
                        title: "DuckDuckGo Search".to_string(),
                        url: "https://duckduckgo.com".to_string(),
                        folder: "Search".to_string(),
                        created_at: 1,
                    },
                    BookmarkEntry {
                        title: "YouTube".to_string(),
                        url: "https://www.youtube.com".to_string(),
                        folder: "Media".to_string(),
                        created_at: 2,
                    },
                    BookmarkEntry {
                        title: "Internet Archive".to_string(),
                        url: "https://archive.org".to_string(),
                        folder: "Media".to_string(),
                        created_at: 3,
                    },
                ]
            })
    }

    pub fn save_bookmarks(bookmarks: &[BookmarkEntry]) {
        let path = Self::app_data_dir().join("bookmarks.json");
        if let Ok(json) = serde_json::to_string_pretty(bookmarks) {
            let _ = fs::write(path, json);
        }
    }

    pub fn export_portable_txt_bookmarks(bookmarks: &[BookmarkEntry]) -> PathBuf {
        let out_path = Self::downloads_root_dir().join("shunno_bookmarks.txt");
        let mut lines = Vec::with_capacity(bookmarks.len());
        for b in bookmarks {
            lines.push(format!("{} | {}", b.title.replace('|', "-"), b.url));
        }
        let _ = fs::write(&out_path, lines.join("\n"));
        out_path
    }

    pub fn import_portable_txt_bookmarks(existing: &mut Vec<BookmarkEntry>) -> usize {
        let in_path = Self::downloads_root_dir().join("shunno_bookmarks.txt");
        let Ok(content) = fs::read_to_string(in_path) else {
            return 0;
        };
        let mut added = 0;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (title, url) = if let Some((t, u)) = trimmed.split_once('|') {
                (t.trim().to_string(), u.trim().to_string())
            } else {
                (trimmed.to_string(), trimmed.to_string())
            };
            if url.starts_with("http") && !existing.iter().any(|b| b.url == url) {
                existing.insert(
                    0,
                    BookmarkEntry {
                        title,
                        url,
                        folder: "General".to_string(),
                        created_at: 0,
                    },
                );
                added += 1;
            }
        }
        if added > 0 {
            Self::save_bookmarks(existing);
        }
        added
    }

    pub fn load_history() -> Vec<HistoryEntry> {
        let path = Self::app_data_dir().join("history.json");
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_history(history: &[HistoryEntry]) {
        let path = Self::app_data_dir().join("history.json");
        let capped: Vec<HistoryEntry> = history.iter().take(HISTORY_CAP).cloned().collect();
        if let Ok(json) = serde_json::to_string_pretty(&capped) {
            let _ = fs::write(path, json);
        }
    }

    pub fn load_session() -> Vec<SavedTab> {
        let path = Self::app_data_dir().join("session.json");
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_session(tabs: &[SavedTab]) {
        let path = Self::app_data_dir().join("session.json");
        if let Ok(json) = serde_json::to_string_pretty(tabs) {
            let _ = fs::write(path, json);
        }
    }
}
