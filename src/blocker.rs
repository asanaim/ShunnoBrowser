//! Ad and tracker blocking with Brave's `adblock` engine (MPL-2.0, used unmodified).
//!
//! - Filter lists are chosen from a fixed catalog and downloaded at runtime, so the EXE ships no
//!   list data. EasyList and EasyPrivacy are on by default.
//! - Each list's text is kept on disk, so turning a list off or editing "My filters" recompiles
//!   without a download. The compiled engine is cached with a key of what it was built from.
//! - "My filters" (filters/user.txt) holds the user's own rules, including those made with the
//!   element picker, in standard Adblock Plus / uBlock syntax.
//! - Until lists are available, the small built-in matcher (`sanitizer::is_blocked_ad_url`) is used.

use crate::sanitizer::is_blocked_ad_url;
use crate::state::BrowserStorage;
use adblock::lists::{FilterSet, ParseOptions};
use adblock::request::Request;
use adblock::Engine;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct ListInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub url: &'static str,
    pub default_on: bool,
}

/// The lists a user can choose from. Only well known, maintained lists; no free URL entry, so a
/// list cannot be pointed at an arbitrary server. Lists written for uBlock may use syntax the
/// engine does not support; those lines are skipped.
pub const CATALOG: &[ListInfo] = &[
    ListInfo { id: "easylist", name: "EasyList", group: "Ads", url: "https://easylist.to/easylist/easylist.txt", default_on: true },
    ListInfo { id: "ubo-filters", name: "uBlock filters", group: "Ads", url: "https://ublockorigin.github.io/uAssets/filters/filters.txt", default_on: false },
    ListInfo { id: "peter-lowe", name: "Peter Lowe\u{2019}s Ad and tracking server list", group: "Ads", url: "https://pgl.yoyo.org/adservers/serverlist.php?hostformat=adblockplus&showintro=1&mimetype=plaintext", default_on: false },
    ListInfo { id: "easyprivacy", name: "EasyPrivacy", group: "Privacy", url: "https://easylist.to/easylist/easyprivacy.txt", default_on: true },
    ListInfo { id: "ubo-privacy", name: "uBlock filters: Privacy", group: "Privacy", url: "https://ublockorigin.github.io/uAssets/filters/privacy.txt", default_on: false },
    ListInfo { id: "ubo-badware", name: "uBlock filters: Badware risks", group: "Security", url: "https://ublockorigin.github.io/uAssets/filters/badware.txt", default_on: false },
    ListInfo { id: "cookies", name: "EasyList Cookie List", group: "Annoyances", url: "https://secure.fanboy.co.nz/fanboy-cookiemonster.txt", default_on: false },
    ListInfo { id: "social", name: "Fanboy\u{2019}s Social Blocking List", group: "Annoyances", url: "https://easylist.to/easylist/fanboy-social.txt", default_on: false },
    ListInfo { id: "annoyance", name: "Fanboy\u{2019}s Annoyance List", group: "Annoyances", url: "https://easylist.to/easylist/fanboy-annoyance.txt", default_on: false },
    ListInfo { id: "indianlist", name: "IndianList (Bengali, Hindi and more)", group: "Regional", url: "https://easylist-downloads.adblockplus.org/indianlist.txt", default_on: false },
    ListInfo { id: "ubo-unbreak", name: "uBlock filters: Unbreak", group: "Fixes", url: "https://ublockorigin.github.io/uAssets/filters/unbreak.txt", default_on: false },
];

pub const INTERVALS: [u32; 4] = [1, 4, 7, 14];
const MAX_COSMETIC_SELECTORS: usize = 4000;
const MAX_LIST_BYTES: usize = 30 * 1024 * 1024;
pub const MAX_USER_BYTES: usize = 1024 * 1024;
// Serialized engines are only readable by the same crate version.
const CACHE_FILE: &str = "engine-adblock-0.13.dat";
const KEY_FILE: &str = "engine.key";
const SETTINGS_FILE: &str = "settings.json";
const USER_FILE: &str = "user.txt";
/// Cache file = magic + SHA-256 of the engine bytes + engine bytes. The checksum is verified before
/// the engine library parses the bytes, so a damaged file (disk error, crash while writing) is
/// discarded instead of being handed to the deserializer at every start.
const CACHE_MAGIC: &[u8; 8] = b"SHUNADB1";

