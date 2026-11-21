# Shunno Browser (`v2.32.0`)

**Shunno Browser** is a fast, lightweight, privacy-first Windows desktop web browser built in **Rust** (`tao` + `wry` / Microsoft Edge WebView2 + `tokio` + `reqwest`) with an integrated **16-segment multi-threaded download accelerator**, **live media stream sniffer**, **floating media download pill**, and **optional FFmpeg DASH HD muxer**.

---

## Key Features

### 1. Multi-Threaded Download Engine & Media Sniffer
* **Up to 16 Parallel HTTP Range Segments:** Automatically probes `Accept-Ranges` and splits large downloads across up to 16 concurrent connections for maximum throughput.
* **Authenticated Session Cookie & Referer Forwarding:** Extracts active tab cookies (`document.cookie`), `Referer`, and `User-Agent` headers so login-protected files, courses, and gated attachments download seamlessly without 403/login redirects.
* **Smart Filename & Extension Resolution:** Resolves clean filenames from HTTP `Content-Disposition` (`filename*=` UTF-8 and `filename=`), page/media titles, and MIME `Content-Type` headers—preventing tokenized CDN streams (`videoplayback`, hashes) from saving as garbled or extensionless files.
* **Floating Shadow-DOM Media Download Menu (`0↓ Download`):** Hovering over any HTML5 video player, `<audio>` player, or downloadable file link displays an isolated floating download pill with **live HTTP `HEAD` / `Range(0-0)` file size probing** (`MB` / `GB`).
* **Native WebView2 `WebResourceRequested` Stream Hook:** Intercepts `.mp4`, `.webm`, `.m4s`, `.m3u8`, `.mpd`, `.mp3`, `.m4a`, `.flac`, and `.ogg` media requests in real time inside the Downloads drawer.
* **Optional FFmpeg DASH Video + Audio Muxer:** Automatically pairs split DASH video and audio streams and muxes them losslessly into a single `.mp4` file (`ffmpeg -c copy`) when `ffmpeg.exe` is available in `PATH` or placed next to `ShunnoBrowser.exe`.
* **Multi-Line Batch URL Downloader:** Paste dozens of URLs (separated by newlines, commas, or spaces) to deduplicate and queue all downloads concurrently.
* **Draggable Floating Download Basket (`🧺 Drop 0↓`):** Enable from `☰ Menu` to get an on-page draggable drop zone where dragging any link, media URL, or multi-line text selection starts an immediate download.

### 2. Privacy Shields, Ad Blocking & YouTube Ad Stripper
* **Network-Level Ad & Tracker Shield:** Blocks ad networks and tracking endpoints directly in WebView2 `WebResourceRequested` before requests leave your PC.
* **Content Blocker (Site Controls, Content Blocker):** Choose filter lists from a fixed catalog (EasyList and EasyPrivacy on by default; uBlock filters, Peter Lowe's list, uBlock Privacy and Badware, EasyList Cookie, Fanboy Social and Annoyance, IndianList, uBlock Unbreak), update them automatically every 1, 4, 7 or 14 days or with Update Now, and keep your own rules in **My Filters** (Adblock Plus / uBlock syntax, with Import and Export as `.txt`).
* **Element Picker:** Right click a page and choose **Hide Element on Page…**, or use **Hide an Element on This Page** in the Content Blocker. Point, click, widen with Broader or Narrower, then **Hide Once** or **Hide Always** (saved as `site##selector` in My Filters).
* **YouTube Pre-Roll Ad Stripper:** Hooks `window.ytInitialPlayerResponse` and `JSON.parse` at document start to strip `adPlacements`, `playerAds`, `adSlots`, and `adBreakHeartbeatParams` before playback begins, paired with a fast 150ms fallback skip trigger.
* **Per-Site Permission Matrix (`site_permissions.json`):** Customize JavaScript, Cookies, Images, Ad Shield, Desktop User-Agent, Reader Mode, and Page Zoom independently per domain.
* **Private / Incognito Tabs (`Ctrl + Shift + N`):** Browse without saving history or session tabs, plus one-click **Clear Site Data** (`localStorage`, `sessionStorage`, and site cookies).

### 3. Clean Interface, Full Dark Mode & Multi-Search
* **Decluttered Top Toolbar:** Minimal tab strip (`+` new tab, `✕` close tab — closing the final tab exits the browser cleanly) and compact navigation bar (`‹`, `›`, `↻`, Search Engine Selector, Omnibox, `⚡ Downloads`, `☰ Menu`).
* **Full Dark Mode (`Ctrl + Shift + D`):** Transforms both the Shunno Browser interface and active webpages into a comfortable dark theme while preserving natural colors on images and videos.
* **Multiple Search Engines + Custom Engine Manager:** Switch instantly between **DuckDuckGo**, **Google**, **Startpage**, **Bing**, and **YouTube**, or add any custom `%s` search provider inside `☰ Menu`.
* **True Live-DOM Offline HTML Snapshot:** Saves the live rendered DOM (`outerHTML` with `<base href>` preserved) as a standalone `.html` file instead of fetching a cold redirect stub.
* **Distraction-Free Reader Mode (`Alt + R`):** Strips page clutter into clean typography for long-form reading.

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + T` | Open New Tab |
| `Ctrl + W` | Close Active Tab (exits browser if last tab) |
| `Ctrl + Tab` / `Ctrl + Shift + Tab` | Switch to Next / Previous Tab |
| `Ctrl + L` / `Alt + D` / `F6` | Focus Address Bar |
| `Ctrl + R` / `F5` | Reload Current Page |
| `Alt + Left` / `Alt + Right` | Navigate Back / Forward |
| `Ctrl + J` | Open Multi-Threaded Downloads & Sniffer Drawer |
| `Ctrl + D` | Bookmark Current Page |
| `Ctrl + Shift + D` | Toggle Full Dark Mode |
| `Alt + R` | Toggle Reader Mode |
| `Ctrl + +` / `Ctrl + -` / `Ctrl + 0` | Zoom In / Zoom Out / Reset Zoom (100%) |

---

## Building on Windows

### Prerequisites
* Windows 10 or Windows 11 (x64) with Microsoft Edge WebView2 Runtime (pre-installed on Windows 10/11)
* [Rust toolchain](https://rustup.rs/) (`stable-x86_64-pc-windows-msvc`)
* *(Optional)* `ffmpeg.exe` in your system `PATH` or in the same folder as `ShunnoBrowser.exe` for automatic DASH HD video+audio muxing.

### Build Command
```powershell
cargo build --release --target x86_64-pc-windows-msvc
```
The compiled executable will be generated at `target/x86_64-pc-windows-msvc/release/ShunnoBrowser.exe`.