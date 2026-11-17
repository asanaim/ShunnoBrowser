use futures_util::StreamExt;
use reqwest::header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, COOKIE, RANGE, REFERER, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: u64,
    pub url: String,
    pub file_name: String,
    pub total_bytes: i64,
    pub downloaded_bytes: u64,
    pub status: String,
    pub speed_bps: u64,
    pub threads: usize,
    pub category: String,
    pub sha256: Option<String>,
    pub saved_path: Option<String>,
    pub segment_map: String,
}

pub struct ShunnoGetDesktop {
    client: Client,
    pub tasks: Arc<Mutex<Vec<DownloadTask>>>,
    active_flags: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
    pub speed_limit_bps: Arc<AtomicU64>,
}

impl ShunnoGetDesktop {
    pub fn new() -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .pool_max_idle_per_host(64)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            tasks: Arc::new(Mutex::new(Vec::new())),
            active_flags: Arc::new(Mutex::new(HashMap::new())),
            speed_limit_bps: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn detect_category(file_name: &str) -> String {
        let ext = file_name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        match ext.as_str() {
            "mp4" | "mkv" | "webm" | "mov" | "avi" | "ts" | "m3u8" => "Videos".to_string(),
            "flac" | "alac" | "m4a" | "wav" | "mp3" | "ogg" | "opus" => "Lossless Audio".to_string(),
            "zip" | "rar" | "7z" | "tar" | "gz" | "iso" => "Archives".to_string(),
            "exe" | "msi" | "apk" => "Programs".to_string(),
            _ => "Documents".to_string(),
        }
    }

    pub fn pause(&self, id: u64) {
        if let Ok(mut map) = self.active_flags.lock() {
            if let Some(flag) = map.remove(&id) {
                flag.store(false, Ordering::SeqCst);
            }
        }
        if let Ok(mut list) = self.tasks.lock() {
            if let Some(t) = list.iter_mut().find(|i| i.id == id) {
                if t.status == "DOWNLOADING" {
                    t.status = "PAUSED".to_string();
                    t.speed_bps = 0;
                }
            }
        }
    }

    pub fn resume_or_fix(&self, rt: &tokio::runtime::Handle, id: u64, new_url: Option<String>) {
        let mut target_task = None;
        if let Ok(mut list) = self.tasks.lock() {
            if let Some(t) = list.iter_mut().find(|i| i.id == id) {
                if let Some(u) = new_url {
                    if u.starts_with("http") {
                        t.url = u;
                    }
                }
                t.status = "DOWNLOADING".to_string();
                target_task = Some(t.clone());
            }
        }
        if let Some(task) = target_task {
            self.spawn_worker(rt, task, None, None);
        }
    }

    pub fn enqueue(
        &self,
        rt: &tokio::runtime::Handle,
        url: String,
        threads: usize,
        referer: Option<String>,
        cookie: Option<String>,
    ) -> DownloadTask {
        let file_name = url
            .split('?')
            .next()
            .and_then(|p| p.rsplit('/').next())
            .filter(|s| !s.is_empty() && s.contains('.'))
            .unwrap_or("download.bin")
            .to_string();

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(1)
            + rand::random::<u16>() as u64;

        let task = DownloadTask {
            id,
            url,
            file_name: file_name.clone(),
            total_bytes: -1,
            downloaded_bytes: 0,
            status: "DOWNLOADING".to_string(),
            speed_bps: 0,
            threads: threads.clamp(1, 32),
            category: Self::detect_category(&file_name),
            sha256: None,
            saved_path: None,
            segment_map: String::new(),
        };

        if let Ok(mut list) = self.tasks.lock() {
            list.insert(0, task.clone());
        }

        self.spawn_worker(rt, task.clone(), referer, cookie);
        task
    }

