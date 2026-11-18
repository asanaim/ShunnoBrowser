# Shunno Browser — Security & Privacy Architecture

1. **Cryptographic Per-Launch IPC Token:** On startup, `src/main.rs` generates a 128-bit random hex token injected into `assets/js/content_guard.js`. Every `window.ipc.postMessage` payload from web content must carry this prefix or is dropped immediately, preventing untrusted webpages from spoofing browser commands.
2. **User-Script Sandboxing & Explicit Approval:** User-scripts live in `%LOCALAPPDATA%\ShunnoBrowser\scripts\` (isolated from `Downloads`), `sanitize_download_filename` blocks `.js` writes from the downloader, and scripts only execute after their SHA-256 digest is explicitly approved in `global_settings.json`.
3. **Path Traversal & Windows Device Name Hardening:** `sanitize_download_filename` strips directory separators, collapses `..`, removes control/illegal characters, and prefixes reserved Windows device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1–LPT9`).
4. **Memory-Safe TLS (`rustls-tls`):** Configured in `Cargo.toml` without C/OpenSSL dependencies.
