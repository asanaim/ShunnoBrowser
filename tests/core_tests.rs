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
#[path = "../src/passwords.rs"]
mod passwords;
#[path = "../src/little.rs"]
mod little;
#[path = "../src/permissions.rs"]
#[allow(dead_code)]
mod permissions;

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
fn userscript_match_is_exact_host_or_subdomain() {
    // No substring matches (audit): a look-alike host never runs the script.
    let pat = parse_match_header("// @match github.com\n");
    assert!(host_matches_pattern("github.com", &pat));
    assert!(host_matches_pattern("gist.github.com", &pat));
    assert!(!host_matches_pattern("github.com.evil.tld", &pat));
    assert!(!host_matches_pattern("notgithub.com", &pat));
    // Standard headers: scheme, wildcard subdomain, path and port are understood.
    let std = parse_match_header("// ==UserScript==\n// @match https://*.example.com/*\n// @match http://news.site.org:8080/a/*\n// @include *://intranet.test/*\n");
    assert_eq!(std, "example.com,news.site.org,intranet.test");
    assert!(host_matches_pattern("www.example.com", &std));
    assert!(host_matches_pattern("news.site.org", &std));
    assert!(!host_matches_pattern("site.org", &std));
    assert_eq!(parse_match_header("// @match *://*/*"), "*");
    assert_eq!(parse_match_header("console.log(1)"), "*");
    assert_eq!(parse_match_header("// @matches nope.com"), "*");
    // Every approved script is guarded by its patterns at page load, top-level only.
    let js = userscripts::guarded_user_scripts(&[("example.com".into(), "window.__a = 1;".into())]);
    assert!(js.contains("window.top !== window") && js.contains("[\"example.com\"]") && js.contains("window.__a = 1;"));
    assert_eq!(userscripts::guarded_user_scripts(&[]), "");
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

// ---------------------------------------------------------------- password vault

#[test]
fn test_password_origins_and_matching() {
    use passwords::{origin_of, same_site};
    assert_eq!(origin_of("https://Accounts.Example.com/login?x=1").as_deref(), Some("https://accounts.example.com"));
    assert_eq!(origin_of("http://127.0.0.1:8080/a").as_deref(), Some("http://127.0.0.1:8080"));
    assert_eq!(origin_of("file:///c:/x"), None);
    assert_eq!(origin_of("javascript:alert(1)"), None);
    assert!(same_site("https://www.github.com", "https://github.com"));
    assert!(!same_site("https://github.com", "http://github.com")); // scheme matters
    assert!(!same_site("https://a.com.bd", "https://b.com.bd")); // no guessing across sites
    assert!(!same_site("https://login.example.com", "https://example.com"));
    assert!(!same_site("https://example.com:8443", "https://example.com"));
}

#[test]
fn test_password_offer_upsert_edit_never() {
    use passwords::{OfferKind, VaultData};
    let mut v = VaultData::default();
    let o = "https://github.com";
    assert_eq!(v.offer_kind(o, "naim", "pw1"), Some(OfferKind::Save));
    let id = v.upsert(o, "naim", "pw1");
    assert_eq!(v.offer_kind("https://www.github.com", "naim", "pw1"), None); // unchanged
    assert_eq!(v.offer_kind(o, "naim", "pw2"), Some(OfferKind::Update));
    assert_eq!(v.upsert(o, "naim", "pw2"), id); // same entry updated
    assert_eq!(v.get(id).unwrap().password, "pw2");
    assert_eq!(v.offer_kind(o, "", ""), None); // empty password never offered
    let id2 = v.upsert(o, "work", "pw3");
    assert_eq!(v.accounts_for("https://www.github.com").len(), 2);
    assert!(v.accounts_for("https://gitlab.com").is_empty());
    assert!(!v.edit(id2, "naim", "x")); // would duplicate (site, username)
    assert!(!v.edit(id2, "work2", "")); // empty password refused
    assert!(v.edit(id2, "work2", "pw4"));
    v.add_never("https://bank.example");
    assert_eq!(v.offer_kind("https://bank.example", "u", "p"), None);
    assert!(v.remove(id));
    assert!(v.get(id).is_none());
    assert_eq!(v.views().len(), 1);
}

#[test]
fn test_password_csv_import_formats() {
    use passwords::{parse_csv, VaultData};
    // RFC 4180: quotes, doubled quotes, comma and newline inside a field, CRLF, BOM.
    let rows = parse_csv("\u{feff}a,b\r\n\"x,1\",\"he said \"\"hi\"\"\nline2\"\r\n");
    assert_eq!(rows, vec![vec!["a".to_string(), "b".to_string()], vec!["x,1".to_string(), "he said \"hi\"\nline2".to_string()]]);

    let chrome = "name,url,username,password,note\n\
github.com,https://github.com/login,naim,\"p,w\"\"1\",\n\
bad,chrome://settings,x,y,\n\
empty,https://e.com/,x,,\n";
    let mut v = VaultData::default();
    assert_eq!(v.import_csv(chrome).unwrap(), (1, 0, 2));
    let c = &v.creds[0];
    assert_eq!((c.origin.as_str(), c.username.as_str(), c.password.as_str()), ("https://github.com", "naim", "p,w\"1"));

    let firefox = "\"url\",\"username\",\"password\",\"httpRealm\",\"formActionOrigin\",\"guid\",\"timeCreated\",\"timeLastUsed\",\"timePasswordChanged\"\n\
\"https://github.com\",\"naim\",\"newpw\",,\"https://github.com\",\"{x}\",\"1\",\"1\",\"1\"\n\
\"https://www.prothomalo.com\",\"নাঈম\",\"পাসওয়ার্ড\",,,\"{y}\",\"1\",\"1\",\"1\"\n";
    assert_eq!(v.import_csv(firefox).unwrap(), (1, 1, 0));
    assert_eq!(v.accounts_for("https://prothomalo.com"), vec![(2, "নাঈম".to_string())]);
    assert_eq!(v.import_csv(firefox).unwrap(), (0, 0, 2)); // idempotent

    let bitwarden = "folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp\n,,login,X,,,0,https://x.org,me,secret,\n";
    assert_eq!(v.import_csv(bitwarden).unwrap(), (1, 0, 0));
    assert!(v.import_csv("just,some,columns\n1,2,3\n").is_err());
    assert!(v.import_csv("").is_err());
}

#[test]
fn test_theme_mode_and_browser_args() {
    use state::{browser_args, effective_dark};
    assert!(effective_dark("system", true));
    assert!(!effective_dark("system", false));
    assert!(effective_dark("dark", false));
    assert!(!effective_dark("light", true));
    assert!(effective_dark("unknown", true)); // falls back to following Windows
    let off = browser_args(false);
    let on = browser_args(true);
    assert!(off.starts_with("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection"));
    assert!(!off.contains("WebContentsForceDark"));
    assert!(on.starts_with(&off) && on.contains("--enable-features=WebContentsForceDark"));
    // Default settings follow the system theme and leave pages undarkened.
    let g = state::GlobalSettings::default();
    assert_eq!(g.theme_mode, "system");
    assert!(!g.force_dark_pages);
}

#[test]
fn test_download_takeover_probe() {
    use std::io::{Read, Write};
    // Tiny HTTP server: one canned response per path.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut s = stream;
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let has_cookie = req.to_ascii_lowercase().contains("cookie: session=abc");
            let resp = match path.as_str() {
                "/ok" => "HTTP/1.1 206 Partial Content\r\nContent-Type: application/zip\r\nContent-Range: bytes 0-0/5000000\r\nConnection: close\r\nContent-Length: 1\r\n\r\nx".to_string(),
                "/noranges" => "HTTP/1.1 200 OK\r\nContent-Type: application/zip\r\nConnection: close\r\nContent-Length: 1\r\n\r\nx".to_string(),
                "/html" => "HTTP/1.1 206 Partial Content\r\nContent-Type: text/html; charset=utf-8\r\nContent-Range: bytes 0-0/5000000\r\nConnection: close\r\nContent-Length: 1\r\n\r\n<".to_string(),
                "/mismatch" => "HTTP/1.1 206 Partial Content\r\nContent-Type: application/zip\r\nContent-Range: bytes 0-0/123\r\nConnection: close\r\nContent-Length: 1\r\n\r\nx".to_string(),
                "/needs-cookie" if has_cookie => "HTTP/1.1 206 Partial Content\r\nContent-Type: application/octet-stream\r\nContent-Range: bytes 0-0/5000000\r\nConnection: close\r\nContent-Length: 1\r\n\r\nx".to_string(),
                _ => "HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n".to_string(),
            };
            let _ = s.write_all(resp.as_bytes());
        }
    });
    let rt = tokio::runtime::Runtime::new().unwrap();
    let e = downloader::NativeDownloadEngine::new(0, false, 3);
    let base = format!("http://127.0.0.1:{port}");
    let probe = |p: &str, cookie: Option<&str>, total: Option<u64>| {
        rt.block_on(e.probe_takeover(&format!("{base}{p}"), None, cookie.map(String::from), total))
    };
    assert_eq!(probe("/ok", None, Some(5_000_000)), Some(5_000_000));
    assert_eq!(probe("/ok", None, None), Some(5_000_000)); // size unknown to the browser
    assert_eq!(probe("/noranges", None, Some(5_000_000)), None); // no resume support: keep WebView2
    assert_eq!(probe("/html", None, Some(5_000_000)), None); // sign-in page, not the file
    assert_eq!(probe("/mismatch", None, Some(5_000_000)), None); // different file
    assert_eq!(probe("/needs-cookie", None, Some(5_000_000)), None);
    assert_eq!(probe("/needs-cookie", Some("session=abc"), Some(5_000_000)), Some(5_000_000));
}

