## v2.1.3 (2026-11-20)
- Fixed `E0061` argument mismatch on `push_chrome_state(...)` inside `TabTitleChanged` (`src/main.rs`).
- Added `move` to `push_chrome_state` closure so `last_chrome_json` moves cleanly into `event_loop.run` (`E0373`).

## v2.1.2 (2026-11-20)
- Fixed `assets/app.ico` 4x4 supersampling RGB double-division bug so the Windows taskbar, Alt+Tab, and `.exe` icon render at full sapphire/white/coral brightness instead of a dark circle.
- Fixed `windows = "0.56.0"` Win32 type signatures (`HINSTANCE(hinst.0)` in `LoadImageW`, `HWND(isize)`, `HWND(0)`, `parent.0 != 0`) in `src/main.rs`.
- Added JSON snapshot deduplication in `push_chrome_state` to skip redundant UI IPC evaluations.

## v2.1.1 (2026-11-20)
- Fixed `HWND` / `HICON` raw pointer conversions (`*mut c_void` vs `isize`) in `windows = "0.56.0"` inside `src/main.rs`.

## v2.1.0 (2026-11-19)
- Fixed `SetWindowSubclass` / `DefSubclassProc` error by using standard `SetWindowLongPtrW(GWLP_WNDPROC)` + `CallWindowProcW`, and removed unused `Icon` import (0 errors, 0 warnings).
- Added **Live Download Speed Sparkline Graph** (`⚡` current & peak throughput SVG area chart) inside **Downloads**.
- Added one-click **Per-Site Cookie & Storage Cleaner (`🧹 Clear Site Data`)** inside **Controls (`•••`)**.

## v2.1.0 (2026-11-19)
- Completed all 20 roadmap items natively in pure Rust: zero localhost open ports, 3px borderless window edge resize grip, F11 fullscreen hiding of both tab & nav bars, single-click URL select-all, full URL display, elevated search engine selector popup, larger 34x34px vector navigation icons, duplicate Skip/Rename/Overwrite dialog, sequential download queue scheduler (`Max 1-4` + delayed start + post-queue actions), `Ctrl+Shift+T` closed-tab restore, pinned icon-only tabs, live `1 / N` Find-in-Page match counter, draggable persistent floating drop basket, and one-click `⬇ Install FFmpeg` auto-extractor.

## v2.1.0 (2026-11-19)
- Fixed `E0063` missing fields (`active_reader` in `ChromeSnapshot`, `reader_active` in `BrowserTab`).
- Removed all external IDM / standalone downloader branding and merged 100% of multi-threaded download manager features natively into Shunno Browser's built-in **Downloads (`⚡`)** manager.
- Added **Drag-to-Reorder Tabs** in the tab strip (`REORDER_TAB`) and **Bookmark Folder Grouping** across the Bookmarks Bar and Library.

## v2.1.0 (2026-11-19)
- Fixed `proxy` clone (`ad_proxy`) in `WebResourceRequestedEventHandler` (`E0382`).
- Fixed Taskbar Icon via `LoadImageW` from embedded EXE icon resource #1 + `SetClassLongPtrW` (`GCLP_HICON`/`GCLP_HICONSM`) + `WM_SETICON`.
- Added native Win32 8-directional mouse window resizing (`WM_NCHITTEST` + `WS_THICKFRAME`) on borderless window.
- Added `ZoomFactorChangedEventHandler` so `Ctrl + Mouse Scroll` zooms smoothly and updates the zoom indicator.
- Eliminated Menu/Download open flicker by keeping the live webpage visible beneath a transparent `chrome_wv` (`SetDefaultBackgroundColor` alpha=0) with a 10% subtle blur.
- Added On/Off toggle for Reader Mode (`📖 Reader`), Rename (`✎`) & Remove (`✕`) on Bookmarks, and coordinate-based IDM media detection + optional `native engine` forwarding.

## v1.7.2 (2026-11-19)
- Fixed `#[cfg(windows)]` / `#[cfg(not(windows))]` attribute pairing on `apply_tab_zoom` and `set_webview_muted` in `src/main.rs` (`E0428`).

## v1.7.1 (2026-11-19)
- Fixed `proxy` clone (`ad_proxy`) in `WebResourceRequestedEventHandler` (`src/main.rs:242`).
- Added native `ICoreWebView2Controller::SetZoomFactor` + capture-phase `Ctrl++` / `Ctrl+-` / `Ctrl+0` / `Ctrl+Wheel` zoom handling.
- Added the Shunno compass-ring emblem inside `New Tab` pills.

## v1.7.1 (2026-11-19)
- Cast `ICoreWebView2` to `ICoreWebView2_11` (`add_ContextMenuRequested`) and `ICoreWebView2Environment` to `ICoreWebView2Environment9` (`CreateContextMenuItem`) in `src/main.rs`.

## v1.6.1 (2026-11-19)
- Fixed Win32 COM stream & memory feature flags (`Win32_System_Com_StructuredStorage`, `Win32_System_Memory`) and `HGLOBAL::default()` in `capture_tab_preview_to_chrome`.
- Replaced Edge WebView2's right-click **"Open link in new window"** context menu item with **"Open link in new tab"**.

## v1.6.0 (2026-11-18)
- Fixed `ICoreWebView2` -> `ICoreWebView2_8` COM interface cast inside `IsDocumentPlayingAudioChangedEventHandler` (`src/main.rs:184`).
- Added throttled `mousemove` fallback for the Trusted-Types safe floating media download pill so players with transparent overlay shields always trigger the pill.

# Changelog

## v1.6.0 (2026-11-18)
- **Clickable Docked Floating Controls & Downloads:** Fixed Win32 child WebView2 Z-order occlusion by docking the active webpage beside the open controls/downloads popover so every button works reliably while keeping the live webpage visible.
- **Tab Audio Mute Indicators (`🔊` / `🔇`):** Added live audio activity detection and one-click per-tab mute toggling in the tab strip.
- **Built-In Picture-in-Picture (`📺 PiP`):** Added native Web PiP button to the floating video hover pill, controls popover, and optional auto-PiP on window focus loss.
- **Download Auto-Cleanup & Duplicate Detection:** Detects existing completed or active downloads before duplicating tasks, adds a `Clear Completed` button, and supports auto-cleaning finished tasks.
- **Trusted-Types Safe YouTube Floating Pill & Ad Stripper:** Rebuilt the Shadow-DOM floating media pill without `.innerHTML` so YouTube's Trusted Types CSP never blocks it, and replaced polling intervals with debounced `MutationObserver` + `requestIdleCallback`.
- **Security & Architecture Hardening:** Extracted `src/sanitizer.rs`, `src/userscripts.rs`, and `assets/js/content_guard.js`; added per-launch cryptographic IPC tokens, SHA-256 user-script approval in `%LOCALAPPDATA%\ShunnoBrowser\scripts\`, Windows filename sanitization, `rustls-tls`, and automated tests.

## v1.0.0
- Initial native Rust + Edge WebView2 release with 32-thread range downloader, per-site policies, dark mode, and custom search engines.
