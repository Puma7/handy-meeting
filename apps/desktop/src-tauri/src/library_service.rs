//! Derived search jobs do not own capture, ASR workers or original transcript writers.
use meeting_core::{
    library::{LibraryIndex, SearchPage},
    metadata, SessionData,
};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Progress {
    pub job_id: String,
    pub kind: String,
    pub status: String,
    pub confirmed_tasks: u64,
    pub total_tasks: u64,
    pub elapsed_ms: u64,
    pub skipped: u64,
    pub error: Option<String>,
    pub cancel_requested: bool,
}
#[derive(Default)]
pub(super) struct Jobs {
    pub state: Mutex<Option<Progress>>,
    cancel: Mutex<Option<Arc<AtomicBool>>>,
    started: Mutex<Option<(String, Instant)>>,
}
impl Jobs {
    pub fn update_work(&self, id: &str, done: u64, total: u64) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(p) = state
                .as_mut()
                .filter(|p| p.job_id == id && p.status == "running")
            {
                p.confirmed_tasks = done;
                p.total_tasks = total;
            }
        }
    }
    pub fn start_work<F>(self: &Arc<Self>, kind: &str, work: F) -> Result<String, String>
    where
        F: FnOnce(&str, &Arc<Self>, &Arc<AtomicBool>) -> meeting_core::Result<()> + Send + 'static,
    {
        let id = meeting_notes::new_job_id();
        let cancel = Arc::new(AtomicBool::new(false));
        let started = Instant::now();
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "Maintenance state unavailable")?;
            if state.as_ref().is_some_and(|p| p.status == "running") {
                return Err("Another maintenance job is already running".into());
            }
            *state = Some(Progress {
                job_id: id.clone(),
                kind: kind.into(),
                status: "running".into(),
                confirmed_tasks: 0,
                total_tasks: 0,
                elapsed_ms: 0,
                skipped: 0,
                error: None,
                cancel_requested: false,
            });
            *self
                .cancel
                .lock()
                .map_err(|_| "Maintenance cancellation unavailable")? = Some(cancel.clone());
        }
        *self
            .started
            .lock()
            .map_err(|_| "Maintenance clock unavailable")? = Some((id.clone(), started));
        let jobs = self.clone();
        let owned = id.clone();
        let spawn=std::thread::Builder::new().name("meeting-maintenance".into()).spawn(move||{

            let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||work(&owned,&jobs,&cancel))).unwrap_or_else(|_|Err("Maintenance stopped unexpectedly. Review the retained operation log before continuing.".into()));
            if let Ok(mut state)=jobs.state.lock(){if let Some(p)=state.as_mut().filter(|p|p.job_id==owned){p.elapsed_ms=started.elapsed().as_millis()as u64;p.status=if result.is_ok(){"complete"}else if cancel.load(Ordering::Acquire){"cancelled"}else{"failed"}.into();p.error=result.err().map(|e|e.to_string());}}
        });
        if spawn.is_err() {
            if let Ok(mut state) = self.state.lock() {
                if let Some(p) = state.as_mut().filter(|p| p.job_id == id) {
                    p.status = "failed".into();
                    p.error = Some("Maintenance could not start".into());
                }
            }
            return Err("Maintenance worker could not start".into());
        }
        Ok(id)
    }
    pub fn snapshot(&self) -> Option<Progress> {
        let mut view = self.state.lock().ok()?.clone()?;
        if view.status == "running" {
            if let Some((id, start)) = self
                .started
                .lock()
                .ok()?
                .as_ref()
                .filter(|(id, _)| id == &view.job_id)
            {
                let _ = id;
                view.elapsed_ms = start.elapsed().as_millis() as u64;
            }
        }
        Some(view)
    }
    pub fn cancel(&self, id: &str) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if let Some(p) = state
            .as_mut()
            .filter(|p| p.job_id == id && p.status == "running")
        {
            if let Ok(flag) = self.cancel.lock() {
                if let Some(flag) = flag.as_ref() {
                    flag.store(true, Ordering::SeqCst);
                    p.cancel_requested = true;
                    return true;
                }
            }
        }
        false
    }
    pub fn start_scan(self: &Arc<Self>, root: PathBuf) -> Result<String, String> {
        let id = meeting_notes::new_job_id();
        let cancel = Arc::new(AtomicBool::new(false));
        let started = Instant::now();
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "Search job state is unavailable")?;
            if state.as_ref().is_some_and(|p| p.status == "running") {
                return Err("The search index is already being updated".into());
            }
            *state = Some(Progress {
                job_id: id.clone(),
                kind: "search-index".into(),
                status: "running".into(),
                confirmed_tasks: 0,
                total_tasks: 0,
                elapsed_ms: 0,
                skipped: 0,
                error: None,
                cancel_requested: false,
            });
            *self
                .cancel
                .lock()
                .map_err(|_| "Search cancellation unavailable")? = Some(cancel.clone());
        }
        *self
            .started
            .lock()
            .map_err(|_| "Search clock unavailable")? = Some((id.clone(), started));
        let jobs = self.clone();
        let owned = id.clone();
        let spawn = std::thread::Builder::new().name("meeting-search-index".into()).spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> meeting_core::Result<()> {
                let sessions = root.join("sessions");
                let sessions = metadata::checked_directory(&sessions)?;
                let total = fs::read_dir(&sessions)?.count() as u64;
                let mut index = LibraryIndex::open(&root)?;
                let mut entries=fs::read_dir(&sessions)?;
                let mut number=0u64;
                loop {
                    let batch:Vec<_>=entries.by_ref().take(500).collect();
                    if batch.is_empty(){break;}
                    let count=batch.len() as u64;
                    let mut skipped=0;
                    index.transaction_batch(|index| {
                      for entry in batch {
                        if cancel.load(Ordering::SeqCst) {
                          return Err("Search indexing cancelled; existing recordings and the last indexed results are preserved".into());
                        }
                        let one = (|| -> meeting_core::Result<()> {
                        let entry = entry?;
                        let id = entry.file_name().to_string_lossy().into_owned();
                        if !metadata::valid_id(&id) {
                            return Ok(());
                        }
                        let path = entry.path();
                        if !path.is_dir() {
                            return Ok(());
                        }
                        let fingerprint = fingerprint(&path, &root, &id)?;
                        if index.current_fingerprint(&id)?.as_deref() == Some(fingerprint.as_str())
                        {
                            return Ok(());
                        }
                        let data = match meeting_core::archive::read_archived_transcript(&path)?{Some(data)=>data,None=>meeting_core::inspect_manifest(&path)?};
                        let meta = metadata::read(&root, &id)?;
                        index.upsert(&data, &meta, &fingerprint)
                        })();
                        if one.is_err(){skipped+=1;}
                      }
                      Ok(())
                    })?;
                    number+=count;
                    if let Ok(mut state) = jobs.state.lock() {
                        if let Some(p) = state.as_mut().filter(|p| p.job_id == owned) {
                            p.total_tasks = total.max(number);
                            p.confirmed_tasks = number;
                            p.elapsed_ms = started.elapsed().as_millis() as u64;
                            p.skipped += skipped;
                        }
                    }
                }
                index.prune_missing_sessions(&sessions,&cancel)?;
                if let Ok(mut state)=jobs.state.lock(){if let Some(p)=state.as_mut().filter(|p|p.job_id==owned){p.total_tasks=p.confirmed_tasks;}}
                Ok(())
            })).unwrap_or_else(|_|Err("Search worker stopped unexpectedly; original sessions are preserved".into()));
            if let Ok(mut state) = jobs.state.lock() {
                if let Some(p) = state.as_mut().filter(|p| p.job_id == owned) {
                    p.elapsed_ms = started.elapsed().as_millis() as u64;
                    p.status = if result.is_ok() {
                        "complete"
                    } else if cancel.load(Ordering::SeqCst) {
                        "cancelled"
                    } else {
                        "failed"
                    }
                    .into();
                    p.error=result.err().map(|_|"Search index could not be fully updated. Original sessions are unchanged. / Der Suchindex konnte nicht vollständig aktualisiert werden. Originale bleiben erhalten.".into());
                }
            }
        });
        if spawn.is_err() {
            if let Ok(mut state) = self.state.lock() {
                if let Some(p) = state.as_mut().filter(|p| p.job_id == id) {
                    p.status = "failed".into();
                    p.error=Some("Search indexing could not start. Original sessions are preserved. / Indexierung konnte nicht gestartet werden; Originale bleiben erhalten.".into());
                }
            }
            return Err("The local search worker could not start".into());
        }
        Ok(id)
    }
}
fn fingerprint(folder: &Path, root: &Path, id: &str) -> meeting_core::Result<String> {
    let mut result = String::new();
    for path in [
        folder.join("session.json"),
        folder.join("session.json.bak"),
        folder.join("audio-archive.json"),
        root.join("user-metadata").join(format!("{id}.json")),
    ] {
        match fs::metadata(&path) {
            Ok(meta) => result.push_str(&format!("{}:{:?};", meta.len(), meta.modified()?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => result.push_str("missing;"),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(result)
}
pub(super) fn search(
    root: &Path,
    query: &str,
    offset: u64,
    limit: u32,
) -> Result<SearchPage, String> {
    LibraryIndex::open(root).and_then(|index|index.search(query,offset,limit)).map_err(|_|"Search is unavailable. Update the local index; original sessions are preserved. / Suche nicht verfügbar. Bitte den lokalen Suchindex aktualisieren.".into())
}
pub(super) fn update(root: &Path, data: &SessionData) -> Result<(), String> {
    let mut index = LibraryIndex::open(root).map_err(|_| "The search index is unavailable")?;
    let meta = metadata::read(root, &data.id)
        .map_err(|_| "Saved meeting labels could not be read; originals are preserved")?;
    index
        .upsert(data, &meta, "explicit-current-session")
        .map_err(|_| "The search index could not be updated".into())
}
#[cfg(test)]
mod clock_tests {
    use super::*;
    #[test]
    fn maintenance_clock_advances_then_freezes_and_terminal_failure_keeps_confirmed_work() {
        let jobs = Arc::new(Jobs::default());
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        jobs.start_work("fixture", move |id, jobs, _| {
            jobs.update_work(id, 1, 3);
            ready_tx.send(()).unwrap();
            finish_rx.recv().unwrap();
            Err("Synthetic failure after one confirmed step".into())
        })
        .unwrap();
        ready_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let before = jobs.snapshot().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let running = jobs.snapshot().unwrap();
        assert!(running.elapsed_ms > before.elapsed_ms);
        finish_tx.send(()).unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        while jobs.snapshot().unwrap().status == "running" {
            assert!(Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let done = jobs.snapshot().unwrap();
        assert_eq!(done.status, "failed");
        assert_eq!(done.confirmed_tasks, 1);
        assert_eq!(done.total_tasks, 3);
        assert!(done.elapsed_ms >= running.elapsed_ms);
        assert!(done.error.unwrap().contains("Synthetic failure"));
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(jobs.snapshot().unwrap().elapsed_ms, done.elapsed_ms);
    }
}
