use crate::state::BrowserStorage;
use futures_util::StreamExt;
use reqwest::header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, COOKIE, RANGE, REFERER, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: u64,
    pub url: String,
    pub file_name: String,
    pub total_bytes: i64,
    pub downloaded_bytes: u64,
    pub status: String, // "DOWNLOADING", "PAUSED", "COMPLETED", "FAILED", "MUXING", "DUPLICATE"
    pub speed_bps: u64,
    pub threads: usize,
    pub category: String,
    pub supports_ranges: bool,
    pub referer: Option<String>,
    pub cookie: Option<String>,
    pub sha256: Option<String>,
    pub saved_path: Option<String>,
    pub error_message: Option<String>,
    pub segment_map: String,
    #[serde(default)]
    pub scheduled_start_epoch: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamProbeResult {
    pub url: String,
    pub size_bytes: Option<u64>,
    pub size_label: String,
    pub resolved_name: Option<String>,
}

pub enum EnqueueOutcome {
    Started(DownloadTask),
    DuplicateDetected(DownloadTask),
}

pub fn sanitize_download_filename(raw: &str) -> String {
    let base = raw
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("download.bin")
        .split('?')
        .next()
        .unwrap_or("download.bin")
        .trim();

    let mut cleaned: String = base
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'..='\x1f' => '_',
            _ => c,
        })
        .collect();

    while cleaned.contains("..") {
        cleaned = cleaned.replace("..", "_");
    }
    cleaned = cleaned.trim_matches(['.', ' ']).to_string();
    if cleaned.is_empty() {
        cleaned = "download.bin".to_string();
    }

    // Prefix Windows reserved device names (CON, PRN, AUX, NUL, COM1-9, LPT1-9)
    let stem = cleaned
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    let is_reserved = matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if is_reserved {
        cleaned = format!("_{}", cleaned);
    }

    // Prevent writing into any scripts subdirectory
    if cleaned.to_ascii_lowercase().ends_with(".js") {
        cleaned.push_str(".download");
    }

    cleaned
}

pub fn format_human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn detect_download_category(file_name: &str) -> String {
    let ext = file_name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "ts" | "m3u8" | "m4s" => "Videos".to_string(),
        "flac" | "alac" | "m4a" | "wav" | "mp3" | "ogg" | "opus" => "Lossless Audio".to_string(),
        "zip" | "rar" | "7z" | "tar" | "gz" | "iso" => "Archives".to_string(),
        "exe" | "msi" => "Programs".to_string(),
        _ => "Documents".to_string(),
    }
}

fn infer_extension_from_mime(mime: &str) -> Option<&'static str> {
    let m = mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    match m.as_str() {
        "video/mp4" => Some("mp4"),
        "video/webm" => Some("webm"),
        "video/x-matroska" => Some("mkv"),
        _ if m.ends_with("mpegurl") => Some("m3u8"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/mp4" | "audio/x-m4a" => Some("m4a"),
        "audio/mpeg" => Some("mp3"),
        "audio/wav" | "audio/x-wav" => Some("wav"),
        "audio/ogg" => Some("ogg"),
        "application/zip" => Some("zip"),
        "application/x-7z-compressed" => Some("7z"),
        "application/x-rar-compressed" | "application/vnd.rar" => Some("rar"),
        "application/pdf" => Some("pdf"),
        "application/x-msdownload" => Some("exe"),
        _ => None,
    }
}

fn parse_content_disposition_filename(header_val: &str) -> Option<String> {
    for part in header_val.split(';') {
        let trimmed = part.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("filename*=") {
            if let Some((_, val)) = trimmed.split_once('=') {
                let cleaned = val
                    .trim()
                    .trim_start_matches("UTF-8''")
                    .trim_start_matches("utf-8''")
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                if !cleaned.is_empty() {
                    return Some(sanitize_download_filename(&cleaned));
                }
            }
        } else if lower.starts_with("filename=") {
            if let Some((_, val)) = trimmed.split_once('=') {
                let cleaned = val
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                if !cleaned.is_empty() {
                    return Some(sanitize_download_filename(&cleaned));
                }
            }
        }
    }
    None
}

pub struct NativeDownloadEngine {
    client: Client,
    pub tasks: Arc<Mutex<Vec<DownloadTask>>>,
    active_flags: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
    pub speed_limit_bps: Arc<AtomicU64>,
    pub auto_clean_completed: Arc<AtomicBool>,
    pub max_concurrent: Arc<AtomicUsize>,
    pub ffmpeg_install_status: Arc<Mutex<String>>,
    pub user_agent: Arc<Mutex<String>>,
}

