use crate::state::BrowserStorage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserScriptInfo {
    pub file_name: String,
    pub sha256: String,
    pub match_pattern: String,
    pub approved: bool,
}

pub fn user_scripts_dir() -> PathBuf {
    // Stored in %LOCALAPPDATA%\ShunnoBrowser\scripts\ (never inside Downloads!)
    let dir = BrowserStorage::app_data_dir().join("scripts");
    let _ = fs::create_dir_all(&dir);
    let readme = dir.join("README_UserScripts.txt");
    if !readme.exists() {
        let _ = fs::write(
            readme,
            "Place .js user-script files here and approve them in Shunno Browser Controls.\n\
             Optional header: // @match example.com\n\
             Unapproved scripts will never execute automatically.",
        );
    }
    dir
}

/// Host patterns from the script's `// @match` / `// @include` lines, comma-separated
/// ("*" when there are none). Scheme, path and port are dropped: `https://*.example.com/*`
/// becomes `example.com` (the site and its subdomains).
pub fn parse_match_header(code: &str) -> String {
    let mut pats: Vec<String> = Vec::new();
    for line in code.lines() {
        let l = line.trim_start();
        let Some(rest) = l.strip_prefix("// @match").or_else(|| l.strip_prefix("// @include")) else { continue };
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            continue; // e.g. "// @matches"
        }
        let p = normalize_pattern(rest);
        if !p.is_empty() && !pats.contains(&p) {
            pats.push(p);
        }
    }
    if pats.is_empty() || pats.iter().any(|p| p == "*") {
        "*".to_string()
    } else {
        pats.join(",")
    }
}

fn normalize_pattern(raw: &str) -> String {
    let mut p = raw.trim().to_ascii_lowercase();
    if let Some(i) = p.find("://") {
        p = p[i + 3..].to_string();
    }
    if let Some(i) = p.find('/') {
        p.truncate(i);
    }
    if let Some(i) = p.rfind(':') {
        if p[i + 1..].chars().all(|c| c.is_ascii_digit() || c == '*') {
            p.truncate(i);
        }
    }
    let p = p.trim_start_matches("*.").trim_matches('.');
    if p.is_empty() || p == "*" {
        "*".to_string()
    } else {
        p.to_string()
    }
}

/// Does `host` match a pattern list from `parse_match_header`? A pattern matches its exact host
/// and its subdomains, never a look-alike (`example.com` does not match `notexample.com` or
/// `example.com.evil.tld`).
#[allow(dead_code)] // the JS guard in guarded_user_scripts applies the same rule; kept for tests
pub fn host_matches_pattern(host: &str, pattern: &str) -> bool {
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    pattern.split(',').map(normalize_pattern).any(|p| p == "*" || (!h.is_empty() && (h == p || h.ends_with(&format!(".{p}")))))
}

pub fn compute_sha256(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex::encode(hasher.finalize())
}

pub fn list_user_scripts(approved_hashes: &[String]) -> Vec<UserScriptInfo> {
    let dir = user_scripts_dir();
    let mut list = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("js") {
                if let (Some(fname), Ok(bytes)) = (
                    path.file_name().and_then(|n| n.to_str()),
                    fs::read(&path),
                ) {
                    let sha = compute_sha256(&bytes);
                    let code = String::from_utf8_lossy(&bytes);
                    let pat = parse_match_header(&code);
                    let approved = approved_hashes.iter().any(|h| h == &sha);
                    list.push(UserScriptInfo {
                        file_name: fname.to_string(),
                        sha256: sha,
                        match_pattern: pat,
                        approved,
                    });
                }
            }
        }
    }
    list.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    list
}

/// All approved scripts, each guarded by its @match patterns, checked against the page's own host
/// when it loads (the script is installed once per tab, and the tab can go to any site later).
/// Top-level pages only, so a script never runs inside another site's embedded frame.
pub fn load_approved_user_scripts(approved_hashes: &[String]) -> String {
    if approved_hashes.is_empty() {
        return String::new();
    }
    let dir = user_scripts_dir();
    let mut parts: Vec<(String, String)> = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("js") {
                if let Ok(bytes) = fs::read(&path) {
                    let sha = compute_sha256(&bytes);
                    if !approved_hashes.iter().any(|h| h == &sha) {
                        continue;
                    }
                    let code = String::from_utf8_lossy(&bytes).into_owned();
                    let pat = parse_match_header(&code);
                    parts.push((pat, code));
                }
            }
        }
    }
    guarded_user_scripts(&parts)
}

/// JavaScript that runs each (patterns, code) pair only on a matching top-level host.
pub fn guarded_user_scripts(parts: &[(String, String)]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n(function(){\n  if (window.top !== window) return;\n  const h = String(location.hostname || '').toLowerCase().replace(/\\.$/, '');\n  const m = (list) => list.some(p => p === '*' || (h && (h === p || h.endsWith('.' + p))));\n",
    );
    for (pat, code) in parts {
        let list: Vec<String> = pat.split(',').map(normalize_pattern).collect();
        let list_json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".into());
        out.push_str(&format!("  if (m({list_json})) {{ try {{ (function(){{\n{code}\n}})(); }} catch (e) {{ console.error('Shunno UserScript:', e); }} }}\n"));
    }
    out.push_str("})();\n");
    out
}
