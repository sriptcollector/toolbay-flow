//! Toolbay account sync: keeps a tool's settings the same on every computer the
//! person signs in on, through `toolbay.ai/api/v1/me/tools/<slug>/sync`.
//!
//! Generic on purpose (any Toolbay tool can copy this file): the tool supplies a
//! [`Store`] that turns its local state into keyed JSON items and applies items
//! that changed elsewhere. This engine does the rest:
//! - one item per entry (`settings`, `dictionary/<id>`...), so two computers
//!   editing different entries never conflict;
//! - last write wins per item, by time of change;
//! - works offline: changes are noticed and timestamped locally and sent later;
//! - first sync with an account: what's in the account wins, entries only this
//!   computer has are added to it (so a new computer needs no setup);
//! - signed out: nothing happens, the tool keeps working locally.
//!
//! Sign-in is the Toolbay app's: its token in the OS keychain, or the
//! `TOOLBAY_TOKEN` it passes when it opens the tool. The token is never written
//! anywhere by this code. Only toolbay.ai is contacted.

pub mod flow;

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

/// A tool's synced data, as keyed JSON items.
pub trait Store: Send + Sync + 'static {
    /// Every item that should be in the account right now.
    fn snapshot(&self) -> BTreeMap<String, Value>;
    /// Items that changed on another computer (`None` = deleted there).
    fn apply(&self, changes: Vec<(String, Option<Value>)>);
    /// Missing from the snapshot but not deleted (e.g. older dictations that
    /// just fell out of the synced window).
    fn retains(&self, _key: &str) -> bool {
        false
    }
    /// Keys not synced right now (e.g. history while that option is off).
    fn ignored(&self, _key: &str) -> bool {
        false
    }
    /// Sync turned off on this computer.
    fn enabled(&self) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct Item {
    version: i64,
    hash: u64,
}

/// What this computer knows about the account copy. `<config>/sync.json`.
#[derive(Serialize, Deserialize, Default)]
struct Local {
    account: String,
    rev: i64,
    synced_at: Option<chrono::DateTime<chrono::Utc>>,
    items: BTreeMap<String, Item>,
    /// Keys changed here and not yet in the account, with the time of change (ms).
    /// 0 means "before this computer was ever synced": the account copy wins.
    dirty: BTreeMap<String, i64>,
}

#[derive(Serialize, Clone, Default)]
pub struct Status {
    /// off | signed_out | syncing | ok | offline | error
    pub state: String,
    pub account: Option<Value>,
    pub synced_at: Option<String>,
    pub pending: usize,
    pub message: String,
}

pub struct Syncer {
    slug: String,
    file: PathBuf,
    store: Box<dyn Store>,
    run: tokio::sync::Mutex<()>,
    status: Mutex<Status>,
    full_next: std::sync::atomic::AtomicBool,
    last_pull: Mutex<Option<std::time::Instant>>,
    on_status: Box<dyn Fn(&Status) + Send + Sync>,
    #[cfg(test)]
    pub server: Option<std::sync::Arc<tests::Server>>,
}

static NOTIFY: Lazy<tokio::sync::Notify> = Lazy::new(tokio::sync::Notify::new);

/// Something local changed: sync in a couple of seconds. Cheap, call it anywhere.
pub fn poke() {
    NOTIFY.notify_one();
}

const BASE: &str = "https://toolbay.ai";

fn base() -> String {
    std::env::var("TOOLBAY_API_BASE").unwrap_or_else(|_| BASE.into()).trim_end_matches('/').to_string()
}

