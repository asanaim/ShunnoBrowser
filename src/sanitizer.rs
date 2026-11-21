use crate::state::GlobalSettings;
use url::Url;

/// `google.com`, `www.google.co.uk`, ... (not `mygoogle.evil.com` or `google.evil.com`).
fn is_google_host(host: &str) -> bool {
    let h = host.strip_prefix("www.").unwrap_or(host);
    let Some(rest) = h.strip_prefix("google.") else { return false };
    let labels: Vec<&str> = rest.split('.').collect();
    match labels.as_slice() {
        [tld] => (2..=3).contains(&tld.len()) && tld.chars().all(|c| c.is_ascii_lowercase()),
        [sld, cc] => matches!(*sld, "co" | "com") && cc.len() == 2 && cc.chars().all(|c| c.is_ascii_lowercase()),
        _ => false,
    }
}

/// A name that is itself a public suffix such as `co.uk` or `com.bd` (never a real site).
fn looks_like_public_suffix(domain: &str) -> bool {
    let labels: Vec<&str> = domain.split('.').collect();
    match labels.as_slice() {
        [_] => true,
        [sld, cc] => cc.len() == 2 && matches!(*sld, "co" | "com" | "net" | "org" | "gov" | "edu" | "ac" | "or" | "ne" | "go" | "gob" | "nic" | "mil"),
        _ => false,
    }
}

pub fn resolve_amp_url(parsed: &Url) -> Option<String> {
    let host = parsed.host_str()?.to_ascii_lowercase();
    let path = parsed.path();

    // Google AMP viewer: https://www.google.com/amp/s/example.com/article?id=1
    if is_google_host(&host) && path.starts_with("/amp/") {
        let rest = path.trim_start_matches("/amp/");
        let mut stripped = rest
            .strip_prefix("s/")
            .map(|s| format!("https://{}", s))
            .unwrap_or_else(|| format!("http://{}", rest));
        // The article's own query string travels on the viewer URL.
        if let Some(q) = parsed.query().filter(|q| !q.is_empty()) {
            stripped.push('?');
            stripped.push_str(q);
        }
        if Url::parse(&stripped).ok().and_then(|u| u.host_str().map(|h| h.contains('.'))).unwrap_or(false) {
            return Some(stripped);
        }
    }

    // AMP subdomain: https://amp.example.com/news -> https://example.com/news
    if let Some(root_domain) = host.strip_prefix("amp.") {
        if root_domain.contains('.') && !looks_like_public_suffix(root_domain) {
            let mut canonical = parsed.clone();
            if canonical.set_host(Some(root_domain)).is_ok() {
                return Some(canonical.to_string());
            }
        }
    }

    None
}

/// Search URL for `query` with the active search engine (always a search, even if `query` looks like a domain).
pub fn search_url(query: &str, global: &GlobalSettings) -> String {
    let encoded: String = query
        .trim()
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect();
    let template = global
        .search_engines
        .get(global.active_search_engine)
        .map(|e| e.query_url.as_str())
        .unwrap_or("https://duckduckgo.com/?q=%s");
    if template.contains("%s") {
        template.replace("%s", &encoded)
    } else {
        format!("{}{}", template, encoded)
    }
}

/// Selected page text as a search query: whitespace collapsed, at most 300 characters.
pub fn selection_query(sel: &str) -> String {
    sel.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(300).collect()
}

pub fn sanitize_url(raw: &str, global: &GlobalSettings) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("shunno://newtab") || trimmed.eq_ignore_ascii_case("about:newtab") {
        return "shunno://newtab".to_string();
    }
    // A local file (typed, or handed over by Explorer) is opened as is, never turned into a search.
    if trimmed.len() > 8 && trimmed.get(..8).is_some_and(|p| p.eq_ignore_ascii_case("file:///")) {
        return trimmed.to_string();
    }
    let normalized = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.contains('.') && !trimmed.contains(' ') {
        format!("https://{}", trimmed)
    } else {
        search_url(trimmed, global)
    };

    if !global.strip_tracking_params {
        return normalized;
    }

    let Ok(mut parsed) = Url::parse(&normalized) else {
        return normalized;
    };

    if let Some(amp_resolved) = resolve_amp_url(&parsed) {
        if let Ok(new_parsed) = Url::parse(&amp_resolved) {
            parsed = new_parsed;
        }
    }

    let tracking_params = [
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "utm_term",
        "utm_content",
        "fbclid",
        "gclid",
        "si",
        "igshid",
        "mc_cid",
        "mc_eid",
        "ref_src",
        "ref_url",
    ];

    let filtered: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(k, _)| !tracking_params.iter().any(|tp| k.eq_ignore_ascii_case(tp)))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if filtered.len() != parsed.query_pairs().count() {
        if filtered.is_empty() {
            parsed.set_query(None);
        } else {
            parsed.query_pairs_mut().clear().extend_pairs(filtered);
        }
    }

    parsed.to_string()
}

pub fn extract_host(url_str: &str) -> String {
    Url::parse(url_str)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
        .unwrap_or_default()
}

