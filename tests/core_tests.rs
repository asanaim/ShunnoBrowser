#![allow(dead_code)]

#[path = "../src/state.rs"]
mod state;
#[path = "../src/sanitizer.rs"]
mod sanitizer;
#[path = "../src/userscripts.rs"]
mod userscripts;
#[path = "../src/downloader.rs"]
mod downloader;
#[path = "../src/https_guard.rs"]
mod https_guard;
#[path = "../src/blocker.rs"]
mod blocker;
#[path = "../src/updater.rs"]
mod updater;
#[path = "../src/importer.rs"]
mod importer;

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
    // Right-click search always searches, even for domain-like or multi-line selections.
    assert_eq!(sanitizer::search_url("example.com", &cfg), "https://duckduckgo.com/?q=example.com");
    assert_eq!(sanitizer::selection_query("  রবীন্দ্রনাথ\n  ঠাকুর \t"), "রবীন্দ্রনাথ ঠাকুর");
    assert_eq!(sanitizer::search_url(&sanitizer::selection_query("C++ & Rust?"), &cfg), "https://duckduckgo.com/?q=C%2B%2B+%26+Rust%3F");
    assert_eq!(sanitizer::selection_query(&"a".repeat(500)).len(), 300);
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
    assert_eq!(detect_download_category("song.mp3"), "Audio");
    assert_eq!(detect_download_category("talk.opus"), "Audio");
    assert_eq!(detect_download_category("master.wav"), "Lossless Audio");
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
    use downloader::{split_route_prefix, KIND_YTDLP, KIND_YTDLP_AUDIO, KIND_YTDLP_FLAC};
    let u = "https://www.youtube.com/watch?v=abc";
    assert_eq!(split_route_prefix(&format!("ytdlp:{u}")), (Some(KIND_YTDLP), u));
    assert_eq!(split_route_prefix(&format!("ytdlp-audio:{u}")), (Some(KIND_YTDLP_AUDIO), u));
    assert_eq!(split_route_prefix(&format!("ytdlp-flac:{u}")), (Some(KIND_YTDLP_FLAC), u));
    assert_eq!(split_route_prefix(u), (None, u));
}