    fn spawn_worker(
        &self,
        rt: &tokio::runtime::Handle,
        task: DownloadTask,
        referer: Option<String>,
        cookie: Option<String>,
    ) {
        self.pause(task.id);
        let running = Arc::new(AtomicBool::new(true));
        if let Ok(mut map) = self.active_flags.lock() {
            map.insert(task.id, running.clone());
        }

        let client = self.client.clone();
        let tasks_ref = self.tasks.clone();
        let flags_ref = self.active_flags.clone();
        let limit_ref = self.speed_limit_bps.clone();

        rt.spawn(async move {
            let res = run_parallel_download(
                client,
                task.id,
                task.url,
                task.file_name,
                task.category,
                task.threads,
                referer,
                cookie,
                running.clone(),
                limit_ref,
                tasks_ref.clone(),
            )
            .await;

            if let Err(err) = res {
                if running.load(Ordering::SeqCst) {
                    if let Ok(mut list) = tasks_ref.lock() {
                        if let Some(t) = list.iter_mut().find(|i| i.id == task.id) {
                            t.status = format!("FAILED: {}", err);
                            t.speed_bps = 0;
                        }
                    }
                }
            }
            if let Ok(mut map) = flags_ref.lock() {
                map.remove(&task.id);
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

#[allow(clippy::too_many_arguments)]
async fn run_parallel_download(
    client: Client,
    id: u64,
    url: String,
    file_name: String,
    category: String,
    requested_threads: usize,
    referer: Option<String>,
    cookie: Option<String>,
    running: Arc<AtomicBool>,
    speed_limit: Arc<AtomicU64>,
    tasks: Arc<Mutex<Vec<DownloadTask>>>,
) -> Result<(), String> {
    let base_dir = dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ShunnoBrowser")
        .join(&category);
    let _ = fs::create_dir_all(&base_dir);
    let part_path = base_dir.join(format!(".{}_{}.part", id, file_name));
    let final_path = base_dir.join(&file_name);

    let mut req = client
        .get(&url)
        .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ShunnoBrowser-Rust/1.0")
        .header(RANGE, "bytes=0-0");
    if let Some(ref r) = referer {
        req = req.header(REFERER, r);
    }
    if let Some(ref c) = cookie {
        req = req.header(COOKIE, c);
    }

    let mut total_len = 0u64;
    let mut supports_range = false;
    if let Ok(resp) = req.send().await {
        if resp.status() == StatusCode::PARTIAL_CONTENT {
            supports_range = true;
            total_len = resp
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split('/').nth(1))
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
        } else {
            total_len = resp
                .headers()
                .get(CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            supports_range = resp
                .headers()
                .get(ACCEPT_RANGES)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.eq_ignore_ascii_case("bytes"))
                .unwrap_or(false);
        }
    }

    let threads = if supports_range && total_len > 512 * 1024 {
        requested_threads.clamp(1, 32)
    } else {
        1
    };

    if let Ok(mut list) = tasks.lock() {
        if let Some(t) = list.iter_mut().find(|i| i.id == id) {
            t.total_bytes = if total_len > 0 { total_len as i64 } else { -1 };
            t.threads = threads;
        }
    }

    if threads > 1 && total_len > 0 {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(&part_path)
            .map_err(|e| e.to_string())?;
        if file.metadata().map(|m| m.len()).unwrap_or(0) != total_len {
            file.set_len(total_len).map_err(|e| e.to_string())?;
        }
        let shared_file = Arc::new(file);
        let thread_done: Arc<Vec<AtomicU64>> =
            Arc::new((0..threads).map(|_| AtomicU64::new(0)).collect());
        let chunk = total_len / (threads as u64);
        let mut seg_totals = vec![0u64; threads];

        for i in 0..threads {
            let s = (i as u64) * chunk;
            let e = if i == threads - 1 { total_len - 1 } else { s + chunk - 1 };
            seg_totals[i] = e - s + 1;
            let off_file = base_dir.join(format!(".{}_t{}.off", id, i));
            if let Ok(txt) = fs::read_to_string(&off_file) {
                if let Ok(saved) = txt.trim().parse::<u64>() {
                    thread_done[i].store(saved.saturating_sub(s).min(seg_totals[i]), Ordering::SeqCst);
                }
            }
        }

        let mon_done = thread_done.clone();
        let mon_run = running.clone();
        let mon_tasks = tasks.clone();
        let mon = tokio::spawn(async move {
            let mut last: u64 = mon_done.iter().map(|a| a.load(Ordering::Relaxed)).sum();
            while mon_run.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(450)).await;
                let cur: u64 = mon_done.iter().map(|a| a.load(Ordering::Relaxed)).sum();
                let speed = cur.saturating_sub(last) * 2;
                last = cur;

                let mut bar = String::from("[");
                for (idx, tot) in seg_totals.iter().enumerate() {
                    let d = mon_done[idx].load(Ordering::Relaxed);
                    let r = if *tot > 0 { d as f64 / *tot as f64 } else { 1.0 };
                    bar.push(if r >= 0.99 { '█' } else if r >= 0.5 { '▓' } else if r > 0.05 { '▒' } else { '░' });
                }
                bar.push(']');

                if let Ok(mut list) = mon_tasks.lock() {
                    if let Some(t) = list.iter_mut().find(|i| i.id == id) {
                        t.downloaded_bytes = cur;
                        t.speed_bps = speed;
                        t.segment_map = bar;
                    }
                }
            }
        });

        let mut workers = Vec::with_capacity(threads);
        for i in 0..threads {
            let start = (i as u64) * chunk;
            let end = if i == threads - 1 { total_len - 1 } else { start + chunk - 1 };
            let mut offset = start + thread_done[i].load(Ordering::SeqCst);
            let off_file = base_dir.join(format!(".{}_t{}.off", id, i));

            let c = client.clone();
            let u = url.clone();
            let r_hdr = referer.clone();
            let c_hdr = cookie.clone();
            let f = shared_file.clone();
            let d = thread_done.clone();
            let run = running.clone();
            let lim = speed_limit.clone();

            workers.push(tokio::spawn(async move {
                if offset > end {
                    return Ok::<(), String>(());
                }
                let mut rb = c
                    .get(&u)
                    .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ShunnoBrowser-Rust/1.0")
                    .header(RANGE, format!("bytes={}-{}", offset, end));
                if let Some(ref r) = r_hdr { rb = rb.header(REFERER, r); }
                if let Some(ref ck) = c_hdr { rb = rb.header(COOKIE, ck); }

                let resp = rb.send().await.map_err(|e| e.to_string())?;
                let mut stream = resp.bytes_stream();
                let mut since_save = 0usize;

                while let Some(chunk_res) = stream.next().await {
                    if !run.load(Ordering::Relaxed) {
                        let _ = fs::write(&off_file, offset.to_string());
                        return Err("Paused".to_string());
                    }
                    let bytes = chunk_res.map_err(|e| e.to_string())?;
                    write_positioned(&f, &bytes, offset).map_err(|e| e.to_string())?;
                    offset += bytes.len() as u64;
                    d[i].fetch_add(bytes.len() as u64, Ordering::Relaxed);
                    since_save += bytes.len();

                    if since_save >= 128 * 1024 {
                        let _ = fs::write(&off_file, offset.to_string());
                        since_save = 0;
                    }

                    let max_bps = lim.load(Ordering::Relaxed);
                    if max_bps > 0 {
                        let per_t = (max_bps / (threads as u64)).max(4096);
                        let ms = ((bytes.len() as u64) * 1000) / per_t;
                        if ms > 5 {
                            tokio::time::sleep(Duration::from_millis(ms)).await;
                        }
                    }
                }
                let _ = fs::write(&off_file, offset.to_string());
                Ok::<(), String>(())
            }));
        }

        for w in workers {
            if let Ok(Err(e)) = w.await {
                mon.abort();
                return Err(e);
            }
        }
        mon.abort();
        for i in 0..threads {
            let _ = fs::remove_file(base_dir.join(format!(".{}_t{}.off", id, i)));
        }
    } else {
        let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
        let mut out = File::create(&part_path).map_err(|e| e.to_string())?;
        let mut stream = resp.bytes_stream();
        let mut done = 0u64;
        while let Some(chunk_res) = stream.next().await {
            if !running.load(Ordering::Relaxed) {
                return Err("Paused".to_string());
            }
            let bytes = chunk_res.map_err(|e| e.to_string())?;
            std::io::Write::write_all(&mut out, &bytes).map_err(|e| e.to_string())?;
            done += bytes.len() as u64;
            if let Ok(mut list) = tasks.lock() {
                if let Some(t) = list.iter_mut().find(|i| i.id == id) {
                    t.downloaded_bytes = done;
                }
            }
        }
    }

    let _ = fs::rename(&part_path, &final_path);
    let mut hasher = Sha256::new();
    if let Ok(mut f) = File::open(&final_path) {
        let mut buf = [0u8; 65536];
        while let Ok(n) = f.read(&mut buf) {
            if n == 0 { break; }
            hasher.update(&buf[..n]);
        }
    }
    let hash_hex = hex::encode(hasher.finalize());
    let final_size = fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);

    if let Ok(mut list) = tasks.lock() {
        if let Some(t) = list.iter_mut().find(|i| i.id == id) {
            t.total_bytes = final_size as i64;
            t.downloaded_bytes = final_size;
            t.status = "COMPLETED".to_string();
            t.speed_bps = 0;
            t.sha256 = Some(hash_hex);
            t.saved_path = Some(final_path.to_string_lossy().to_string());
        }
    }
    Ok(())
}
