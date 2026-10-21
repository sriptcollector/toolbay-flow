//! Local SQLite store: dictation history, dictionary + snippets, notes.

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct Db(Mutex<Connection>);

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct HistoryItem {
    pub id: i64,
    pub created_at: String,
    pub mode: String,
    pub raw_text: String,
    pub text: String,
    pub app: String,
    pub window_title: String,
    pub url: String,
    pub language: String,
    pub duration_ms: i64,
    pub latency_ms: i64,
    pub words: i64,
    pub status: String,
    pub has_audio: bool,
    pub edited_text: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DictEntry {
    pub id: i64,
    pub phrase: String,
    /// For replacement rules ("misspelling -> fix") and snippets (trigger -> expansion).
    pub replacement: Option<String>,
    pub is_snippet: bool,
    pub starred: bool,
    pub source: String,
    pub uses: i64,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Note {
    pub id: i64,
    pub content: String,
    pub pinned: bool,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Stats {
    pub total_words: i64,
    pub total_dictations: i64,
    pub words_this_week: i64,
    pub avg_wpm: f64,
    pub streak_days: i64,
    pub minutes_saved: f64,
    pub top_apps: Vec<(String, i64)>,
    pub daily: Vec<(String, i64)>,
}

fn now() -> String {
    chrono::Local::now().to_rfc3339()
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode=WAL;
            PRAGMA auto_vacuum=INCREMENTAL;
            CREATE TABLE IF NOT EXISTS history (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              created_at TEXT NOT NULL,
              mode TEXT NOT NULL DEFAULT 'dictation',
              raw_text TEXT NOT NULL DEFAULT '',
              text TEXT NOT NULL DEFAULT '',
              app TEXT NOT NULL DEFAULT '',
              window_title TEXT NOT NULL DEFAULT '',
              url TEXT NOT NULL DEFAULT '',
              language TEXT NOT NULL DEFAULT '',
              duration_ms INTEGER NOT NULL DEFAULT 0,
              latency_ms INTEGER NOT NULL DEFAULT 0,
              words INTEGER NOT NULL DEFAULT 0,
              status TEXT NOT NULL DEFAULT 'pasted',
              audio BLOB,
              edited_text TEXT,
              archived INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS history_created ON history(created_at);
            CREATE TABLE IF NOT EXISTS dictionary (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              phrase TEXT NOT NULL,
              replacement TEXT,
              is_snippet INTEGER NOT NULL DEFAULT 0,
              starred INTEGER NOT NULL DEFAULT 0,
              source TEXT NOT NULL DEFAULT 'manual',
              uses INTEGER NOT NULL DEFAULT 0,
              created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS notes (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              content TEXT NOT NULL DEFAULT '',
              pinned INTEGER NOT NULL DEFAULT 0,
              updated_at TEXT NOT NULL
            );
            "#,
        )?;
        // Stable ids so the same entry can be matched on every computer (account sync).
        for t in ["history", "dictionary"] {
            let _ = conn.execute(&format!("ALTER TABLE {t} ADD COLUMN sync_id TEXT"), []);
            let _ = conn.execute(&format!("UPDATE {t} SET sync_id=lower(hex(randomblob(16))) WHERE sync_id IS NULL"), []);
            let _ = conn.execute(&format!("CREATE UNIQUE INDEX IF NOT EXISTS {t}_sync ON {t}(sync_id)"), []);
        }
        // Entries left by development builds (the bar page URL pasted as text).
        let _ = conn.execute("DELETE FROM history WHERE text LIKE 'http://localhost:1437/%'", []);
        Ok(Db(Mutex::new(conn)))
    }

    // ---------- history ----------
    pub fn add_history(&self, h: &HistoryItem, audio: Option<&[u8]>) -> Result<i64> {
        crate::sync::poke();
        let c = self.0.lock();
        c.execute(
            "INSERT INTO history (created_at, mode, raw_text, text, app, window_title, url, language, duration_ms, latency_ms, words, status, audio, sync_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,lower(hex(randomblob(16))))",
            params![now(), h.mode, h.raw_text, h.text, h.app, h.window_title, h.url, h.language, h.duration_ms, h.latency_ms, h.words, h.status, audio],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn update_history_text(&self, id: i64, raw: &str, text: &str, status: &str) -> Result<()> {
        let words = count_words(text) as i64;
        self.0.lock().execute(
            "UPDATE history SET raw_text=?2, text=?3, status=?4, words=?5 WHERE id=?1",
            params![id, raw, text, status, words],
        )?;
        Ok(())
    }

    pub fn set_edited(&self, id: i64, edited: &str) -> Result<()> {
        crate::sync::poke();
        self.0.lock().execute("UPDATE history SET edited_text=?2 WHERE id=?1", params![id, edited])?;
        Ok(())
    }

    pub fn list_history(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<HistoryItem>> {
        let c = self.0.lock();
        let mut st = c.prepare(
            "SELECT id, created_at, mode, raw_text, text, app, window_title, url, language, duration_ms, latency_ms, words, status, audio IS NOT NULL, edited_text
             FROM history WHERE archived=0 AND (?1='' OR text LIKE '%'||?1||'%' OR raw_text LIKE '%'||?1||'%' OR app LIKE '%'||?1||'%')
             ORDER BY created_at DESC, id DESC LIMIT ?2 OFFSET ?3",
        )?;
        let rows = st.query_map(params![query, limit, offset], |r| {
            Ok(HistoryItem {
                id: r.get(0)?,
                created_at: r.get(1)?,
                mode: r.get(2)?,
                raw_text: r.get(3)?,
                text: r.get(4)?,
                app: r.get(5)?,
                window_title: r.get(6)?,
                url: r.get(7)?,
                language: r.get(8)?,
                duration_ms: r.get(9)?,
                latency_ms: r.get(10)?,
                words: r.get(11)?,
                status: r.get(12)?,
                has_audio: r.get(13)?,
                edited_text: r.get(14)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_audio(&self, id: i64) -> Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .lock()
            .query_row("SELECT audio FROM history WHERE id=?1", params![id], |r| r.get::<_, Option<Vec<u8>>>(0))
            .optional()?
            .flatten())
    }

    pub fn last_text(&self) -> Result<Option<String>> {
        Ok(self
            .0
            .lock()
            .query_row(
                "SELECT text FROM history WHERE archived=0 AND text<>'' ORDER BY created_at DESC, id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn delete_history(&self, id: i64) -> Result<()> {
        crate::sync::poke();
        self.0.lock().execute("DELETE FROM history WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<()> {
        crate::sync::poke();
        let c = self.0.lock();
        c.execute("DELETE FROM history", [])?;
        c.execute_batch("PRAGMA incremental_vacuum;")?;
        Ok(())
    }

    /// Applies the data policy: drop audio older than 14 days, delete rows per policy.
    pub fn apply_retention(&self, policy: &crate::settings::DataPolicy) -> Result<()> {
        use crate::settings::DataPolicy::*;
        let c = self.0.lock();
        let cutoff_audio = (chrono::Local::now() - chrono::Duration::days(14)).to_rfc3339();
        c.execute("UPDATE history SET audio=NULL WHERE created_at < ?1", params![cutoff_audio])?;
        match policy {
            StoreNormally => {}
            DeleteAfter24h => {
                let cutoff = (chrono::Local::now() - chrono::Duration::hours(24)).to_rfc3339();
                c.execute("DELETE FROM history WHERE created_at < ?1", params![cutoff])?;
            }
            NeverStore => {
                c.execute("DELETE FROM history", [])?;
            }
        }
        c.execute_batch("PRAGMA incremental_vacuum;")?;
        Ok(())
    }

    // ---------- dictionary & snippets ----------
    pub fn list_dict(&self, snippets: bool) -> Result<Vec<DictEntry>> {
        let c = self.0.lock();
        let mut st = c.prepare(
            "SELECT id, phrase, replacement, is_snippet, starred, source, uses, created_at FROM dictionary
             WHERE is_snippet=?1 ORDER BY starred DESC, id DESC",
        )?;
        let rows = st.query_map(params![snippets as i64], |r| {
            Ok(DictEntry {
                id: r.get(0)?,
                phrase: r.get(1)?,
                replacement: r.get(2)?,
                is_snippet: r.get::<_, i64>(3)? != 0,
                starred: r.get::<_, i64>(4)? != 0,
                source: r.get(5)?,
                uses: r.get(6)?,
                created_at: r.get(7)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn upsert_dict(&self, e: &DictEntry) -> Result<i64> {
        crate::sync::poke();
        let c = self.0.lock();
        if e.id > 0 {
            c.execute(
                "UPDATE dictionary SET phrase=?2, replacement=?3, is_snippet=?4, starred=?5 WHERE id=?1",
                params![e.id, e.phrase, e.replacement, e.is_snippet as i64, e.starred as i64],
            )?;
            return Ok(e.id);
        }
        // Avoid duplicate vocabulary words.
        if !e.is_snippet {
            if let Some(id) = c
                .query_row(
                    "SELECT id FROM dictionary WHERE is_snippet=0 AND lower(phrase)=lower(?1)",
                    params![e.phrase],
                    |r| r.get::<_, i64>(0),
                )
                .optional()?
            {
                return Ok(id);
            }
        }
        c.execute(
            "INSERT INTO dictionary (phrase, replacement, is_snippet, starred, source, created_at, sync_id) VALUES (?1,?2,?3,?4,?5,?6,lower(hex(randomblob(16))))",
            params![e.phrase, e.replacement, e.is_snippet as i64, e.starred as i64, if e.source.is_empty() { "manual" } else { &e.source }, now()],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn delete_dict(&self, id: i64) -> Result<()> {
        crate::sync::poke();
        self.0.lock().execute("DELETE FROM dictionary WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn bump_dict_use(&self, id: i64) {
        let _ = self.0.lock().execute("UPDATE dictionary SET uses=uses+1 WHERE id=?1", params![id]);
    }

    // ---------- notes ----------
    pub fn list_notes(&self) -> Result<Vec<Note>> {
        let c = self.0.lock();
        let mut st = c.prepare("SELECT id, content, pinned, updated_at FROM notes ORDER BY pinned DESC, updated_at DESC")?;
        let rows = st.query_map([], |r| {
            Ok(Note { id: r.get(0)?, content: r.get(1)?, pinned: r.get::<_, i64>(2)? != 0, updated_at: r.get(3)? })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn save_note(&self, n: &Note) -> Result<i64> {
        let c = self.0.lock();
        if n.id > 0 {
            c.execute(
                "UPDATE notes SET content=?2, pinned=?3, updated_at=?4 WHERE id=?1",
                params![n.id, n.content, n.pinned as i64, now()],
            )?;
            Ok(n.id)
        } else {
            c.execute("INSERT INTO notes (content, pinned, updated_at) VALUES (?1,?2,?3)", params![n.content, n.pinned as i64, now()])?;
            Ok(c.last_insert_rowid())
        }
    }

    pub fn delete_note(&self, id: i64) -> Result<()> {
        self.0.lock().execute("DELETE FROM notes WHERE id=?1", params![id])?;
        Ok(())
    }

    // ---------- account sync ----------
    /// Dictionary words and snippets as (sync id, entry).
    pub fn sync_dict(&self) -> Result<Vec<(String, DictEntry)>> {
        let c = self.0.lock();
        let mut st = c.prepare("SELECT sync_id, phrase, replacement, is_snippet, starred, source, created_at FROM dictionary")?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                DictEntry {
                    phrase: r.get(1)?,
                    replacement: r.get(2)?,
                    is_snippet: r.get::<_, i64>(3)? != 0,
                    starred: r.get::<_, i64>(4)? != 0,
                    source: r.get(5)?,
                    created_at: r.get(6)?,
                    ..Default::default()
                },
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Writes an entry that came from another computer. A vocabulary word this
    /// computer already has under another id takes over the incoming id.
    pub fn sync_put_dict(&self, sid: &str, e: &DictEntry) -> Result<()> {
        let c = self.0.lock();
        let existing: Option<i64> = c.query_row("SELECT id FROM dictionary WHERE sync_id=?1", params![sid], |r| r.get(0)).optional()?;
        let existing = match existing {
            Some(id) => Some(id),
            None if !e.is_snippet => c
                .query_row("SELECT id FROM dictionary WHERE is_snippet=0 AND lower(phrase)=lower(?1)", params![e.phrase], |r| r.get(0))
                .optional()?,
            None => None,
        };
        match existing {
            Some(id) => c.execute(
                "UPDATE dictionary SET phrase=?2, replacement=?3, is_snippet=?4, starred=?5, sync_id=?6 WHERE id=?1",
                params![id, e.phrase, e.replacement, e.is_snippet as i64, e.starred as i64, sid],
            )?,
            None => c.execute(
                "INSERT INTO dictionary (phrase, replacement, is_snippet, starred, source, created_at, sync_id) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![e.phrase, e.replacement, e.is_snippet as i64, e.starred as i64, if e.source.is_empty() { "manual" } else { &e.source }, if e.created_at.is_empty() { now() } else { e.created_at.clone() }, sid],
            )?,
        };
        Ok(())
    }

    pub fn sync_delete(&self, table: &str, sid: &str) -> Result<()> {
        let t = if table == "history" { "history" } else { "dictionary" };
        self.0.lock().execute(&format!("DELETE FROM {t} WHERE sync_id=?1"), params![sid])?;
        Ok(())
    }

    pub fn sync_has(&self, table: &str, sid: &str) -> bool {
        let t = if table == "history" { "history" } else { "dictionary" };
        self.0.lock().query_row(&format!("SELECT 1 FROM {t} WHERE sync_id=?1"), params![sid], |_| Ok(())).is_ok()
    }

    /// The most recent dictations (text only, never audio) as (sync id, item).
    pub fn sync_history(&self, limit: i64) -> Result<Vec<(String, HistoryItem)>> {
        let c = self.0.lock();
        let mut st = c.prepare(
            "SELECT sync_id, created_at, mode, raw_text, text, app, window_title, language, duration_ms, latency_ms, words, status, edited_text
             FROM history WHERE archived=0 AND status<>'cancelled' ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = st.query_map(params![limit], |r| {
            Ok((
                r.get::<_, String>(0)?,
                HistoryItem {
                    created_at: r.get(1)?,
                    mode: r.get(2)?,
                    raw_text: r.get(3)?,
                    text: r.get(4)?,
                    app: r.get(5)?,
                    window_title: r.get(6)?,
                    language: r.get(7)?,
                    duration_ms: r.get(8)?,
                    latency_ms: r.get(9)?,
                    words: r.get(10)?,
                    status: r.get(11)?,
                    edited_text: r.get(12)?,
                    ..Default::default()
                },
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn sync_put_history(&self, sid: &str, h: &HistoryItem) -> Result<()> {
        let c = self.0.lock();
        let n = c.execute(
            "UPDATE history SET text=?2, raw_text=?3, edited_text=?4, status=?5, words=?6 WHERE sync_id=?1",
            params![sid, h.text, h.raw_text, h.edited_text, h.status, h.words],
        )?;
        if n == 0 {
            c.execute(
                "INSERT INTO history (created_at, mode, raw_text, text, app, window_title, url, language, duration_ms, latency_ms, words, status, edited_text, sync_id)
                 VALUES (?1,?2,?3,?4,?5,?6,'',?7,?8,?9,?10,?11,?12,?13)",
                params![h.created_at, h.mode, h.raw_text, h.text, h.app, h.window_title, h.language, h.duration_ms, h.latency_ms, h.words, h.status, h.edited_text, sid],
            )?;
        }
        Ok(())
    }

    // ---------- stats ----------
    pub fn stats(&self) -> Result<Stats> {
        let c = self.0.lock();
        let mut s = Stats::default();
        let (w, n, dur): (i64, i64, i64) = c.query_row(
            "SELECT COALESCE(SUM(words),0), COUNT(*), COALESCE(SUM(duration_ms),0) FROM history WHERE status IN ('pasted','copied')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        s.total_words = w;
        s.total_dictations = n;
        s.avg_wpm = if dur > 0 { w as f64 / (dur as f64 / 60000.0) } else { 0.0 };
        // Typing at ~40 wpm versus speaking.
        s.minutes_saved = (w as f64 / 40.0 - dur as f64 / 60000.0).max(0.0);
        let week = (chrono::Local::now() - chrono::Duration::days(7)).to_rfc3339();
        s.words_this_week = c.query_row(
            "SELECT COALESCE(SUM(words),0) FROM history WHERE created_at >= ?1 AND status IN ('pasted','copied')",
            params![week],
            |r| r.get(0),
        )?;
        let mut st = c.prepare(
            "SELECT app, SUM(words) w FROM history WHERE status IN ('pasted','copied') AND app<>'' GROUP BY app ORDER BY w DESC LIMIT 8",
        )?;
        s.top_apps = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.filter_map(|r| r.ok()).collect();
        let mut st = c.prepare(
            "SELECT substr(created_at,1,10) d, SUM(words) FROM history WHERE status IN ('pasted','copied') GROUP BY d ORDER BY d DESC LIMIT 120",
        )?;
        s.daily = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.filter_map(|r| r.ok()).collect();
        // Streak: consecutive days ending today or yesterday.
        let days: std::collections::HashSet<String> = s.daily.iter().map(|d| d.0.clone()).collect();
        let mut day = chrono::Local::now().date_naive();
        if !days.contains(&day.to_string()) {
            day = day.pred_opt().unwrap();
        }
        while days.contains(&day.to_string()) {
            s.streak_days += 1;
            day = day.pred_opt().unwrap();
        }
        Ok(s)
    }
}

pub fn count_words(s: &str) -> usize {
    s.split_whitespace().filter(|w| w.chars().any(|c| c.is_alphanumeric())).count()
}
