use futures_util::StreamExt;
use reqwest::header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, COOKIE, RANGE, REFERER, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
    pub sha256: Option<String>,
    pub saved_path: Option<String>,
}

pub struct ShunnoGetDesktop {
    client: Client,
    pub tasks: Arc<Mutex<Vec<DownloadTask>>>,
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
        }
    }

    pub fn enqueue(&self, rt: &tokio::runtime::Handle, url: String, referer: Option<String>, cookie: Option<String>) -> DownloadTask {
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
            url: url.clone(),
            file_name: file_name.clone(),
            total_bytes: -1,
            downloaded_bytes: 0,
            status: "DOWNLOADING".to_string(),
            speed_bps: 0,
            threads: 16,
            sha256: None,
            saved_path: None,
        };

        if let Ok(mut list) = self.tasks.lock() {
            list.insert(0, task.clone());
        }

        let client = self.client.clone();
        let tasks_ref = self.tasks.clone();
        rt.spawn(async move {
            let _ = run_parallel_download(client, id, url, file_name, referer, cookie, tasks_ref).await;
        });

        task
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

async fn run_parallel_download(
    client: Client,
    id: u64,
    url: String,
    file_name: String,
    referer: Option<String>,
    cookie: Option<String>,
    tasks: Arc<Mutex<Vec<DownloadTask>>>,
) -> Result<(), String> {
    let base_dir = dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ShunnoBrowser");
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

    let threads = if supports_range && total_len > 512 * 1024 { 16 } else { 1 };
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
        file.set_len(total_len).map_err(|e| e.to_string())?;
        let shared_file = Arc::new(file);
        let done_counter = Arc::new(AtomicU64::new(0));
        let running = Arc::new(AtomicBool::new(true));
        let chunk = total_len / (threads as u64);

        let mon_done = done_counter.clone();
        let mon_run = running.clone();
        let mon_tasks = tasks.clone();
        let mon = tokio::spawn(async move {
            let mut last = 0u64;
            while mon_run.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let cur = mon_done.load(Ordering::Relaxed);
                let speed = cur.saturating_sub(last) * 2;
                last = cur;
                if let Ok(mut list) = mon_tasks.lock() {
                    if let Some(t) = list.iter_mut().find(|i| i.id == id) {
                        t.downloaded_bytes = cur;
                        t.speed_bps = speed;
                    }
                }
            }
        });

        let mut workers = Vec::with_capacity(threads);
        for i in 0..threads {
            let start = (i as u64) * chunk;
            let end = if i == threads - 1 { total_len - 1 } else { start + chunk - 1 };
            let c = client.clone();
            let u = url.clone();
            let r_hdr = referer.clone();
            let c_hdr = cookie.clone();
            let f = shared_file.clone();
            let d = done_counter.clone();

            workers.push(tokio::spawn(async move {
                let mut rb = c
                    .get(&u)
                    .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ShunnoBrowser-Rust/1.0")
                    .header(RANGE, format!("bytes={}-{}", start, end));
                if let Some(ref r) = r_hdr { rb = rb.header(REFERER, r); }
                if let Some(ref ck) = c_hdr { rb = rb.header(COOKIE, ck); }

                let resp = rb.send().await.map_err(|e| e.to_string())?;
                let mut stream = resp.bytes_stream();
                let mut offset = start;
                while let Some(chunk_res) = stream.next().await {
                    let bytes = chunk_res.map_err(|e| e.to_string())?;
                    write_positioned(&f, &bytes, offset).map_err(|e| e.to_string())?;
                    offset += bytes.len() as u64;
                    d.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                }
                Ok::<(), String>(())
            }));
        }

        for w in workers {
            let _ = w.await;
        }
        running.store(false, Ordering::Relaxed);
        mon.abort();
    } else {
        let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
        let mut out = File::create(&part_path).map_err(|e| e.to_string())?;
        let mut stream = resp.bytes_stream();
        let mut done = 0u64;
        while let Some(chunk_res) = stream.next().await {
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