#[test]
fn test_little_command_line_urls() {
    use little::{url_from_arg, url_from_args};
    assert_eq!(url_from_arg("https://example.com/a?b=1").as_deref(), Some("https://example.com/a?b=1"));
    assert_eq!(url_from_arg("\"http://Example.com\"").as_deref(), Some("http://example.com/"));
    assert_eq!(url_from_arg("--flag"), None);
    assert_eq!(url_from_arg("javascript:alert(1)"), None);
    assert_eq!(url_from_arg("C:\\Users\\x\\notes.txt"), None); // only web documents
    assert_eq!(url_from_arg("relative.html"), None);
    assert_eq!(url_from_arg("file:///C:/a.html").as_deref(), Some("file:///C:/a.html"));
    let args = vec!["ShunnoBrowser.exe".to_string(), "--x".to_string(), "https://a.org".to_string()];
    assert_eq!(url_from_args(args).as_deref(), Some("https://a.org/"));
    assert_eq!(url_from_args(vec!["ShunnoBrowser.exe".to_string()]), None);
}

#[test]
fn test_spaces_logic() {
    use state::{pick_tab_for_space, sanitize_space_name, step_in_space, Space, SpacesFile};
    // (tab id, space) in strip order
    let tabs = [(10, 1), (11, 2), (12, 1), (13, 2), (14, 1)];
    assert_eq!(pick_tab_for_space(&tabs, 2, None), Some(1));
    assert_eq!(pick_tab_for_space(&tabs, 2, Some(13)), Some(3)); // remembered tab
    assert_eq!(pick_tab_for_space(&tabs, 2, Some(12)), Some(1)); // remembered tab moved away
    assert_eq!(pick_tab_for_space(&tabs, 3, None), None); // empty Space
    assert_eq!(step_in_space(&tabs, 0, true), 2);
    assert_eq!(step_in_space(&tabs, 4, true), 0); // wraps within Space 1
    assert_eq!(step_in_space(&tabs, 0, false), 4);
    assert_eq!(step_in_space(&tabs, 3, true), 1);
    assert_eq!(step_in_space(&[(1, 1), (2, 2)], 0, true), 0); // alone in its Space
    // Loading: empty or broken files become one valid Space; active always exists.
    let f = SpacesFile { spaces: vec![], active: 9 }.normalized();
    assert_eq!(f.spaces.len(), 1);
    assert_eq!(f.active, f.spaces[0].id);
    let f = SpacesFile {
        spaces: vec![Space { id: 4, name: "  \u{0007}Work ".into(), color: "red".into(), icon: "💼✨🔥".into() }],
        active: 7,
    }
    .normalized();
    assert_eq!((f.spaces[0].name.as_str(), f.spaces[0].color.as_str(), f.active), ("Work", "#0A84FF", 4));
    assert_eq!(f.spaces[0].icon.chars().count(), 2);
    assert_eq!(f.resolve(0), 4); // old sessions without Spaces land in the first Space
    assert_eq!(f.next_id(), 5);
    assert_eq!(sanitize_space_name(""), "Space");
    assert_eq!(sanitize_space_name(&"x".repeat(40)).len(), 24);
}

