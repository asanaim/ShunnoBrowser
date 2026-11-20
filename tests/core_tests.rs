#![allow(dead_code)]

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

#[test]
fn test_content_range_and_disposition_parsing() {
    use downloader::{parse_content_disposition_filename, parse_content_range};
    assert_eq!(parse_content_range("bytes 0-0/1234"), Some((0, 0, Some(1234))));
    assert_eq!(parse_content_range("bytes 10-19/*"), Some((10, 19, None)));
    assert_eq!(parse_content_range("bytes 9-1/5"), None);
    assert_eq!(
        parse_content_disposition_filename("attachment; filename=\"a.bin\"; filename*=UTF-8''r%C3%A9sum%C3%A9.zip").as_deref(),
        Some("résumé.zip")
    );
}

#[test]
fn test_download_routing() {
    use downloader::{is_hls_url, is_ytdlp_url};
    assert!(is_ytdlp_url("https://www.youtube.com/watch?v=abc"));
    assert!(is_ytdlp_url("https://youtu.be/abc"));
    assert!(is_ytdlp_url("https://cdn.example.com/x/manifest.mpd?t=1"));
    assert!(!is_ytdlp_url("https://www.youtube.com/feed/subscriptions"));
    assert!(is_hls_url("https://a.b/x/index.m3u8?token=1"));
    assert!(!is_hls_url("https://a.b/x/video.mp4"));
}

#[test]
fn test_page_cannot_target_private_hosts() {
    use sanitizer::is_public_http_url;
    assert!(is_public_http_url("https://example.com/f.zip"));
    assert!(is_public_http_url("https://8.8.8.8/"));
    for u in [
        "http://127.0.0.1:8080/x",
        "http://192.168.0.1/",
        "http://169.254.169.254/latest",
        "http://localhost/",
        "http://router/",
        "http://[::1]/",
        "http://[fd00::1]/",
        "http://100.64.1.1/",
        "file:///c:/x",
    ] {
        assert!(!is_public_http_url(u), "{}", u);
    }
}

#[test]
fn test_hls_playlist_parsing() {
    use downloader::parse_hls_playlist;
    let base = url::Url::parse("https://cdn.x.com/v/master.m3u8").unwrap();
    let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=800000,CODECS=\"avc1,mp4a\"\nlow/index.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=5000000\n/abs/hi.m3u8\n";
    let pl = parse_hls_playlist(master, &base);
    let best = pl.variants.iter().max_by_key(|v| v.0).unwrap();
    assert_eq!(best.1.as_str(), "https://cdn.x.com/abs/hi.m3u8");
    let media = "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXT-X-KEY:METHOD=AES-128,URI=\"k\"\n#EXTINF:4,\nseg1.m4s\n";
    let pm = parse_hls_playlist(media, &base);
    assert!(pm.encrypted && pm.init_segment.is_some());
    assert_eq!(pm.segments[0].as_str(), "https://cdn.x.com/v/seg1.m4s");
}

#[test]
fn test_builtin_ad_matcher() {
    use sanitizer::is_blocked_ad_url;
    assert!(is_blocked_ad_url("https://securepubads.g.doubleclick.net/gampad/ads?x=1"));
    assert!(is_blocked_ad_url("https://www.youtube.com/pagead/viewthroughconversion/1"));
    assert!(is_blocked_ad_url("https://rr1.googlevideo.com/initplayback?source=youtube&oad=1"));
    assert!(!is_blocked_ad_url("https://www.youtube.com/watch?v=abc"));
    assert!(!is_blocked_ad_url("https://rr1.googlevideo.com/videoplayback?itag=22"));
    assert!(!is_blocked_ad_url("https://notdoubleclick.net.example.com/x"));
    assert!(!is_blocked_ad_url("https://example.com/img.png"));
}
