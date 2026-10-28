//! Local session reads, explicit legacy imports and derived library history.
use super::*;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

impl Service {
    /// Resolve known legacy references or validated Rust identities into snapshots; reuse owned
    /// stores and verified archive markers, rejecting folder/manifest identity mismatches.
    pub(crate) fn data(&self, id: &str) -> AppResult<SessionData> {
        if self
            .maintenance_jobs
            .snapshot()
            .is_some_and(|p| p.status == "running" && p.kind != "cleanup-preview")
            && self.lock()?.transitioning
        {
            return Err(
                "Archive maintenance is running; wait or cancel before opening this meeting".into(),
            );
        }
        if valid_legacy_id(id) {
            let path = self
                .lock()?
                .legacy
                .get(id)
                .cloned()
                .ok_or("Dieser Importverweis ist nicht bekannt.")?;
            let mut data=meeting_core::read_legacy(&path).map_err(|_|"Die ursprüngliche Sitzung konnte nicht gelesen werden. Originaldateien bleiben unverändert.")?;
            data.id = id.into();
            return Ok(data);
        }
        validate_id(id)?;
        {
            let inner = self.lock()?;
            for store in inner
                .active
                .as_ref()
                .map(|a| &a.store)
                .into_iter()
                .chain(inner.view.iter())
            {
                let data = store
                    .snapshot()
                    .map_err(|_| "Sitzungsstand konnte nicht gelesen werden.")?;
                if data.id == id {
                    return Ok(data);
                }
            }
        }
        let folder = self.root.join("sessions").join(id);
        if let Some(data)=meeting_core::archive::read_archived_transcript(&folder).map_err(|_|"Der archivierte Transkriptstand konnte nicht gegen seine Originalquellen geprüft werden. Vorhandene Dateien bleiben erhalten. / Archived transcript provenance check failed; original files are preserved.")? { return Ok(data); }
        let store=SessionStore::open(&folder).map_err(|_|"Sitzung konnte nicht geöffnet werden. Originaldateien bleiben erhalten; eine andere Instanz könnte sie verwenden.")?;
        let data = store
            .snapshot()
            .map_err(|_| "Sitzungsstand konnte nicht gelesen werden.")?;
        if data.id != id {
            return Err("Sitzungskennung und Ordner stimmen nicht überein. Die Originaldateien bleiben erhalten.".into());
        }
        self.lock()?.view = Some(store);
        Ok(data)
    }
    /// Hold mutation admission while verifying native manifest approval and reading a legacy
    /// original. Persist only its reference and derived identity; the capture stays read-only.
    pub(crate) fn import(&self, manifest: String) -> AppResult<MeetingView> {
        let _lease = self.reserve_operation(operations::Kind::Mutation)?;
        let path = PathBuf::from(manifest);
        self.selections
            .verify(path_selection::Purpose::Legacy, &path)?;
        let mut data=meeting_core::read_legacy(&path).map_err(|_|"Bitte eine gültige session.json der bisherigen Version auswählen. Originaldateien werden ausschließlich gelesen.")?;
        let path = path
            .canonicalize()
            .map_err(|_| "Originalsitzung wurde nicht gefunden.")?;
        let mut hash = DefaultHasher::new();
        path.hash(&mut hash);
        let id = format!("legacy-{:016x}", hash.finish());
        let mapping = self.root.join("legacy-map").join(format!("{id}.json"));
        if !mapping.exists() {
            create_json(&mapping, &path)?;
        }
        self.lock()?.legacy.insert(id.clone(), path);
        data.id = id;
        Ok(self.meeting_view(data, false))
    }
    /// Build summaries from a completed index or session folders, merging current and known
    /// legacy sessions; omit full transcript parts from the returned list.
    pub(crate) fn history(&self) -> AppResult<Vec<MeetingView>> {
        // Only an index this process has fully scanned is complete; while a scan runs, failed
        // or was cancelled, the session folders themselves are the history.
        let index_complete = self.root.join("library-index/library.sqlite3").exists()
            && self
                .library_jobs
                .snapshot()
                .is_some_and(|p| p.status == "complete");
        let mut sessions = if index_complete {
            let mut all = Vec::new();
            loop {
                let page = library_service::search(&self.root, "", all.len() as u64, 100)?;
                let count = page.items.len();
                for item in page.items {
                    all.push(item.session_summary().map_err(|_| {
                        "The local history index is invalid; originals are preserved"
                    })?);
                }
                if count == 0 || all.len() as u64 >= page.total {
                    break all;
                }
            }
        } else {
            meeting_core::list_sessions(&self.root.join("sessions"))
                .map_err(|_| "Sitzungshistorie konnte nicht gelesen werden.")?
        };
        let (current, legacy) = {
            let inner = self.lock()?;
            (
                inner
                    .active
                    .as_ref()
                    .map(|a| a.store.clone())
                    .or(inner.view.clone()),
                inner.legacy.clone(),
            )
        };
        if let Some(current) = current {
            if let Ok(data) = current.snapshot() {
                sessions.retain(|old| old.id != data.id);
                sessions.push(data);
            }
        }
        for (id, path) in legacy {
            if let Ok(mut data) = meeting_core::read_legacy(&path) {
                data.id = id;
                sessions.push(data);
            }
        }
        sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        Ok(sessions
            .into_iter()
            .map(|mut data| {
                data.parts.clear();
                data.warnings.clear();
                self.meeting_view(data, false)
            })
            .collect())
    }
}
