#[path = "../src/state.rs"]
mod state;
#[path = "../src/sanitizer.rs"]
mod sanitizer;
#[path = "../src/userscripts.rs"]
mod userscripts;
#[path = "../src/downloader.rs"]
mod downloader;

use sanitizer::{extract_host, sanitize_url};
use state::GlobalSettings;
use userscripts::{compute_sha256, host_matches_pattern, parse_match_header};
use downloader::{detect_download_category, format_human_size, sanitize_download_filename};

#[test]
fn test_url_tracking_param_stripping_and_amp() {
    let cfg = GlobalSettings::default();
    let dirty = "https://example.com/news?id=42&utm_source=twitter&fbclid=abc123&gclid=xyz";
    assert_eq!(sanitize_url(dirty, &cfg), "https://example.com/news?id=42");

    let amp = "https://www.google.com/amp/s/example.com/story?utm_medium=social";
    assert_eq!(sanitize_url(amp, &cfg), "https://example.com/story");
}

#[test]
fn test_search_engine_query_formatting() {
    let mut cfg = GlobalSettings::default();
    cfg.active_search_engine = 0; // DuckDuckGo
    assert_eq!(
        sanitize_url("rust webview2", &cfg),
        "https://duckduckgo.com/?q=rust+webview2"
    );
}

#[test]
fn test_userscript_match_and_sha256() {
    let script = "// @match example.com\nconsole.log('ok');";
    let pat = parse_match_header(script);
    assert_eq!(pat, "example.com");
    assert!(host_matches_pattern("sub.example.com", &pat));
    assert!(!host_matches_pattern("other.org", &pat));
    assert_eq!(compute_sha256(b"shunno").len(), 64);
}

#[test]
fn test_filename_sanitization_and_reserved_names() {
    assert_eq!(sanitize_download_filename("../../evil/CON.txt"), "_CON.txt");
    assert_eq!(sanitize_download_filename("NUL"), "_NUL");
    assert_eq!(sanitize_download_filename("payload.js"), "payload.js.download");
    assert_eq!(detect_download_category("track01.flac"), "Lossless Audio");
    assert_eq!(format_human_size(1048576), "1.0 MB");
    assert_eq!(extract_host("https://github.com/asanaim"), "github.com");
}
