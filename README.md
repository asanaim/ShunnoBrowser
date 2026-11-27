# Shunno Browser (শূন্য ব্রাউজার) — v2.36.0

**Shunno Browser** is a lightweight, fast desktop web browser and multi-threaded download manager for Windows 10 & 11 built in pure **Rust** (`tao` + `wry` Edge WebView2 + `tokio` + `reqwest` with native Windows TLS / SChannel).

## Direct Download
- **[Download ShunnoBrowser.exe](https://github.com/asanaim/ShunnoBrowser/releases/latest/download/ShunnoBrowser.exe)**

---

## Key Features

### 1. Downloads Multi-Threaded Download Engine (`1–32 Threads`)
- **32-Thread HTTP Range Acceleration:** Verifies `206 Partial Content` + `Content-Range` before splitting downloads across up to **32 concurrent async Tokio streams** with live segment progress bars (`[████▓▓▒▒]`) and graceful single-stream fallback on `200 OK` servers.
- **Duplicate Detection & Auto-Cleanup:** Detects existing active or completed files before creating duplicate tasks (offering one-click **Open** or **Download Again**), plus **🧹 Clean** and **Auto-Clean Done** switches.
- **Trusted-Types Safe Floating Media Pill + Picture-in-Picture (`📺 PiP`):** Hovering over YouTube videos, HTML5 `<video>`/`<audio>` players, or downloadable links floats an isolated Shadow-DOM **`0↓ Download`** pill with live HTTP `HEAD`/`Range` file-size probing (`MB`/`GB`), stream quality selector, and a one-click **`📺 PiP`** button.
- **HLS (`.m3u8`) & DASH:** HLS is downloaded through FFmpeg (encrypted, byte-ranged and fMP4 streams included) with a plain segment fallback; split DASH video + audio are muxed losslessly (`-c copy`). FFmpeg is auto-installed on first use (SHA-256 verified) into `%LOCALAPPDATA%\ShunnoBrowser\bin\`, or picked up from `PATH`.
- **YouTube via yt-dlp:** YouTube watch/shorts/live pages and `.mpd` manifests are routed to yt-dlp (auto-installed, SHA-256 verified), which merges the best video and audio into MP4 with live progress, pause and resume.
- **Real Pause/Resume:** multi-connection and single-connection transfers resume from `.offset` sidecars; browser-native (WebView2) downloads are paused/resumed through `ICoreWebView2DownloadOperation`.

### 2. Tab Audio Mute Indicators, Ad-Free Browsing & Speed
- **Tab Audio Mute Indicators (`🔊` / `🔇`):** Detects playing audio via native `ICoreWebView2_8` and HTML5 media events, letting you mute/unmute individual tabs with one click.
- **YouTube Pre-Roll Ad Stripper & Debounced Cleaner:** Strips `adPlacements` and `playerAds` from YouTube player JSON before playback starts, blocks ad requests natively in `WebResourceRequested`, and uses a `MutationObserver` (zero idle polling) for ad skipping and cookie banner rejection.
- **Background Media Playback & Video Speed (`0.75×` – `4.0×`):** Keeps media playing in background tabs and adjusts playback speed on any webpage.

### 3. Docked Controls, Dark Mode & Hardened Security
- **Non-Overlapping Docked Controls & Downloads (`•••` / `⚡`):** Opens cleanly alongside the live webpage so every button is immediately clickable while keeping the page visible.
- **Full Dark Mode & Custom Search Engines:** Toggle Dark Mode across both browser UI and webpages, switch between DuckDuckGo, Google, Startpage, Bing, and YouTube, or add custom `%s` search engines.
- **Cryptographic IPC Token & Explicit User-Script Approval:** Protects `postMessage` with a per-launch random token and requires explicit SHA-256 approval for scripts in `%LOCALAPPDATA%\ShunnoBrowser\scripts\`.

### 4. Keyboard Shortcuts
- **`F11`** — Toggle Borderless Full Screen
- **`Ctrl + T`** — New Tab (`+` sits right next to the active tab)
- **`Ctrl + W`** — Close Active Tab (exits cleanly when closing the final tab)
- **`Ctrl + L`** — Focus Address Bar
- **`Ctrl + F`** — Find in Page
- **`Ctrl + J`** — Open Downloads Downloads
- **`Ctrl + R`** — Reload Page
- **`Esc`** — Close Side Panel


## License
Free to use. Proprietary, closed source: Copyright (c) 2026 Sayed Naim. All rights reserved. The source is visible for transparency; it is not open source. See `LICENSE`.
