mod agent;
mod asr;
mod credential_commands;
mod credential_service;
mod library_commands;
mod library_service;
mod meeting_files;
mod name_reference;
mod notes_commands;
mod notes_service;
mod retention_commands;
mod speaker_commands;
mod speaker_jobs;
mod template_commands;
mod update_commands;
mod updater;

use handy_bridge::{HandyBridge, ModelCatalog};
use meeting_core::{AudioDevice, CaptureController, CaptureOptions, SessionData, SessionStore};
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    fs::{self, OpenOptions},
    hash::{Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    thread::JoinHandle,
};
use tauri::{Emitter, Manager, State};
type AppResult<T> = Result<T, String>;
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelView {
    id: String,
    name: String,
    is_downloaded: bool,
    supports_streaming: bool,
    compatibility_error: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HandyStatus {
    executable: Option<String>,
    version: Option<String>,
    models: Vec<ModelView>,
    error: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedAudio {
    system: u64,
    microphone: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MeetingView {
    #[serde(flatten)]
    data: SessionData,
    live_preview_limited: bool,
    pending_audio_ms: u64,
    saved_audio_ms: SavedAudio,
    user_metadata: meeting_core::metadata::Metadata,
    metadata_error: Option<String>,
}
fn meeting_view(mut data: SessionData, preview: bool) -> MeetingView {
    let (mut system, mut microphone, mut pending_system, mut pending_microphone) = (0, 0, 0, 0);
    for p in &data.parts {
        let end = (p.start_sample + p.confirmed_sample_count) * 1000
            / u64::from(meeting_core::SAMPLE_RATE);
        let pending = if p.status == "complete" && p.model_id.as_deref() == Some(&data.model_id) {
            0
        } else if p.model_id.as_deref() == Some(&data.model_id) {
            p.confirmed_sample_count.saturating_sub(p.processed_samples)
        } else {
            p.confirmed_sample_count
        };
        if p.source == "system" {
            system = system.max(end);
            pending_system += pending;
        } else if p.source == "microphone" {
            microphone = microphone.max(end);
            pending_microphone += pending;
        }
    }
    data.parts
        .sort_by(|a, b| (a.start_sample, &a.source).cmp(&(b.start_sample, &b.source)));
    let limited = preview && data.parts.len() > 16;
    if limited {
        data.parts.drain(..data.parts.len() - 16);
    }
    MeetingView {
        data,
        live_preview_limited: limited,
        pending_audio_ms: pending_system.max(pending_microphone) * 1000
            / u64::from(meeting_core::SAMPLE_RATE),
        saved_audio_ms: SavedAudio { system, microphone },
        user_metadata: Default::default(),
        metadata_error: None,
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppView {
    version: &'static str,
    handy: Option<HandyStatus>,
    active_meeting: Option<MeetingView>,
    recording: bool,
    error: Option<String>,
    data_directory: String,
    notes_running: bool,
    transitioning: bool,
    agent: Option<agent::View>,
}
struct Active {
    owner: Option<String>,
    store: SessionStore,
    capture: Option<CaptureController>,
    stop: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}
#[derive(Default)]
struct Inner {
    handy: Option<HandyStatus>,
    executable: Option<PathBuf>,
    active: Option<Active>,
    view: Option<SessionStore>,
    legacy: HashMap<String, PathBuf>,
    error: Option<String>,
    transitioning: bool,
}
struct Service {
    agent: agent::Controller,
    root: PathBuf,
    inner: Mutex<Inner>,
    transition: Mutex<()>,
    refresh_gate: Mutex<()>,
    notes_jobs: notes_service::NotesJobs,
    library_jobs: Arc<library_service::Jobs>,
    updater: updater::Updater,
    speaker_jobs: Arc<speaker_jobs::Jobs>,
    maintenance_jobs: Arc<library_service::Jobs>,
}
impl Service {
    fn meeting_view(&self, data: SessionData, preview: bool) -> MeetingView {
        let mut view = meeting_view(data, preview);
        if view.data.read_only && !view.data.id.starts_with("legacy-") {
            view.saved_audio_ms = SavedAudio {
                system: 0,
                microphone: 0,
            };
            view.pending_audio_ms = 0;
        }
        match meeting_core::metadata::read(&self.root,&view.data.id) {
            Ok(mut meta) => {
                if let Some(title) = &meta.title { view.data.title = title.clone(); }
                if view.data.read_only && !view.data.id.starts_with("legacy-") {
                    meta.audio_removed = true;
                }
                view.user_metadata = meta;
            },
            Err(_)=>view.metadata_error=Some("Saved meeting labels could not be read. Originals are preserved. / Gespeicherte Meetingbezeichnungen konnten nicht gelesen werden.".into()),
        }
        view
    }
    fn new(root: PathBuf) -> AppResult<Self> {
        fs::create_dir_all(root.join("sessions"))
            .map_err(|_| "Der lokale Sitzungsordner konnte nicht angelegt werden.")?;
        fs::create_dir_all(root.join("legacy-map"))
            .map_err(|_| "Der lokale Importordner konnte nicht angelegt werden.")?;
        let mut inner = Inner::default();
        for entry in fs::read_dir(root.join("legacy-map"))
            .map_err(|_| "Importverweise konnten nicht gelesen werden.")?
            .flatten()
        {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let (Some(id), Ok(meta)) = (
                    path.file_stem().and_then(|s| s.to_str()),
                    fs::metadata(&path),
                ) {
                    if valid_legacy_id(id) && meta.len() <= 32768 {
                        if let Ok(bytes) = fs::read(&path) {
                            if let Ok(manifest) = serde_json::from_slice::<PathBuf>(&bytes) {
                                inner.legacy.insert(id.to_owned(), manifest);
                            }
                        }
                    }
                }
            }
        }
        Ok(Self {
            agent: agent::Controller::default(),
            root,
            inner: Mutex::new(inner),
            transition: Mutex::new(()),
            refresh_gate: Mutex::new(()),
            notes_jobs: notes_service::NotesJobs::default(),
            library_jobs: Arc::new(library_service::Jobs::default()),
            updater: updater::Updater::default(),
            speaker_jobs: Arc::new(speaker_jobs::Jobs::default()),
            maintenance_jobs: Arc::new(library_service::Jobs::default()),
        })
    }
    fn lock(&self) -> AppResult<MutexGuard<'_, Inner>> {
        self.inner.lock().map_err(|_| {
            "Der Anwendungszustand ist gesperrt. Gesicherte Sitzungen bleiben erhalten.".into()
        })
    }
    fn state(&self) -> AppResult<AppView> {
        let inner = self.lock()?;
        let store = inner
            .active
            .as_ref()
            .map(|a| &a.store)
            .or(inner.view.as_ref());
        let active_meeting = if let Some(store) = store {
            let data = store
                .preview_snapshot(16)
                .map_err(|_| "Sitzungsstand konnte nicht gelesen werden.")?;
            let progress = store
                .progress()
                .map_err(|_| "Sicherungsstand konnte nicht gelesen werden.")?;
            let mut view = self.meeting_view(data, false);
            view.live_preview_limited = progress.total_parts > 16;
            view.pending_audio_ms = progress.pending_ms;
            view.saved_audio_ms = SavedAudio {
                system: progress.system_confirmed_ms,
                microphone: progress.microphone_confirmed_ms,
            };
            Some(view)
        } else {
            None
        };
        let recording = inner.active.as_ref().is_some_and(|a| a.capture.is_some());
        let capture = inner.active.as_ref().and_then(|a| a.capture.as_ref());
        let capture_failed = capture.is_some_and(|c| !c.is_running());
        let stopped_tracks = capture.map(|c| c.stopped_tracks()).unwrap_or_default();
        let error = if capture_failed {
            Some("Die Audioaufnahme wurde unterbrochen. Bereits gesicherte Daten bleiben erhalten; bitte das Meeting beenden und das Audiogerät prüfen.".into())
        } else if !stopped_tracks.is_empty() {
            let names = stopped_tracks
                .iter()
                .map(|s| {
                    if *s == "microphone" {
                        "Mikrofon / microphone"
                    } else {
                        "Systemaudio / system audio"
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            Some(format!("Eine Audiospur nimmt nicht mehr auf: {names}. Die übrige Spur läuft weiter, gesicherte Daten bleiben erhalten; bitte das Gerät prüfen. / One audio track stopped recording; the other track continues and saved audio is retained."))
        } else {
            inner.error.clone()
        };
        Ok(AppView {
            version: VERSION,
            handy: inner.handy.clone(),
            active_meeting,
            recording,
            error,
            data_directory: self.root.to_string_lossy().into_owned(),
            notes_running: self.notes_jobs.running(),
            transitioning: inner.transitioning,
            agent: self.agent.view()?,
        })
    }
    fn refresh(
        &self,
        explicit: Option<String>,
    ) -> AppResult<(HandyBridge, ModelCatalog, HandyStatus)> {
        let _refresh = self
            .refresh_gate
            .lock()
            .map_err(|_| "Die Handy-Modellabfrage ist gesperrt.")?;
        let explicitly_selected = explicit.is_some();
        let path = explicit
            .map(PathBuf::from)
            .or(self.lock()?.executable.clone());
        let attempt = (|| {
            // A chosen path is recorded below either way, but only a Handy program is started.
            if path.as_deref().is_some_and(|p| !plausible_handy_path(p)) {
                return Err("Bitte die Handy-Programmdatei auswählen (handy, Handy.exe, Handy.app oder eine Handy-AppImage). / Choose the Handy program itself.".to_string());
            }
            let bridge = HandyBridge::discover(path.as_deref()).map_err(|e| e.to_string())?;
            let catalog = bridge.refresh().map_err(|e| e.to_string())?;
            let models = catalog
                .models
                .iter()
                .map(|m| ModelView {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    is_downloaded: m.is_downloaded && !m.is_downloading,
                    supports_streaming: m.supports_streaming,
                    compatibility_error: if m.ready_for_streaming() {
                        bridge
                            .resolve_streaming_model(m)
                            .err()
                            .map(|e| e.to_string())
                    } else if m.is_downloading {
                        Some("Das Modell wird in Handy noch heruntergeladen.".into())
                    } else {
                        None
                    },
                })
                .collect();
            let status = HandyStatus {
                executable: Some(bridge.executable().to_string_lossy().into_owned()),
                version: handy_bridge::installed_version(bridge.executable()),
                models,
                error: None,
            };
            Ok::<_, String>((bridge, catalog, status))
        })();
        let mut inner = self.lock()?;
        if explicitly_selected {
            // An invalid newly selected installation must not quietly fall
            // back to a previously selected Handy executable on the next run.
            inner.executable = path.clone();
        }
        match &attempt {
            Ok((bridge, _, status)) => {
                inner.executable = Some(bridge.executable().to_owned());
                inner.handy = Some(status.clone());
                inner.error = None;
            }
            Err(error) => {
                inner.handy = Some(HandyStatus {
                    executable: path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    version: None,
                    models: vec![],
                    error: Some(error.clone()),
                });
            }
        }
        attempt
    }
    fn start(&self, request: StartRequest) -> AppResult<MeetingView> {
        self.start_for(request, None)
    }
    fn start_for(&self, request: StartRequest, owner: Option<String>) -> AppResult<MeetingView> {
        let _gate = self
            .transition
            .lock()
            .map_err(|_| "Aufnahmesteuerung ist gesperrt.")?;
        self.reap_finished()?;
        if let Some(owner) = owner.as_deref() {
            self.agent.start_permitted(owner)?;
        }
        {
            let mut inner = self.lock()?;
            if inner.active.is_some() || inner.transitioning {
                return Err(
                    "Es läuft bereits eine Aufnahme oder Wiederaufnahme. Bitte zuerst abschließen."
                        .into(),
                );
            }
            inner.transitioning = true;
        }
        let result = self.start_inner(request, owner);
        let mut inner = self.lock()?;
        inner.transitioning = false;
        if let Err(error) = &result {
            inner.error = Some(error.clone());
        }
        result
    }
    fn start_inner(&self, request: StartRequest, owner: Option<String>) -> AppResult<MeetingView> {
        if request.title.chars().count() > 200 || request.model_id.len() > 4096 {
            return Err("Titel oder Modellkennung ist zu lang.".into());
        }
        let (bridge, catalog, _) = self.refresh(None)?;
        let model = catalog
            .model(&request.model_id)
            .map_err(|e| e.to_string())?;
        let resolved = bridge
            .resolve_streaming_model(model)
            .map_err(|e| e.to_string())?;
        let devices = meeting_core::list_devices()
            .map_err(|_| "Audiogeräte konnten nicht gelesen werden. Geräteverbindung prüfen.")?;
        let system = devices
            .iter()
            .find(|d| d.source == "system" && d.id == request.system_device_id)
            .ok_or("Bitte eine verfügbare Computerton-Ausgabe auswählen.")?;
        let microphone = request
            .microphone_device_id
            .as_ref()
            .map(|id| {
                devices
                    .iter()
                    .find(|d| d.source == "microphone" && &d.id == id)
                    .ok_or("Das gewählte Mikrofon ist nicht verfügbar.")
            })
            .transpose()?;
        let backend = backend_directory()?;
        let store=SessionStore::create(&self.root.join("sessions"),&request.title,&model.id,&system.name,microphone.map(|m|m.name.as_str())).map_err(|_|"Die Sitzung konnte nicht angelegt werden. Freien Speicherplatz und Ordnerrechte prüfen.")?;
        let provenance = serde_json::json!({"applicationVersion":VERSION,"handyInstallation":catalog.installation,"catalogFingerprint":catalog.fingerprint_sha256,"catalogObservedAtUnixMs":catalog.fetched_at_unix_ms,"model":model,"resolvedModel":resolved,"nativeRuntime":handy_bridge::native::runtime_identity(),"capabilityPolicy":"metadata-before-capture; actual-streaming-capability-in-isolated-worker","trackInference":"one-isolated-model-per-track"});
        handy_bridge::write_model_snapshot(store.folder(), model, &resolved, &provenance)
            .map_err(|error| error.to_string())?;
        // Begin durable audio before any model load. Worker failures never own capture.
        let capture = self.agent.capture_if_permitted(owner.as_deref(), || {
            CaptureController::start(store.clone(), CaptureOptions {
                output_id: Some(system.id.clone()),
                microphone_id: microphone.map(|m|m.id.clone()),
                include_microphone: microphone.is_some(),
            }).map_err(|_| {
                let _=store.warn("Aufnahme konnte nicht vollständig gestartet werden. Gesicherte Daten bleiben erhalten; Audiogerät und Berechtigungen prüfen.");
                "Audioaufnahme konnte nicht gestartet werden. Audiogerät und Berechtigungen prüfen.".to_string()
            })
        })?;
        let stop = Arc::new(AtomicBool::new(false));
        let mut workers = vec![asr::spawn_track(
            store.clone(),
            resolved.path.clone(),
            backend.clone(),
            "system".into(),
            stop.clone(),
        )];
        if microphone.is_some() {
            workers.push(asr::spawn_track(
                store.clone(),
                resolved.path,
                backend,
                "microphone".into(),
                stop.clone(),
            ));
        }
        let data = store
            .preview_snapshot(16)
            .map_err(|_| "Sitzungsstand nicht lesbar.")?;
        let mut inner = self.lock()?;
        inner.error = None;
        inner.active = Some(Active {
            owner,
            store,
            capture: Some(capture),
            stop,
            workers,
        });
        Ok(self.meeting_view(data, true))
    }
    fn stop(&self) -> AppResult<MeetingView> {
        self.stop_for(None, None)
    }
    fn stop_for(&self, owner: Option<&str>, session: Option<&str>) -> AppResult<MeetingView> {
        let _gate = self
            .transition
            .lock()
            .map_err(|_| "Aufnahmesteuerung ist gesperrt.")?;
        let mut active = {
            let mut inner = self.lock()?;
            if owner.is_some()
                && inner
                    .active
                    .as_ref()
                    .is_none_or(|active| active.owner.as_deref() != owner)
            {
                return Err("This agent does not own the active recording".into());
            }
            if session.is_some()
                && !inner.active.as_ref().is_some_and(|active| {
                    active
                        .store
                        .preview_snapshot(0)
                        .is_ok_and(|data| Some(data.id.as_str()) == session)
                })
            {
                return Err("The requested recording is no longer active".into());
            }
            inner.transitioning = true;
            match inner.active.take() {
                Some(active) => active,
                None => {
                    inner.transitioning = false;
                    return Err("Es läuft keine Aufnahme oder Wiederaufnahme.".into());
                }
            }
        };
        self.lock()?.view = Some(active.store.clone());
        let capture_result = active
            .capture
            .as_mut()
            .map(CaptureController::stop)
            .transpose();
        active.capture = None;
        if active.owner.is_some() {
            self.agent.capture_ended();
        }
        active.stop.store(true, Ordering::Release);
        // Audio is final once capture has stopped. Recognition of the remaining sections can take
        // minutes on slow hardware, so it continues as a capture-less catch-up (like `resume`)
        // instead of holding the transition; `reap_finished` collects it later.
        let catching_up = active.workers.iter().any(|worker| !worker.is_finished());
        if catching_up {
            if capture_result.is_ok() {
                let _ = active.store.set_status("catching-up");
            }
        } else {
            for worker in active.workers.drain(..) {
                if worker.join().is_err() {
                    let _=active.store.notice("Ein Erkennungsprozess wurde unterbrochen. Offene Abschnitte können später nachgeholt werden.");
                }
            }
            if let Ok(data) = active.store.snapshot() {
                if library_service::update(&self.root, &data).is_err() {
                    let _ = active.store.notice("Die Aufnahme ist gespeichert; bitte den lokalen Suchindex unter Einstellungen aktualisieren. / Recording saved; refresh the local search index in Settings.");
                }
            }
        }
        let result: AppResult<MeetingView> = if capture_result.is_err() {
            Err("Die Aufnahme wurde mit einer Sicherungswarnung beendet. Bitte den Sitzungsstand prüfen.".into())
        } else {
            active
                .store
                .snapshot()
                .map(|d| self.meeting_view(d, false))
                .map_err(|_| "Der gespeicherte Sitzungsstand konnte nicht gelesen werden.".into())
        };
        let mut inner = self.lock()?;
        if catching_up {
            inner.active = Some(active);
        }
        inner.transitioning = false;
        if let Err(error) = &result {
            inner.error = Some(error.clone());
        }
        result
    }
    fn reap_finished(&self) -> AppResult<()> {
        let finished = {
            let mut inner = self.lock()?;
            if inner.active.as_ref().is_some_and(|a| {
                a.capture.is_none() && a.workers.iter().all(JoinHandle::is_finished)
            }) {
                inner.active.take()
            } else {
                None
            }
        };
        if let Some(mut active) = finished {
            for worker in active.workers.drain(..) {
                let _ = worker.join();
            }
            self.lock()?.view = Some(active.store.clone());
            Self::finalize_completed_recognition(&active.store)?;
            if let Ok(data) = active.store.snapshot() {
                let _ = library_service::update(&self.root, &data);
            }
        }
        Ok(())
    }
    // A host can stop after the last durable final text checkpoint but before
    // its worker updates the global status. No model call is needed to finish
    // that already-confirmed state, and failed/open/foreign-model parts must
    // not be hidden by a vacuously empty worker list.
    fn finalize_completed_recognition(store: &SessionStore) -> AppResult<bool> {
        let status = store
            .preview_snapshot(0)
            .map_err(|_| "Sitzungsstand konnte nicht gelesen werden.")?
            .status;
        if !["recorded", "catching-up", "complete"].contains(&status.as_str())
            || !store
                .recognition_complete()
                .map_err(|_| "Erkennungsabschluss konnte nicht geprüft werden.")?
        {
            return Ok(false);
        }
        if status != "complete" {
            store.set_status("complete").map_err(|_| "Der bestätigte Erkennungsabschluss konnte nicht gesichert werden. Vorhandene Audio- und Textstände bleiben erhalten.")?;
        }
        Ok(true)
    }
    fn data(&self, id: &str) -> AppResult<SessionData> {
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
    fn resume(&self, id: String) -> AppResult<MeetingView> {
        let _gate = self
            .transition
            .lock()
            .map_err(|_| "Aufnahmesteuerung ist gesperrt.")?;
        self.reap_finished()?;
        let unavailable = {
            let inner = self.lock()?;
            inner.active.is_some() || inner.transitioning
        };
        if unavailable {
            return Err("Bitte zuerst die laufende Sitzung abschließen.".into());
        }
        if valid_legacy_id(&id) {
            return Err("Originalsitzungen werden in dieser Beta ausschließlich gelesen. Zum Nachholen die bisherige Version verwenden.".into());
        }
        let data = self.data(&id)?;
        if data.read_only {
            return Err("Audio ist lokal archiviert. Bitte zuerst Audio wiederherstellen oder eine neue Transkription aus einer Archivkopie anlegen. / Audio is archived locally. Restore audio or create a new transcription from an archive copy first.".into());
        }
        let store = self
            .lock()?
            .view
            .as_ref()
            .filter(|s| s.preview_snapshot(0).is_ok_and(|current| current.id == id))
            .cloned()
            .ok_or("Sitzung konnte nicht wiederaufgenommen werden.")?;
        if Self::finalize_completed_recognition(&store)? {
            return Ok(self.meeting_view(
                store
                    .snapshot()
                    .map_err(|_| "Sitzungsstand konnte nicht gelesen werden.")?,
                false,
            ));
        }
        let (_bridge, catalog, _) = self.refresh(None)?;
        let model=catalog.model(&data.model_id).map_err(|_|"Das ursprüngliche Modell ist in Handy nicht verfügbar. Es wird kein anderes Modell eingesetzt.")?;
        let snapshot_folder = self.root.join("sessions").join(&id);
        let identity =
            handy_bridge::read_model_snapshot(&snapshot_folder).map_err(|e| e.to_string())?;
        handy_bridge::verify_catalog_model(&identity, model).map_err(|e| e.to_string())?;
        handy_bridge::verify_model_snapshot(&snapshot_folder, &identity.canonical_path)
            .map_err(|e| e.to_string())?;
        // Resume pins the actual weights captured at recording start. It never
        // resolves an updated Handy catalog entry to replacement weights.
        let pinned_model_path = identity.canonical_path;
        let backend = backend_directory()?;
        store
            .set_status("catching-up")
            .map_err(|_| "Sitzungsstatus konnte nicht gesichert werden.")?;
        let stop = Arc::new(AtomicBool::new(true));
        let workers = ["system", "microphone"]
            .into_iter()
            .filter(|source| {
                data.parts
                    .iter()
                    .any(|p| p.source == *source && p.status != "complete")
            })
            .map(|source| {
                asr::spawn_track(
                    store.clone(),
                    pinned_model_path.clone(),
                    backend.clone(),
                    source.into(),
                    stop.clone(),
                )
            })
            .collect();
        self.lock()?.active = Some(Active {
            owner: None,
            store: store.clone(),
            capture: None,
            stop,
            workers,
        });
        Ok(self.meeting_view(
            store
                .snapshot()
                .map_err(|_| "Sitzungsstand nicht lesbar.")?,
            false,
        ))
    }
    fn import(&self, manifest: String) -> AppResult<MeetingView> {
        let path = PathBuf::from(manifest);
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
    fn history(&self) -> AppResult<Vec<MeetingView>> {
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
    fn analyze(&self, request: NotesRequest) -> AppResult<meeting_notes::Analysis> {
        notes_service::analyze(self, request)
    }
}
fn note_sources(data: &SessionData) -> AppResult<Vec<meeting_notes::Source>> {
    let mut parts: Vec<_> = data.parts.iter().collect();
    parts.sort_by(|a, b| (a.start_sample, &a.source).cmp(&(b.start_sample, &b.source)));
    let mut sources = vec![];
    for part in parts {
        if part.status != "complete" {
            return Err("Unvollständige Transkriptabschnitte dürfen nicht als abgeschlossene Quelle ausgewertet werden.".into());
        }
        if let Some(text) = &part.text {
            let chars: Vec<_> = text.chars().collect();
            for chunk in chars.chunks(1600) {
                let value: String = chunk.iter().collect();
                if value.trim().is_empty() {
                    continue;
                }
                sources.push(meeting_notes::Source {
                    id: format!("S{:05}", sources.len() + 1),
                    source: part.source.clone(),
                    start_ms: part.start_sample * 1000 / u64::from(meeting_core::SAMPLE_RATE),
                    end_ms: (part.start_sample + part.sample_count) * 1000
                        / u64::from(meeting_core::SAMPLE_RATE),
                    text: value,
                });
            }
        }
    }
    if sources.is_empty() {
        return Err("Das abgeschlossene Transkript enthält noch keinen auswertbaren Text.".into());
    }
    Ok(sources)
}
fn validate_id(id: &str) -> AppResult<()> {
    if id.len() != 32 || !id.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Ungültige Sitzungskennung.".into());
    }
    Ok(())
}
/// Defence in depth for an executable path that arrives through IPC: only something named like
/// the Handy program (or its macOS bundle) is ever started for model discovery.
fn plausible_handy_path(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    name.starts_with("handy")
        && (name == "handy"
            || name.ends_with(".exe")
            || name.ends_with(".app")
            || name.ends_with(".appimage")
            || !name.contains('.'))
}
fn valid_legacy_id(id: &str) -> bool {
    id.strip_prefix("legacy-")
        .is_some_and(|s| s.len() == 16 && s.bytes().all(|c| c.is_ascii_hexdigit()))
}
fn create_json(path: &Path, value: &impl Serialize) -> AppResult<()> {
    let mut file=OpenOptions::new().create_new(true).write(true).open(path).map_err(|_|"Ein neuer Arbeitsstand konnte nicht angelegt werden. Vorhandene Dateien bleiben erhalten.")?;
    file.write_all(
        &serde_json::to_vec_pretty(value)
            .map_err(|_| "Arbeitsstand konnte nicht serialisiert werden.")?,
    )
    .and_then(|_| file.sync_all())
    .map_err(|_| "Arbeitsstand konnte nicht dauerhaft gesichert werden.".into())
}
fn backend_directory() -> AppResult<PathBuf> {
    if let Some(value) = std::env::var_os("HANDYMEETING_NATIVE_DIR") {
        let path = PathBuf::from(value);
        if path.is_dir() {
            return Ok(path);
        }
        return Err("Der konfigurierte Erkennungsordner wurde nicht gefunden.".into());
    }
    let executable = std::env::current_exe().map_err(|_| "Programmordner ist nicht verfügbar.")?;
    let directory = executable
        .parent()
        .ok_or("Programmordner ist nicht verfügbar.")?;
    if directory.join("native").is_dir() {
        Ok(directory.join("native"))
    } else {
        Ok(directory.to_owned())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StartRequest {
    title: String,
    model_id: String,
    system_device_id: String,
    microphone_device_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NotesRequest {
    #[serde(default)]
    use_saved_key: bool,
    #[serde(default)]
    job_id: Option<String>,
    meeting_id: String,
    template: meeting_notes::Template,
    endpoint: String,
    model: String,
    mode: String,
    cloud_consent: bool,
    api_key: String,
    #[serde(default)]
    resume_run_id: Option<String>,
}
async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|_| {
            "Der Arbeitsschritt wurde unterbrochen. Bereits gesicherte Daten bleiben erhalten."
                .to_string()
        })?
}
#[tauri::command]
async fn get_app_state(service: State<'_, Arc<Service>>) -> AppResult<AppView> {
    let service = Arc::clone(&service);
    blocking(move || {
        // A finished catch-up after stop or resume is collected here, so it never keeps
        // reporting active work. A failed collection is retried on the next poll.
        let _ = service.reap_finished();
        service.state()
    })
    .await
}
#[tauri::command]
async fn refresh_handy(
    service: State<'_, Arc<Service>>,
    executable: Option<String>,
) -> AppResult<HandyStatus> {
    let service = Arc::clone(&service);
    blocking(move || match service.refresh(executable) {
        Ok((_, _, status)) => Ok(status),
        Err(_) => service
            .lock()?
            .handy
            .clone()
            .ok_or("Handy konnte nicht gelesen werden.".into()),
    })
    .await
}
#[tauri::command]
async fn list_devices() -> AppResult<Vec<AudioDevice>> {
    blocking(|| {
        meeting_core::list_devices()
            .map_err(|e| format!("Audiogeräte konnten nicht aufgelistet werden: {e} / Audio devices could not be listed."))
    })
    .await
}
#[tauri::command]
async fn start_meeting(
    service: State<'_, Arc<Service>>,
    request: StartRequest,
) -> AppResult<MeetingView> {
    let service = Arc::clone(&service);
    blocking(move || service.start(request)).await
}
#[tauri::command]
async fn stop_meeting(service: State<'_, Arc<Service>>) -> AppResult<MeetingView> {
    let service = Arc::clone(&service);
    blocking(move || service.stop()).await
}
#[tauri::command]
async fn agent_decide_recording(
    service: State<'_, Arc<Service>>,
    request_id: String,
    approve: bool,
    request: Option<StartRequest>,
) -> AppResult<()> {
    let service = Arc::clone(&service);
    blocking(move || service.decide_agent(&request_id, approve, request)).await
}
#[tauri::command]
fn agent_revoke(service: State<'_, Arc<Service>>) -> AppResult<()> {
    service.agent.revoke();
    Ok(())
}
#[tauri::command]
async fn list_meetings(service: State<'_, Arc<Service>>) -> AppResult<Vec<MeetingView>> {
    let service = Arc::clone(&service);
    blocking(move || service.history()).await
}
#[tauri::command]
async fn get_meeting(service: State<'_, Arc<Service>>, id: String) -> AppResult<MeetingView> {
    let service = Arc::clone(&service);
    blocking(move || service.data(&id).map(|d| service.meeting_view(d, false))).await
}
#[tauri::command]
async fn resume_meeting(service: State<'_, Arc<Service>>, id: String) -> AppResult<MeetingView> {
    let service = Arc::clone(&service);
    blocking(move || service.resume(id)).await
}
#[tauri::command]
async fn import_legacy(
    service: State<'_, Arc<Service>>,
    manifest: String,
) -> AppResult<MeetingView> {
    let service = Arc::clone(&service);
    blocking(move || service.import(manifest)).await
}
#[tauri::command]
async fn copy_text(text: String) -> AppResult<()> {
    blocking(move || {
        if text.len() > 32 * 1024 * 1024 {
            return Err(
                "Der Text ist zum Kopieren zu groß. Bitte den gespeicherten Export verwenden."
                    .into(),
            );
        }
        let mut clipboard =
            arboard::Clipboard::new().map_err(|_| "Die Zwischenablage ist nicht verfügbar.")?;
        clipboard
            .set_text(text)
            .map_err(|_| "Text konnte nicht in die Zwischenablage kopiert werden.".into())
    })
    .await
}
#[tauri::command]
async fn open_meeting_folder(service: State<'_, Arc<Service>>, id: String) -> AppResult<()> {
    let service = Arc::clone(&service);
    blocking(move || {
        // Resolve through the known session identity, never a frontend-supplied path.
        service.data(&id)?;
        let folder = if valid_legacy_id(&id) {
            let manifest = service
                .lock()?
                .legacy
                .get(&id)
                .cloned()
                .ok_or("Unknown imported session")?;
            manifest
                .parent()
                .ok_or("Session folder missing")?
                .to_owned()
        } else {
            meeting_files::session_folder(&service.root, &id)?
        };
        meeting_files::open_folder(&folder)
    })
    .await
}
#[tauri::command]
async fn export_transcript(
    service: State<'_, Arc<Service>>,
    id: String,
    locale: String,
) -> AppResult<String> {
    let service = Arc::clone(&service);
    blocking(move || {
        let data = service.data(&id)?;
        if data.read_only {
            return Err("Diese Originalsitzung ist schreibgeschützt. Das Transkript kann weiterhin vollständig kopiert werden. / This original session is read-only; the full transcript can still be copied.".into());
        }
        let folder = meeting_files::session_folder(&service.root, &id)?;
        meeting_files::save_transcript(&folder, &data, &locale)
    }).await
}
#[tauri::command]
async fn generate_notes(
    service: State<'_, Arc<Service>>,
    request: NotesRequest,
) -> AppResult<meeting_notes::Analysis> {
    let service = Arc::clone(&service);
    blocking(move || service.analyze(request)).await
}
#[tauri::command]
async fn list_analyses(
    service: State<'_, Arc<Service>>,
    meeting_id: String,
) -> AppResult<Vec<meeting_notes::Analysis>> {
    let service = Arc::clone(&service);
    blocking(move||{if valid_legacy_id(&meeting_id){return Ok(vec![]);}validate_id(&meeting_id)?;let root=service.root.join("sessions").join(meeting_id).join("analyses");if !root.exists(){return Ok(vec![]);}let mut results=vec![];for entry in fs::read_dir(root).map_err(|_|"Gespeicherte Auswertungen konnten nicht gelesen werden.")?.flatten(){let path=entry.path().join("result.json");if path.exists(){let result=meeting_notes::read_analysis(&path).map_err(|_|"Eine gespeicherte Auswertung besteht die Quellenprüfung nicht. Originaldateien bleiben erhalten.")?;results.push(result);}}results.sort_by(|a,b|b.created_at.cmp(&a.created_at));Ok(results)}).await
}

pub fn run() {
    let root = std::env::var_os("HANDYMEETING_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir().map(|base| base.join("HandyMeeting/RustBeta")))
        .expect("Lokaler Anwendungsordner ist nicht verfügbar.");
    let service =
        Arc::new(Service::new(root).expect("Lokale Sitzungsablage konnte nicht geöffnet werden."));
    tauri::Builder::default().manage(service).setup(|app| {
        let service=Arc::clone(app.state::<Arc<Service>>().inner());
        let _=service.library_jobs.start_scan(service.root.clone());
        if std::env::args().skip(1).eq(["--agent", "serve"]) { agent::start_stdio(Arc::clone(&service)).map_err(std::io::Error::other)?; }
        tauri::async_runtime::spawn_blocking(move|| {let _=service.refresh(None);});
        Ok(())
    }).invoke_handler(tauri::generate_handler![get_app_state,retention_commands::list_audio_copies,retention_commands::transcribe_audio_copy,retention_commands::compact_archive,retention_commands::list_archives,retention_commands::list_unfinished_deletions,retention_commands::resume_permanent_deletion,retention_commands::restore_archive,retention_commands::apply_retention,retention_commands::get_retention_policy,retention_commands::save_retention_policy,retention_commands::preview_retention,retention_commands::get_retention_plan,retention_commands::get_maintenance_progress,retention_commands::cancel_maintenance,library_commands::edit_meeting_metadata,library_commands::search_meetings,library_commands::rebuild_search_index,library_commands::get_library_progress,library_commands::cancel_library_job,speaker_commands::get_speaker_labels,speaker_commands::save_speaker_labels,speaker_commands::start_speaker_experiment,speaker_commands::get_speaker_progress,speaker_commands::get_speaker_sections,speaker_commands::cancel_speaker_experiment,update_commands::check_for_update,update_commands::download_update,update_commands::get_update_progress,update_commands::cancel_update,update_commands::install_update,refresh_handy,list_devices,start_meeting,stop_meeting,agent_decide_recording,agent_revoke,list_meetings,get_meeting,resume_meeting,import_legacy,copy_text,open_meeting_folder,export_transcript,generate_notes,list_analyses,notes_commands::list_unfinished_analyses,notes_commands::list_text_models,notes_commands::get_notes_progress,notes_commands::cancel_notes,notes_commands::notes_markdown,template_commands::list_templates,template_commands::save_templates,template_commands::import_templates,credential_commands::credential_status,credential_commands::save_credential,credential_commands::remove_credential])
    .on_window_event(|window,event|{if let tauri::WindowEvent::CloseRequested{api,..}=event{let service=window.state::<Arc<Service>>();let blocked=service.lock().is_ok_and(|inner|inner.transitioning||inner.active.as_ref().is_some_and(|a|a.capture.is_some()));if blocked{api.prevent_close();if let Ok(mut inner)=service.lock(){inner.error=Some(if inner.transitioning { "Ein Vorgang läuft noch. Bitte warten, bis Audioabschluss und Verarbeitung beendet sind.".into() } else { "Bitte zuerst „Meeting beenden“ wählen. Audio wird gesichert und offene Abschnitte werden verarbeitet.".into() });}let _=window.emit("recording-close-blocked",());}}})
    .run(tauri::generate_context!()).expect("HandyMeeting konnte nicht gestartet werden.");
}
pub fn run_cli(args: &[String]) -> Option<AppResult<()>> {
    match args.first().map(String::as_str) {
        Some("--agent-protocol-check-stdio") => Some(if args.len() == 1 {
            agent::check_stdio()
        } else {
            Err("The synthetic protocol check accepts no arguments".into())
        }),
        Some("--credential-checks") => Some((|| {
            if args.len() != 2 {
                return Err("Exactly one new credential-check directory is required".into());
            }
            credential_service::native_checks(Path::new(&args[1]))
        })()),
        Some("--companion-capabilities") => Some((|| {
            let version = match &args[1..] {
                [] => 1,
                [flag, value] if flag == "--contract-version" => value
                    .parse::<u32>()
                    .map_err(|_| "Invalid contract version")?,
                _ => return Err("Use --companion-capabilities [--contract-version 1]".into()),
            };
            if version != 1 {
                return Err("Unsupported companion contract version".into());
            }
            let bridge = HandyBridge::discover(None).map_err(|error| error.to_string())?;
            let catalog = bridge.refresh().map_err(|error| error.to_string())?;
            let capabilities = handy_bridge::companion_capabilities(
                version,
                &catalog,
                handy_bridge::installed_version(bridge.executable()),
            )
            .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string(&capabilities).map_err(|_| "Capabilities unavailable")?
            );
            Ok(())
        })()),
        Some("--agent") if args == ["--agent", "serve"] => None,
        Some("--agent") => Some((|| {
            if args != ["--agent", "capabilities"] {
                return Err("Use --agent capabilities or --agent serve; recording requires visible in-app consent.".into());
            }
            println!("{}", agent::capabilities());
            Ok(())
        })()),
        Some("--asr-worker") => Some(asr::worker_main(&args[1..])),
        Some("--speaker-worker") => Some(speaker_jobs::worker_main(&args[1..])),
        Some("--synthetic-smoke") => Some(asr::synthetic_smoke(&args[1..])),
        Some("--list-text-models") => Some((|| {
            if args.len() != 2 {
                return Err("Genau eine lokale KI-Adresse wird benötigt.".into());
            }
            let models = meeting_notes::local_model_ids(&args[1]).map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&models)
                    .map_err(|_| "Modellliste konnte nicht ausgegeben werden.")?
            );
            Ok(())
        })()),
        Some("--speaker-name-reference-check") => Some(name_reference::check(&args[1..])),
        Some("--model-probe") => Some((|| {
            if args.len() != 3 {
                return Err("Invalid model probe arguments".into());
            }
            let _ = handy_bridge::native::NativeEngine::load(
                Path::new(&args[1]),
                true,
                Some(Path::new(&args[2])),
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })()),
        Some("--integration-checks") => Some(integration_checks(args.get(1).map(String::as_str))),
        Some("--release-checks") => {
            Some(application_checks(args.get(1).map(String::as_str), false))
        }
        _ => None,
    }
}
fn integration_checks(output: Option<&str>) -> AppResult<()> {
    application_checks(output, true)
}

/// Release mode never discovers Handy, enumerates devices, loads weights or
/// creates a window. Installed-metadata mode adds read-only local queries.
fn application_checks(output: Option<&str>, include_installed: bool) -> AppResult<()> {
    let output = PathBuf::from(output.ok_or("Integration check output directory is required")?);
    if output.exists() {
        return Err("Integration check output already exists; preserve previous evidence.".into());
    }
    fs::create_dir_all(&output).map_err(|_| "Cannot create integration check directory")?;
    let service = Service::new(output.join("application-data"))?;
    let installed = if include_installed {
        let (_, catalog, _) = service.refresh(None)?;
        let devices =
            meeting_core::list_devices().map_err(|_| "Read-only device enumeration failed")?;
        Some((
            catalog.models.len(),
            catalog.installed_streaming_models().count(),
            devices.len(),
        ))
    } else {
        None
    };
    let runtime = handy_bridge::native::runtime_identity();
    if runtime.compiled_version != "0.4.1"
        || runtime.loaded_version != "0.4.1"
        || runtime.public_header_hash.is_empty()
    {
        return Err("Packaged native runtime does not match the pinned 0.4.1 bindings".into());
    }
    let legacy = output.join("legacy-fixture");
    fs::create_dir_all(legacy.join("audio"))
        .map_err(|_| "Cannot create synthetic legacy fixture")?;
    let mut parts = vec![];
    for index in 0..25 {
        let start = index * 16_000;
        let file = format!("system-{start:012}.wav");
        fs::write(
            legacy.join("audio").join(&file),
            b"synthetic existing audio fixture",
        )
        .map_err(|_| "Cannot create fixture")?;
        parts.push(serde_json::json!({"File":file,"Source":"system","StartSample":start,"SampleCount":16000,"HasSignal":true,"AudioClosed":true,"Status":"complete","Text":format!("Originalabschnitt {index}: Dieser Text bleibt unverändert."),"ModelId":"synthetic-fixture","ProcessedSamples":16000}));
    }
    let manifest = legacy.join("session.json");
    create_json(
        &manifest,
        &serde_json::json!({"SchemaVersion":1,"Id":"0123456789abcdef0123456789abcdef","Title":"Synthetische Originalfassung","StartedAt":"2026-10-25T00:00:00+00:00","EndedAt":"2026-10-25T00:00:25+00:00","Status":"complete","ModelId":"synthetic-fixture","ChunkSeconds":30,"DurationMs":25000,"Parts":parts}),
    )?;
    let before = fs::read(&manifest).map_err(|_| "Cannot read fixture")?;
    let imported = service.import(manifest.to_string_lossy().into_owned())?;
    if !imported.data.read_only
        || !imported.data.id.starts_with("legacy-")
        || imported.data.parts.len() != 25
    {
        return Err("Legacy read-only import invariant failed".into());
    }
    let full = service.data(&imported.data.id)?;
    let preview = meeting_view(full.clone(), true);
    if preview.data.parts.len() != 16
        || !preview.live_preview_limited
        || preview.saved_audio_ms.system != 25000
    {
        return Err("Bounded preview or global saved-audio calculation failed".into());
    }
    if serde_json::to_string(&meeting_view(full.clone(), false))
        .map_err(|_| "Cannot serialize full copy")?
        .matches("Originalabschnitt")
        .count()
        != 25
    {
        return Err("Full transcript copy lost older sections".into());
    }
    let sources = note_sources(&full)?;
    if sources.len() != 25 || sources[0].id != "S00001" {
        return Err("Stable source mapping failed".into());
    }
    if fs::read(&manifest).map_err(|_| "Cannot reread fixture")? != before {
        return Err("Legacy original was modified".into());
    }
    for part in &full.parts {
        if fs::read(legacy.join("audio").join(&part.file))
            .map_err(|_| "Cannot reread synthetic audio fixture")?
            != b"synthetic existing audio fixture"
        {
            return Err("Legacy synthetic audio original was modified".into());
        }
    }
    if service.data("../../outside").is_ok() {
        return Err("Session traversal was accepted".into());
    }
    let reloaded = Service::new(service.root.clone())?;
    if reloaded.data(&imported.data.id)?.parts.len() != 25 {
        return Err("Explicit legacy mapping did not survive restart".into());
    }
    let _ = service.state()?;
    create_json(
        &output.join(if include_installed {
            "integration-checks.json"
        } else {
            "release-checks.json"
        }),
        &serde_json::json!({"success":true,"applicationVersion":VERSION,"checks":8,"syntheticChecks":7,"nativeRuntimeChecks":1,"checkNames":["native-runtime-abi","read-only-legacy-import","bounded-preview-global-progress","full-transcript-copy","stable-source-ids","original-manifest-and-audio-bytes","session-path-traversal","import-mapping-after-restart"],"installedMetadataQueried":include_installed,"installedHandyModelCount":installed.map(|s|s.0),"installedStreamingModels":installed.map(|s|s.1),"audioDeviceCount":installed.map(|s|s.2),"legacyOriginalUnchanged":true,"legacyMappingSurvivedRestart":true,"previewParts":16,"fullCopyParts":25,"stableSourceCount":sources.len(),"nativeRuntimeMatchesPinnedBindings":true,"nativeRuntime":runtime,"synthetic":true,"realCaptureOpened":false}),
    )
}

#[cfg(test)]
mod service_tests {
    use super::*;

    fn fixture_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "handymeeting-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn only_handy_named_programs_are_accepted_as_explicit_executables() {
        for ok in [
            "/usr/bin/handy",
            "C:/Program Files/Handy/Handy.exe",
            "/Applications/Handy.app",
            "/home/u/Applications/Handy_0.9.8_amd64.AppImage",
        ] {
            assert!(plausible_handy_path(Path::new(ok)), "{ok}");
        }
        for bad in [
            "/bin/sh",
            "C:/Windows/System32/cmd.exe",
            "/tmp/handy.sh",
            "/tmp/payload",
        ] {
            assert!(!plausible_handy_path(Path::new(bad)), "{bad}");
        }
    }
    #[test]
    fn invalid_explicit_installation_never_reverts_to_an_earlier_executable() {
        let root = fixture_root("installation-choice");
        let service = Service::new(root.clone()).unwrap();
        service.lock().unwrap().executable = Some(root.join("previous-handy.exe"));
        let selected = root.join("new-installation-missing.exe");
        assert!(service
            .refresh(Some(selected.to_string_lossy().into_owned()))
            .is_err());
        assert_eq!(
            service.lock().unwrap().executable.as_deref(),
            Some(selected.as_path())
        );
        assert!(service.refresh(None).is_err());
        assert_eq!(
            service.lock().unwrap().executable.as_deref(),
            Some(selected.as_path())
        );
        assert!(service
            .lock()
            .unwrap()
            .handy
            .as_ref()
            .unwrap()
            .models
            .is_empty());
    }

    #[test]
    fn mismatching_session_folder_identity_is_rejected_without_rewriting_it() {
        let root = fixture_root("session-identity");
        let service = Service::new(root).unwrap();
        let store = SessionStore::create(
            &service.root.join("sessions"),
            "Synthetic identity fixture",
            "fixture-model",
            "fixture-output",
            None,
        )
        .unwrap();
        let id = store.snapshot().unwrap().id;
        let manifest = store.folder().join("session.json");
        drop(store);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        value["id"] = serde_json::json!("ffffffffffffffffffffffffffffffff");
        let bytes = serde_json::to_vec_pretty(&value).unwrap();
        fs::write(&manifest, &bytes).unwrap();
        assert!(service.data(&id).is_err());
        assert_eq!(fs::read(&manifest).unwrap(), bytes);
    }

    #[test]
    fn stop_returns_while_recognition_catches_up_and_a_poll_collects_it_later() {
        let root = fixture_root("stop-catch-up");
        let store = SessionStore::create(
            &root.join("sessions"),
            "Synthetic slow backlog",
            "fixture-model",
            "fixture-output",
            None,
        )
        .unwrap();
        let mut writer = meeting_core::PcmWriter::new(store.clone(), "system").unwrap();
        writer.append(&[1_i16, 2, 3, 4]).unwrap();
        writer.finish().unwrap();
        drop(writer);
        store.set_status("recorded").unwrap();
        let service = Service::new(root.clone()).unwrap();
        // A recognition worker that is still busy with the backlog when the user stops.
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let worker = std::thread::spawn(move || {
            let _ = wait.recv();
        });
        service.lock().unwrap().active = Some(Active {
            owner: None,
            store: store.clone(),
            capture: None,
            stop: Arc::new(AtomicBool::new(false)),
            workers: vec![worker],
        });
        let started = std::time::Instant::now();
        let view = service.stop().unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        assert_eq!(view.data.status, "catching-up");
        {
            let inner = service.lock().unwrap();
            assert!(!inner.transitioning, "stop must not hold the transition");
            assert!(inner.active.as_ref().is_some_and(|a| a.capture.is_none()));
        }
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while service.lock().unwrap().active.is_some() {
            assert!(std::time::Instant::now() < deadline);
            service.reap_finished().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn resume_finalizes_durable_final_text_after_host_exit_without_loading_a_model() {
        for global_status in ["recorded", "catching-up"] {
            let root = fixture_root("resume-final-checkpoint");
            let store = SessionStore::create(
                &root.join("sessions"),
                "Synthetic host-exit window",
                "fixture-model",
                "fixture-output",
                None,
            )
            .unwrap();
            let mut writer = meeting_core::PcmWriter::new(store.clone(), "system").unwrap();
            writer.append(&[1_i16, 2, 3, 4]).unwrap();
            writer.finish().unwrap();
            drop(writer);
            store.set_status("recorded").unwrap();
            let data = store.snapshot().unwrap();
            let id = data.id.clone();
            let part = &data.parts[0];
            store
                .append_transcript(meeting_core::TranscriptCheckpoint {
                    sequence: 0,
                    at: String::new(),
                    file: part.file.clone(),
                    model_id: data.model_id.clone(),
                    full: "Bestätigter Originaltext.".into(),
                    committed: "Bestätigter Originaltext.".into(),
                    tentative: String::new(),
                    processed_samples: part.confirmed_sample_count,
                    final_text: true,
                })
                .unwrap();
            if global_status == "catching-up" {
                store.set_status("catching-up").unwrap();
            }
            let folder = store.folder().to_owned();
            let audio = fs::read(folder.join("audio").join(&part.file)).unwrap();
            let journal = fs::read(folder.join("transcript-journal.jsonl")).unwrap();
            drop(store);
            // Reopen the exact durable state a killed host leaves between its
            // final checkpoint and global completion write. No capture occurs.
            let service = Service::new(root.clone()).unwrap();
            service.lock().unwrap().executable = Some(root.join("intentionally-missing-handy.exe"));
            let resumed = service.resume(id.clone()).unwrap();
            assert_eq!(resumed.data.status, "complete");
            assert_eq!(
                resumed.data.parts[0].text.as_deref(),
                Some("Bestätigter Originaltext.")
            );
            assert!(service.lock().unwrap().handy.is_none());
            assert!(service.lock().unwrap().active.is_none());
            assert_eq!(
                fs::read(folder.join("audio").join(&part.file)).unwrap(),
                audio
            );
            assert_eq!(
                fs::read(folder.join("transcript-journal.jsonl")).unwrap(),
                journal
            );
            let persisted: serde_json::Value =
                serde_json::from_slice(&fs::read(folder.join("session.json")).unwrap()).unwrap();
            assert_eq!(persisted["status"], "complete");
        }
    }

    #[test]
    fn resume_does_not_finalize_failed_or_open_audio_parts() {
        for (part_status, open) in [("error", false), ("complete", true)] {
            let root = fixture_root("resume-incomplete");
            let store = SessionStore::create(
                &root.join("sessions"),
                "Synthetic incomplete state",
                "fixture-model",
                "fixture-output",
                None,
            )
            .unwrap();
            let mut writer = meeting_core::PcmWriter::new(store.clone(), "system").unwrap();
            writer.append(&[1_i16, 2, 3, 4]).unwrap();
            if open {
                writer.checkpoint().unwrap();
            } else {
                writer.finish().unwrap();
            }
            drop(writer);
            store.set_status("recorded").unwrap();
            let id = store.snapshot().unwrap().id;
            let manifest = store.folder().join("session.json");
            drop(store);
            let mut value: serde_json::Value =
                serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
            value["parts"][0]["status"] = part_status.into();
            value["parts"][0]["modelId"] = "fixture-model".into();
            value["parts"][0]["processedSamples"] = 4.into();
            fs::write(&manifest, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
            let service = Service::new(root.clone()).unwrap();
            service.lock().unwrap().executable = Some(root.join("intentionally-missing-handy.exe"));
            assert!(service.resume(id.clone()).is_err());
            assert_eq!(service.data(&id).unwrap().status, "recorded");
        }
    }
}