fn wrap_cache(bytes: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut out = Vec::with_capacity(bytes.len() + 40);
    out.extend_from_slice(CACHE_MAGIC);
    out.extend_from_slice(&Sha256::digest(bytes));
    out.extend_from_slice(bytes);
    out
}

/// The engine bytes from a cache file, if it is intact.
pub fn unwrap_cache(file: &[u8]) -> Option<&[u8]> {
    use sha2::{Digest, Sha256};
    let rest = file.strip_prefix(CACHE_MAGIC.as_slice())?;
    let (sum, body) = (rest.get(..32)?, rest.get(32..)?);
    (Sha256::digest(body).as_slice() == sum && !body.is_empty()).then_some(body)
}

fn default_enabled() -> Vec<String> {
    CATALOG.iter().filter(|l| l.default_on).map(|l| l.id.to_string()).collect()
}
fn default_true() -> bool {
    true
}
fn default_interval() -> u32 {
    7
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct FilterSettings {
    #[serde(default = "default_enabled")]
    pub enabled: Vec<String>,
    #[serde(default = "default_true")]
    pub auto_update: bool,
    #[serde(default = "default_interval")]
    pub interval_days: u32,
    /// Unix seconds of the last successful download; 0 = never (the cache file's age is used).
    #[serde(default)]
    pub last_update: u64,
}

impl Default for FilterSettings {
    fn default() -> Self {
        FilterSettings { enabled: default_enabled(), auto_update: true, interval_days: 7, last_update: 0 }
    }
}

impl FilterSettings {
    /// Unknown ids dropped, catalog order, interval one of `INTERVALS`.
    pub fn normalized(mut self) -> Self {
        self.enabled = CATALOG.iter().filter(|l| self.enabled.iter().any(|e| e == l.id)).map(|l| l.id.to_string()).collect();
        if !INTERVALS.contains(&self.interval_days) {
            self.interval_days = 7;
        }
        self
    }
    pub fn is_on(&self, id: &str) -> bool {
        self.enabled.iter().any(|e| e == id)
    }
}

#[derive(Serialize, Clone)]
pub struct ListView {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub on: bool,
    /// True when the list's text is on disk.
    pub have: bool,
}

#[derive(Serialize, Clone)]
pub struct FiltersView {
    pub lists: Vec<ListView>,
    pub auto_update: bool,
    pub interval_days: u32,
    pub user_rules: usize,
    pub status: String,
    pub notice: String,
    pub busy: bool,
}

#[derive(Default)]
pub struct Blocker {
    engine: RwLock<Option<Arc<Engine>>>,
    status: Mutex<String>,
    notice: Mutex<String>,
    busy: std::sync::atomic::AtomicBool,
    /// One compile or download at a time; later requests wait and then see the newest settings.
    work: Mutex<()>,
    /// One list download round at a time (an automatic update and "Update Now" can overlap).
    refresh_gate: tokio::sync::Mutex<()>,
    /// Refreshes running or waiting (the panel shows "Updating" until the last has finished).
    refreshing: std::sync::atomic::AtomicUsize,
    /// Recent verdicts by exact (url, page, type): a page asks about the same CDN files and
    /// trackers again and again. Cleared whenever the engine is replaced.
    verdicts: Mutex<HashMap<String, bool>>,
    /// Counts engine replacements: a verdict from the engine before a swap is not stored after it.
    engine_gen: std::sync::atomic::AtomicU64,
}

/// Entries kept before the verdict cache starts over.
const VERDICT_CACHE_MAX: usize = 4096;

/// Every read, change and write of settings.json goes through here, so two changes at once cannot
/// each start from the same old file and lose the other's.
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());
fn update_settings(f: impl FnOnce(&mut FilterSettings)) {
    let _g = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load_settings();
    f(&mut s);
    save_settings(&s.normalized());
}

