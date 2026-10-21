//! Two simulated computers syncing through an in-memory copy of the toolbay.ai
//! sync endpoint (same rules as the real one: per-key versions, baseVersion
//! checks, tombstones, ?since).

use super::*;
use std::sync::Arc;

#[derive(Default)]
pub struct Server {
    inner: Mutex<(i64, BTreeMap<String, Value>)>, // rev, key -> {value, version, updatedAt, deleted, rev}
}

impl Server {
    pub fn handle(&self, method: &str, since: Option<i64>, body: Option<Value>) -> (u16, Value) {
        let mut g = self.inner.lock();
        let (rev, items) = &mut *g;
        match method {
            "GET" => {
                let out: serde_json::Map<String, Value> = items
                    .iter()
                    .filter(|(_, it)| match since {
                        Some(n) => it["rev"].as_i64().unwrap() > n,
                        None => !it["deleted"].as_bool().unwrap_or(false),
                    })
                    .map(|(k, it)| {
                        let mut it = it.clone();
                        it.as_object_mut().unwrap().remove("rev");
                        (k.clone(), it)
                    })
                    .collect();
                (200, json!({ "rev": *rev, "full": since.is_none(), "items": out }))
            }
            "PUT" => {
                let mut applied = serde_json::Map::new();
                let mut conflicts = serde_json::Map::new();
                for c in body.unwrap()["changes"].as_array().unwrap() {
                    let k = c["key"].as_str().unwrap().to_string();
                    let cur = items.get(&k).map(|it| it["version"].as_i64().unwrap()).unwrap_or(0);
                    if c["baseVersion"].as_i64().unwrap() != cur {
                        let mut it = items.get(&k).cloned().unwrap_or(Value::Null);
                        if let Some(o) = it.as_object_mut() {
                            o.remove("rev");
                        }
                        conflicts.insert(k, it);
                        continue;
                    }
                    *rev += 1;
                    let at = chrono::Utc::now().to_rfc3339();
                    let deleted = c["deleted"].as_bool().unwrap_or(false);
                    let value = if deleted { Value::Null } else { c["value"].clone() };
                    items.insert(k.clone(), json!({ "value": value, "version": cur + 1, "updatedAt": at, "deleted": deleted, "rev": *rev }));
                    applied.insert(k, json!({ "version": cur + 1, "updatedAt": at }));
                }
                (200, json!({ "rev": *rev, "applied": applied, "conflicts": conflicts }))
            }
            _ => {
                items.clear();
                (200, json!({ "ok": true }))
            }
        }
    }
}

#[derive(Default)]
struct Mem(Mutex<BTreeMap<String, Value>>);

impl Store for Arc<Mem> {
    fn snapshot(&self) -> BTreeMap<String, Value> {
        self.0.lock().iter().filter(|(k, _)| !k.starts_with("history/")).map(|(k, v)| (k.clone(), v.clone())).collect()
    }
    fn apply(&self, changes: Vec<(String, Option<Value>)>) {
        let mut m = self.0.lock();
        for (k, v) in changes {
            match v {
                Some(v) => m.insert(k, v),
                None => m.remove(&k),
            };
        }
    }
    fn ignored(&self, key: &str) -> bool {
        key.starts_with("history/")
    }
}

struct Pc {
    mem: Arc<Mem>,
    sync: Syncer,
    _dir: PathBuf,
}

