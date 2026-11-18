use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub const APP_VERSION: &str = "v1.4.0";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchEngine {
    pub name: String,
    pub query_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SitePolicy {
    pub adblock: bool,
    pub js_enabled: bool,
    pub images_enabled: bool,
    pub desktop_ua: bool,
    pub sponsorblock: bool,
    pub auto_reject_cookies: bool,
}

impl Default for SitePolicy {
    fn default() -> Self {
        Self {
            adblock: true,
            js_enabled: true,
            images_enabled: true,
            desktop_ua: false,
            sponsorblock: true,
            auto_reject_cookies: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BookmarkEntry {
    pub title: String,
    pub url: String,
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
    #[serde(default)]
    pub approved_script_hashes: Vec<String>,
    pub default_threads: usize,
    pub speed_limit_bps: u64,
    pub active_search_engine: usize,
    pub search_engines: Vec<SearchEngine>,
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
            approved_script_hashes: Vec::new(),
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

    pub fn downloads_root_dir() -> PathBuf {
        let base = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let dir = base.join("ShunnoBrowser");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn ffmpeg_bin_dir() -> PathBuf {
        let dir = Self::downloads_root_dir().join("bin");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn find_ffmpeg() -> Option<PathBuf> {
        let local_exe = Self::ffmpeg_bin_dir().join("ffmpeg.exe");
        if local_exe.exists() {
            return Some(local_exe);
        }
        if let Ok(out) = Command::new("ffmpeg").arg("-version").output() {
            if out.status.success() {
                return Some(PathBuf::from("ffmpeg"));
            }
        }
        None
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
        cfg
    }

    pub fn save_global_settings(settings: &GlobalSettings) {
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
                        created_at: 1,
                    },
                    BookmarkEntry {
                        title: "YouTube".to_string(),
                        url: "https://www.youtube.com".to_string(),
                        created_at: 2,
                    },
                    BookmarkEntry {
                        title: "Internet Archive (Lossless Audio & Video)".to_string(),
                        url: "https://archive.org".to_string(),
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
        let capped: Vec<HistoryEntry> = history.iter().take(250).cloned().collect();
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
