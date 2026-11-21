# Shunno Browser: Security & Privacy Architecture

1. **Two IPC trust zones.** The browser UI (`src/ui_chrome.rs`) runs in its own WebView with its own IPC handler and may issue privileged commands. Web content gets `assets/js/content_guard.js`, whose IPC handler accepts only a small reporting/download command set.
2. **Per-launch IPC token.** `src/main.rs` generates a 128-bit token injected into the content script. Every content message must carry it (constant-time compare); there is no token-free prefix. The script binds `chrome.webview.postMessage` at document start so later page code cannot intercept the token. The token stops other frames and scripts from forging messages; it is not a boundary against the page's own driver, so every content command is treated as untrusted input.
3. **Page-originated network requests are restricted.** `PROBE_STREAM`, `FLOAT_DL` and grabber probes may not target loopback, private, link-local, CGNAT or metadata hosts (`sanitizer::is_public_http_url`) unless the page itself is on that host, because the engine bypasses CORS and probe results are returned to the page. `OFFLINE_HTML` is accepted only after the user requests Save Page for that tab.
4. **UI rendering.** Page-controlled strings are HTML-escaped; values inside inline handlers are JS-escaped then HTML-escaped (`encodeAttr`).
5. **User-script sandboxing & explicit approval.** User-scripts live in `%LOCALAPPDATA%\ShunnoBrowser\scripts\`, `sanitize_download_filename` blocks `.js` writes from the downloader, and scripts only execute after their SHA-256 digest is approved in `global_settings.json`.
6. **Path traversal & Windows device names.** `sanitize_download_filename` strips separators, collapses `..`, removes control/illegal characters, caps length and prefixes reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1` to `LPT9`).
7. **Verified helper tools.** ffmpeg and yt-dlp are downloaded over HTTPS and accepted only if their SHA-256 matches the publisher's checksum file. They run without a console window and are killed as a process tree on pause.
8. **TLS.** `reqwest` uses native Windows TLS (SChannel via `native-tls`).
9. **Downloads are marked as from the Internet.** Files the engine saves (and saved pages) get the
   NTFS `Zone.Identifier` stream (Mark of the Web), so SmartScreen and Office Protected View check
   them; WebView2 marks its own downloads. File names lose bidirectional control characters
   (`photo\u202Egpj.exe`). A finished file never overwrites another: the final name is claimed with
   create-new before the move.
10. **Site settings follow the site, not the tab.** JavaScript, mobile User-Agent, ad and image
   blocking are set at every top-level NavigationStarting from the destination host's policy; the
   content script reads its flags from a per-host table by the top-level host.
11. **Helper processes end with the browser.** FFmpeg and yt-dlp run inside a kill-on-close job
   object. HLS playlists are fetched with network protocols only (`-protocol_whitelist`), and a public
   playlist may not point the engine at local addresses.
12. **Links from other programs** (second launch, `WM_COPYDATA`) are accepted only as http(s),
   `file:///` or a local page path; never `javascript:` or internal schemes.
13. **Not yet done.** Release binaries are not Authenticode-signed; SmartScreen will warn until a code-signing certificate is added to CI.

## Password vault (v2.17.0)

* **Storage.** `%LOCALAPPDATA%\ShunnoBrowser\passwords.vault`, encrypted with Windows DPAPI (`CryptProtectData`, current-user scope, app-specific entropy). Another Windows account, or a copy of the file on another PC, cannot decrypt it. Malware running as the same Windows user can, as with every browser's local store.
* **Matching.** Exact scheme, host and port; a leading `www.` is ignored. There is no cross-subdomain or registrable-domain matching, so `a.com.bd` never sees `b.com.bd` passwords, and `http://` never receives an `https://` password.
* **Filling.** Focusing a login field sends only usernames to the page, inside a closed shadow root. The password is injected only after the user picks an account, and the site is re-checked at that moment. The page cannot replace the fill entry points (non-writable, non-configurable).
* **Saving.** A submitted login only creates a pending offer; nothing is stored until the user clicks Save. Private tabs never offer to save. "Never" is remembered per site.
* **Reveal and copy.** Gated by Windows Hello (PIN, face, fingerprint) with a 2-minute grace period. If Hello is not set up, the action proceeds and the panel recommends setting it up.
* **CSV import.** Plaintext exports are read once; the panel offers to delete the file afterwards.
* **WebView2's built-in saver is disabled** so there is one prompt and one store. Passwords it saved earlier cannot be exported through any WebView2 API; "Clear old built-in passwords" removes them.