#[test]
fn test_reader_prefs_validation() {
    use state::valid_reader_prefs;
    assert!(valid_reader_prefs(r#"{"size":21,"font":"sans","theme":"sepia","width":"wide"}"#).is_some());
    assert!(valid_reader_prefs("[1,2]").is_none());
    assert!(valid_reader_prefs("not json").is_none());
    assert!(valid_reader_prefs(&format!(r#"{{"x":"{}"}}"#, "a".repeat(400))).is_none());
    // The reader script must keep its placeholder until the browser fills it.
    let reader = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/js/reader.js")).unwrap();
    assert_eq!(reader.matches("__SHUNNO_READER_PREFS__").count(), 1);
    let rd = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/js/readability.min.js")).unwrap();
    assert!(rd.contains("Apache License") && rd.contains("Readability"));
}

#[test]
fn permissions_resolve_and_store() {
    use permissions::{resolve, Decision, Store};
    // Saved Block always wins; saved Allow is ignored in private tabs; otherwise defaults.
    assert_eq!(resolve(Some(Decision::Block), "camera", true), Some(Decision::Block));
    assert_eq!(resolve(Some(Decision::Allow), "camera", false), Some(Decision::Allow));
    assert_eq!(resolve(Some(Decision::Allow), "camera", true), None);
    assert_eq!(resolve(None, "camera", false), None);
    assert_eq!(resolve(None, "autoplay", false), Some(Decision::Allow));

    let mut st = Store::default();
    st.set("https://meet.example", "camera", Decision::Allow);
    st.set("https://meet.example", "microphone", Decision::Block);
    st.set("https://meet.example", "bogus", Decision::Allow); // unknown kind rejected
    st.set("", "camera", Decision::Allow); // empty origin rejected
    assert_eq!(st.get("https://meet.example", "camera"), Some(Decision::Allow));
    assert_eq!(st.get("https://meet.example", "bogus"), None);
    assert_eq!(st.entries().len(), 2);
    st.reset("https://meet.example", Some("camera"));
    assert_eq!(st.entries().len(), 1);
    st.reset("https://meet.example", None);
    assert!(st.entries().is_empty());

    // Round trip through JSON keeps lowercase decisions.
    st.set("https://a.example", "location", Decision::Block);
    let t = serde_json::to_string(&st).unwrap();
    assert!(t.contains("\"block\""));
    let back: Store = serde_json::from_str(&t).unwrap();
    assert_eq!(back.get("https://a.example", "location"), Some(Decision::Block));
}

#[test]
fn profiles_ids_dirs_and_sanitizing() {
    use state::*;
    assert!(valid_profile_id("work") && valid_profile_id("side-2"));
    for bad in ["", "Work", "../x", "a/b", "-x", "x y", &"a".repeat(33)] {
        assert!(!valid_profile_id(bad), "{bad}");
    }
    let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(profile_from_args(args(&["shunno.exe", "--profile=work", "https://x.org"])), "work");
    assert_eq!(profile_from_args(args(&["shunno.exe", "--profile=..\\evil"])), DEFAULT_PROFILE);
    assert_eq!(profile_from_args(args(&["shunno.exe"])), DEFAULT_PROFILE);

    let shared = std::path::Path::new("/data/ShunnoBrowser");
    assert_eq!(profile_dir_in(shared, DEFAULT_PROFILE), shared);
    assert_eq!(profile_dir_in(shared, "work"), shared.join("Profiles").join("work"));

    let p = |id: &str, name: &str, color: &str| Profile { id: id.into(), name: name.into(), color: color.into() };
    let list = sanitize_profiles(vec![p("work", " Work ", "#FF9F0A"), p("work", "Dup", "#0A84FF"), p("Bad Id", "x", "#0A84FF"), p("school", "", "#30D158"), p("home", "Home", "red")]);
    let ids: Vec<&str> = list.iter().map(|q| q.id.as_str()).collect();
    assert_eq!(ids, ["default", "work", "home"]);
    assert_eq!(list[1].name, "Work");
    assert_eq!(list[2].color, PROFILE_COLORS[0]);

    assert_eq!(new_profile_id("Work Stuff!", &list), "work-stuff");
    assert_eq!(new_profile_id("Work", &list), "work-2");
    assert_eq!(new_profile_id("কাজ", &list), "profile");
    assert_eq!(new_profile_id("Default", &list), "profile");
}

#[test]
fn launch_profile_routing() {
    use state::*;
    let p = |id: &str| Profile { id: id.into(), name: id.into(), color: PROFILE_COLORS[0].into() };
    let known = vec![p("default"), p("work"), p("school")];
    // Explicit --profile wins.
    assert_eq!(choose_launch_profile(Some("school"), true, "work", Some("work"), &known), "school");
    // A link follows the setting: a fixed profile, or the last used one.
    assert_eq!(choose_launch_profile(None, true, "work", Some("school"), &known), "work");
    assert_eq!(choose_launch_profile(None, true, LINKS_LAST, Some("school"), &known), "school");
    // Unknown or deleted targets fall back to the default profile.
    assert_eq!(choose_launch_profile(None, true, "gone", None, &known), DEFAULT_PROFILE);
    assert_eq!(choose_launch_profile(None, true, LINKS_LAST, Some("gone"), &known), DEFAULT_PROFILE);
    assert_eq!(choose_launch_profile(None, true, LINKS_LAST, None, &known), DEFAULT_PROFILE);
    // A plain launch (Start menu, no link) opens the default profile.
    assert_eq!(choose_launch_profile(None, false, "work", Some("school"), &known), DEFAULT_PROFILE);
}

#[test]
fn tab_groups_order_and_drop() {
    use state::*;
    let n = None;
    // Members gather at their group's first tab, everything else keeps its order.
    assert_eq!(group_contiguous_order(&[n, Some(1), n, Some(2), Some(1), Some(2)]), vec![0, 1, 4, 2, 3, 5]);
    assert_eq!(group_contiguous_order(&[n, n, n]), vec![0, 1, 2]);
    assert_eq!(group_contiguous_order(&[]), Vec::<usize>::new());
    // Dropped between two tabs of group 1: joins it.
    assert_eq!(group_after_drop(&[Some(1), n, Some(1)], 1), Some(1));
    // Dropped at the edge next to its own group: stays.
    assert_eq!(group_after_drop(&[Some(1), Some(1), n], 1), Some(1));
    // Dragged away from its group: leaves it.
    assert_eq!(group_after_drop(&[n, Some(1), n, Some(1)], 1), None);
    // Ungrouped tab at the edge of a group stays ungrouped.
    assert_eq!(group_after_drop(&[Some(2), n, n], 1), None);
    // Names are trimmed and capped; unknown colours fall back.
    assert_eq!(clean_group("  Design  ", "#BF5AF2"), ("Design".to_string(), "#BF5AF2".to_string()));
    assert_eq!(clean_group(&"x".repeat(50), "javascript:"), ("x".repeat(30), GROUP_COLORS[0].to_string()));
    // Old sessions without groups still load.
    let t: SavedTab = serde_json::from_str(r#"{"title":"a","url":"https://a.b","space":0}"#).unwrap();
    assert_eq!(t.group, None);
}

#[test]
fn session_split_into_windows() {
    use state::*;
    let t = |u: &str, w: u32| SavedTab { title: u.into(), url: u.into(), space: 0, group: None, window: w };
    let ws = session_windows(vec![t("a", 0), t("b", 2), t("c", 0), t("d", 2)]);
    let urls: Vec<Vec<&str>> = ws.iter().map(|w| w.iter().map(|x| x.url.as_str()).collect()).collect();
    assert_eq!(urls, vec![vec!["a", "c"], vec!["b", "d"]]);
    assert!(session_windows(vec![]).is_empty());
    // Sessions saved before windows existed load as one window; window 0 is not written out.
    let old: SavedTab = serde_json::from_str(r#"{"title":"x","url":"https://x.org","space":1}"#).unwrap();
    assert_eq!(old.window, 0);
    assert!(!serde_json::to_string(&old).unwrap().contains("window"));
}

#[path = "../src/extensions.rs"]
#[allow(dead_code)]
mod extensions;

#[test]
fn extensions_crx_manifest_and_urls() {
    use extensions::*;
    // CRX3: magic, version 3, header length, header, then the zip.
    let mut crx3 = b"Cr24".to_vec();
    crx3.extend(3u32.to_le_bytes());
    crx3.extend(5u32.to_le_bytes());
    crx3.extend([0u8; 5]);
    crx3.extend(b"PK\x03\x04rest");
    assert_eq!(crx_zip_offset(&crx3), Ok(17));
    // CRX2: magic, version 2, key length, signature length, key, signature, zip.
    let mut crx2 = b"Cr24".to_vec();
    crx2.extend(2u32.to_le_bytes());
    crx2.extend(2u32.to_le_bytes());
    crx2.extend(3u32.to_le_bytes());
    crx2.extend([1u8; 5]);
    crx2.extend(b"PK\x03\x04");
    assert_eq!(crx_zip_offset(&crx2), Ok(21));
    assert_eq!(crx_zip_offset(b"PK\x03\x04zip"), Ok(0));
    assert!(crx_zip_offset(b"MZ\x90\x00 not an extension").is_err());
    let mut bad = b"Cr24".to_vec();
    bad.extend(3u32.to_le_bytes());
    bad.extend(u32::MAX.to_le_bytes());
    bad.extend([0u8; 8]);
    assert!(crx_zip_offset(&bad).is_err());

    // Manifest with a localized name, MV3 action popup and options_ui.
    let dir = std::env::temp_dir().join(format!("shunno-ext-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("_locales/en")).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        r#"{"manifest_version":3,"name":"__MSG_appName__","version":"1.2.3","default_locale":"en",
            "action":{"default_popup":"popup.html"},"options_ui":{"page":"opts/index.html"},"icons":{"16":"i16.png","48":"i48.png"}}"#,
    )
    .unwrap();
    std::fs::write(dir.join("_locales/en/messages.json"), r#"{"APPNAME":{"message":"Shunno Test"}}"#).unwrap();
    std::fs::write(dir.join("i48.png"), b"\x89PNGfake").unwrap();
    let m = read_manifest(&dir).unwrap();
    assert_eq!(m.name, "Shunno Test");
    assert_eq!(m.version, "1.2.3");
    assert_eq!(m.popup.as_deref(), Some("popup.html"));
    assert_eq!(m.options.as_deref(), Some("opts/index.html"));
    assert!(m.icon.as_deref().unwrap_or("").starts_with("data:image/png;base64,"));
    assert!(read_manifest(&dir.join("_locales")).is_err());
    let _ = std::fs::remove_dir_all(&dir);

    // Page URLs only for real ids and relative pages.
    let id = "cjpalhdlnbpafiamejdnhcphjbkeiagm";
    assert_eq!(page_url(id, &m, "popup").as_deref(), Some("chrome-extension://cjpalhdlnbpafiamejdnhcphjbkeiagm/popup.html"));
    assert_eq!(page_url("bad/id", &m, "popup"), None);
    let evil = Meta { popup: Some("../../x.html".into()), ..m.clone() };
    assert_eq!(page_url(id, &evil, "popup"), None);
    assert_eq!(page_url(id, &m, "other"), None);

    assert_eq!(base64(b"Man"), "TWFu");
    assert_eq!(base64(b"Ma"), "TWE=");
    assert_eq!(base64(b"M"), "TQ==");
    assert_eq!(slug("uBlock Origin!"), "ublock-origin");
    assert_eq!(slug("مرحبا"), "extension");
}

#[test]
fn audit_local_files_and_odd_input() {
    let cfg = GlobalSettings::default();
    // Local files open as typed (Explorer hands these over), not as a web address or search.
    assert_eq!(sanitize_url("file:///C:/Users/me/page.html", &cfg), "file:///C:/Users/me/page.html");
    assert_eq!(sanitize_url("FILE:///D:/a%20b.pdf", &cfg), "FILE:///D:/a%20b.pdf");
    // Multi-byte text near the start must not panic.
    let _ = sanitize_url("ফাইল:///খ.html", &cfg);
    let _ = sanitize_url("é", &cfg);
    let _ = sanitize_url("日本語のテキスト", &cfg);
}

#[test]
fn history_import_reads_the_write_ahead_log() {
    // Chrome/Edge open: recent visits are only in History-wal (fixture made with SQLite in WAL mode,
    // 5 checkpointed rows, 300 committed rows in the WAL).
    let db = std::fs::read(fixture("wal_history.sqlite")).unwrap();
    let wal = std::fs::read(fixture("wal_history.sqlite-wal")).unwrap();
    let main_only = importer::parse_chromium_history(&db, usize::MAX).unwrap();
    assert_eq!(main_only.len(), 5);
    let merged = importer::apply_wal(&db, &wal).expect("usable WAL");
    let all = importer::parse_chromium_history(&merged, usize::MAX).unwrap();
    assert_eq!(all.len(), 305);
    assert_eq!(all[0].url, "https://new299.example.com/p");
    assert!(all.iter().all(|h| !h.url.contains("uncommitted")));
    // Garbage or truncated logs are ignored, never a panic.
    assert!(importer::apply_wal(&db, b"not a wal").is_none());
    assert!(importer::apply_wal(&db, &wal[..100]).is_none());
    let mut bad = wal.clone();
    for i in (32..bad.len()).step_by(41) {
        bad[i] ^= 0x5a;
    }
    if let Some(m) = importer::apply_wal(&db, &bad) {
        let _ = importer::parse_chromium_history(&m, 10);
    }
}

#[test]
fn amp_rewrite_only_for_google_and_real_sites() {
    let cfg = GlobalSettings::default();
    assert_eq!(sanitize_url("https://www.google.com/amp/s/example.com/story?id=7&utm_source=x", &cfg), "https://example.com/story?id=7");
    assert_eq!(sanitize_url("https://www.google.co.uk/amp/s/news.example.org/a", &cfg), "https://news.example.org/a");
    // Look-alike hosts are left alone.
    assert_eq!(sanitize_url("https://mygoogle.evil.com/amp/s/bank.example/x", &cfg), "https://mygoogle.evil.com/amp/s/bank.example/x");
    assert_eq!(sanitize_url("https://google.evil.com/amp/s/bank.example/x", &cfg), "https://google.evil.com/amp/s/bank.example/x");
    // amp. subdomains: real sites only, never down to a public suffix.
    assert_eq!(sanitize_url("https://amp.example.com/news", &cfg), "https://example.com/news");
    assert_eq!(sanitize_url("https://amp.co.uk/page", &cfg), "https://amp.co.uk/page");
    assert_eq!(sanitize_url("https://amp.com.bd/page", &cfg), "https://amp.com.bd/page");
}

#[test]
fn public_address_check_covers_embedded_and_reserved_ranges() {
    use sanitizer::{is_public_http_url, is_public_ip};
    let ip = |s: &str| s.parse::<std::net::IpAddr>().unwrap();
    for bad in ["127.0.0.1", "10.1.2.3", "192.168.0.1", "172.16.0.1", "169.254.169.254", "100.64.0.1", "0.1.2.3", "198.18.0.1", "240.0.0.1", "255.255.255.255",
                "::1", "::", "fc00::1", "fe80::1", "fec0::1", "::ffff:127.0.0.1", "::ffff:10.0.0.1", "::127.0.0.1", "64:ff9b::a00:1", "2002:7f00:1::", "2001:db8::1"] {
        assert!(!is_public_ip(ip(bad)), "{bad} must not count as public");
    }
    for good in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111", "::ffff:8.8.8.8", "64:ff9b::808:808"] {
        assert!(is_public_ip(ip(good)), "{good} is public");
    }
    assert!(!is_public_http_url("http://[::ffff:7f00:1]/"));
    assert!(!is_public_http_url("http://router.lan/"));
    assert!(is_public_http_url("https://example.com/"));
}
