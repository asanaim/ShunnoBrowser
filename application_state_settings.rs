use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SitePermissions {
    pub javascript: bool,
    pub cookies: bool,
    pub images: bool,
    pub popups: bool,
    pub ad_block: bool,
    pub desktop_ua: bool,
    pub reader_mode: bool,
    pub dark_mode_override: bool,
    pub zoom_level: f64,
}

impl Default for SitePermissions {
    fn default() -> Self {
        Self {
            javascript: true,
            cookies: true,
            images: true,
            popups: false,
            ad_block: true,
            desktop_ua: true,
            reader_mode: false,
            dark_mode_override: false,
            zoom_level: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchEngine {
    pub id: String,
    pub name: String,
    pub url_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub home_url: String,
    pub active_engine_id: String,
    pub search_engines: Vec<SearchEngine>,
    pub full_dark_mode: bool,
    pub floating_basket_enabled: bool,
    pub auto_mux_dash: bool,
    pub download_dir: String,
    pub max_segments: usize,
    pub restore_session: bool,
    pub block_trackers: bool,
    pub strip_youtube_ads: bool,
    pub default_permissions: SitePermissions,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        let dl_dir = dirs::download_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\Downloads"))
            .join("ShunnoDownloads");
        let _ = fs::create_dir_all(&dl_dir);

        Self {
            home_url: "https://duckduckgo.com".to_string(),
            active_engine_id: "ddg".to_string(),
            search_engines: vec![
                SearchEngine {
                    id: "ddg".to_string(),
                    name: "DuckDuckGo".to_string(),
                    url_template: "https://duckduckgo.com/?q=%s".to_string(),
                },
                SearchEngine {
                    id: "google".to_string(),
                    name: "Google".to_string(),
                    url_template: "https://www.google.com/search?q=%s".to_string(),
                },
                SearchEngine {
                    id: "startpage".to_string(),
                    name: "Startpage".to_string(),
                    url_template: "https://www.startpage.com/sp/search?query=%s".to_string(),
                },
                SearchEngine {
                    id: "bing".to_string(),
                    name: "Bing".to_string(),
                    url_template: "https://www.bing.com/search?q=%s".to_string(),
                },
                SearchEngine {
                    id: "youtube".to_string(),
                    name: "YouTube".to_string(),
                    url_template: "https://www.youtube.com/results?search_query=%s".to_string(),
                },
            ],
            full_dark_mode: true,
            floating_basket_enabled: false,
            auto_mux_dash: true,
            download_dir: dl_dir.to_string_lossy().to_string(),
            max_segments: 8,
            restore_session: true,
            block_trackers: true,
            strip_youtube_ads: true,
            default_permissions: SitePermissions::default(),
        }
    }
}

impl GlobalSettings {
    pub fn active_search_template(&self) -> String {
        self.search_engines
            .iter()
            .find(|e| e.id == self.active_engine_id)
            .or_else(|| self.search_engines.first())
            .map(|e| e.url_template.clone())
            .unwrap_or_else(|| "https://duckduckgo.com/?q=%s".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub title: String,
    pub url: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub title: String,
    pub url: String,
    pub visited_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabSession {
    pub id: String,
    pub title: String,
    pub url: String,
    pub is_incognito: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSession {
    pub active_tab_id: String,
    pub tabs: Vec<TabSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Muxing,
    Paused,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: String,
    pub filename: String,
    pub url: String,
    pub save_path: String,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub speed_bps: u64,
    pub segments: usize,
    pub supports_range: bool,
    pub status: DownloadStatus,
    pub category: String,
    pub created_at: DateTime<Utc>,
}

pub struct AppStorage {
    pub data_dir: PathBuf,
    pub settings: GlobalSettings,
    pub site_permissions: HashMap<String, SitePermissions>,
    pub bookmarks: Vec<Bookmark>,
    pub history: Vec<HistoryEntry>,
    pub downloads: Vec<DownloadItem>,
}

impl AppStorage {
    pub fn load() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ShunnoBrowser");
        let _ = fs::create_dir_all(&data_dir);

        let settings: GlobalSettings = Self::read_json(&data_dir.join("global_settings.json"))
            .unwrap_or_default();
        let site_permissions: HashMap<String, SitePermissions> =
            Self::read_json(&data_dir.join("site_permissions.json")).unwrap_or_default();
        let bookmarks: Vec<Bookmark> =
            Self::read_json(&data_dir.join("bookmarks.json")).unwrap_or_default();
        let history: Vec<HistoryEntry> =
            Self::read_json(&data_dir.join("history.json")).unwrap_or_default();
        let mut downloads: Vec<DownloadItem> =
            Self::read_json(&data_dir.join("downloads.json")).unwrap_or_default();

        for item in &mut downloads {
            if matches!(
                item.status,
                DownloadStatus::Downloading | DownloadStatus::Queued | DownloadStatus::Muxing
            ) {
                item.status = DownloadStatus::Paused;
                item.speed_bps = 0;
            }
        }

        Self {
            data_dir,
            settings,
            site_permissions,
            bookmarks,
            history,
            downloads,
        }
    }

    fn read_json<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Option<T> {
        let raw = fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    fn write_json<T: Serialize>(path: &PathBuf, value: &T) {
        if let Ok(json) = serde_json::to_string_pretty(value) {
            let _ = fs::write(path, json);
        }
    }

    pub fn save_settings(&self) {
        Self::write_json(&self.data_dir.join("global_settings.json"), &self.settings);
    }

    pub fn save_permissions(&self) {
        Self::write_json(
            &self.data_dir.join("site_permissions.json"),
            &self.site_permissions,
        );
    }

    pub fn save_bookmarks(&self) {
        Self::write_json(&self.data_dir.join("bookmarks.json"), &self.bookmarks);
    }

    pub fn save_history(&self) {
        Self::write_json(&self.data_dir.join("history.json"), &self.history);
    }

    pub fn save_downloads(&self) {
        Self::write_json(&self.data_dir.join("downloads.json"), &self.downloads);
    }

    pub fn save_session(&self, session: &SavedSession) {
        Self::write_json(&self.data_dir.join("session.json"), session);
    }

    pub fn load_session(&self) -> Option<SavedSession> {
        if !self.settings.restore_session {
            return None;
        }
        Self::read_json(&self.data_dir.join("session.json"))
    }

    pub fn permissions_for_host(&self, host: &str) -> SitePermissions {
        self.site_permissions
            .get(host)
            .cloned()
            .unwrap_or_else(|| self.settings.default_permissions.clone())
    }
}