pub fn global() -> &'static Blocker {
    static B: OnceLock<Blocker> = OnceLock::new();
    B.get_or_init(Blocker::default)
}

fn filters_dir() -> PathBuf {
    let dir = BrowserStorage::shared_data_dir().join("filters");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn cache_path() -> PathBuf {
    filters_dir().join(CACHE_FILE)
}

fn list_path(id: &str) -> PathBuf {
    filters_dir().join(format!("list-{}.txt", id))
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn load_settings() -> FilterSettings {
    fs::read_to_string(filters_dir().join(SETTINGS_FILE))
        .ok()
        .and_then(|t| serde_json::from_str::<FilterSettings>(&t).ok())
        .unwrap_or_default()
        .normalized()
}

fn save_settings(s: &FilterSettings) {
    if let Ok(json) = serde_json::to_string_pretty(s) {
        let _ = crate::state::write_atomic(&filters_dir().join(SETTINGS_FILE), json.as_bytes());
    }
}

pub fn user_rules_text() -> String {
    fs::read_to_string(filters_dir().join(USER_FILE)).unwrap_or_default()
}

/// Rule lines (not blank, not comments) in a filter text.
pub fn rule_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('!') && !l.starts_with('['))
}

/// Lines of "My filters" the engine cannot use, as (line number, text).
pub fn invalid_rules(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('!') && !t.starts_with('[')
        })
        .filter(|(_, l)| adblock::lists::parse_filter(l.trim(), false, ParseOptions::default()).is_err())
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .collect()
}