#[test]
fn test_video_and_audio_of_same_page_are_separate_downloads() {
    use downloader::{EnqueueOutcome, NativeDownloadEngine, KIND_YTDLP, KIND_YTDLP_AUDIO, KIND_YTDLP_FLAC};
    let rt = tokio::runtime::Runtime::new().unwrap();
    let e = NativeDownloadEngine::new(0, false, 3);
    let u = "https://www.youtube.com/watch?v=shunnoTest1";
    let later = Some(86_400); // scheduled, so nothing runs
    let mut go = |raw: String| e.enqueue(rt.handle(), raw, 1, None, None, "CHECK", later);
    assert!(matches!(go(format!("ytdlp:{u}")), EnqueueOutcome::Started));
    assert!(matches!(go(format!("ytdlp-audio:{u}")), EnqueueOutcome::Started));
    assert!(matches!(go(format!("ytdlp-flac:{u}")), EnqueueOutcome::Started));
    assert!(matches!(go(format!("ytdlp-audio:{u}")), EnqueueOutcome::DuplicateDetected(_)));
    let tasks = e.tasks.lock().unwrap().clone();
    let mine: Vec<_> = tasks.iter().filter(|t| t.url == u).collect();
    let kind_cat = |k: &str| mine.iter().find(|t| t.kind == k).map(|t| (t.category.clone(), t.file_name.clone())).unwrap();
    assert_eq!(kind_cat(KIND_YTDLP).0, "Videos");
    assert_eq!(kind_cat(KIND_YTDLP_AUDIO), ("Audio".to_string(), "Audio (resolving title).m4a".to_string()));
    assert_eq!(kind_cat(KIND_YTDLP_FLAC), ("Lossless Audio".to_string(), "Audio (resolving title).flac".to_string()));
    for t in mine {
        e.delete(t.id);
    }
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

#[test]
fn test_https_only_guard() {
    use https_guard::{Decision, HttpsGuard};
    let g = HttpsGuard::default();
    assert_eq!(g.decide(1, "http://example.com/a"), Decision::Allow, "disabled guard must not act");
    g.set_enabled(true);

    // Upgrade public http, keep path and query; drop explicit :80.
    assert_eq!(g.decide(1, "http://example.com:80/a?b=1"), Decision::Upgrade("https://example.com/a?b=1".into()));
    // Successful secure load clears in-flight state; a later http link is upgraded again (no false loop).
    assert_eq!(g.on_navigation_done(1, true, false), None);
    assert!(matches!(g.decide(1, "http://example.com/b"), Decision::Upgrade(_)));
    // Secure load redirected straight back to http: warn instead of looping.
    assert_eq!(g.decide(1, "http://example.com/b"), Decision::Interstitial("http://example.com/b".into()));

    // Never touch LAN, loopback, intranet, custom ports, https.
    for u in ["http://192.168.1.1/", "http://localhost:3000/", "http://router/", "http://example.org:8080/", "https://example.org/"] {
        assert_eq!(g.decide(2, u), Decision::Allow, "{}", u);
    }

    // Failed upgraded load (refused/timeout/cert): our cancel is ignored, the failure yields the http URL.
    assert!(matches!(g.decide(3, "http://nossl.example.net/x"), Decision::Upgrade(_)));
    assert_eq!(g.on_navigation_done(3, false, true), None);
    assert_eq!(g.on_navigation_done(3, false, false), Some("http://nossl.example.net/x".into()));

    // A forged allow mark from a page does not downgrade; the real one-time link does, once per host.
    assert!(matches!(g.decide(4, "http://bank.example.com/#shunno-allow-http-0123456789abcdef"), Decision::Upgrade(_)));
    let _ = g.on_navigation_done(4, true, false);
    let link = g.issue_allow_link("http://nossl.example.net/x").unwrap();
    assert_eq!(g.decide(5, &link), Decision::Allow);
    assert_eq!(g.decide(5, "http://nossl.example.net/other"), Decision::Allow, "host stays allowed for the session");
    // Replaying the same nonce for a different host grants nothing.
    let other = link.replace("nossl.example.net", "bank.example.com");
    assert!(matches!(g.decide(6, &other), Decision::Upgrade(_)));

    let html = https_guard::interstitial_html("http://a.example/<x>", "http://a.example/#shunno-allow-http-1");
    assert!(html.contains("a.example") && !html.contains("<x>"));
}

#[test]
fn test_adblock_engine_network_and_cosmetic() {
    let list = "[Adblock Plus 2.0]\n||doubleclick.net^\n||tracker.example^$third-party\n@@||tracker.example/consent.js\n##.adsbygoogle\nnews.example##.promo-box\nnews.example#@#.adsbygoogle\n";
    let (engine, rules) = blocker::build_engine(&[list.to_string()]);
    assert!(rules >= 5);
    let check = |u: &str, src: &str, t: &str| {
        engine
            .check_network_request(&adblock::request::Request::new(u, src, t, "GET").unwrap())
            .should_block()
    };
    assert!(check("https://securepubads.g.doubleclick.net/gpt.js", "https://site.example/", "script"));
    assert!(check("https://tracker.example/t.js", "https://site.example/", "script"));
    assert!(!check("https://tracker.example/t.js", "https://tracker.example/", "script"), "first-party allowed");
    assert!(!check("https://tracker.example/consent.js", "https://site.example/", "script"), "exception honoured");

    let generic = blocker::cosmetic_css_for(&engine, "https://other.example/", &["adsbygoogle".into(), "article".into()], &[]);
    assert!(generic.contains(".adsbygoogle{display:none !important}"));
    assert!(!generic.contains(".article"));
    let specific = blocker::cosmetic_css_for(&engine, "https://news.example/a", &["adsbygoogle".into()], &[]);
    assert!(specific.contains(".promo-box"));
    assert!(!specific.contains(".adsbygoogle"), "site exception removes the generic rule");
}

#[test]
fn test_every_content_script_placeholder_is_replaced() {
    // A placeholder left in the injected script is a ReferenceError that kills every in-page feature.
    let js = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/js/content_guard.js")).unwrap();
    let main = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")).unwrap();
    let mut rest = js.as_str();
    let mut found = 0;
    while let Some(i) = rest.find("__SHUNNO_") {
        let tail = &rest[i + 9..];
        let end = tail.find("__").expect("unterminated placeholder");
        let name = format!("__SHUNNO_{}__", &tail[..end]);
        assert!(main.contains(&format!(".replace(\"{}\"", name)), "{} is never replaced in build_content_init_script", name);
        found += 1;
        rest = &tail[end + 2..];
    }
    assert!(found >= 9);
}

#[test]
fn test_update_manifest_signature_and_versions() {
    use updater::{is_newer, verify_manifest, Manifest};
    // Test-only key pair produced by scripts/sign_update.py (not the release key).
    let pk = "62d2b1deff4a99ffb134ccec895afbcb821cb4a6b8b9e8284c51e5ecd40f2c6b";
    let good = || Manifest {
        version: "9.9.9".into(),
        url: "https://github.com/asanaim/ShunnoBrowser/releases/download/latest/ShunnoBrowser.exe".into(),
        sha256: "a56c5875d765a618180353e49e08ae1625b30116c13e422dbc324e58ece4de85".into(),
        signature: "18ee4ca67592f107f1caed94fbe1307abce3a59080968da29026b7ba6b9c940390b17984e3e801a6e56b6081ad5f082c830bcb5945ee071165e3139f84f00a04".into(),
    };
    assert!(verify_manifest(&good(), pk), "signature from the Python CI signer must verify in Rust");
    let mut m = good(); m.version = "9.9.10".into(); assert!(!verify_manifest(&m, pk));
    let mut m = good(); m.sha256 = m.sha256.replace('a', "b"); assert!(!verify_manifest(&m, pk));
    let mut m = good(); m.url = "https://evil.example/ShunnoBrowser.exe".into(); assert!(!verify_manifest(&m, pk));
    let mut m = good(); m.signature = m.signature.replace('1', "2"); assert!(!verify_manifest(&m, pk));
    let mut m = good(); m.signature.clear(); assert!(!verify_manifest(&m, pk));
    assert!(!verify_manifest(&good(), "00".repeat(32).as_str()), "wrong key");
    assert!(!verify_manifest(&good(), ""), "no key configured");

    assert!(is_newer("2.12.1", "v2.12.0"));
    assert!(is_newer("v2.13.0", "v2.12.9"));
    assert!(is_newer("3.0.0", "v2.99.99"));
    assert!(!is_newer("2.12.0", "v2.12.0"));
    assert!(!is_newer("2.9.10", "v2.10.0"), "numeric, not string, comparison");
}

#[test]
fn test_update_install_swaps_and_keeps_old() {
    let dir = std::env::temp_dir().join(format!("shunno_upd_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join("ShunnoBrowser.exe");
    std::fs::write(&exe, b"MZ old").unwrap();
    updater::install(&exe, b"MZ new").unwrap();
    assert_eq!(std::fs::read(&exe).unwrap(), b"MZ new");
    assert_eq!(std::fs::read(dir.join("ShunnoBrowser.old.exe")).unwrap(), b"MZ old");
    assert!(!dir.join("ShunnoBrowser.new.exe").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- browser import

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

#[test]
fn import_chromium_bookmarks_flattens_folders_and_filters_schemes() {
    let json = std::fs::read_to_string(fixture("chromium_bookmarks.json")).unwrap();
    let got = importer::parse_chromium_bookmarks(&json, "Chrome").unwrap();
    let urls: Vec<&str> = got.iter().map(|b| b.url.as_str()).collect();
    assert!(!urls.iter().any(|u| u.starts_with("javascript:")));
    let rust = got.iter().find(|b| b.url == "https://www.rust-lang.org/").unwrap();
    assert_eq!(rust.folder, "Chrome");
    assert_eq!(rust.created_at, 1_600_000_000);
    let docs = got.iter().find(|b| b.url == "https://docs.rs/").unwrap();
    assert_eq!(docs.folder, "Chrome/Dev");
    assert_eq!(docs.title, "https://docs.rs/");
    // Nested folders collapse to the first level.
    assert_eq!(got.iter().find(|b| b.url == "https://crates.io/").unwrap().folder, "Chrome/Dev");
    let mut existing = Vec::new();
    assert_eq!(importer::merge_bookmarks(&mut existing, got.clone()), 3); // duplicate URL skipped
    assert_eq!(importer::merge_bookmarks(&mut existing, got), 0); // idempotent
}

#[test]
fn import_chromium_history_reads_sqlite_btree_with_overflow() {
    let db = std::fs::read(fixture("chromium_history.sqlite")).unwrap();
    let all = importer::parse_chromium_history(&db, usize::MAX).unwrap();
    // 800 rows, 8 hidden, chrome:// and file:// rejected.
    assert_eq!(all.len(), 792);
    assert!(all.windows(2).all(|w| w[0].visited_at >= w[1].visited_at));
    assert_eq!(all[0].url, "https://site799.example.com/page?q=799");
    assert_eq!(all[0].visited_at, 1_700_000_799);
    assert_eq!(all[0].title, "Page 799 \u{2014} টেস্ট");
    let long = all.iter().find(|h| h.url.len() > 3000 && h.url.ends_with(&"x".repeat(10))).unwrap();
    assert!(long.url.len() == "https://long.example.com/".len() + 3333 || long.url.len() == "https://long.example.com/".len() + 3777 || long.url.len() == "https://long.example.com/".len() + 3007);
    assert_eq!(all.iter().filter(|h| h.url.starts_with("https://long.")).count(), 3);
    let top = importer::parse_chromium_history(&db, 250).unwrap();
    assert_eq!(top.len(), 250);
}

#[test]
fn import_history_rejects_garbage_without_panicking() {
    assert!(importer::parse_chromium_history(b"not a db", 10).is_err());
    let mut db = std::fs::read(fixture("chromium_history.sqlite")).unwrap();
    // Corrupt every page after the schema page; must return Ok/Err, never panic.
    for i in (512..db.len()).step_by(37) {
        db[i] = db[i].wrapping_mul(31).wrapping_add(7);
    }
    let _ = importer::parse_chromium_history(&db, 250);
    db.truncate(700);
    let _ = importer::parse_chromium_history(&db, 250);
}

#[test]
fn import_history_merge_dedupes_and_caps() {
    use state::HistoryEntry;
    let h = |u: &str, t: u64| HistoryEntry { title: u.into(), url: u.into(), visited_at: t };
    let mut existing = vec![h("https://a/", 10), h("https://b/", 5)];
    let added = importer::merge_history(&mut existing, vec![h("https://a/", 99), h("https://c/", 7), h("https://d/", 1)], 3);
    assert_eq!(added, 2);
    assert_eq!(existing.iter().map(|x| x.url.as_str()).collect::<Vec<_>>(), ["https://a/", "https://c/", "https://b/"]);
}

#[test]
fn test_auto_archive_selection() {
    use state::{tabs_to_archive, ArchiveProbe};
    let p = |last: u64, pinned: bool, audio: bool, visible: bool, incognito: bool| ArchiveProbe {
        last_active_secs: last, pinned, playing_audio: audio, visible, incognito,
    };
    let now = 100_000;
    let old = now - 13 * 3600;
    let tabs = [
        p(old, false, false, false, false), // 0: archive
        p(old, true, false, false, false),  // 1: pinned
        p(old, false, true, false, false),  // 2: audible
        p(old, false, false, true, false),  // 3: visible
        p(old, false, false, false, true),  // 4: private
        p(now - 3600, false, false, false, false), // 5: recent
        p(old, false, false, false, false), // 6: archive
    ];
    assert_eq!(tabs_to_archive(now, 12, &tabs), vec![6, 0]); // highest index first
    assert!(tabs_to_archive(now, 0, &tabs).is_empty()); // off
    assert!(tabs_to_archive(now, 24, &tabs).is_empty());
    // Never archives every tab.
    assert!(tabs_to_archive(now, 12, &[p(old, false, false, false, false)]).is_empty());
}
