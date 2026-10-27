//! Rebuildable local SQLite/FTS index. Original session files remain authoritative.
use crate::{metadata, storage::reject_link, Result, SessionData};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::Duration};

pub struct LibraryIndex {
    db: Connection,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub id: String,
    pub title: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub status: String,
    pub model_id: String,
    pub pinned: bool,
    pub audio_removed: bool,
    pub metadata_revision: u64,
    pub source_id: Option<String>,
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub excerpt: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub items: Vec<SearchHit>,
    pub total: u64,
    pub offset: u64,
    pub limit: u32,
}
impl SearchHit {
    pub fn session_summary(&self) -> Result<SessionData> {
        Ok(serde_json::from_value(
            serde_json::json!({"schemaVersion":2,"id":self.id,"title":self.title,
          "startedAt":self.started_at,"durationMs":self.duration_ms,"status":self.status,"modelId":self.model_id,
          "parts":[],"warnings":[],"readOnly":self.audio_removed}),
        )?)
    }
}

impl LibraryIndex {
    /// Opens the index for a user-requested rebuild. A database SQLite cannot read, or one
    /// that fails its integrity checks, is moved aside, never deleted, and a fresh index is created in its place. Returns the
    /// folder that now holds the unreadable files.
    pub fn open_for_rebuild(root: &Path) -> Result<(Self, Option<std::path::PathBuf>)> {
        match Self::open(root) {
            // A readable header can still hide damaged tables or FTS postings, and
            // unchanged sessions are skipped by the scan, so check before reuse.
            // The damaged connection is closed before its files are moved.
            Ok(index) => {
                if !index.damaged()? {
                    return Ok((index, None));
                }
            }
            Err(error) if unreadable_database(error.as_ref()) => (),
            Err(error) => return Err(error),
        }
        let aside = set_aside_unreadable(root)?;
        Ok((Self::open(root)?, Some(aside)))
    }
    /// SQLite's quick check plus the FTS5 integrity check. Only corruption reports `true`.
    fn damaged(&self) -> Result<bool> {
        let quick: String = match self.db.query_row("PRAGMA quick_check", [], |r| r.get(0)) {
            Ok(value) => value,
            Err(error) if unreadable_database(&error) => return Ok(true),
            Err(error) => return Err(error.into()),
        };
        if quick != "ok" {
            return Ok(true);
        }
        match self.db.execute(
            "INSERT INTO transcript_search(transcript_search) VALUES('integrity-check')",
            [],
        ) {
            Ok(_) => Ok(false),
            Err(error) if unreadable_database(&error) => Ok(true),
            Err(error) => Err(error.into()),
        }
    }
    pub fn open(root: &Path) -> Result<Self> {
        metadata::checked_directory(root)?;
        let folder = root.join("library-index");
        match fs::create_dir(&folder) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.into()),
        }
        metadata::checked_directory(&folder)?;
        let path = folder.join("library.sqlite3");
        for name in [
            "library.sqlite3",
            "library.sqlite3-wal",
            "library.sqlite3-shm",
        ] {
            let file = folder.join(name);
            if file.exists() {
                reject_link(&file)?;
            }
        }
        let db = Connection::open(path)?;
        db.busy_timeout(Duration::from_secs(3))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
          CREATE TABLE IF NOT EXISTS meetings (
            id TEXT PRIMARY KEY, title TEXT NOT NULL, started TEXT NOT NULL, duration INTEGER NOT NULL,
            status TEXT NOT NULL, model TEXT NOT NULL, pinned INTEGER NOT NULL, audio_removed INTEGER NOT NULL,
            revision INTEGER NOT NULL, fingerprint TEXT NOT NULL);
          CREATE INDEX IF NOT EXISTS meetings_time ON meetings(pinned DESC,started DESC,id);
          CREATE VIRTUAL TABLE IF NOT EXISTS transcript_search USING fts5(
            meeting_id UNINDEXED, source_id UNINDEXED, start_ms UNINDEXED, end_ms UNINDEXED,
            title, body, tokenize='unicode61 remove_diacritics 2');")?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS search_rows(row_id INTEGER PRIMARY KEY, meeting_id TEXT NOT NULL);
          CREATE INDEX IF NOT EXISTS search_rows_meeting ON search_rows(meeting_id);")?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 2 {
            db.execute_batch(
                "BEGIN IMMEDIATE;
              INSERT OR IGNORE INTO search_rows SELECT rowid,meeting_id FROM transcript_search;
              PRAGMA user_version=2; COMMIT;",
            )?;
        }
        Ok(Self { db })
    }
    pub fn current_fingerprint(&self, id: &str) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .db
            .query_row("SELECT fingerprint FROM meetings WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .optional()?)
    }
    /// Remove only derived rows for truly missing native session folders.
    /// Unreadable paths and imported legacy sessions remain indexed.
    pub fn prune_missing_sessions(
        &mut self,
        sessions: &Path,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<u64> {
        metadata::checked_directory(sessions)?;
        let ids: Vec<String> = {
            let mut statement = self.db.prepare("SELECT id FROM meetings LIMIT 1000001")?;
            let rows = statement.query_map([], |row| row.get(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        if ids.len() > 1000000 {
            return Err("Search index exceeds safe pruning bound".into());
        }
        let mut removed = 0;
        self.transaction_batch(|db|{
            for id in &ids{
                if cancel.load(std::sync::atomic::Ordering::Acquire){return Err("Search pruning cancelled; original sessions unchanged".into());}
                if !metadata::valid_id(id)||id.starts_with("legacy-"){continue;}
                if matches!(fs::symlink_metadata(sessions.join(id)),Err(e) if e.kind()==std::io::ErrorKind::NotFound){db.remove(id)?;removed+=1;}
            }Ok(())
        })?;
        Ok(removed)
    }
    pub fn upsert(
        &mut self,
        data: &SessionData,
        meta: &metadata::Metadata,
        fingerprint: &str,
    ) -> Result<()> {
        if !metadata::valid_id(&data.id)
            || data.duration_ms > i64::MAX as u64
            || meta.revision > i64::MAX as u64
        {
            return Err("Invalid index document identity".into());
        }
        let title = meta.title.as_deref().unwrap_or(&data.title);
        let tx = self.db.savepoint()?;
        tx.execute(
            "DELETE FROM transcript_search WHERE rowid IN (SELECT row_id FROM search_rows WHERE meeting_id=?1)",
            [&data.id],
        )?;
        tx.execute("DELETE FROM search_rows WHERE meeting_id=?1", [&data.id])?;
        tx.execute("INSERT INTO meetings VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
          ON CONFLICT(id) DO UPDATE SET title=excluded.title,started=excluded.started,duration=excluded.duration,
          status=excluded.status,model=excluded.model,pinned=excluded.pinned,audio_removed=excluded.audio_removed,
          revision=excluded.revision,fingerprint=excluded.fingerprint",
          params![data.id,title,data.started_at,data.duration_ms as i64,data.status,data.model_id,meta.pinned,meta.audio_removed||(data.read_only&&!data.id.starts_with("legacy-")),meta.revision as i64,fingerprint])?;
        {
            let mut insert=tx.prepare_cached("INSERT INTO transcript_search(meeting_id,source_id,start_ms,end_ms,title,body) VALUES(?1,?2,?3,?4,?5,?6)")?;
            insert.execute(params![data.id, "", 0, 0, title, ""])?;
            tx.execute(
                "INSERT INTO search_rows VALUES(?1,?2)",
                params![tx.last_insert_rowid(), data.id],
            )?;
            for part in &data.parts {
                let Some(text) = part.text.as_deref().filter(|s| !s.trim().is_empty()) else {
                    continue;
                };
                let start = part.start_sample.saturating_mul(1000) / u64::from(crate::SAMPLE_RATE);
                let end = part
                    .start_sample
                    .saturating_add(part.confirmed_sample_count)
                    .saturating_mul(1000)
                    / u64::from(crate::SAMPLE_RATE);
                if start > i64::MAX as u64 || end > i64::MAX as u64 {
                    return Err("Index section time overflow".into());
                }
                insert.execute(params![
                    data.id,
                    part.file,
                    start as i64,
                    end as i64,
                    "",
                    text
                ])?;
                tx.execute(
                    "INSERT INTO search_rows VALUES(?1,?2)",
                    params![tx.last_insert_rowid(), data.id],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn remove(&mut self, id: &str) -> Result<()> {
        let tx = self.db.savepoint()?;
        tx.execute("DELETE FROM transcript_search WHERE rowid IN (SELECT row_id FROM search_rows WHERE meeting_id=?1)", [id])?;
        tx.execute("DELETE FROM search_rows WHERE meeting_id=?1", [id])?;
        tx.execute("DELETE FROM meetings WHERE id=?1", [id])?;
        tx.commit()?;
        Ok(())
    }
    pub fn search(&self, query: &str, offset: u64, limit: u32) -> Result<SearchPage> {
        if query.chars().count() > 200 || !(1..=100).contains(&limit) || offset > i64::MAX as u64 {
            return Err("Invalid search range".into());
        }
        let expression = query
            .split_whitespace()
            // A token without letters or digits becomes an empty FTS phrase that matches nothing.
            .filter(|word| word.chars().any(char::is_alphanumeric))
            .take(20)
            .map(|word| format!("\"{}\"*", word.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" AND ");
        let (total, items) = if expression.is_empty() {
            let total = self
                .db
                .query_row("SELECT count(*) FROM meetings", [], |r| r.get::<_, i64>(0))?;
            let mut stmt=self.db.prepare("SELECT id,title,started,duration,status,model,pinned,audio_removed,revision,NULL,NULL,NULL,'' FROM meetings ORDER BY pinned DESC,started DESC,id LIMIT ?1 OFFSET ?2")?;
            let rows = stmt
                .query_map(params![limit, offset as i64], hit)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            (total, rows)
        } else {
            let total = self.db.query_row(
                "SELECT count(*) FROM transcript_search WHERE transcript_search MATCH ?1",
                [&expression],
                |r| r.get::<_, i64>(0),
            )?;
            let mut stmt=self.db.prepare("SELECT m.id,m.title,m.started,m.duration,m.status,m.model,m.pinned,m.audio_removed,m.revision,
              NULLIF(s.source_id,''),s.start_ms,s.end_ms,snippet(transcript_search,5,'','', ' … ',30)
              FROM transcript_search s JOIN meetings m ON m.id=s.meeting_id
              WHERE transcript_search MATCH ?1 ORDER BY m.pinned DESC,m.started DESC,m.id,s.rowid LIMIT ?2 OFFSET ?3")?;
            let rows = stmt
                .query_map(params![expression, limit, offset as i64], hit)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            (total, rows)
        };
        Ok(SearchPage {
            items,
            total: total as u64,
            offset,
            limit,
        })
    }
    pub fn transaction_batch(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        // Explicit batches make the 100k benchmark representative of one indexing run.
        self.db.execute_batch("BEGIN IMMEDIATE")?;
        let result = operation(self);
        if result.is_ok() {
            self.db.execute_batch("COMMIT")?;
        } else {
            self.db.execute_batch("ROLLBACK")?;
        }
        result
    }
}
fn hit(row: &rusqlite::Row<'_>) -> rusqlite::Result<SearchHit> {
    Ok(SearchHit {
        id: row.get(0)?,
        title: row.get(1)?,
        started_at: row.get(2)?,
        duration_ms: row.get::<_, i64>(3)? as u64,
        status: row.get(4)?,
        model_id: row.get(5)?,
        pinned: row.get(6)?,
        audio_removed: row.get(7)?,
        metadata_revision: row.get::<_, i64>(8)? as u64,
        source_id: row.get(9)?,
        start_ms: row.get::<_, Option<i64>>(10)?.map(|v| v as u64),
        end_ms: row.get::<_, Option<i64>>(11)?.map(|v| v as u64),
        excerpt: row.get(12)?,
    })
}

fn unreadable_database(error: &(dyn std::error::Error + 'static)) -> bool {
    error.downcast_ref::<rusqlite::Error>().is_some_and(|e| {
        matches!(
            e.sqlite_error_code(),
            Some(rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt)
        )
    })
}
/// Only regular index files are moved; a link or a missing folder stops the recovery.
fn set_aside_unreadable(root: &Path) -> Result<std::path::PathBuf> {
    let folder = metadata::checked_directory(&root.join("library-index"))?;
    let aside = folder.join(format!(
        "unreadable-{}-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir(&aside)?;
    for name in [
        "library.sqlite3",
        "library.sqlite3-wal",
        "library.sqlite3-shm",
    ] {
        let file = folder.join(name);
        match fs::symlink_metadata(&file) {
            Ok(meta) if meta.file_type().is_file() => fs::rename(&file, aside.join(name))?,
            Ok(_) => return Err("Search index file is not a regular file".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(aside)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pruning_removes_only_missing_native_rows_and_cancellation_preserves_existing_index() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path().join("sessions");
        fs::create_dir(&sessions).unwrap();
        let mut index = LibraryIndex::open(root.path()).unwrap();
        let absent = data("0123456789abcdef0123456789abcdef");
        let present = data("1123456789abcdef0123456789abcdef");
        fs::create_dir(sessions.join(&present.id)).unwrap();
        fs::write(
            sessions.join(&present.id).join("user-note.txt"),
            b"retained",
        )
        .unwrap();
        index
            .upsert(&absent, &metadata::Metadata::default(), "old")
            .unwrap();
        index
            .upsert(&present, &metadata::Metadata::default(), "present")
            .unwrap();
        assert!(index
            .prune_missing_sessions(&sessions, &std::sync::atomic::AtomicBool::new(true))
            .is_err());
        assert_eq!(index.search("", 0, 25).unwrap().total, 2);
        assert_eq!(
            index
                .prune_missing_sessions(&sessions, &std::sync::atomic::AtomicBool::new(false))
                .unwrap(),
            1
        );
        assert_eq!(index.search("", 0, 25).unwrap().items[0].id, present.id);
        assert_eq!(
            fs::read(sessions.join(present.id).join("user-note.txt")).unwrap(),
            b"retained"
        );
    }
    #[test]
    fn rebuild_moves_an_unreadable_index_aside_and_starts_fresh() {
        // Synthetic damage: the derived index file holds bytes SQLite cannot read.
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("library-index");
        fs::create_dir(&folder).unwrap();
        let damaged = b"not a sqlite database, synthetic damage ".repeat(200);
        fs::write(folder.join("library.sqlite3"), &damaged).unwrap();
        assert!(LibraryIndex::open(root.path()).is_err());
        let (mut index, aside) = LibraryIndex::open_for_rebuild(root.path()).unwrap();
        let aside = aside.expect("the unreadable index is set aside");
        // The root is canonical internally (/private/var on macOS, \\?\ on Windows).
        assert!(aside.starts_with(folder.canonicalize().unwrap()));
        assert_eq!(fs::read(aside.join("library.sqlite3")).unwrap(), damaged);
        let d = data("0123456789abcdef0123456789abcdef");
        index
            .upsert(&d, &metadata::Metadata::default(), "fixture")
            .unwrap();
        drop(index);
        assert_eq!(
            LibraryIndex::open(root.path())
                .unwrap()
                .search("Prüfbericht", 0, 20)
                .unwrap()
                .total,
            1
        );
        // A readable index is reused as it is.
        let (_, again) = LibraryIndex::open_for_rebuild(root.path()).unwrap();
        assert!(again.is_none());
    }
    #[test]
    fn rebuild_replaces_an_index_whose_fts_postings_are_damaged() {
        // Synthetic damage: the SQLite header is intact but FTS posting blocks are gone.
        let root = tempfile::tempdir().unwrap();
        let mut index = LibraryIndex::open(root.path()).unwrap();
        let d = data("0123456789abcdef0123456789abcdef");
        index
            .upsert(&d, &metadata::Metadata::default(), "fixture")
            .unwrap();
        drop(index);
        let path = root.path().join("library-index/library.sqlite3");
        let raw = Connection::open(&path).unwrap();
        assert!(
            raw.execute("DELETE FROM transcript_search_data WHERE id > 10", [])
                .unwrap()
                > 0
        );
        drop(raw);
        let reopened = LibraryIndex::open(root.path()).unwrap();
        assert!(reopened.damaged().unwrap());
        drop(reopened);
        let (index, aside) = LibraryIndex::open_for_rebuild(root.path()).unwrap();
        assert!(aside.unwrap().join("library.sqlite3").is_file());
        assert!(!index.damaged().unwrap());
        assert_eq!(index.search("", 0, 25).unwrap().total, 0);
    }
    #[cfg(unix)]
    #[test]
    fn rebuild_never_moves_a_linked_index() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("library-index");
        fs::create_dir(&folder).unwrap();
        let target = root.path().join("elsewhere.sqlite3");
        fs::write(&target, b"user file").unwrap();
        std::os::unix::fs::symlink(&target, folder.join("library.sqlite3")).unwrap();
        assert!(LibraryIndex::open_for_rebuild(root.path()).is_err());
        assert!(fs::symlink_metadata(folder.join("library.sqlite3"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&target).unwrap(), b"user file");
    }
    fn data(id: &str) -> SessionData {
        serde_json::from_value(serde_json::json!({"schemaVersion":2,"id":id,"title":"Original title","startedAt":"2026-10-27T06:00:00Z","status":"complete","modelId":"synthetic","parts":[{"file":"system-0.wav","source":"system","startSample":16000,"sampleCount":32000,"confirmedSampleCount":32000,"text":"Anna bestätigt den Prüfbericht am Freitag.","status":"complete"}]})).unwrap()
    }
    #[test]
    fn unicode_search_returns_original_section_times_and_literal_evidence() {
        let root = tempfile::tempdir().unwrap();
        let mut index = LibraryIndex::open(root.path()).unwrap();
        let d = data("0123456789abcdef0123456789abcdef");
        index
            .upsert(&d, &metadata::Metadata::default(), "fixture")
            .unwrap();
        let found = index.search("Prüfbericht", 0, 20).unwrap();
        assert_eq!(found.total, 1);
        let item = &found.items[0];
        assert_eq!(item.source_id.as_deref(), Some("system-0.wav"));
        assert_eq!(item.start_ms, Some(1000));
        assert_eq!(item.end_ms, Some(3000));
        assert!(item.excerpt.contains("Prüfbericht"));
        assert!(index
            .search("\" OR 1=1 --", 0, 20)
            .unwrap()
            .items
            .is_empty());
        assert!(index.search("Anna", 0, 101).is_err());
        // Standalone punctuation such as a dash must not empty the whole AND query.
        assert_eq!(index.search("Prüfbericht – /", 0, 20).unwrap().total, 1);
    }
    #[test]
    fn renamed_pinned_metadata_is_searchable_without_changing_the_source() {
        let root = tempfile::tempdir().unwrap();
        let mut index = LibraryIndex::open(root.path()).unwrap();
        let d = data("0123456789abcdef0123456789abcdef");
        let original = serde_json::to_vec(&d).unwrap();
        let meta = metadata::Metadata {
            revision: 1,
            title: Some("Renamed priority".into()),
            pinned: true,
            audio_removed: false,
        };
        index.upsert(&d, &meta, "changed").unwrap();
        let found = index.search("priority", 0, 20).unwrap();
        assert!(found.items[0].pinned);
        assert_eq!(found.items[0].metadata_revision, 1);
        assert_eq!(serde_json::to_vec(&d).unwrap(), original);
        index.remove(&d.id).unwrap();
        assert_eq!(index.search("", 0, 20).unwrap().total, 0);
    }
    #[test]
    fn failed_batch_rolls_back_search_and_labels_without_harming_prior_documents() {
        let root = tempfile::tempdir().unwrap();
        let mut index = LibraryIndex::open(root.path()).unwrap();
        let first = data("0123456789abcdef0123456789abcdef");
        let second = data("1123456789abcdef0123456789abcdef");
        index
            .upsert(&first, &metadata::Metadata::default(), "prior")
            .unwrap();
        let result = index.transaction_batch(|db| {
            db.remove(&first.id)?;
            db.upsert(&second, &metadata::Metadata::default(), "pending")?;
            Err("Synthetic indexing interruption".into())
        });
        assert!(result.is_err());
        let page = index.search("Prüfbericht", 0, 20).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, first.id);
        index
            .transaction_batch(|db| db.upsert(&second, &metadata::Metadata::default(), "confirmed"))
            .unwrap();
        assert_eq!(index.search("Prüfbericht", 0, 20).unwrap().total, 2);
    }
}
