use crate::state::GlobalSettings;
use url::Url;

pub fn resolve_amp_url(parsed: &Url) -> Option<String> {
    let host = parsed.host_str()?.to_ascii_lowercase();
    let path = parsed.path();

    // Google AMP viewer: https://www.google.com/amp/s/example.com/article
    if host.contains("google.") && path.starts_with("/amp/") {
        let rest = path.trim_start_matches("/amp/");
        let stripped = rest
            .strip_prefix("s/")
            .map(|s| format!("https://{}", s))
            .unwrap_or_else(|| format!("http://{}", rest));
        if Url::parse(&stripped).is_ok() {
            return Some(stripped);
        }
    }

    // AMP subdomain: https://amp.example.com/news -> https://example.com/news
    if let Some(root_domain) = host.strip_prefix("amp.") {
        if root_domain.contains('.') {
            let mut canonical = parsed.clone();
            if canonical.set_host(Some(root_domain)).is_ok() {
                return Some(canonical.to_string());
            }
        }
    }

    None
}

pub fn sanitize_url(raw: &str, global: &GlobalSettings) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "https://duckduckgo.com".to_string();
    }
    let normalized = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.contains('.') && !trimmed.contains(' ') {
        format!("https://{}", trimmed)
    } else {
        let encoded: String = trimmed
            .bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
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
