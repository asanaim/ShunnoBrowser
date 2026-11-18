# Shunno Browser (শূন্য ব্রাউজার) — v1.0.0

**Shunno Browser** is a lightweight, fast desktop web browser and multi-threaded download manager for Windows 10 & 11 built in pure **Rust** (`tao` + `wry` Edge WebView2 + `tokio` + `reqwest`).

## Direct Download
- **[Download ShunnoBrowser.exe](https://github.com/asanaim/ShunnoBrowser-Windows/releases/latest/download/ShunnoBrowser.exe)**

---

## Complete Feature List

### 1. Shunno Get Multi-Threaded Download Engine (`1–32 Threads`)
- **32-Thread Parallel HTTP Range Acceleration:** Automatically probes servers for `Accept-Ranges: bytes` and splits downloads across up to **32 concurrent async Tokio streams** writing directly via OS positioned file writes (`FileExt::seek_write`) with live visual segment bars (`[████▓▓▒▒]`).
- **True Pause, Resume & Expired Link Repair (`Fix Link`):** Checkpoints per-thread byte offsets in `%LocalAppData%\ShunnoBrowser\parts` and persists queue state in `downloads_queue.json`, allowing you to replace expired CDN URLs without losing already-downloaded bytes.
- **Floating Hover Download Pill + Live Size Probing (`MB` / `GB`):** Hovering over any video player, `<audio>` stream, or downloadable file link floats an isolated Shadow-DOM **`0↓ Download`** pill directly over the object with live HTTP `HEAD` / `Range(0-0)` file-size probing and a multi-stream quality selector.
- **Drag-and-Drop Floating Download Basket (`0↓ Drop Link`):** Drag any link, selected URL text, or multi-line link list onto the floating corner drop basket to immediately queue 16-thread downloads.
- **WebView2 Native `WebResourceRequested` Network Hook:** Intercepts `.mp4`, `.m3u8` (HLS), `.mpd` (DASH), `.m4s`, `.flac`, `.m4a` (ALAC), `.wav`, `.mp3`, `.zip`, `.7z`, `.rar`, `.iso`, `.exe`, `.msi`, `.apk`, and `.pdf` streams directly at Chromium's native network layer.
- **Active Session Cloning & Smart Filename Resolution:** Attaches the active tab's `Cookie` and `Referer` headers to every download and resolves real filenames from `Content-Disposition` headers and `Content-Type` MIME types.
- **HLS (`.m3u8`) Stream Grabber & Optional FFmpeg DASH Muxer:** Parses `.m3u8` master/media playlists into `.mp4`, and includes a built-in **Mux Video + Audio** tool that combines split DASH video and audio streams losslessly (`-c copy`) when `ffmpeg.exe` is placed in `Downloads\ShunnoBrowser\bin\` or on your system `PATH`.
- **Wildcard Batch Grabber, Speed Limiter & SHA-256 Checksum:** Expands sequential URL ranges (`track_[01-12].flac`), processes multi-line URL pastes, throttles bandwidth (`10 MB/s`, `5 MB/s`, `2 MB/s`, `512 KB/s`), categorizes finished files into `Downloads\ShunnoBrowser\<Category>`, and computes a **SHA-256** hash for every completed file.

### 2. Ad-Free Browsing & YouTube Media Enhancements
- **Native Network & DOM Ad/Tracker Blocker:** Blocks ad/tracker domains at the WebView2 `WebResourceRequested` layer with HTTP 403 before bytes are downloaded, plus cosmetic DOM filtering.
- **YouTube Pre-Roll Ad Stripper & Fast Skipper:** Hooks `window.ytInitialPlayerResponse`, `JSON.parse`, and `Response.prototype.json` before playback starts to strip `adPlacements`, `playerAds`, `adSlots`, and `adBreakHeartbeatParams`, removes Shorts shelves, and includes a 150ms fallback ad skipper.
- **Background Media Playback:** Locks the Page Visibility API (`document.hidden = false`, `visibilityState = 'visible'`) so YouTube and audio streams continue playing seamlessly in background tabs or when minimized.
- **HTML5 Video/Audio Speed Controller (`0.75×` – `4.0×`):** Cycle playback speed across any web media player from `☰ Menu`.

### 3. Clean Interface, Privacy & Power-User Controls
- **Full Dark Mode Switch:** Toggle Full Dark Mode across both the browser interface and active webpages from `☰ Menu`.
- **Multi-Tab Browsing, Private Tabs & Session Restore:** Open multiple tabs (`+ Tab`), isolated Private Incognito tabs (`🕵 Private`), automatically restore open tabs on restart (`session.json`), and close the browser cleanly when closing the final tab.
- **Customizable Search Engines:** Switch between **DuckDuckGo**, **Google**, **Startpage**, **Bing**, and **YouTube** from the toolbar dropdown, or add your own custom `%s` search engine in `☰ Menu`.
- **Per-Site Granular Controls (`site_policies.json`):** Toggle **Ad & Tracker Blocker**, **YouTube Ad & Shorts Clean**, **JavaScript**, **Load Images**, **Desktop User-Agent**, and **Auto-Reject Cookie Banners** independently per domain.
- **Reader View, Offline HTML Snapshots, Bookmarks, History & UserScripts:**
  - **Clean Reader View (`📖`):** Reformats articles into distraction-free typography.
  - **True Offline Page Snapshots (`💾`):** Saves complete `<!DOCTYPE html>` page snapshots to `Downloads\ShunnoBrowser\`.
  - **Tracking Link Sanitizer & Pop-Up Blocker:** Automatically strips `utm_*`, `fbclid`, `gclid`, `si`, and `igshid` parameters from URLs and blocks pop-up ad windows.
  - **Portable Bookmarks & History (`⭐`):** Persistent bookmarks and history with `.txt` bookmark import/export.
  - **UserScripts (`Downloads\ShunnoBrowser\scripts\*.js`):** Automatically executes domain-matched (`// @match`) custom JavaScript files on page load.

### 4. Keyboard Shortcuts
- **`Ctrl + T`** — Open New Tab
- **`Ctrl + W`** — Close Active Tab (exits browser if last tab)
- **`Ctrl + L`** — Focus & Select Address Bar
- **`Ctrl + F`** — Find in Page
- **`Ctrl + J`** — Open Shunno Get Downloads Drawer
- **`Ctrl + R`** — Reload Active Page
- **`Esc`** — Close Active Drawer