impl NativeDownloadEngine {
    pub fn new(initial_speed_limit_bps: u64, initial_auto_clean: bool, initial_max_concurrent: usize) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(25))
            .redirect(reqwest::redirect::Policy::limited(20))
            .pool_max_idle_per_host(64)
            .build()
            .unwrap_or_else(|_| Client::new());

        let loaded = Self::load_queue();
        Self {
            client,
            tasks: Arc::new(Mutex::new(loaded)),
            active_flags: Arc::new(Mutex::new(HashMap::new())),
            speed_limit_bps: Arc::new(AtomicU64::new(initial_speed_limit_bps)),
            auto_clean_completed: Arc::new(AtomicBool::new(initial_auto_clean)),
            max_concurrent: Arc::new(AtomicUsize::new(initial_max_concurrent.clamp(1, 8))),
            ffmpeg_install_status: Arc::new(Mutex::new(String::new())),
            user_agent: Arc::new(Mutex::new("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0".to_string())),
        }
    }

    pub fn install_ffmpeg_bundle(&self, rt: &tokio::runtime::Handle) {
        if BrowserStorage::find_ffmpeg().is_some() {
            if let Ok(mut st) = self.ffmpeg_install_status.lock() {
                *st = "Ready".to_string();
            }
            return;
        }
        if let Ok(mut st) = self.ffmpeg_install_status.lock() {
            if st.starts_with("Downloading") || st.starts_with("Extracting") {
                return;
            }
            *st = "Downloading FFmpeg...".to_string();
        }
        let client = self.client.clone();
        let status_arc = self.ffmpeg_install_status.clone();
        rt.spawn(async move {
            let zip_url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
            let res = async {
                let resp = client
                    .get(zip_url)
                    .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0")
            .header("Accept", "*/*")
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;
                if !resp.status().is_success() {
                    return Err(format!("HTTP {}", resp.status()));
                }
                let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
                if let Ok(mut st) = status_arc.lock() {
                    *st = "Extracting ffmpeg.exe...".to_string();
                }
                let cursor = std::io::Cursor::new(bytes);
                let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
                let out_exe = BrowserStorage::ffmpeg_bin_dir().join("ffmpeg.exe");
                for i in 0..archive.len() {
                    let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
                    if entry.name().ends_with("/bin/ffmpeg.exe") || entry.name().eq_ignore_ascii_case("ffmpeg.exe") {
                        let mut out_f = File::create(&out_exe).map_err(|e| e.to_string())?;
                        std::io::copy(&mut entry, &mut out_f).map_err(|e| e.to_string())?;
                        return Ok::<(), String>(());
                    }
                }
                Err("ffmpeg.exe not found in archive".to_string())
            }
            .await;

            if let Ok(mut st) = status_arc.lock() {
                match res {
                    Ok(()) => *st = "Installed!".to_string(),
                    Err(e) => *st = format!("Failed: {}", e),
                }
            }
        });
    }

    pub fn set_user_agent(&self, ua: &str) {
        let trimmed = ua.trim();
        if !trimmed.is_empty() {
            if let Ok(mut guard) = self.user_agent.lock() {
                *guard = trimmed.to_string();
            }
        }
    }

    pub fn current_user_agent(&self) -> String {
        self.user_agent
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0".to_string())
    }

    pub fn register_native_download(&self, id: u64, url: String, file_name: String, saved_path: String, total_bytes: i64) {
        let safe_name = sanitize_download_filename(&file_name);
        let category = detect_download_category(&safe_name);
        let task = DownloadTask {
            id,
            url,
            file_name: safe_name,
            total_bytes,
            downloaded_bytes: 0,
            status: "DOWNLOADING".to_string(),
            speed_bps: 0,
            threads: 1,
            category,
            supports_ranges: false,
            referer: None,
            cookie: None,
            sha256: None,
            saved_path: Some(saved_path),
            error_message: None,
            segment_map: "[Browser Native]".to_string(),
            scheduled_start_epoch: None,
        };
        if let Ok(mut list) = self.tasks.lock() {
            list.insert(0, task);
        }
        self.save_queue();
    }

    pub fn update_native_download_progress(&self, id: u64, received: u64, total: i64, speed_bps: u64) {
        if let Ok(mut list) = self.tasks.lock() {
            if let Some(t) = list.iter_mut().find(|i| i.id == id) {
                t.downloaded_bytes = received;
                if total > 0 {
                    t.total_bytes = total;
                }
                t.speed_bps = speed_bps;
            }
        }
    }

    pub fn finish_native_download(&self, id: u64, ok: bool, final_path: Option<String>, err_msg: Option<String>) {
        let sha_hex = if ok {
            final_path.as_ref().and_then(|p| {
                let mut f = File::open(p).ok()?;
                let mut hasher = Sha256::new();
                let mut buf = [0u8; 65536];
                while let Ok(n) = f.read(&mut buf) {
                    if n == 0 { break; }
                    hasher.update(&buf[..n]);
                }
                Some(hex::encode(hasher.finalize()))
            })
        } else {
            None
        };
        let final_size = final_path
            .as_ref()
            .and_then(|p| fs::metadata(p).ok().map(|m| m.len()));
        let auto_clean = self.auto_clean_completed.load(Ordering::Relaxed);

        if let Ok(mut list) = self.tasks.lock() {
            if ok && auto_clean {
                list.retain(|i| i.id != id);
            } else if let Some(t) = list.iter_mut().find(|i| i.id == id) {
                t.speed_bps = 0;
                if ok {
                    t.status = "COMPLETED".to_string();
                    if let Some(sz) = final_size {
                        t.total_bytes = sz as i64;
                        t.downloaded_bytes = sz;
                    }
                    t.sha256 = sha_hex;
                    if let Some(p) = final_path {
                        t.saved_path = Some(p);
                    }
                } else {
                    t.status = "FAILED".to_string();
                    t.error_message = err_msg.or_else(|| Some("Interrupted".to_string()));
                }
            }
        }
        self.save_queue();
    }

    pub fn export_history_csv(&self) -> Result<PathBuf, String> {
        let out_path = BrowserStorage::downloads_root_dir().join("download_history.csv");
        let list = self.tasks.lock().map_err(|e| e.to_string())?;
        let mut csv = String::from("ID,FileName,Category,Status,TotalBytes,SHA256,SavedPath,URL\n");
        let esc = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        for t in list.iter() {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{}\n",
                t.id,
                esc(&t.file_name),
                esc(&t.category),
                esc(&t.status),
                t.total_bytes,
                esc(t.sha256.as_deref().unwrap_or("")),
                esc(t.saved_path.as_deref().unwrap_or("")),
                esc(&t.url),
            ));
        }
        fs::write(&out_path, csv).map_err(|e| e.to_string())?;
        Ok(out_path)
    }

    pub fn clean_stale_and_completed(&self) -> usize {
        let removed = if let Ok(mut list) = self.tasks.lock() {
            let before = list.len();
            list.retain(|t| {
                if t.status == "FAILED" {
                    return false;
                }
                if t.status == "COMPLETED" {
                    let exists = t
                        .saved_path
                        .as_ref()
                        .map(|p| Path::new(p).exists())
                        .unwrap_or(false);
                    return exists && !self.auto_clean_completed.load(Ordering::Relaxed);
                }
                true
            });
            before.saturating_sub(list.len())
        } else {
            0
        };
        if removed > 0 {
            self.save_queue();
        }
        removed
    }

    pub fn pump_scheduler(&self, rt: &tokio::runtime::Handle) {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let max_conc = self.max_concurrent.load(Ordering::Relaxed).max(1);

        let to_start: Vec<DownloadTask> = if let Ok(mut list) = self.tasks.lock() {
            let active_count = list
                .iter()
                .filter(|t| t.status == "DOWNLOADING" || t.status == "MUXING")
                .count();
            if active_count >= max_conc {
                Vec::new()
            } else {
                let slots = max_conc - active_count;
                let mut picked = Vec::new();
                for t in list.iter_mut().rev() {
                    if picked.len() >= slots {
                        break;
                    }
                    if t.status == "QUEUED" {
                        let due = t.scheduled_start_epoch.map(|ep| now_secs >= ep).unwrap_or(true);
                        if due {
                            t.status = "DOWNLOADING".to_string();
                            t.scheduled_start_epoch = None;
                            picked.push(t.clone());
                        }
                    }
                }
                picked
            }
        } else {
            Vec::new()
        };

        if !to_start.is_empty() {
            self.save_queue();
            for task in to_start {
                self.spawn_worker(rt, task);
            }
        }
    }

    pub async fn probe_stream_metadata(
        &self,
        url: String,
        referer: Option<String>,
        cookie: Option<String>,
    ) -> StreamProbeResult {
        let mut req = self
            .client
            .head(&url)
            .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0")
            .header("Accept", "*/*");
        if let Some(ref r) = referer {
            if !r.is_empty() {
                req = req.header(REFERER, r);
            }
        }
        if let Some(ref c) = cookie {
            if !c.is_empty() {
                req = req.header(COOKIE, c);
            }
        }

        let mut size_bytes: Option<u64> = None;
        let mut resolved_name: Option<String> = None;

        if let Ok(resp) = req.send().await {
            size_bytes = resp
                .headers()
                .get(CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .filter(|&n| n > 0);

            if let Some(cd) = resp.headers().get("content-disposition").and_then(|v| v.to_str().ok()) {
                resolved_name = parse_content_disposition_filename(cd);
            }
        }

        if size_bytes.is_none() {
            let mut range_req = self
                .client
                .get(&url)
                .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0")
            .header("Accept", "*/*")
                .header(RANGE, "bytes=0-0");
            if let Some(ref r) = referer {
                if !r.is_empty() {
                    range_req = range_req.header(REFERER, r);
                }
            }
            if let Some(ref c) = cookie {
                if !c.is_empty() {
                    range_req = range_req.header(COOKIE, c);
                }
            }
            if let Ok(resp) = range_req.send().await {
                if resp.status() == StatusCode::PARTIAL_CONTENT {
                    size_bytes = resp
                        .headers()
                        .get(CONTENT_RANGE)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.split('/').nth(1))
                        .and_then(|s| s.parse::<u64>().ok())
                        .filter(|&n| n > 0);
                }
            }
        }

        let size_label = match size_bytes {
            Some(b) => format_human_size(b),
            None => "Stream".to_string(),
        };

        StreamProbeResult {
            url,
            size_bytes,
            size_label,
            resolved_name,
        }
    }

    fn parts_dir() -> PathBuf {
        let dir = BrowserStorage::app_data_dir().join("parts");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn queue_file_path() -> PathBuf {
        BrowserStorage::app_data_dir().join("downloads_queue.json")
    }

    fn load_queue() -> Vec<DownloadTask> {
        let path = Self::queue_file_path();
        if let Ok(raw) = fs::read_to_string(path) {
            if let Ok(mut items) = serde_json::from_str::<Vec<DownloadTask>>(&raw) {
                for it in &mut items {
                    if it.status == "DOWNLOADING" || it.status == "MUXING" {
                        it.status = "PAUSED".to_string();
                        it.speed_bps = 0;
                    }
                }
                return items;
            }
        }
        Vec::new()
    }

    pub fn save_queue(&self) {
        if let Ok(list) = self.tasks.lock() {
            if let Ok(json) = serde_json::to_string_pretty(&*list) {
                let _ = fs::write(Self::queue_file_path(), json);
            }
        }
    }

    pub fn clear_completed(&self) -> usize {
        let removed = if let Ok(mut list) = self.tasks.lock() {
            let before = list.len();
            list.retain(|t| t.status != "COMPLETED");
            before.saturating_sub(list.len())
        } else {
            0
        };
        if removed > 0 {
            self.save_queue();
        }
        removed
    }

    pub fn pause(&self, id: u64) {
        if let Ok(mut map) = self.active_flags.lock() {
            if let Some(flag) = map.remove(&id) {
                flag.store(false, Ordering::SeqCst);
            }
        }
        if let Ok(mut list) = self.tasks.lock() {
            if let Some(t) = list.iter_mut().find(|i| i.id == id) {
                if t.status == "DOWNLOADING" || t.status == "MUXING" {
                    t.status = "PAUSED".to_string();
                    t.speed_bps = 0;
                }
            }
        }
        self.save_queue();
    }

    pub fn delete(&self, id: u64) {
        self.pause(id);
        let part_prefix = format!("{}_", id);
        if let Ok(entries) = fs::read_dir(Self::parts_dir()) {
            for e in entries.flatten() {
                if let Some(name) = e.file_name().to_str() {
                    if name.starts_with(&part_prefix) {
                        let _ = fs::remove_file(e.path());
                    }
                }
            }
        }
        if let Ok(mut list) = self.tasks.lock() {
            list.retain(|i| i.id != id);
        }
        self.save_queue();
    }

    pub fn resume_or_restart(&self, rt: &tokio::runtime::Handle, id: u64, new_url: Option<String>) {
        let task_opt = if let Ok(mut list) = self.tasks.lock() {
            if let Some(t) = list.iter_mut().find(|i| i.id == id) {
                if let Some(u) = new_url {
                    t.url = u;
                }
                t.status = "DOWNLOADING".to_string();
                t.error_message = None;
                Some(t.clone())
            } else {
                None
            }
        } else {
            None
        };

        if let Some(task) = task_opt {
            self.save_queue();
            self.spawn_worker(rt, task);
        }
    }

    pub fn enqueue(
        &self,
        rt: &tokio::runtime::Handle,
        url: String,
        threads: usize,
        referer: Option<String>,
        cookie: Option<String>,
        dup_mode: &str, // "CHECK", "RENAME", "OVERWRITE"
        delay_secs: Option<u64>,
    ) -> EnqueueOutcome {
        let trimmed_u = url.trim();
        let normalized_url = if trimmed_u.starts_with("http://") || trimmed_u.starts_with("https://") {
            trimmed_u.to_string()
        } else {
            format!("https://{}", trimmed_u)
        };

        let mut raw_name = normalized_url
            .split('?')
            .next()
            .and_then(|p| p.rsplit('/').next())
            .filter(|s| !s.is_empty() && s.contains('.'))
            .map(sanitize_download_filename)
            .unwrap_or_else(|| {
                format!(
                    "shunno_download_{}.bin",
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs() % 10000)
                        .unwrap_or(1)
                )
            });

        let category = detect_download_category(&raw_name);
        let dest_candidate = BrowserStorage::downloads_root_dir().join(&category).join(&raw_name);

        if dup_mode == "CHECK" {
            if let Ok(list) = self.tasks.lock() {
                if let Some(existing) = list.iter().find(|t| {
                    (t.url == normalized_url || t.file_name.eq_ignore_ascii_case(&raw_name))
                        && (t.status == "DOWNLOADING"
                            || t.status == "QUEUED"
                            || t.status == "MUXING"
                            || (t.status == "COMPLETED"
                                && t.saved_path
                                    .as_ref()
                                    .map(|p| Path::new(p).exists())
                                    .unwrap_or(false)))
                }) {
                    return EnqueueOutcome::DuplicateDetected(existing.clone());
                }
            }
            if dest_candidate.exists() {
                return EnqueueOutcome::DuplicateDetected(DownloadTask {
                    id: 0,
                    url: normalized_url,
                    file_name: raw_name,
                    total_bytes: fs::metadata(&dest_candidate).map(|m| m.len() as i64).unwrap_or(-1),
                    downloaded_bytes: 0,
                    status: "COMPLETED".to_string(),
                    speed_bps: 0,
                    threads: threads.clamp(1, 32),
                    category,
                    supports_ranges: true,
                    referer,
                    cookie,
                    sha256: None,
                    saved_path: Some(dest_candidate.to_string_lossy().to_string()),
                    error_message: None,
                    segment_map: String::new(),
                    scheduled_start_epoch: None,
                });
            }
        } else if dup_mode == "OVERWRITE" {
            if dest_candidate.exists() {
                let _ = fs::remove_file(&dest_candidate);
            }
            if let Ok(mut list) = self.tasks.lock() {
                list.retain(|t| !(t.url == normalized_url && t.status == "COMPLETED"));
            }
        } else if dup_mode == "RENAME" {
            let stem = Path::new(&raw_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("file")
                .to_string();
            let ext = Path::new(&raw_name)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("bin")
                .to_string();
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() % 100_000)
                .unwrap_or(1);
            raw_name = format!("{}_{}.{}", stem, stamp, ext);
        }

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let sched_epoch = delay_secs.filter(|&d| d > 0).map(|d| now_secs + d);

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(1)
            + rand::random::<u16>() as u64;

        let max_conc = self.max_concurrent.load(Ordering::Relaxed).max(1);
        let active_now = self
            .tasks
            .lock()
            .map(|l| l.iter().filter(|t| t.status == "DOWNLOADING" || t.status == "MUXING").count())
            .unwrap_or(0);

        let should_queue = sched_epoch.is_some() || active_now >= max_conc;

        let task = DownloadTask {
            id,
            url: normalized_url,
            file_name: raw_name,
            total_bytes: -1,
            downloaded_bytes: 0,
            status: if should_queue { "QUEUED".to_string() } else { "DOWNLOADING".to_string() },
            speed_bps: 0,
            threads: threads.clamp(1, 32),
            category,
            supports_ranges: true,
            referer,
            cookie,
            sha256: None,
            saved_path: None,
            error_message: None,
            segment_map: if sched_epoch.is_some() { "[Scheduled]".to_string() } else { String::new() },
            scheduled_start_epoch: sched_epoch,
        };

        if let Ok(mut list) = self.tasks.lock() {
            list.insert(0, task.clone());
        }
        self.save_queue();
        if !should_queue {
            self.spawn_worker(rt, task.clone());
        }
        EnqueueOutcome::Started(task)
    }

    pub fn enqueue_dash_mux(
        &self,
        rt: &tokio::runtime::Handle,
        video_url: String,
        audio_url: String,
        output_title: String,
        referer: Option<String>,
        cookie: Option<String>,
    ) {
        let clean_title = sanitize_download_filename(&output_title.replace(' ', "_"));
        let base_name = if clean_title.is_empty() || clean_title == "download.bin" {
            format!(
                "shunno_muxed_{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() % 10000)
                    .unwrap_or(1)
            )
        } else {
            clean_title.trim_end_matches(".mp4").to_string()
        };
        let final_mp4_name = format!("{}.mp4", base_name);

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(1)
            + rand::random::<u16>() as u64;

        let task = DownloadTask {
            id,
            url: video_url.clone(),
            file_name: final_mp4_name.clone(),
            total_bytes: -1,
            downloaded_bytes: 0,
            status: "DOWNLOADING".to_string(),
            speed_bps: 0,
            threads: 16,
            category: "Videos".to_string(),
            supports_ranges: true,
            referer: referer.clone(),
            cookie: cookie.clone(),
            sha256: None,
            saved_path: None,
            error_message: None,
            segment_map: "[DASH Video+Audio]".to_string(),
            scheduled_start_epoch: None,
        };

        if let Ok(mut list) = self.tasks.lock() {
            list.insert(0, task.clone());
        }
        self.save_queue();

        let running = Arc::new(AtomicBool::new(true));
        if let Ok(mut map) = self.active_flags.lock() {
            map.insert(id, running.clone());
        }

        let client = self.client.clone();
        let tasks_ref = self.tasks.clone();
        let flags_ref = self.active_flags.clone();
        let limit_ref = self.speed_limit_bps.clone();
        let auto_clean_ref = self.auto_clean_completed.clone();

        rt.spawn(async move {
            let vid_part = Self::parts_dir().join(format!("{}_video.part", id));
            let aud_part = Self::parts_dir().join(format!("{}_audio.part", id));
            let muxed_part = Self::parts_dir().join(format!("{}_muxed.mp4", id));

            let mut v_task = task.clone();
            v_task.url = video_url;
            let mut a_task = task.clone();
            a_task.url = audio_url;

            let dummy_tasks: Arc<Mutex<Vec<DownloadTask>>> = Arc::new(Mutex::new(Vec::new()));
            let res_v = download_raw_to_path(&client, &v_task, &vid_part, &running, &limit_ref, &dummy_tasks).await;
            let res_a = download_raw_to_path(&client, &a_task, &aud_part, &running, &limit_ref, &dummy_tasks).await;

            if let (Ok(()), Ok(())) = (res_v, res_a) {
                if let Ok(mut list) = tasks_ref.lock() {
                    if let Some(t) = list.iter_mut().find(|i| i.id == id) {
                        t.status = "MUXING".to_string();
                        t.segment_map = "[FFmpeg Muxing...]".to_string();
                    }
                }

                if let Some(ffmpeg_bin) = BrowserStorage::find_ffmpeg() {
                    let status = Command::new(ffmpeg_bin)
                        .args([
                            "-y",
                            "-i",
                            &vid_part.to_string_lossy(),
                            "-i",
                            &aud_part.to_string_lossy(),
                            "-c",
                            "copy",
                            &muxed_part.to_string_lossy(),
                        ])
                        .status();

                    let _ = fs::remove_file(&vid_part);
                    let _ = fs::remove_file(&aud_part);

                    match status {
                        Ok(s) if s.success() && muxed_part.exists() => {
                            let _ = finalize_completed(&task, &muxed_part, &final_mp4_name, &tasks_ref, auto_clean_ref.load(Ordering::Relaxed));
                        }
                        _ => {
                            if let Ok(mut list) = tasks_ref.lock() {
                                if let Some(t) = list.iter_mut().find(|i| i.id == id) {
                                    t.status = "FAILED".to_string();
                                    t.error_message = Some("FFmpeg muxing failed".to_string());
                                }
                            }
                        }
                    }
                } else {
                    let _ = fs::remove_file(&aud_part);
                    let _ = finalize_completed(&task, &vid_part, &final_mp4_name, &tasks_ref, auto_clean_ref.load(Ordering::Relaxed));
                }
            } else if let Ok(mut list) = tasks_ref.lock() {
                if let Some(t) = list.iter_mut().find(|i| i.id == id) {
                    t.status = "FAILED".to_string();
                    t.error_message = Some("Failed to download DASH video or audio stream".to_string());
                }
            }

            if let Ok(mut map) = flags_ref.lock() {
                map.remove(&id);
            }
            if let Ok(list) = tasks_ref.lock() {
                if let Ok(json) = serde_json::to_string_pretty(&*list) {
                    let _ = fs::write(Self::queue_file_path(), json);
                }
            }
        });
    }

    fn spawn_worker(&self, rt: &tokio::runtime::Handle, task: DownloadTask) {
        self.pause(task.id);
        let running = Arc::new(AtomicBool::new(true));
        if let Ok(mut map) = self.active_flags.lock() {
            map.insert(task.id, running.clone());
        }
        if let Ok(mut list) = self.tasks.lock() {
            if let Some(t) = list.iter_mut().find(|i| i.id == task.id) {
                t.status = "DOWNLOADING".to_string();
                t.error_message = None;
            }
        }

        let client = self.client.clone();
        let tasks_ref = self.tasks.clone();
        let flags_ref = self.active_flags.clone();
        let limit_ref = self.speed_limit_bps.clone();
        let auto_clean_ref = self.auto_clean_completed.clone();

        rt.spawn(async move {
            let safe_part_name = sanitize_download_filename(&task.file_name);
            let part_path = Self::parts_dir().join(format!("{}_{}.part", task.id, safe_part_name));
            let res = if task.url.to_ascii_lowercase().contains(".m3u8") {
                run_hls_download(&client, &task, &part_path, &running, &tasks_ref, auto_clean_ref.load(Ordering::Relaxed)).await
            } else {
                run_parallel_download(&client, &task, &part_path, &running, &limit_ref, &tasks_ref, auto_clean_ref.load(Ordering::Relaxed)).await
            };

            match res {
                Ok(()) => {}
                Err(err) => {
                    if running.load(Ordering::SeqCst) {
                        if let Ok(mut list) = tasks_ref.lock() {
                            if let Some(t) = list.iter_mut().find(|i| i.id == task.id) {
                                t.status = "FAILED".to_string();
                                t.speed_bps = 0;
                                t.error_message = Some(err);
                            }
                        }
                    } else if let Ok(mut list) = tasks_ref.lock() {
                        if let Some(t) = list.iter_mut().find(|i| i.id == task.id) {
                            t.status = "PAUSED".to_string();
                            t.speed_bps = 0;
                        }
                    }
                }
            }

            if let Ok(mut map) = flags_ref.lock() {
                map.remove(&task.id);
            }
            if let Ok(list) = tasks_ref.lock() {
                if let Ok(json) = serde_json::to_string_pretty(&*list) {
                    let _ = fs::write(Self::queue_file_path(), json);
                }
            }
        });
    }
}