impl Pc {
    fn new(name: &str, srv: &Arc<Server>, items: &[(&str, Value)]) -> Pc {
        let dir = std::env::temp_dir().join(format!("tbflow-sync-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mem = Arc::new(Mem::default());
        for (k, v) in items {
            mem.0.lock().insert(k.to_string(), v.clone());
        }
        let mut sync = Syncer::new("toolbay-flow", dir.clone(), Box::new(mem.clone()), Box::new(|_| {}));
        sync.server = Some(srv.clone());
        Pc { mem, sync, _dir: dir }
    }
    fn get(&self, k: &str) -> Option<Value> {
        self.mem.0.lock().get(k).cloned()
    }
    fn set(&self, k: &str, v: Value) {
        self.mem.0.lock().insert(k.into(), v);
    }
    async fn sync(&self) {
        self.sync.sync_once().await;
        assert_eq!(self.sync.status().state, "ok");
    }
}

async fn tick() {
    tokio::time::sleep(Duration::from_millis(5)).await;
}

#[tokio::test]
async fn new_computer_gets_everything_and_adds_its_own() {
    let srv = Arc::new(Server::default());
    let a = Pc::new("a", &srv, &[("settings", json!({"sounds": false, "language": "es"})), ("dictionary/w1", json!({"phrase": "Toolbay"}))]);
    a.sync().await;
    // A new computer with default settings and one word of its own.
    let b = Pc::new("b", &srv, &[("settings", json!({"sounds": true, "language": "en"})), ("dictionary/w2", json!({"phrase": "Orion"}))]);
    b.sync().await;
    assert_eq!(b.get("settings"), Some(json!({"sounds": false, "language": "es"})), "the account wins on first sign-in");
    assert_eq!(b.get("dictionary/w1"), Some(json!({"phrase": "Toolbay"})));
    assert_eq!(b.get("dictionary/w2"), Some(json!({"phrase": "Orion"})), "local-only entries are kept");
    a.sync().await;
    assert_eq!(a.get("dictionary/w2"), Some(json!({"phrase": "Orion"})), "and reach the other computer");
}

#[tokio::test]
async fn different_entries_never_conflict_and_newest_edit_wins() {
    let srv = Arc::new(Server::default());
    let a = Pc::new("a", &srv, &[("settings", json!({"v": 0})), ("snippets/s1", json!({"t": "x"}))]);
    a.sync().await;
    let b = Pc::new("b", &srv, &[]);
    b.sync().await;

    // Both edit while "offline": A notices its edit first, B later.
    a.set("snippets/s1", json!({"t": "from a"}));
    a.set("settings", json!({"v": "a"}));
    tick().await;
    b.set("settings", json!({"v": "b"}));
    // A goes online first, then B (whose settings edit is newer).
    a.sync().await;
    tick().await;
    b.sync().await;
    a.sync().await;
    assert_eq!(a.get("snippets/s1"), Some(json!({"t": "from a"})));
    assert_eq!(b.get("snippets/s1"), Some(json!({"t": "from a"})));
    assert_eq!(a.get("settings"), Some(json!({"v": "b"})), "the newer edit wins on both");
    assert_eq!(b.get("settings"), Some(json!({"v": "b"})));
}

#[tokio::test]
async fn older_offline_edit_loses_to_newer_account_copy() {
    let srv = Arc::new(Server::default());
    let a = Pc::new("a", &srv, &[("settings", json!({"v": 0}))]);
    a.sync().await;
    let b = Pc::new("b", &srv, &[]);
    b.sync().await;
    // B edits offline and its change is noticed (timestamped) now...
    b.set("settings", json!({"v": "old b"}));
    {
        let mut l = b.sync.load();
        b.sync.detect(&mut l);
        b.sync.save(&l);
    }
    tick().await;
    // ...then A makes a newer edit and syncs it.
    a.set("settings", json!({"v": "new a"}));
    a.sync().await;
    b.sync().await;
    assert_eq!(b.get("settings"), Some(json!({"v": "new a"})));
    a.sync().await;
    assert_eq!(a.get("settings"), Some(json!({"v": "new a"})));
}

#[tokio::test]
async fn deletes_reach_other_computers_and_ignored_keys_stay_home() {
    let srv = Arc::new(Server::default());
    let a = Pc::new("a", &srv, &[("dictionary/w1", json!({"phrase": "x"})), ("history/h1", json!({"text": "private"}))]);
    a.sync().await;
    let b = Pc::new("b", &srv, &[]);
    b.sync().await;
    assert!(b.get("dictionary/w1").is_some());
    assert!(srv.inner.lock().1.get("history/h1").is_none(), "history is not uploaded while it's off");
    a.mem.0.lock().remove("dictionary/w1");
    a.sync().await;
    b.sync().await;
    assert!(b.get("dictionary/w1").is_none());
}

#[tokio::test]
async fn signed_out_changes_upload_after_sign_in_without_losing_anything() {
    let srv = Arc::new(Server::default());
    let a = Pc::new("a", &srv, &[("settings", json!({"v": 1}))]);
    a.sync().await;
    // Offline round: detect only.
    a.set("dictionary/w9", json!({"phrase": "offline word"}));
    {
        let mut l = a.sync.load();
        a.sync.detect(&mut l);
        a.sync.save(&l);
        assert_eq!(l.dirty.len(), 1);
    }
    a.sync().await;
    assert!(srv.inner.lock().1.contains_key("dictionary/w9"));
}

/// Same two-computer round trip against the real toolbay.ai endpoint, under a
/// throwaway tool slug, using the Toolbay app's sign-in on this computer.
/// `cargo test --lib live_ -- --ignored`
#[tokio::test]
#[ignore]
async fn live_round_trip_against_toolbay_ai() {
    let slug = "toolbay-flow-selftest";
    let mk = |name: &str, items: &[(&str, Value)]| {
        let dir = std::env::temp_dir().join(format!("tbflow-live-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mem = Arc::new(Mem::default());
        for (k, v) in items {
            mem.0.lock().insert(k.to_string(), v.clone());
        }
        (mem.clone(), Syncer::new(slug, dir, Box::new(mem), Box::new(|_| {})))
    };
    let (_, wipe) = mk("wipe", &[]);
    wipe.delete_remote().await.expect("signed in to the Toolbay app on this computer");
    let (ma, a) = mk("a", &[("settings", json!({"v": 1})), ("dictionary/w1", json!({"phrase": "Toolbay"}))]);
    a.sync_once().await;
    assert_eq!(a.status().state, "ok", "{}", a.status().message);
    let (mb, b) = mk("b", &[("settings", json!({"v": 0})), ("dictionary/w2", json!({"phrase": "Orion"}))]);
    b.sync_once().await;
    assert_eq!(mb.0.lock().get("settings"), Some(&json!({"v": 1})));
    assert!(mb.0.lock().contains_key("dictionary/w1"));
    mb.0.lock().insert("settings".into(), json!({"v": 2}));
    mb.0.lock().remove("dictionary/w1");
    b.sync_once().await;
    a.sync_once().await;
    assert_eq!(ma.0.lock().get("settings"), Some(&json!({"v": 2})));
    assert!(!ma.0.lock().contains_key("dictionary/w1"), "delete reached A");
    assert!(ma.0.lock().contains_key("dictionary/w2"));
    wipe.delete_remote().await.unwrap();
}
