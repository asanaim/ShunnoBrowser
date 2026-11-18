## v1.1.1 (2026-11-18)
- Fixed `ICoreWebView2` -> `ICoreWebView2_8` COM interface cast inside `IsDocumentPlayingAudioChangedEventHandler` (`src/main.rs:184`).
- Added throttled `mousemove` fallback for the Trusted-Types safe floating media download pill so players with transparent overlay shields always trigger the pill.

# Changelog

## v1.1.0 (2026-11-18)
- **Clickable Docked Floating Controls & Downloads:** Fixed Win32 child WebView2 Z-order occlusion by docking the active webpage beside the open controls/downloads popover so every button works reliably while keeping the live webpage visible.
- **Tab Audio Mute Indicators (`🔊` / `🔇`):** Added live audio activity detection and one-click per-tab mute toggling in the tab strip.
- **Built-In Picture-in-Picture (`📺 PiP`):** Added native Web PiP button to the floating video hover pill, controls popover, and optional auto-PiP on window focus loss.
- **Download Auto-Cleanup & Duplicate Detection:** Detects existing completed or active downloads before duplicating tasks, adds a `Clear Completed` button, and supports auto-cleaning finished tasks.
- **Trusted-Types Safe YouTube Floating Pill & Ad Stripper:** Rebuilt the Shadow-DOM floating media pill without `.innerHTML` so YouTube's Trusted Types CSP never blocks it, and replaced polling intervals with debounced `MutationObserver` + `requestIdleCallback`.
- **Security & Architecture Hardening:** Extracted `src/sanitizer.rs`, `src/userscripts.rs`, and `assets/js/content_guard.js`; added per-launch cryptographic IPC tokens, SHA-256 user-script approval in `%LOCALAPPDATA%\ShunnoBrowser\scripts\`, Windows filename sanitization, `rustls-tls`, and automated tests.

## v1.0.0
- Initial native Rust + Edge WebView2 release with 32-thread range downloader, per-site policies, dark mode, and custom search engines.