fn client() -> &'static reqwest::Client {
    static C: Lazy<reqwest::Client> = Lazy::new(|| {
        reqwest::Client::builder()
            .user_agent(concat!("ToolbayFlow/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("http client")
    });
    &C
}

/// Tokens to try: the Toolbay app's current sign-in first (it changes when the
/// person signs in or out there), then the one it passed at launch.
fn tokens() -> Vec<String> {
    let mut v = vec![];
    if let Ok(t) = keyring::Entry::new("ai.toolbay.desktop", "session").and_then(|e| e.get_password()) {
        v.push(t);
    }
    if let Ok(t) = std::env::var("TOOLBAY_TOKEN") {
        if !t.is_empty() && !v.contains(&t) {
            v.push(t);
        }
    }
    v
}

/// The signed-in user (`/api/v1/me`) and the token that works, if any.
pub async fn account() -> Result<Option<(String, Value)>> {
    let mut offline = None;
    for t in tokens() {
        match client().get(format!("{}/api/v1/me", base())).bearer_auth(&t).send().await {
            Ok(r) if r.status().is_success() => {
                let v: Value = r.json().await?;
                let u = v.get("user").cloned().unwrap_or(v);
                if u.is_object() {
                    return Ok(Some((t, u)));
                }
            }
            Ok(_) => {} // revoked or expired: try the next one
            Err(e) => offline = Some(e),
        }
    }
    match offline {
        Some(e) => Err(e.into()),
        None => Ok(None),
    }
}

/// Stable 64-bit FNV-1a of an item's JSON (object keys are sorted by serde_json).
fn hash(v: &Value) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in v.to_string().bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn ms(iso: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(iso).map(|d| d.timestamp_millis()).unwrap_or(0)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Largest item the server accepts is 64 KB; stay under it.
const MAX_ITEM: usize = 60_000;
/// Full re-read after this long offline (the server keeps deletes for 90 days).
const FULL_AFTER_DAYS: i64 = 80;

impl Syncer {
    pub fn new(slug: &str, config_dir: PathBuf, store: Box<dyn Store>, on_status: Box<dyn Fn(&Status) + Send + Sync>) -> Self {
        Syncer {
            slug: slug.into(),
            file: config_dir.join("sync.json"),
            store,
            run: tokio::sync::Mutex::new(()),
            status: Mutex::new(Status { state: "signed_out".into(), ..Default::default() }),
            full_next: Default::default(),
            last_pull: Mutex::new(None),
            on_status,
            #[cfg(test)]
            server: None,
        }
    }

    fn load(&self) -> Local {
        std::fs::read_to_string(&self.file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    fn save(&self, l: &Local) {
        let tmp = self.file.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec(l).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(tmp, &self.file);
        }
    }

    pub fn status(&self) -> Status {
        self.status.lock().clone()
    }

    fn set_status(&self, f: impl FnOnce(&mut Status)) {
        let s = {
            let mut s = self.status.lock();
            f(&mut s);
            s.clone()
        };
        (self.on_status)(&s);
    }

    fn url(&self) -> String {
        format!("{}/api/v1/me/tools/{}/sync", base(), self.slug)
    }

    /// One request to the sync endpoint: (status, JSON body).
    async fn call(&self, method: &str, token: &str, since: Option<i64>, body: Option<Value>) -> Result<(u16, Value)> {
        #[cfg(test)]
        if let Some(srv) = &self.server {
            return Ok(srv.handle(method, since, body));
        }
        let mut req = match method {
            "GET" => client().get(self.url()),
            "PUT" => client().put(self.url()),
            _ => client().delete(self.url()),
        }
        .bearer_auth(token);
        if let Some(n) = since {
            req = req.query(&[("since", n)]);
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        let r = req.send().await?;
        let status = r.status().as_u16();
        Ok((status, r.json().await.unwrap_or(Value::Null)))
    }

    async fn user(&self) -> Result<Option<(String, Value)>> {
        #[cfg(test)]
        if self.server.is_some() {
            return Ok(Some(("test".into(), json!({ "id": "u1", "name": "Test" }))));
        }
        account().await
    }

    /// Runs forever: on start, a couple of seconds after each local change, and
    /// every five minutes.
    pub async fn run_forever(self: std::sync::Arc<Self>) {
        let mut force = true;
        loop {
            self.round_or_skip(force).await;
            force = tokio::select! {
                _ = NOTIFY.notified() => {
                    // Let a burst of edits settle into one upload.
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    false
                }
                _ = tokio::time::sleep(Duration::from_secs(300)) => true,
            };
        }
    }

    /// One full round. Never fails: problems end up in the status.
    pub async fn sync_once(&self) {
        self.round_or_skip(true).await
    }

    /// `force: false` (after a local change): skip the network when nothing synced
    /// changed and the account was read in the last minute. Dictating with
    /// history sync off changes nothing synced, so it costs no requests.
    async fn round_or_skip(&self, force: bool) {
        let _g = self.run.lock().await;
        let mut l = self.load();
        // Noticing local changes needs no network, so offline edits keep their real time.
        self.detect(&mut l);
        let pending = l.dirty.len();
        let recent = self.last_pull.lock().is_some_and(|t| t.elapsed() < Duration::from_secs(60));
        if !force && pending == 0 && recent && !self.full_next.load(std::sync::atomic::Ordering::SeqCst) {
            self.save(&l);
            return;
        }
        if !self.store.enabled() {
            self.save(&l);
            self.set_status(|s| {
                s.state = "off".into();
                s.pending = pending;
            });
            return;
        }
        let res = self.round(&mut l).await;
        self.save(&l);
        let pending = l.dirty.len();
        let synced_at = l.synced_at.map(|t| t.to_rfc3339());
        self.set_status(|s| {
            s.pending = pending;
            s.synced_at = synced_at;
            match res {
                Ok(Some(user)) => {
                    s.state = "ok".into();
                    s.account = Some(user);
                    s.message.clear();
                }
                Ok(None) => {
                    s.state = "signed_out".into();
                    s.account = None;
                    s.message.clear();
                }
                Err(e) => {
                    let offline = e.downcast_ref::<reqwest::Error>().is_some_and(|e| e.is_connect() || e.is_timeout());
                    s.state = if offline { "offline" } else { "error" }.into();
                    s.message = if offline {
                        "Can't reach Toolbay right now. Your changes are saved on this computer and will sync when you're back online.".into()
                    } else {
                        "Sync didn't finish. Flow will try again in a few minutes.".into()
                    };
                    log::warn!("sync: {e:#}");
                }
            }
        });
    }

    /// Compares the store with what was last synced and marks changed keys.
    fn detect(&self, l: &mut Local) {
        let local = self.store.snapshot();
        let first = l.account.is_empty();
        let t = if first { 0 } else { now_ms() };
        for (k, v) in &local {
            let same = l.items.get(k).is_some_and(|it| it.hash == hash(v) && it.version > 0);
            if same {
                l.dirty.remove(k);
            } else {
                l.dirty.entry(k.clone()).or_insert(t);
            }
        }
        let gone: Vec<String> = l
            .items
            .keys()
            .filter(|k| !local.contains_key(*k) && !self.store.ignored(k) && !self.store.retains(k))
            .cloned()
            .collect();
        for k in gone {
            l.dirty.entry(k).or_insert(t);
        }
        // Changes to keys that exist neither here nor in the account: nothing to do.
        l.dirty.retain(|k, _| local.contains_key(k) || l.items.contains_key(k));
        l.dirty.retain(|k, _| !self.store.ignored(k));
    }

    async fn round(&self, l: &mut Local) -> Result<Option<Value>> {
        let Some((token, user)) = self.user().await? else { return Ok(None) };
        let id = user["id"].as_str().map(String::from).unwrap_or_else(|| user["id"].to_string());
        if l.account != id {
            // First sync with this account (or a different account): the account
            // copy wins, entries only this computer has are added to it.
            let first = std::mem::take(&mut l.dirty);
            *l = Local { account: id, ..Default::default() };
            self.detect(l);
            for (k, t) in first {
                l.dirty.entry(k).or_insert(t);
            }
            for t in l.dirty.values_mut() {
                *t = 0;
            }
        }
        self.set_status(|s| s.state = "syncing".into());
        self.pull(l, &token).await?;
        self.push(l, &token).await?;
        l.synced_at = Some(chrono::Utc::now());
        Ok(Some(user))
    }

    async fn pull(&self, l: &mut Local, token: &str) -> Result<()> {
        let stale = l.synced_at.is_none_or(|t| chrono::Utc::now() - t > chrono::Duration::days(FULL_AFTER_DAYS));
        let full = l.rev == 0 || stale || self.full_next.swap(false, std::sync::atomic::Ordering::SeqCst);
        let (code, body) = self.call("GET", token, (!full).then_some(l.rev), None).await?;
        if code != 200 {
            return Err(anyhow!("sync read failed: {code}"));
        }
        let items = body["items"].as_object().cloned().unwrap_or_default();
        let local = self.store.snapshot();
        let mut apply = vec![];
        if body["full"].as_bool().unwrap_or(full) {
            // Anything synced before but no longer in the account was deleted elsewhere.
            let gone: Vec<String> = l.items.keys().filter(|k| !items.contains_key(*k) && !self.store.ignored(k)).cloned().collect();
            for k in gone {
                l.items.remove(&k);
                if !l.dirty.contains_key(&k) {
                    apply.push((k, None));
                }
            }
        }
        for (k, it) in items {
            if self.store.ignored(&k) {
                continue;
            }
            let version = it["version"].as_i64().unwrap_or(0);
            let remote_ms = ms(it["updatedAt"].as_str().unwrap_or(""));
            let ours_newer = l.dirty.get(&k).is_some_and(|t| *t > remote_ms);
            if it["deleted"].as_bool().unwrap_or(false) {
                if ours_newer {
                    l.items.insert(k, Item { version, hash: 0 });
                } else {
                    l.items.remove(&k);
                    l.dirty.remove(&k);
                    if local.contains_key(&k) {
                        apply.push((k, None));
                    }
                }
                continue;
            }
            let v = it["value"].clone();
            let h = hash(&v);
            if ours_newer {
                // Keep our change; the next upload replaces this version.
                l.items.insert(k, Item { version, hash: h });
            } else {
                if local.get(&k).map(hash) != Some(h) {
                    apply.push((k.clone(), Some(v)));
                }
                l.items.insert(k.clone(), Item { version, hash: h });
                l.dirty.remove(&k);
            }
        }
        if !apply.is_empty() {
            self.store.apply(apply);
        }
        l.rev = body["rev"].as_i64().unwrap_or(l.rev);
        *self.last_pull.lock() = Some(std::time::Instant::now());
        Ok(())
    }

    async fn push(&self, l: &mut Local, token: &str) -> Result<()> {
        for _attempt in 0..3 {
            if l.dirty.is_empty() {
                return Ok(());
            }
            let local = self.store.snapshot();
            let mut changes = vec![];
            for k in l.dirty.keys().cloned().collect::<Vec<_>>() {
                let base = l.items.get(&k).map(|i| i.version).unwrap_or(0);
                match local.get(&k) {
                    Some(v) if v.to_string().len() > MAX_ITEM => {
                        log::warn!("sync: {k} is too large to sync, skipped");
                        l.dirty.remove(&k);
                    }
                    Some(v) => changes.push(json!({ "key": k, "baseVersion": base, "value": v })),
                    None if base > 0 => changes.push(json!({ "key": k, "baseVersion": base, "deleted": true })),
                    None => {
                        l.dirty.remove(&k);
                        l.items.remove(&k);
                    }
                }
            }
            if changes.is_empty() {
                return Ok(());
            }
            let mut conflicted = false;
            for chunk in changes.chunks(200) {
                let (code, body) = self.call("PUT", token, None, Some(json!({ "changes": chunk }))).await?;
                if code == 429 {
                    return Ok(()); // busy: the next round sends it
                }
                if code != 200 {
                    return Err(anyhow!("sync write failed: {code}"));
                }
                for (k, a) in body["applied"].as_object().cloned().unwrap_or_default() {
                    let version = a["version"].as_i64().unwrap_or(0);
                    l.dirty.remove(&k);
                    match local.get(&k) {
                        Some(v) => {
                            l.items.insert(k, Item { version, hash: hash(v) });
                        }
                        None => {
                            l.items.remove(&k);
                        }
                    }
                }
                let mut apply = vec![];
                for (k, srv) in body["conflicts"].as_object().cloned().unwrap_or_default() {
                    conflicted = true;
                    if srv.is_null() {
                        // Gone from the account: write it fresh.
                        l.items.remove(&k);
                        continue;
                    }
                    let version = srv["version"].as_i64().unwrap_or(0);
                    let remote_ms = ms(srv["updatedAt"].as_str().unwrap_or(""));
                    if l.dirty.get(&k).is_some_and(|t| *t > remote_ms) {
                        l.items.insert(k, Item { version, hash: 0 });
                    } else {
                        let deleted = srv["deleted"].as_bool().unwrap_or(false);
                        let v = srv["value"].clone();
                        l.dirty.remove(&k);
                        if deleted {
                            l.items.remove(&k);
                            apply.push((k, None));
                        } else {
                            l.items.insert(k.clone(), Item { version, hash: hash(&v) });
                            apply.push((k, Some(v)));
                        }
                    }
                }
                if !apply.is_empty() {
                    self.store.apply(apply);
                }
            }
            if !conflicted {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Re-reads everything from the account on the next round (after an option
    /// that changes what is synced, such as history, is turned on).
    pub fn reread(&self) {
        self.full_next.store(true, std::sync::atomic::Ordering::SeqCst);
        poke();
    }

    /// Removes this tool's data from the account. This computer keeps its copy.
    pub async fn delete_remote(&self) -> Result<()> {
        let _g = self.run.lock().await;
        let (token, _) = self.user().await?.ok_or_else(|| anyhow!("Sign in to Toolbay first."))?;
        let (code, _) = self.call("DELETE", &token, None, None).await?;
        if code != 200 {
            return Err(anyhow!("Couldn't remove the synced copy. Try again in a minute."));
        }
        let mut l = self.load();
        let account = std::mem::take(&mut l.account);
        l = Local { account, ..Default::default() };
        self.save(&l);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
