//! Rebuildable local SQLite/FTS index. Original session files remain authoritative.
use crate::{metadata, storage::reject_link, Result, SessionData};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::Duration};

/// Version 3 adds the meeting title as `context` to every section row (issue #22).
const SCHEMA_VERSION: i64 = 3;
/// Row layout: one meeting row (`source_id` empty, `title` set, `body` and `context` empty) and
/// one row per transcript section (`title` empty, `body` = section text, `context` = title).
const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS meetings (
    id TEXT PRIMARY KEY, title TEXT NOT NULL, started TEXT NOT NULL, duration INTEGER NOT NULL,
    status TEXT NOT NULL, model TEXT NOT NULL, pinned INTEGER NOT NULL, audio_removed INTEGER NOT NULL,
    revision INTEGER NOT NULL, fingerprint TEXT NOT NULL);
  CREATE INDEX IF NOT EXISTS meetings_time ON meetings(pinned DESC,started DESC,id);
  CREATE VIRTUAL TABLE IF NOT EXISTS transcript_search USING fts5(
    meeting_id UNINDEXED, source_id UNINDEXED, start_ms UNINDEXED, end_ms UNINDEXED,
    title, body, context, tokenize='unicode61 remove_diacritics 2');
  CREATE TABLE IF NOT EXISTS search_rows(row_id INTEGER PRIMARY KEY, meeting_id TEXT NOT NULL);
  CREATE INDEX IF NOT EXISTS search_rows_meeting ON search_rows(meeting_id);";

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
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version != SCHEMA_VERSION {
            Self::upgrade(&db)?;
        }
        Ok(Self { db })
    }
    /// The index is derived from the session folders. A version-2 index is converted row by
    /// row (title context added to section rows); any other version is rebuilt empty and
    /// refilled by the next scan, so an older or newer layout is never misread.
    fn upgrade(db: &Connection) -> Result<()> {
        db.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| -> Result<()> {
            let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
            if version == SCHEMA_VERSION {
                return Ok(()); // Another process finished the upgrade first.
            }
            let fts_exists: bool = db.query_row(
                "SELECT count(*)>0 FROM sqlite_master WHERE name='transcript_search'",
                [],
                |r| r.get(0),
            )?;
            if version == 2 && fts_exists {
                db.execute_batch("ALTER TABLE transcript_search RENAME TO transcript_search_v2;")?;
                db.execute_batch(SCHEMA)?;
                db.execute_batch(
                    "INSERT INTO transcript_search(rowid,meeting_id,source_id,start_ms,end_ms,title,body,context)
                       SELECT s.rowid,s.meeting_id,s.source_id,s.start_ms,s.end_ms,s.title,s.body,
                         CASE WHEN s.source_id='' THEN '' ELSE coalesce((SELECT m.title FROM meetings m WHERE m.id=s.meeting_id),'') END
                       FROM transcript_search_v2 s;
                     DROP TABLE transcript_search_v2;",
                )?;
            } else {
                db.execute_batch(
                    "DROP TABLE IF EXISTS transcript_search; DROP TABLE IF EXISTS search_rows;
                     DROP TABLE IF EXISTS meetings;",
                )?;
                db.execute_batch(SCHEMA)?;
            }
            db.execute_batch(&format!("PRAGMA user_version={SCHEMA_VERSION};"))?;
            Ok(())
        })();
        match result {
            Ok(()) => db.execute_batch("COMMIT")?,
            Err(e) => {
                db.execute_batch("ROLLBACK")?;
                return Err(e);
            }
        }
        Ok(())
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
            let mut insert=tx.prepare_cached("INSERT INTO transcript_search(meeting_id,source_id,start_ms,end_ms,title,body,context) VALUES(?1,?2,?3,?4,?5,?6,?7)")?;
            insert.execute(params![data.id, "", 0, 0, title, "", ""])?;
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
                    text,
                    title
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
        let terms = query
            .split_whitespace()
            // A token without letters or digits becomes an empty FTS phrase that matches nothing.
            .filter(|word| word.chars().any(char::is_alphanumeric))
            .take(20)
            .map(|word| format!("\"{}\"*", word.replace('"', "\"\"")))
            .collect::<Vec<_>>();
        let expression = match_expression(&terms);
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
/// Every term must occur in the row. A meeting row matches when all terms are in its title.
/// A section row matches when all terms are in its text or meeting title and at least one is
/// in its text, so a title word plus a transcript word finds that section, while a title-only
/// query still yields the single meeting-level hit instead of one hit per section.
fn match_expression(terms: &[String]) -> String {
    if terms.is_empty() {
        return String::new();
    }
    let join = |column: &str, operator: &str| {
        terms
            .iter()
            .map(|term| format!("{column} : {term}"))
            .collect::<Vec<_>>()
            .join(operator)
    };
    format!(
        "({}) OR (({}) AND ({}))",
        join("title", " AND "),
        join("{body context}", " AND "),
        join("body", " OR ")
    )
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
    fn two_sections(id: &str) -> SessionData {
        serde_json::from_value(serde_json::json!({"schemaVersion":2,"id":id,"title":"Quartalsplanung Vertrieb","startedAt":"2026-10-27T06:00:00Z","status":"complete","modelId":"synthetic","parts":[
            {"file":"system-0.wav","source":"system","startSample":16000,"sampleCount":32000,"confirmedSampleCount":32000,"text":"Anna bestätigt den Prüfbericht am Freitag.","status":"complete"},
            {"file":"system-1.wav","source":"system","startSample":64000,"sampleCount":32000,"confirmedSampleCount":16000,"text":"Ben fragt nach dem Budget.","status":"complete"}]})).unwrap()
    }
    #[test]
    fn query_mixing_title_and_transcript_words_finds_the_matching_section_only() {
        let root = tempfile::tempdir().unwrap();
        let mut index = LibraryIndex::open(root.path()).unwrap();
        let d = two_sections("0123456789abcdef0123456789abcdef");
        index
            .upsert(&d, &metadata::Metadata::default(), "fixture")
            .unwrap();
        // Regression for #22: title and section text used to sit in different rows.
        let mixed = index.search("Quartalsplanung Prüfbericht", 0, 20).unwrap();
        assert_eq!(mixed.total, 1);
        assert_eq!(mixed.items[0].source_id.as_deref(), Some("system-0.wav"));
        assert_eq!(mixed.items[0].start_ms, Some(1000));
        assert_eq!(mixed.items[0].end_ms, Some(3000));
        assert!(mixed.items[0].excerpt.contains("Prüfbericht"));
        let second = index.search("vertrieb budget", 0, 20).unwrap();
        assert_eq!(second.total, 1);
        assert_eq!(second.items[0].source_id.as_deref(), Some("system-1.wav"));
        assert_eq!(second.items[0].start_ms, Some(4000));
        assert_eq!(second.items[0].end_ms, Some(5000));
        // A title-only query stays one meeting-level hit, not one hit per section.
        let title = index.search("Quartalsplanung", 0, 20).unwrap();
        assert_eq!(title.total, 1);
        assert_eq!(title.items[0].source_id, None);
        // Words from two different sections are not combined into one hit.
        assert_eq!(index.search("Prüfbericht Budget", 0, 20).unwrap().total, 0);
        assert_eq!(
            index
                .search("Quartalsplanung Unbekannt", 0, 20)
                .unwrap()
                .total,
            0
        );
        // A renamed title replaces the original one in every section row.
        let meta = metadata::Metadata {
            revision: 1,
            title: Some("Umbenannt".into()),
            pinned: false,
            audio_removed: false,
        };
        index.upsert(&d, &meta, "renamed").unwrap();
        assert_eq!(index.search("Umbenannt Budget", 0, 20).unwrap().total, 1);
        assert_eq!(
            index.search("Quartalsplanung Budget", 0, 20).unwrap().total,
            0
        );
        assert_eq!(index.search("", 0, 20).unwrap().total, 1);
        let rows: i64 = index
            .db
            .query_row("SELECT count(*) FROM search_rows", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 3, "one meeting row and one row per section");
    }
    #[test]
    fn version_two_index_is_converted_and_unknown_versions_are_rebuilt() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("library-index");
        fs::create_dir(&folder).unwrap();
        let id = "0123456789abcdef0123456789abcdef";
        {
            // Layout written by HandyMeeting up to 1.0.2 (schema version 2).
            let db = Connection::open(folder.join("library.sqlite3")).unwrap();
            db.execute_batch("CREATE TABLE meetings (
                id TEXT PRIMARY KEY, title TEXT NOT NULL, started TEXT NOT NULL, duration INTEGER NOT NULL,
                status TEXT NOT NULL, model TEXT NOT NULL, pinned INTEGER NOT NULL, audio_removed INTEGER NOT NULL,
                revision INTEGER NOT NULL, fingerprint TEXT NOT NULL);
              CREATE VIRTUAL TABLE transcript_search USING fts5(
                meeting_id UNINDEXED, source_id UNINDEXED, start_ms UNINDEXED, end_ms UNINDEXED,
                title, body, tokenize='unicode61 remove_diacritics 2');
              CREATE TABLE search_rows(row_id INTEGER PRIMARY KEY, meeting_id TEXT NOT NULL);
              PRAGMA user_version=2;").unwrap();
            db.execute(
                "INSERT INTO meetings VALUES(?1,'Quartalsplanung','2026-10-27T06:00:00Z',3000,'complete','synthetic',0,0,0,'v2')",
                [id],
            )
            .unwrap();
            for (source, start, end, title, body) in [
                ("", 0, 0, "Quartalsplanung", ""),
                (
                    "system-0.wav",
                    1000,
                    3000,
                    "",
                    "Anna bestätigt den Prüfbericht.",
                ),
            ] {
                db.execute(
                    "INSERT INTO transcript_search(meeting_id,source_id,start_ms,end_ms,title,body) VALUES(?1,?2,?3,?4,?5,?6)",
                    params![id, source, start, end, title, body],
                )
                .unwrap();
                db.execute(
                    "INSERT INTO search_rows VALUES(?1,?2)",
                    params![db.last_insert_rowid(), id],
                )
                .unwrap();
            }
        }
        let mut index = LibraryIndex::open(root.path()).unwrap();
        let version: i64 = index
            .db
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert_eq!(
            index.current_fingerprint(id).unwrap().as_deref(),
            Some("v2")
        );
        let found = index.search("Quartalsplanung Prüfbericht", 0, 20).unwrap();
        assert_eq!(found.total, 1);
        assert_eq!(found.items[0].source_id.as_deref(), Some("system-0.wav"));
        assert_eq!(found.items[0].start_ms, Some(1000));
        assert_eq!(index.search("Quartalsplanung", 0, 20).unwrap().total, 1);
        // Converted rows stay attributable, so a later update replaces them instead of duplicating.
        index
            .upsert(&data(id), &metadata::Metadata::default(), "fresh")
            .unwrap();
        assert_eq!(index.search("Prüfbericht", 0, 20).unwrap().total, 1);
        assert_eq!(index.search("Quartalsplanung", 0, 20).unwrap().total, 0);
        drop(index);
        // A layout this version does not know is rebuilt, never misread.
        Connection::open(folder.join("library.sqlite3"))
            .unwrap()
            .execute_batch("PRAGMA user_version=99;")
            .unwrap();
        let mut index = LibraryIndex::open(root.path()).unwrap();
        assert_eq!(index.search("", 0, 20).unwrap().total, 0);
        assert_eq!(index.current_fingerprint(id).unwrap(), None);
        index
            .upsert(&data(id), &metadata::Metadata::default(), "rebuilt")
            .unwrap();
        assert_eq!(
            index.search("Original Prüfbericht", 0, 20).unwrap().total,
            1
        );
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