#[cfg(windows)]
fn write_positioned(file: &File, mut buf: &[u8], mut offset: u64) -> std::io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        let n = file.seek_write(buf, offset)?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::WriteZero, "zero write"));
        }
        buf = &buf[n..];
        offset += n as u64;
    }
    Ok(())
}

#[cfg(not(windows))]
fn write_positioned(file: &File, buf: &[u8], offset: u64) -> std::io::Result<()> {
    use std::os::unix::fs::FileExt;
    file.write_all_at(buf, offset)
}

async fn download_raw_to_path(
    client: &Client,
    task: &DownloadTask,
    part_path: &Path,
    running: &Arc<AtomicBool>,
    speed_limit: &Arc<AtomicU64>,
    tasks: &Arc<Mutex<Vec<DownloadTask>>>,
) -> Result<(), String> {
    let mut req = client
        .get(&task.url)
        .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0")
            .header("Accept", "*/*")
        .header(RANGE, "bytes=0-0");
    if let Some(ref r) = task.referer {
        if !r.is_empty() {
            req = req.header(REFERER, r);
        }
    }
    if let Some(ref c) = task.cookie {
        if !c.is_empty() {
            req = req.header(COOKIE, c);
        }
    }

    let mut total_len = 0u64;
    let mut supports_range = false;
    let mut reused_full_resp: Option<reqwest::Response> = None;
    let mut probe_resp_opt = req.send().await.ok();
    if let Some(ref r) = probe_resp_opt {
        if !r.status().is_success() && r.status() != StatusCode::PARTIAL_CONTENT {
            probe_resp_opt = None;
        }
    }
    if let Some(resp) = probe_resp_opt {
        let mut resolved_from_header = false;
        if let Some(cd) = resp.headers().get("content-disposition").and_then(|v| v.to_str().ok()) {
            if let Some(clean_cd_name) = parse_content_disposition_filename(cd) {
                resolved_from_header = true;
                if let Ok(mut list) = tasks.lock() {
                    if let Some(t) = list.iter_mut().find(|i| i.id == task.id) {
                        t.file_name = clean_cd_name.clone();
                        t.category = detect_download_category(&clean_cd_name);
                    }
                }
            }
        }

        if !resolved_from_header && task.file_name.ends_with(".bin") {
            if let Some(ct) = resp.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
                if let Some(ext) = infer_extension_from_mime(ct) {
                    let upgraded = format!("{}.{}", &task.file_name[..task.file_name.len() - 4], ext);
                    if let Ok(mut list) = tasks.lock() {
                        if let Some(t) = list.iter_mut().find(|i| i.id == task.id) {
                            t.file_name = upgraded.clone();
                            t.category = detect_download_category(&upgraded);
                        }
                    }
                }
            }
        }

        // Strict verification: only enable multi-thread seek_write if server actually honors 206 Partial Content with Content-Range
        if resp.status() == StatusCode::PARTIAL_CONTENT {
            if let Some(cr) = resp.headers().get(CONTENT_RANGE).and_then(|v| v.to_str().ok()) {
                if let Some(len_str) = cr.split('/').nth(1) {
                    if let Ok(parsed_len) = len_str.trim().parse::<u64>() {
                        if parsed_len > 0 {
                            total_len = parsed_len;
                            supports_range = true;
                        }
                    }
                }
            }
        } else {
            total_len = resp
                .headers()
                .get(CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            let _ = resp
                .headers()
                .get(ACCEPT_RANGES)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.eq_ignore_ascii_case("bytes"));
            supports_range = false; // Server ignored Range: bytes=0-0 and returned 200 OK; reuse this live response directly
            if resp.status() == StatusCode::OK {
                reused_full_resp = Some(resp);
            }
        }
    }

    let threads = if supports_range && total_len > 512 * 1024 {
        task.threads.clamp(1, 32)
    } else {
        1
    };

    if let Ok(mut list) = tasks.lock() {
        if let Some(t) = list.iter_mut().find(|i| i.id == task.id) {
            t.total_bytes = if total_len > 0 { total_len as i64 } else { -1 };
            t.supports_ranges = threads > 1;
            t.threads = threads;
        }
    }

    if threads > 1 && total_len > 0 {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(part_path)
            .map_err(|e| e.to_string())?;
        if file.metadata().map(|m| m.len()).unwrap_or(0) != total_len {
            file.set_len(total_len).map_err(|e| e.to_string())?;
        }
        let shared_file = Arc::new(file);
        let chunk = total_len / (threads as u64);
        let thread_progress: Arc<Vec<AtomicU64>> =
            Arc::new((0..threads).map(|_| AtomicU64::new(0)).collect());
        let mut thread_totals = vec![0u64; threads];

        for i in 0..threads {
            let start = (i as u64) * chunk;
            let end = if i == threads - 1 { total_len - 1 } else { start + chunk - 1 };
            let seg_len = end - start + 1;
            thread_totals[i] = seg_len;

            let offset_path = NativeDownloadEngine::parts_dir().join(format!("{}_t{}.offset", task.id, i));
            if let Ok(txt) = fs::read_to_string(&offset_path) {
                if let Ok(saved) = txt.trim().parse::<u64>() {
                    thread_progress[i].store(saved.saturating_sub(start).min(seg_len), Ordering::SeqCst);
                }
            }
        }

        let mon_prog = thread_progress.clone();
        let mon_totals = thread_totals.clone();
        let mon_run = running.clone();
        let mon_tasks = tasks.clone();
        let task_id = task.id;

        let mon = tokio::spawn(async move {
            let mut last: u64 = mon_prog.iter().map(|a| a.load(Ordering::Relaxed)).sum();
            while mon_run.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let cur: u64 = mon_prog.iter().map(|a| a.load(Ordering::Relaxed)).sum();
                let speed = cur.saturating_sub(last) * 2;
                last = cur;

                let mut seg_bar = String::with_capacity(mon_totals.len() + 2);
                seg_bar.push('[');
                for (idx, tot) in mon_totals.iter().enumerate() {
                    let done = mon_prog[idx].load(Ordering::Relaxed);
                    let r = if *tot > 0 { (done as f64) / (*tot as f64) } else { 1.0 };
                    seg_bar.push(if r >= 0.99 { '█' } else if r >= 0.5 { '▓' } else if r > 0.05 { '▒' } else { '░' });
                }
                seg_bar.push(']');

                if let Ok(mut list) = mon_tasks.lock() {
                    if let Some(t) = list.iter_mut().find(|i| i.id == task_id) {
                        t.downloaded_bytes = cur;
                        t.speed_bps = speed;
                        t.segment_map = seg_bar;
                    }
                }
            }
        });

        let mut workers = Vec::with_capacity(threads);
        for i in 0..threads {
            let start = (i as u64) * chunk;
            let end = if i == threads - 1 { total_len - 1 } else { start + chunk - 1 };
            let done_init = thread_progress[i].load(Ordering::SeqCst);
            let mut offset = start + done_init;

            let c = client.clone();
            let u = task.url.clone();
            let r_hdr = task.referer.clone();
            let c_hdr = task.cookie.clone();
            let f = shared_file.clone();
            let prog = thread_progress.clone();
            let run = running.clone();
            let lim = speed_limit.clone();
            let offset_path = NativeDownloadEngine::parts_dir().join(format!("{}_t{}.offset", task.id, i));

            workers.push(tokio::spawn(async move {
                let mut last_err = String::new();
                for attempt in 0..3u32 {
                    if offset > end {
                        return Ok::<(), String>(());
                    }
                    if !run.load(Ordering::Relaxed) {
                        let _ = fs::write(&offset_path, offset.to_string());
                        return Err("Paused".to_string());
                    }

                    let mut rb = c
                        .get(&u)
                        .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0")
                        .header("Accept", "*/*")
                        .header(RANGE, format!("bytes={}-{}", offset, end));
                    if let Some(ref r) = r_hdr {
                        if !r.is_empty() { rb = rb.header(REFERER, r); }
                    }
                    if let Some(ref ck) = c_hdr {
                        if !ck.is_empty() { rb = rb.header(COOKIE, ck); }
                    }

                    let resp = match rb.send().await {
                        Ok(r) if r.status().is_success() => r,
                        Ok(r) => {
                            last_err = format!("HTTP {}", r.status());
                            if attempt < 2 && run.load(Ordering::Relaxed) {
                                tokio::time::sleep(Duration::from_secs(1u64 << attempt)).await;
                                continue;
                            }
                            return Err(last_err);
                        }
                        Err(e) => {
                            last_err = e.to_string();
                            if attempt < 2 && run.load(Ordering::Relaxed) {
                                tokio::time::sleep(Duration::from_secs(1u64 << attempt)).await;
                                continue;
                            }
                            return Err(last_err);
                        }
                    };

                    let mut stream = resp.bytes_stream();
                    let mut since_save = 0usize;
                    let mut stream_err: Option<String> = None;

                    while let Some(chunk_res) = stream.next().await {
                        if !run.load(Ordering::Relaxed) {
                            let _ = fs::write(&offset_path, offset.to_string());
                            return Err("Paused".to_string());
                        }
                        match chunk_res {
                            Ok(bytes) => {
                                write_positioned(&f, &bytes, offset).map_err(|e| e.to_string())?;
                                offset += bytes.len() as u64;
                                prog[i].fetch_add(bytes.len() as u64, Ordering::Relaxed);
                                since_save += bytes.len();

                                if since_save >= 128 * 1024 {
                                    let _ = fs::write(&offset_path, offset.to_string());
                                    since_save = 0;
                                }

                                let max_bps = lim.load(Ordering::Relaxed);
                                if max_bps > 0 {
                                    let per_t = (max_bps / (threads as u64)).max(4096);
                                    let sleep_ms = ((bytes.len() as u64) * 1000) / per_t;
                                    if sleep_ms > 5 {
                                        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = fs::write(&offset_path, offset.to_string());
                                stream_err = Some(e.to_string());
                                break;
                            }
                        }
                    }

                    if let Some(err_msg) = stream_err {
                        last_err = err_msg;
                        if attempt < 2 && run.load(Ordering::Relaxed) {
                            tokio::time::sleep(Duration::from_secs(1u64 << attempt)).await;
                            continue;
                        }
                        return Err(last_err);
                    }

                    let _ = fs::write(&offset_path, offset.to_string());
                    return Ok(());
                }
                Err(last_err)
            }));
        }

        let mut first_err = None;
        for w in workers {
            if let Ok(Err(e)) = w.await {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
        mon.abort();
        for i in 0..threads {
            let _ = fs::remove_file(NativeDownloadEngine::parts_dir().join(format!("{}_t{}.offset", task.id, i)));
        }

        if let Some(err) = first_err {
            if err == "Paused" || !running.load(Ordering::Relaxed) {
                return Err("Paused".to_string());
            }
            // Fallback cleanly to a single sequential stream if CDN throttles parallel Range workers (e.g. 429/403/503)
            if let Ok(mut list) = tasks.lock() {
                if let Some(t) = list.iter_mut().find(|i| i.id == task.id) {
                    t.threads = 1;
                    t.supports_ranges = false;
                    t.segment_map = "[1T Fallback]".to_string();
                }
            }
            let mut rb = client
                .get(&task.url)
                .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0")
                .header("Accept", "*/*");
            if let Some(ref r) = task.referer {
                if !r.is_empty() { rb = rb.header(REFERER, r); }
            }
            if let Some(ref ck) = task.cookie {
                if !ck.is_empty() { rb = rb.header(COOKIE, ck); }
            }
            let resp = rb.send().await.map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("HTTP {}", resp.status()));
            }
            let mut out = File::create(part_path).map_err(|e| e.to_string())?;
            let mut stream = resp.bytes_stream();
            let mut done = 0u64;
            let mut last_tick = Instant::now();
            let mut bytes_tick = 0u64;
            while let Some(chunk_res) = stream.next().await {
                if !running.load(Ordering::Relaxed) {
                    return Err("Paused".to_string());
                }
                let bytes = chunk_res.map_err(|e| e.to_string())?;
                std::io::Write::write_all(&mut out, &bytes).map_err(|e| e.to_string())?;
                done += bytes.len() as u64;
                bytes_tick += bytes.len() as u64;
                let elapsed = last_tick.elapsed().as_millis() as u64;
                if elapsed >= 500 {
                    let speed = (bytes_tick * 1000) / elapsed.max(1);
                    last_tick = Instant::now();
                    bytes_tick = 0;
                    if let Ok(mut list) = tasks.lock() {
                        if let Some(t) = list.iter_mut().find(|i| i.id == task.id) {
                            t.downloaded_bytes = done;
                            t.speed_bps = speed;
                        }
                    }
                }
            }
            return Ok(());
        }
    } else {
        let resp = if let Some(r) = reused_full_resp {
            r
        } else {
            let mut rb = client
                .get(&task.url)
                .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0")
                .header("Accept", "*/*");
            if let Some(ref r) = task.referer {
                if !r.is_empty() { rb = rb.header(REFERER, r); }
            }
            if let Some(ref ck) = task.cookie {
                if !ck.is_empty() { rb = rb.header(COOKIE, ck); }
            }
            let r = rb.send().await.map_err(|e| e.to_string())?;
            if !r.status().is_success() {
                return Err(format!("HTTP {}", r.status()));
            }
            r
        };

        let mut out = File::create(part_path).map_err(|e| e.to_string())?;
        let mut stream = resp.bytes_stream();
        let mut done = 0u64;
        let mut last_tick = Instant::now();
        let mut bytes_tick = 0u64;

        while let Some(chunk_res) = stream.next().await {
            if !running.load(Ordering::Relaxed) {
                return Err("Paused".to_string());
            }
            let bytes = chunk_res.map_err(|e| e.to_string())?;
            std::io::Write::write_all(&mut out, &bytes).map_err(|e| e.to_string())?;
            done += bytes.len() as u64;
            bytes_tick += bytes.len() as u64;

            let elapsed = last_tick.elapsed().as_millis() as u64;
            if elapsed >= 500 {
                let speed = (bytes_tick * 1000) / elapsed.max(1);
                last_tick = Instant::now();
                bytes_tick = 0;
                if let Ok(mut list) = tasks.lock() {
                    if let Some(t) = list.iter_mut().find(|i| i.id == task.id) {
                        t.downloaded_bytes = done;
                        t.speed_bps = speed;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn run_parallel_download(
    client: &Client,
    task: &DownloadTask,
    part_path: &Path,
    running: &Arc<AtomicBool>,
    speed_limit: &Arc<AtomicU64>,
    tasks: &Arc<Mutex<Vec<DownloadTask>>>,
    auto_clean: bool,
) -> Result<(), String> {
    download_raw_to_path(client, task, part_path, running, speed_limit, tasks).await?;

    let current_name = tasks
        .lock()
        .ok()
        .and_then(|l| l.iter().find(|i| i.id == task.id).map(|t| t.file_name.clone()))
        .unwrap_or_else(|| task.file_name.clone());

    finalize_completed(task, part_path, &current_name, tasks, auto_clean)
}

async fn run_hls_download(
    client: &Client,
    task: &DownloadTask,
    part_path: &Path,
    running: &Arc<AtomicBool>,
    tasks: &Arc<Mutex<Vec<DownloadTask>>>,
    auto_clean: bool,
) -> Result<(), String> {
    let mut manifest = client
        .get(&task.url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let mut base_url = task
        .url
        .rfind('/')
        .map(|i| task.url[..=i].to_string())
        .unwrap_or_default();

    let mut lines: Vec<String> = manifest
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if lines
        .iter()
        .any(|l| l.to_ascii_uppercase().starts_with("#EXT-X-STREAM-INF"))
    {
        if let Some(variant) = lines.iter().rev().find(|l| !l.starts_with('#')).cloned() {
            let resolved = if variant.starts_with("http") {
                variant
            } else {
                format!("{}{}", base_url, variant)
            };
            manifest = client
                .get(&resolved)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .text()
                .await
                .map_err(|e| e.to_string())?;
            if let Some(i) = resolved.rfind('/') {
                base_url = resolved[..=i].to_string();
            }
            lines = manifest
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
        }
    }

    let segments: Vec<String> = lines
        .into_iter()
        .filter(|l| !l.starts_with('#'))
        .map(|l| if l.starts_with("http") { l } else { format!("{}{}", base_url, l) })
        .collect();

    if segments.is_empty() {
        return Err("No media segments in M3U8 playlist".to_string());
    }

    let mut out = File::create(part_path).map_err(|e| e.to_string())?;
    let mut done = 0u64;
    for (idx, seg) in segments.iter().enumerate() {
        if !running.load(Ordering::Relaxed) {
            return Err("Paused".to_string());
        }
        let bytes = client
            .get(seg)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;
        std::io::Write::write_all(&mut out, &bytes).map_err(|e| e.to_string())?;
        done += bytes.len() as u64;
        let est = (done * (segments.len() as u64)) / ((idx + 1) as u64);
        if let Ok(mut list) = tasks.lock() {
            if let Some(t) = list.iter_mut().find(|i| i.id == task.id) {
                t.downloaded_bytes = done;
                t.total_bytes = est as i64;
                t.segment_map = format!("[HLS {}/{}]", idx + 1, segments.len());
            }
        }
    }

    let out_name = if task.file_name.to_ascii_lowercase().ends_with(".m3u8") {
        format!("{}.mp4", &task.file_name[..task.file_name.len() - 5])
    } else {
        task.file_name.clone()
    };

    finalize_completed(task, part_path, &out_name, tasks, auto_clean)
}

fn finalize_completed(
    task: &DownloadTask,
    part_path: &Path,
    final_name: &str,
    tasks: &Arc<Mutex<Vec<DownloadTask>>>,
    auto_clean: bool,
) -> Result<(), String> {
    let safe_final = sanitize_download_filename(final_name);
    let target_dir = BrowserStorage::downloads_root_dir().join(&task.category);
    let _ = fs::create_dir_all(&target_dir);
    let mut dest_path = target_dir.join(&safe_final);

    if dest_path.exists() {
        let stem = dest_path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = dest_path.extension().and_then(|s| s.to_str()).unwrap_or("bin");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() % 100_000)
            .unwrap_or(1);
        dest_path = target_dir.join(format!("{}_{}.{}", stem, stamp, ext));
    }

    if fs::rename(part_path, &dest_path).is_err() {
        fs::copy(part_path, &dest_path).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(part_path);
    }

    let mut hasher = Sha256::new();
    if let Ok(mut f) = File::open(&dest_path) {
        let mut buf = [0u8; 65536];
        while let Ok(n) = f.read(&mut buf) {
            if n == 0 { break; }
            hasher.update(&buf[..n]);
        }
    }
    let sha_hex = hex::encode(hasher.finalize());
    let final_size = fs::metadata(&dest_path).map(|m| m.len()).unwrap_or(0);
    let saved_str = dest_path.to_string_lossy().to_string();

    if let Ok(mut list) = tasks.lock() {
        if auto_clean {
            list.retain(|i| i.id != task.id);
        } else if let Some(t) = list.iter_mut().find(|i| i.id == task.id) {
            t.file_name = safe_final;
            t.total_bytes = final_size as i64;
            t.downloaded_bytes = final_size;
            t.status = "COMPLETED".to_string();
            t.speed_bps = 0;
            t.sha256 = Some(sha_hex);
            t.saved_path = Some(saved_str);
        }
    }
    Ok(())
}
