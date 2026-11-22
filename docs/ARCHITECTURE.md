# Shunno Browser — Architecture Overview (`v2.34.2`)

## 1. Process & Window Model
- **Window & Event Loop (`tao`):** Native Win32 window hosting two child `wry` Edge WebView2 surfaces (`chrome_wv` and active `BrowserTab.webview`).
- **Non-Overlapping Split Bounds (`layout_webviews`):** Because Win32 WebView2 child HWNDs do not alpha-composite over sibling HWNDs, opening `Controls`, `Downloads`, or `Library` docks the `440px` panel cleanly on the right while resizing the active tab viewport to `(0, 86, width - 440, height - 86)`. This guarantees 100% click responsiveness while keeping the running webpage visible beside the panel.

## 2. Module Layout
- `src/main.rs`: Event loop, tab lifecycle, native `ICoreWebView2_2` / `ICoreWebView2_8` hooks, and authenticated IPC router.
- `src/state.rs`: Persistent JSON storage (`%LOCALAPPDATA%\ShunnoBrowser\`) for global settings, per-site policies, bookmarks, history, and session restore.
- `src/sanitizer.rs`: Tracking parameter removal (`utm_*`, `fbclid`, `gclid`, `si`, `igshid`) and AMP-to-canonical URL resolution.
- `src/userscripts.rs`: SHA-256 hash verification and `@match` domain gating for approved user-scripts.
- `src/downloader.rs`: 32-thread HTTP range downloader (`NativeDownloaderDesktop`) with `206 Partial Content` verification, duplicate detection, auto-cleanup, HLS `.m3u8` assembly, and optional FFmpeg DASH muxing.
- `src/blocker.rs`: Brave `adblock` engine. Filter list catalog, per list text cache (`filters/list-<id>.txt`), My Filters (`filters/user.txt`), settings (`filters/settings.json`), and the compiled engine cache with a build key (`filters/engine.key`) so a change recompiles without a download. Element picker rules are validated as `host##selector` for the tab's own host.
- `assets/js/content_guard.js`: Element picker (closed Shadow DOM, constructable stylesheet, trusted input only).
- `assets/js/content_guard.js`: Trusted-Types safe Shadow-DOM floating media pill (`PiP` + stream picker), drag-and-drop download basket, debounced `MutationObserver` cleaner, and audio playback reporter.