/// Normalizes "My filters": LF line ends, no trailing spaces, size capped.
pub fn clean_user_text(text: &str) -> String {
    let mut out = String::new();
    for line in text.replace("\r\n", "\n").replace('\r', "\n").lines() {
        let l = line.trim_end();
        if out.len() + l.len() + 1 > MAX_USER_BYTES {
            break;
        }
        out.push_str(l);
        out.push('\n');
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    if out.trim().is_empty() {
        String::new()
    } else {
        out
    }
}

/// A filter list download that is really a list, not an error page or captive portal.
pub fn looks_like_list(text: &str) -> bool {
    let head: String = text.chars().take(2048).collect::<String>().to_ascii_lowercase();
    if head.trim_start().starts_with('<') || head.contains("<html") || head.contains("<!doctype") {
        return false;
    }
    rule_lines(text).take(20).count() >= 20
}

/// A hiding rule from the element picker: `host##selector`, refused if either part could escape
/// a CSS rule or name another kind of filter.
pub fn picker_rule(host: &str, selector: &str) -> Option<String> {
    let host = host.trim().trim_start_matches("www.").to_ascii_lowercase();
    let sel = selector.trim();
    let host_ok = !host.is_empty()
        && host.len() <= 253
        && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && host.contains('.');
    let sel_ok = !sel.is_empty()
        && sel.len() <= 600
        && !sel.contains(['{', '}', '<', '\n', '\r'])
        && !sel.contains("##")
        && !sel.contains("#@#")
        && !sel.contains("#?#")
        && !sel.contains("#$#")
        && !sel.contains("+js(")
        && !sel.starts_with('+');
    if !host_ok || !sel_ok {
        return None;
    }
    let rule = format!("{}##{}", host, sel);
    adblock::lists::parse_filter(&rule, false, ParseOptions::default()).ok()?;
    Some(rule)
}

/// What the cached engine was built from: enabled lists (with their file size and time) and the
/// user's rules. A different key means the cache is out of date.
fn build_key(settings: &FilterSettings, user: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for id in &settings.enabled {
        h.update(id.as_bytes());
        let meta = fs::metadata(list_path(id)).ok();
        let len = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let mt = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        h.update(format!("|{}|{};", len, mt).as_bytes());
    }
    h.update(b"#user#");
    h.update(user.as_bytes());
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Compiles filter list texts into an engine. Pure; used by refresh and by tests.
pub fn build_engine(list_texts: &[String]) -> (Engine, usize) {
    let mut set = FilterSet::new(false);
    let mut rules = 0usize;
    for text in list_texts {
        rules += text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('!')).count();
        set.add_filter_list(text.clone(), ParseOptions::default());
    }
    (Engine::new_with_filter_set(set), rules)
}

/// Builds the CSS that hides ad elements. Each selector becomes its own rule so one invalid
/// selector cannot disable the rest.
pub fn cosmetic_css_for(engine: &Engine, page_url: &str, classes: &[String], ids: &[String]) -> String {
    let res = engine.url_cosmetic_resources(page_url);
    let mut selectors: Vec<String> = res.hide_selectors.into_iter().collect();
    if !res.generichide && (!classes.is_empty() || !ids.is_empty()) {
        selectors.extend(engine.hidden_class_id_selectors(
            classes.iter().cloned(),
            ids.iter().cloned(),
            &res.exceptions,
        ));
    }
    let mut seen = HashSet::new();
    let mut css = String::new();
    for sel in selectors.into_iter().filter(|s| seen.insert(s.clone())).take(MAX_COSMETIC_SELECTORS) {
        // Filter lists are third-party input: refuse anything that could escape a rule body.
        if sel.contains(['{', '}', '<']) || sel.trim().is_empty() {
            continue;
        }
        css.push_str(&sel);
        css.push_str("{display:none !important}\n");
    }
    css
}

impl Blocker {
    pub fn set_status(&self, s: String) {
        if let Ok(mut st) = self.status.lock() {
            *st = s;
        }
    }

    pub fn status(&self) -> String {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// A one line result for the Content Blocker panel (import, export, picker).
    pub fn set_notice(&self, s: String) {
        if let Ok(mut n) = self.notice.lock() {
            *n = s;
        }
    }

    /// Shown as "Updating…" in the panel from the moment a change is requested.
    pub fn mark_busy(&self) {
        self.busy.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn has_lists(&self) -> bool {
        self.engine.read().map(|e| e.is_some()).unwrap_or(false)
    }

    pub fn view(&self) -> FiltersView {
        let st = load_settings();
        FiltersView {
            lists: CATALOG
                .iter()
                .map(|l| ListView { id: l.id, name: l.name, group: l.group, on: st.is_on(l.id), have: list_path(l.id).exists() })
                .collect(),
            auto_update: st.auto_update,
            interval_days: st.interval_days,
            user_rules: rule_lines(&user_rules_text()).count(),
            status: self.status(),
            notice: self.notice.lock().map(|n| n.clone()).unwrap_or_default(),
            busy: self.busy.load(std::sync::atomic::Ordering::Relaxed),
        }
    }

    fn summary(&self, settings: &FilterSettings, rules: Option<usize>) -> String {
        let n = settings.enabled.len();
        let lists = match n {
            0 => "No lists".to_string(),
            1 => CATALOG.iter().find(|l| settings.is_on(l.id)).map(|l| l.name.to_string()).unwrap_or_default(),
            2 => settings
                .enabled
                .iter()
                .filter_map(|id| CATALOG.iter().find(|l| l.id == id).map(|l| l.name))
                .collect::<Vec<_>>()
                .join(" + "),
            n => format!("{} lists", n),
        };
        let user = rule_lines(&user_rules_text()).count();
        let mut out = lists;
        if user > 0 {
            out.push_str(&format!(", {} of my filters", user));
        }
        let mut detail = Vec::new();
        if let Some(r) = rules {
            detail.push(format!("{} rules", r));
        }
        detail.push(updated_label(settings));
        format!("{} ({})", out, detail.join(", "))
    }

    /// Loads the cached engine from disk (fast, no network). Call off the UI thread. A cache built
    /// from other lists or other user rules is recompiled from the lists on disk when they are all
    /// there; otherwise the next refresh downloads what is missing.
    pub fn load_cache(&self) {
        let settings = load_settings();
        let user = user_rules_text();
        let key_now = build_key(&settings, &user);
        let key_file = fs::read_to_string(filters_dir().join(KEY_FILE)).ok();
        // Before v2.31 there was no key: that cache holds EasyList + EasyPrivacy and no user rules.
        let legacy = key_file.is_none() && settings.enabled == default_enabled() && rule_lines(&user).next().is_none();
        let key_ok = key_file.as_deref().map(str::trim) == Some(key_now.as_str()) || legacy;
        let can_rebuild = self.missing_lists(&settings).is_empty();
        if !key_ok && can_rebuild {
            if let Err(e) = self.rebuild() {
                self.set_status(format!("Built-in list in use: {}", e));
            }
            return;
        }
        // Up to date, or out of date with a list still to download: the cached engine is used until
        // the download (is_stale is true then) recompiles.
        if let Ok(bytes) = fs::read(cache_path()) {
            let mut engine = Engine::default();
            if unwrap_cache(&bytes).is_some_and(|b| engine.deserialize(b).is_ok()) {
                if let Ok(mut slot) = self.engine.write() {
                    *slot = Some(Arc::new(engine));
                }
                self.clear_verdicts();
                self.set_status(self.summary(&settings, None));
                return;
            }
            let _ = fs::remove_file(cache_path());
        }
        self.set_status("Built-in list (downloading filter lists)".to_string());
    }

    fn missing_lists(&self, settings: &FilterSettings) -> Vec<&'static ListInfo> {
        CATALOG.iter().filter(|l| settings.is_on(l.id) && !list_path(l.id).exists()).collect()
    }

    /// True when a download is due: an enabled list is not on disk, nothing is loaded yet, or
    /// automatic updates are on and the interval has passed.
    pub fn is_stale(&self) -> bool {
        let settings = load_settings();
        if !self.missing_lists(&settings).is_empty() && !legacy_cache_covers(&settings) {
            return true;
        }
        if !self.has_lists() && !settings.enabled.is_empty() {
            return true;
        }
        if !settings.auto_update {
            return false;
        }
        last_update_age(&settings).map(|age| age > Duration::from_secs(settings.interval_days as u64 * 86_400)).unwrap_or(true)
    }

    /// Compiles the enabled lists on disk plus "My filters", caches the engine and swaps it in.
    /// Blocking; call off the UI thread.
    pub fn rebuild(&self) -> Result<usize, String> {
        let _guard = self.work.lock().unwrap_or_else(|e| e.into_inner());
        self.rebuild_locked()
    }

    fn rebuild_locked(&self) -> Result<usize, String> {
        let settings = load_settings();
        let user = user_rules_text();
        // A list chosen but not downloaded yet (offline): keep the engine in use rather than replace
        // it with one that lacks that list. The next successful download recompiles.
        let missing = self.missing_lists(&settings);
        if !missing.is_empty() && self.has_lists() {
            let names: Vec<&str> = missing.iter().map(|l| l.name).collect();
            self.set_status(format!("Waiting to download {}; current rules stay in use", names.join(", ")));
            return Err(format!("not downloaded yet: {}", names.join(", ")));
        }
        let mut texts = Vec::new();
        for id in &settings.enabled {
            if let Ok(t) = fs::read_to_string(list_path(id)) {
                texts.push(t);
            }
        }
        if rule_lines(&user).next().is_some() {
            texts.push(user.clone());
        }
        if texts.is_empty() {
            // Nothing chosen: block nothing beyond the small built-in matcher.
            if let Ok(mut slot) = self.engine.write() {
                *slot = None;
            }
            self.clear_verdicts();
            let _ = fs::remove_file(cache_path());
            let _ = fs::remove_file(filters_dir().join(KEY_FILE));
            self.set_status("No filter lists chosen (built-in list in use)".to_string());
            return Ok(0);
        }
        let key = build_key(&settings, &user);
        let (engine, rules) = build_engine(&texts);
        let bytes = engine.serialize();
        let _ = crate::state::write_atomic(&cache_path(), &wrap_cache(&bytes));
        let _ = crate::state::write_atomic(&filters_dir().join(KEY_FILE), key.as_bytes());
        // Swap in the compact deserialized form: much smaller than the freshly built engine.
        let mut compact = Engine::default();
        let engine = if compact.deserialize(&bytes).is_ok() { compact } else { engine };
        if let Ok(mut slot) = self.engine.write() {
            *slot = Some(Arc::new(engine));
        }
        self.clear_verdicts();
        self.set_status(self.summary(&settings, Some(rules)));
        Ok(rules)
    }

    /// Downloads enabled lists (all of them, or only those not on disk) and recompiles. A list
    /// that fails to download keeps its previous text; the error is reported in the status.
    pub async fn refresh(&self, client: &reqwest::Client, only_missing: bool) -> Result<usize, String> {
        use std::sync::atomic::Ordering::Relaxed;
        self.refreshing.fetch_add(1, Relaxed);
        self.busy.store(true, Relaxed);
        let gate = self.refresh_gate.lock().await;
        let r = self.refresh_inner(client, only_missing).await;
        drop(gate);
        if self.refreshing.fetch_sub(1, Relaxed) == 1 {
            self.busy.store(false, Relaxed);
        }
        r
    }

    async fn refresh_inner(&self, client: &reqwest::Client, only_missing: bool) -> Result<usize, String> {
        let settings = load_settings();
        let mut errors = Vec::new();
        let mut fetched = 0usize;
        for l in CATALOG.iter().filter(|l| settings.is_on(l.id)) {
            if only_missing && list_path(l.id).exists() {
                continue;
            }
            match fetch_list(client, l).await {
                Ok(text) => {
                    let _ = crate::state::write_atomic(&list_path(l.id), text.as_bytes());
                    fetched += 1;
                }
                Err(e) => errors.push(format!("{}: {}", l.name, e)),
            }
        }
        if fetched > 0 && !only_missing {
            let now = now_secs();
            update_settings(|s| s.last_update = now);
        }
        let built = tokio::task::spawn_blocking(|| global().rebuild()).await.map_err(|e| e.to_string())?;
        if !errors.is_empty() {
            let kept = if self.has_lists() { "other lists in use" } else { "built-in list in use" };
            let msg = format!("Update failed for {} ({}): {}", errors.len(), kept, errors.join("; "));
            self.set_status(msg.clone());
            return Err(msg);
        }
        built
    }

    /// Turns a catalog list on or off. Returns false for an unknown id.
    pub fn set_list(&self, id: &str, on: bool) -> bool {
        if !CATALOG.iter().any(|l| l.id == id) {
            return false;
        }
        update_settings(|s| {
            s.enabled.retain(|e| e != id);
            if on {
                s.enabled.push(id.to_string());
            }
        });
        true
    }

    pub fn set_auto_update(&self, on: bool) {
        update_settings(|s| s.auto_update = on);
    }

    pub fn set_interval(&self, days: u32) {
        if INTERVALS.contains(&days) {
            update_settings(|s| s.interval_days = days);
        }
    }

    /// Replaces "My filters". Returns the lines the engine cannot use.
    pub fn save_user_rules(&self, text: &str) -> Vec<(usize, String)> {
        let clean = clean_user_text(text);
        let path = filters_dir().join(USER_FILE);
        if clean.is_empty() {
            let _ = fs::remove_file(&path);
        } else {
            let _ = crate::state::write_atomic(&path, clean.as_bytes());
        }
        invalid_rules(&clean)
    }

    /// Adds rules to "My filters", skipping those already there. Returns how many were added.
    pub fn add_user_rules(&self, text: &str) -> usize {
        let current = user_rules_text();
        let have: HashSet<String> = current.lines().map(|l| l.trim().to_string()).collect();
        let mut seen = HashSet::new();
        let new: Vec<String> = clean_user_text(text)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !have.contains(l) && seen.insert(l.clone()))
            .collect();
        if new.is_empty() {
            return 0;
        }
        let mut merged = current.trim_end().to_string();
        if !merged.is_empty() {
            merged.push('\n');
        }
        merged.push_str(&new.join("\n"));
        self.save_user_rules(&merged);
        new.iter().filter(|l| !l.starts_with('!')).count()
    }

    fn engine(&self) -> Option<Arc<Engine>> {
        self.engine.read().ok().and_then(|e| e.clone())
    }

    /// `req_type` uses adblock names: script, image, stylesheet, font, media, xmlhttprequest,
    /// websocket, ping, subdocument, other.
    pub fn should_block(&self, url: &str, source_url: &str, req_type: &str) -> bool {
        match self.engine() {
            Some(engine) => {
                let key = format!("{req_type}\n{source_url}\n{url}");
                if let Some(v) = self.verdicts.lock().ok().and_then(|m| m.get(&key).copied()) {
                    return v;
                }
                let gen = self.engine_gen.load(std::sync::atomic::Ordering::Acquire);
                let verdict = match Request::new(url, source_url, req_type, "GET") {
                    Ok(req) => engine.check_network_request(&req).should_block(),
                    Err(_) => false,
                };
                if let Ok(mut m) = self.verdicts.lock() {
                    if self.engine_gen.load(std::sync::atomic::Ordering::Acquire) == gen {
                        if m.len() >= VERDICT_CACHE_MAX {
                            m.clear();
                        }
                        m.insert(key, verdict);
                    }
                }
                verdict
            }
            None => is_blocked_ad_url(url),
        }
    }

    fn clear_verdicts(&self) {
        self.engine_gen.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        if let Ok(mut m) = self.verdicts.lock() {
            m.clear();
        }
    }

    /// CSS hiding ad containers on `page_url`; empty when lists are not loaded yet.
    pub fn cosmetic_css(&self, page_url: &str, classes: &[String], ids: &[String]) -> String {
        self.engine()
            .map(|e| cosmetic_css_for(&e, page_url, classes, ids))
            .unwrap_or_default()
    }
}

/// The pre v2.31 cache (EasyList + EasyPrivacy, no key) covers the default selection, so its
/// list texts not being on disk is no reason to download before the weekly refresh.
fn legacy_cache_covers(settings: &FilterSettings) -> bool {
    settings.enabled == default_enabled()
        && !filters_dir().join(KEY_FILE).exists()
        && cache_path().exists()
        && rule_lines(&user_rules_text()).next().is_none()
}

async fn fetch_list(client: &reqwest::Client, l: &ListInfo) -> Result<String, String> {
    let resp = client.get(l.url).timeout(Duration::from_secs(60)).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    if resp.content_length().is_some_and(|n| n as usize > MAX_LIST_BYTES) {
        return Err("list is too large".to_string());
    }
    // Chunked responses have no length up front: stop as soon as the running total passes the cap.
    let mut resp = resp;
    let mut bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > MAX_LIST_BYTES {
            return Err("list is too large".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    // Guard against captive portals and error pages masquerading as lists.
    if !looks_like_list(&text) {
        return Err("response is not a filter list".to_string());
    }
    Ok(text)
}

fn last_update_age(settings: &FilterSettings) -> Option<Duration> {
    let t = if settings.last_update > 0 {
        UNIX_EPOCH + Duration::from_secs(settings.last_update)
    } else {
        // Before the first full update: the oldest list on disk, else the (pre v2.31) cache file.
        settings
            .enabled
            .iter()
            .filter_map(|id| fs::metadata(list_path(id)).and_then(|m| m.modified()).ok())
            .min()
            .or_else(|| fs::metadata(cache_path()).and_then(|m| m.modified()).ok())?
    };
    SystemTime::now().duration_since(t).ok()
}

fn updated_label(settings: &FilterSettings) -> String {
    match last_update_age(settings).map(|d| d.as_secs() / 86_400) {
        None => "not downloaded yet".to_string(),
        Some(0) => "updated today".to_string(),
        Some(1) => "updated yesterday".to_string(),
        Some(n) => format!("updated {} days ago", n),
    }
}