/// True only for http(s) URLs whose host is a public name or public IP literal.
/// Used to stop web pages from making the native download engine reach
/// loopback, LAN, link-local or cloud-metadata targets (the engine bypasses CORS,
/// and probe results are reported back to the page).
pub fn is_public_http_url(url_str: &str) -> bool {
    let Ok(parsed) = Url::parse(url_str) else {
        return false;
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return false;
    }
    match parsed.host() {
        Some(url::Host::Domain(d)) => is_public_domain(d),
        Some(url::Host::Ipv4(ip)) => is_public_ip(std::net::IpAddr::V4(ip)),
        Some(url::Host::Ipv6(ip)) => is_public_ip(std::net::IpAddr::V6(ip)),
        None => false,
    }
}

/// A host name that should reach the public internet (not a local-only name).
pub fn is_public_domain(d: &str) -> bool {
    let d = d.trim_end_matches('.').to_ascii_lowercase();
    !(d.is_empty()
        || d == "localhost"
        || d.ends_with(".localhost")
        || d.ends_with(".local")
        || d.ends_with(".internal")
        || d.ends_with(".lan")
        || d.ends_with(".home.arpa")
        || !d.contains('.'))
}

/// An address on the public internet: not loopback, private, link-local, carrier-grade NAT,
/// reserved, multicast or documentation space, including IPv4 addresses embedded in IPv6
/// (mapped, compatible, NAT64, 6to4, Teredo).
pub fn is_public_ip(ip: std::net::IpAddr) -> bool {
    use std::net::{IpAddr, Ipv4Addr};
    fn v4_public(ip: Ipv4Addr) -> bool {
        let o = ip.octets();
        !(ip.is_private()
            || ip.is_loopback()
            || ip.is_link_local()
            || ip.is_unspecified()
            || ip.is_broadcast()
            || ip.is_multicast()
            || ip.is_documentation()
            || o[0] == 0 // 0.0.0.0/8
            || (o[0] == 100 && (o[1] & 0xC0) == 64) // CGNAT 100.64.0.0/10
            || (o[0] == 192 && o[1] == 0 && o[2] == 0) // IETF 192.0.0.0/24
            || (o[0] == 198 && (o[1] & 0xFE) == 18) // benchmarking 198.18.0.0/15
            || o[0] >= 240) // reserved 240.0.0.0/4
    }
    match ip {
        IpAddr::V4(v4) => v4_public(v4),
        IpAddr::V6(v6) => {
            let s = v6.segments();
            let embedded = |a: u16, b: u16| Ipv4Addr::new((a >> 8) as u8, a as u8, (b >> 8) as u8, b as u8);
            if let Some(v4) = v6.to_ipv4_mapped() {
                return v4_public(v4); // ::ffff:a.b.c.d
            }
            if s[..6] == [0, 0, 0, 0, 0, 0] {
                // ::a.b.c.d (IPv4-compatible), and :: / ::1
                return !v6.is_unspecified() && !v6.is_loopback() && v4_public(embedded(s[6], s[7]));
            }
            if s[0] == 0x0064 && s[1] == 0xff9b {
                return v4_public(embedded(s[6], s[7])); // NAT64 64:ff9b::/96
            }
            if s[0] == 0x2002 {
                return v4_public(embedded(s[1], s[2])); // 6to4 2002::/16
            }
            if s[0] == 0x2001 && s[1] == 0 {
                return v4_public(embedded(!s[6], !s[7])); // Teredo 2001::/32 (client address inverted)
            }
            !(v6.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
                || (s[0] & 0xffc0) == 0xfe80 // link local fe80::/10
                || (s[0] & 0xffc0) == 0xfec0 // site local fec0::/10 (deprecated)
                || (s[0] == 0x2001 && s[1] == 0x0db8) // documentation 2001:db8::/32
                || (s[0] == 0x0100 && s[1..4] == [0, 0, 0])) // discard 100::/64
        }
    }
}

/// Hosts and paths blocked by the built-in ad filter (replaced by full filter lists later).
pub fn is_blocked_ad_url(url_str: &str) -> bool {
    let Ok(u) = Url::parse(url_str) else {
        return false;
    };
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    let on = |d: &str| host == d || host.ends_with(&format!(".{}", d));
    const AD_HOSTS: [&str; 11] = [
        "doubleclick.net",
        "googlesyndication.com",
        "googleadservices.com",
        "adnxs.com",
        "adsrvr.org",
        "taboola.com",
        "outbrain.com",
        "popads.net",
        "popcash.net",
        "propellerads.com",
        "exoclick.com",
    ];
    if AD_HOSTS.iter().any(|d| on(d)) {
        return true;
    }
    let path = u.path();
    if on("youtube.com")
        && (path.starts_with("/pagead/")
            || path.starts_with("/api/stats/ads")
            || path.starts_with("/ptracking")
            || path.starts_with("/get_midroll_"))
    {
        return true;
    }
    if on("googlevideo.com") {
        if let Some(i) = url_str.find("/initplayback") {
            return url_str[i..].contains("oad");
        }
    }
    false
}
