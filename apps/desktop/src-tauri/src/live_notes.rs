//! Live notes worker: reads only the confirmed transcript that the UI also reads and sends text
//! to the chosen model. It never holds the recording transition, a capture handle or audio.
use crate::{valid_legacy_id, validate_id, AppResult, Service};
use meeting_core::SessionData;
use meeting_notes::live::{self, LiveRunner, LiveText, LiveView, Trigger};
use meeting_notes::AnalysisControl;
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

/// How often the worker looks for newly confirmed text.
const POLL: Duration = Duration::from_millis(2000);
/// A failed request is retried after this pause, doubling up to `MAX_BACKOFF`.
const FIRST_BACKOFF: Duration = Duration::from_secs(20);
const MAX_BACKOFF: Duration = Duration::from_secs(300);
/// Without new text, a missing overview is retried at most this often.
const OVERVIEW_RETRY: Duration = Duration::from_secs(60);
/// After the meeting ended, a failing overview is attempted this often before the worker ends.
const MAX_OVERVIEW_FAILURES: u32 = 3;
pub(crate) const DEFAULT_INTERVAL_MS: u64 = 3 * 60_000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveNotesRequest {
    meeting_id: String,
    endpoint: String,
    model: String,
    mode: String,
    #[serde(default)]
    cloud_consent: bool,
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    use_saved_key: bool,
    #[serde(default)]
    interval_ms: Option<u64>,
    language: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveNotesStatus {
    meeting_id: String,
    /// off | starting | waiting | updating | finished | stopped | failed
    status: String,
    running: bool,
    error: Option<String>,
    pending_chars: usize,
    interval_ms: u64,
    next_update_in_ms: Option<u64>,
    last_trigger: Option<Trigger>,
    view: Option<LiveView>,
}

struct Shared {
    status: String,
    error: Option<String>,
    pending_chars: usize,
    last_update: Instant,
    last_trigger: Option<Trigger>,
    retry_at: Option<Instant>,
    view: Option<LiveView>,
}

struct Job {
    meeting_id: String,
    control: AnalysisControl,
    manual: Arc<AtomicBool>,
    interval_ms: Arc<AtomicU64>,
    shared: Arc<Mutex<Shared>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Job {
    fn running(&self) -> bool {
        self.shared
            .lock()
            .is_ok_and(|s| ["starting", "waiting", "updating"].contains(&s.status.as_str()))
    }
    fn status(&self) -> LiveNotesStatus {
        let interval_ms = self.interval_ms.load(Ordering::Acquire);
        let shared = self.shared.lock();
        let Ok(shared) = shared else {
            return off(
                &self.meeting_id,
                None,
                Some("Live-Zusammenfassung ist gesperrt.".into()),
            );
        };
        let running = ["starting", "waiting", "updating"].contains(&shared.status.as_str());
        let next_update_in_ms = (running && shared.status == "waiting").then(|| {
            let elapsed = shared.last_update.elapsed().as_millis() as u64;
            let due = interval_ms.saturating_sub(elapsed);
            shared.retry_at.map_or(due, |at| {
                due.max(at.saturating_duration_since(Instant::now()).as_millis() as u64)
            })
        });
        LiveNotesStatus {
            meeting_id: self.meeting_id.clone(),
            status: shared.status.clone(),
            running,
            error: shared.error.clone(),
            pending_chars: shared.pending_chars,
            interval_ms,
            next_update_in_ms,
            last_trigger: shared.last_trigger,
            view: shared.view.clone(),
        }
    }
}

fn off(meeting_id: &str, view: Option<LiveView>, error: Option<String>) -> LiveNotesStatus {
    LiveNotesStatus {
        meeting_id: meeting_id.into(),
        status: "off".into(),
        running: false,
        error,
        pending_chars: 0,
        interval_ms: DEFAULT_INTERVAL_MS,
        next_update_in_ms: None,
        last_trigger: None,
        view,
    }
}

#[derive(Default)]
pub(crate) struct LiveNotes(Mutex<Option<Job>>);

fn clamp_interval(ms: Option<u64>) -> u64 {
    ms.unwrap_or(DEFAULT_INTERVAL_MS)
        .clamp(live::MIN_INTERVAL_MS, live::MAX_INTERVAL_MS)
}

fn live_root(service: &Service, meeting_id: &str) -> std::path::PathBuf {
    service
        .root
        .join("sessions")
        .join(meeting_id)
        .join("live-notes")
}

/// Confirmed text of complete parts and whether the meeting can still produce more.
pub(crate) fn live_texts(data: &SessionData) -> (Vec<LiveText>, bool) {
    let texts = data
        .parts
        .iter()
        .filter(|p| p.status == "complete")
        .filter_map(|p| {
            let text = p.text.as_ref()?;
            let rate = u64::from(meeting_core::SAMPLE_RATE);
            Some(LiveText {
                key: p.file.clone(),
                track: p.source.clone(),
                start_ms: p.start_sample * 1000 / rate,
                end_ms: (p.start_sample + p.sample_count) * 1000 / rate,
                text: text.clone(),
            })
        })
        .collect();
    let active = ["recording", "streaming", "catching-up"].contains(&data.status.as_str());
    (texts, !active)
}

impl Service {
    /// A snapshot of one meeting for the live worker. It prefers the stores the application
    /// already holds and otherwise opens the session briefly without replacing the store the
    /// user is viewing.
    fn live_snapshot(&self, id: &str) -> AppResult<SessionData> {
        {
            let inner = self.lock()?;
            for store in inner
                .active
                .as_ref()
                .map(|a| &a.store)
                .into_iter()
                .chain(inner.view.iter())
            {
                if let Ok(data) = store.snapshot() {
                    if data.id == id {
                        return Ok(data);
                    }
                }
            }
        }
        let folder = self.root.join("sessions").join(id);
        let store = meeting_core::SessionStore::open(&folder)
            .map_err(|_| "Die Sitzung ist gerade nicht lesbar.")?;
        let data = store
            .snapshot()
            .map_err(|_| "Sitzungsstand konnte nicht gelesen werden.")?;
        if data.id != id {
            return Err("Sitzungskennung und Ordner stimmen nicht überein.".into());
        }
        Ok(data)
    }
}

impl LiveNotes {
    fn lock(&self) -> AppResult<std::sync::MutexGuard<'_, Option<Job>>> {
        self.0
            .lock()
            .map_err(|_| "Live-Zusammenfassung ist gesperrt.".into())
    }

    pub(crate) fn status(&self, service: &Service, meeting_id: &str) -> AppResult<LiveNotesStatus> {
        validate_id(meeting_id)?;
        if let Some(job) = self.lock()?.as_ref().filter(|j| j.meeting_id == meeting_id) {
            return Ok(job.status());
        }
        match live::read_latest(&live_root(service, meeting_id)) {
            Ok(view) => Ok(off(meeting_id, view, None)),
            Err(_) => Ok(off(
                meeting_id,
                None,
                Some("Gespeicherte Live-Zusammenfassungen konnten nicht gelesen werden. Sie bleiben unverändert erhalten.".into()),
            )),
        }
    }

    pub(crate) fn refresh(&self) -> AppResult<bool> {
        Ok(self
            .lock()?
            .as_ref()
            .filter(|j| j.running())
            .map(|j| j.manual.store(true, Ordering::Release))
            .is_some())
    }

    pub(crate) fn set_interval(&self, interval_ms: u64) -> AppResult<bool> {
        Ok(self
            .lock()?
            .as_ref()
            .map(|j| {
                j.interval_ms
                    .store(clamp_interval(Some(interval_ms)), Ordering::Release)
            })
            .is_some())
    }

    pub(crate) fn stop(&self) -> AppResult<bool> {
        let guard = self.lock()?;
        let Some(job) = guard.as_ref().filter(|j| j.running()) else {
            return Ok(false);
        };
        job.control.cancel();
        if let Ok(mut shared) = job.shared.lock() {
            shared.status = "stopped".into();
        }
        Ok(true)
    }

    /// Starts live notes for one meeting. Saved keys still need cloud text release for this
    /// meeting; the release is never stored and does not carry over to a restart.
    pub(crate) fn start(
        &self,
        service: &Arc<Service>,
        mut request: LiveNotesRequest,
    ) -> AppResult<LiveNotesStatus> {
        let mut key = Zeroizing::new(std::mem::take(&mut request.api_key));
        if valid_legacy_id(&request.meeting_id) {
            return Err(
                "Importierte Sitzungen der bisherigen Version werden nicht verändert.".into(),
            );
        }
        validate_id(&request.meeting_id)?;
        if !["de", "en"].contains(&request.language.as_str()) {
            return Err("Unbekannte Sprache.".into());
        }
        let mut guard = self.lock()?;
        if guard.as_ref().is_some_and(Job::running) {
            return Err(
                "Eine Live-Zusammenfassung läuft bereits. Bitte sie zuerst beenden.".into(),
            );
        }
        // A stopped worker may still be finishing a cancelled request; it must be gone before
        // a new runner reads the stored windows, so two writers never share a run.
        if let Some(handle) = guard.as_mut().and_then(|job| job.worker.take()) {
            let _ = handle.join();
        }
        // The meeting must exist and be readable before any model client is created.
        let data = service.live_snapshot(&request.meeting_id)?;
        // Held by the worker until it ends, so archive moves or installation never run while
        // live notes write into the session folder.
        let lease = service.reserve_operation(crate::operations::Kind::LiveNotes)?;
        if request.use_saved_key {
            if !key.is_empty() {
                return Err("Bitte entweder den gespeicherten Schlüssel oder einen Schlüssel nur für diese Sitzung wählen.".into());
            }
            key = crate::credential_service::load_for_run(
                &request.endpoint,
                &request.mode,
                request.cloud_consent,
            )?;
        }
        let connection = meeting_notes::Connection {
            endpoint: request.endpoint,
            model: request.model,
            mode: request.mode,
            cloud_consent: request.cloud_consent,
            api_key: std::mem::take(&mut *key),
        };
        let runner = LiveRunner::start(
            &live_root(service, &request.meeting_id),
            &request.meeting_id,
            connection,
            &request.language,
        )
        .map_err(|e| e.to_string())?;
        let (texts, _) = live_texts(&data);
        let shared = Arc::new(Mutex::new(Shared {
            status: "starting".into(),
            error: None,
            pending_chars: live::pending_chars(&runner.pending(&texts)),
            last_update: Instant::now(),
            last_trigger: None,
            retry_at: None,
            view: Some(runner.view()),
        }));
        let pending_at_start = shared.lock().is_ok_and(|s| s.pending_chars > 0);
        let mut job = Job {
            meeting_id: request.meeting_id.clone(),
            control: AnalysisControl::default(),
            // Text that already exists is summarized right away instead of after the interval.
            manual: Arc::new(AtomicBool::new(pending_at_start)),
            interval_ms: Arc::new(AtomicU64::new(clamp_interval(request.interval_ms))),
            shared,
            worker: None,
        };
        let worker = Worker {
            _lease: lease,
            service: Arc::clone(service),
            meeting_id: request.meeting_id,
            control: job.control.clone(),
            manual: Arc::clone(&job.manual),
            interval_ms: Arc::clone(&job.interval_ms),
            shared: Arc::clone(&job.shared),
        };
        job.worker = Some(
            thread::Builder::new()
                .name("live-notes".into())
                .spawn(move || worker.run(runner))
                .map_err(|_| "Die Live-Zusammenfassung konnte nicht gestartet werden.")?,
        );
        let status = job.status();
        *guard = Some(job);
        Ok(status)
    }
}

/// Whether only the overview should be rebuilt now: a bounded number of attempts, never in a
/// tight loop, also after the meeting has ended.
fn overview_retry(due: bool, failures: u32, manual: bool, since_attempt: Option<Duration>) -> bool {
    due && failures < MAX_OVERVIEW_FAILURES
        && (manual || since_attempt.is_none_or(|elapsed| elapsed >= OVERVIEW_RETRY))
}

struct Worker {
    _lease: crate::operations::Lease,
    service: Arc<Service>,
    meeting_id: String,
    control: AnalysisControl,
    manual: Arc<AtomicBool>,
    interval_ms: Arc<AtomicU64>,
    shared: Arc<Mutex<Shared>>,
}

impl Worker {
    fn set(&self, change: impl FnOnce(&mut Shared)) {
        if let Ok(mut shared) = self.shared.lock() {
            // A stop wins over everything the worker reports afterwards.
            if shared.status != "stopped" {
                change(&mut shared);
            }
        }
    }

    fn wait(&self) {
        let until = Instant::now() + POLL;
        while Instant::now() < until && !self.control.is_cancelled() {
            if self.manual.load(Ordering::Acquire) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn run(self, mut runner: LiveRunner) {
        let mut failures: u32 = 0;
        let mut overview_failures: u32 = 0;
        let mut overview_attempt: Option<Instant> = None;
        self.set(|s| s.status = "waiting".into());
        while !self.control.is_cancelled() {
            let data = match self.service.live_snapshot(&self.meeting_id) {
                Ok(data) => data,
                Err(error) => {
                    self.set(|s| s.error = Some(error));
                    self.wait();
                    continue;
                }
            };
            let (texts, finished) = live_texts(&data);
            let pending = runner.pending(&texts);
            let pending_chars = live::pending_chars(&pending);
            let (since, retry_at) = self
                .shared
                .lock()
                .map(|s| (s.last_update.elapsed(), s.retry_at))
                .unwrap_or((Duration::ZERO, None));
            self.set(|s| s.pending_chars = pending_chars);
            let backing_off = retry_at.is_some_and(|at| Instant::now() < at);
            let manual = self.manual.swap(false, Ordering::AcqRel);
            let interval = self.interval_ms.load(Ordering::Acquire);
            let mut trigger = live::decide(
                &pending,
                since.as_millis() as u64,
                interval,
                manual,
                finished,
            );
            // A missing overview is retried at a bounded pace, also after the meeting ended.
            let overview_open = runner.overview_due() && overview_failures < MAX_OVERVIEW_FAILURES;
            if trigger.is_none()
                && overview_retry(
                    runner.overview_due(),
                    overview_failures,
                    manual,
                    overview_attempt.map(|at| at.elapsed()),
                )
            {
                trigger = Some(Trigger::Manual);
            }
            if finished && trigger.is_none() && !overview_open {
                self.set(|s| {
                    s.status = "finished".into();
                    s.pending_chars = 0;
                });
                return;
            }
            match trigger {
                Some(trigger) if !backing_off || manual => {
                    self.set(|s| s.status = "updating".into());
                    overview_attempt = Some(Instant::now());
                    let result = runner.update(&pending, trigger, &self.control);
                    if self.control.is_cancelled() {
                        break;
                    }
                    match result {
                        Ok(outcome) => {
                            failures = 0;
                            if outcome.overview_error.is_some() {
                                overview_failures = overview_failures.saturating_add(1);
                            } else if outcome.overview.is_some() {
                                overview_failures = 0;
                            }
                            let summarized = outcome.window.is_some();
                            let view = runner.view();
                            self.set(|s| {
                                s.status = "waiting".into();
                                s.error = outcome.overview_error.clone();
                                s.last_update = Instant::now();
                                s.last_trigger = Some(trigger);
                                s.retry_at = None;
                                s.view = Some(view);
                            });
                            // A remaining backlog is handled by the next decision without waiting;
                            // an overview-only retry always waits.
                            if summarized {
                                continue;
                            }
                        }
                        Err(error) => {
                            failures = failures.saturating_add(1);
                            let pause = FIRST_BACKOFF
                                .saturating_mul(1 << failures.min(5).saturating_sub(1))
                                .min(MAX_BACKOFF);
                            self.set(|s| {
                                s.status = "waiting".into();
                                s.error = Some(error.to_string());
                                s.retry_at = Some(Instant::now() + pause);
                            });
                            if finished && failures >= 3 {
                                // The meeting is over; stop retrying but keep everything saved.
                                self.set(|s| s.status = "failed".into());
                                return;
                            }
                        }
                    }
                }
                _ => {}
            }
            self.wait();
        }
        self.set(|s| s.status = "stopped".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meeting_core::SessionStore;

    fn fixture_root(label: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "handymeeting-live-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn request(meeting_id: &str, mode: &str, endpoint: &str) -> LiveNotesRequest {
        LiveNotesRequest {
            meeting_id: meeting_id.into(),
            endpoint: endpoint.into(),
            model: "synthetic".into(),
            mode: mode.into(),
            cloud_consent: false,
            api_key: String::new(),
            use_saved_key: false,
            interval_ms: None,
            language: "de".into(),
        }
    }

    fn wait_for(service: &Service, id: &str, wanted: &str) -> LiveNotesStatus {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let status = service.live_notes.status(service, id).unwrap();
            if status.status == wanted {
                return status;
            }
            assert!(Instant::now() < deadline, "still {}", status.status);
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn only_complete_parts_count_and_an_active_meeting_is_not_finished() {
        let data: SessionData = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1, "id": "a".repeat(32), "title": "t", "startedAt": "2026-10-28T00:00:00Z",
            "status": "recording",
            "parts": [
                {"file":"system-000000000000.wav","source":"system","startSample":0,"sampleCount":480000,"status":"complete","text":"Erster Abschnitt."},
                {"file":"microphone-000000480000.wav","source":"microphone","startSample":480000,"sampleCount":16000,"status":"pending","committedText":"noch offen"},
                {"file":"system-000000480000.wav","source":"system","startSample":480000,"sampleCount":16000,"status":"complete"}
            ]
        }))
        .unwrap();
        let (texts, finished) = live_texts(&data);
        assert!(!finished);
        assert_eq!(texts.len(), 1);
        let rate = u64::from(meeting_core::SAMPLE_RATE);
        assert_eq!(texts[0].end_ms, 480_000 * 1000 / rate);
        let mut done = data.clone();
        done.status = "complete".into();
        assert!(live_texts(&done).1);
        done.status = "catching-up".into();
        assert!(!live_texts(&done).1, "catch-up still produces text");
    }

    #[test]
    fn a_failing_overview_is_retried_at_a_bounded_pace_and_then_given_up() {
        let now = Some(Duration::from_secs(1));
        let later = Some(OVERVIEW_RETRY);
        assert!(overview_retry(true, 0, false, None));
        assert!(!overview_retry(true, 0, false, now), "no tight retry loop");
        assert!(overview_retry(true, 1, false, later));
        assert!(
            overview_retry(true, 1, true, now),
            "a manual update may retry at once"
        );
        assert!(!overview_retry(true, MAX_OVERVIEW_FAILURES, true, later));
        assert!(!overview_retry(false, 0, true, None));
    }

    #[test]
    fn running_live_notes_keep_archive_and_installation_waiting_until_they_end() {
        let root = fixture_root("lease");
        let service = Arc::new(Service::new(root.clone()).unwrap());
        let store =
            SessionStore::create(&root.join("sessions"), "Synthetic", "m", "s", None).unwrap();
        let id = store.snapshot().unwrap().id;
        service.lock().unwrap().view = Some(store);
        service
            .live_notes
            .start(&service, request(&id, "local", "http://127.0.0.1:9/v1/"))
            .unwrap();
        wait_for(&service, &id, "waiting");
        for kind in [
            crate::operations::Kind::Maintenance,
            crate::operations::Kind::Installation,
        ] {
            assert!(service.reserve_operation(kind).is_err(), "{kind:?}");
        }
        // Post-meeting analysis is not blocked by live notes.
        drop(
            service
                .reserve_operation(crate::operations::Kind::Analysis)
                .unwrap(),
        );
        service.live_notes.stop().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while service
            .reserve_operation(crate::operations::Kind::Maintenance)
            .is_err()
        {
            assert!(Instant::now() < deadline, "the lease ends with the worker");
            thread::sleep(Duration::from_millis(20));
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cloud_without_release_and_legacy_meetings_are_refused_before_anything_is_written() {
        let root = fixture_root("refused");
        let service = Arc::new(Service::new(root.clone()).unwrap());
        let store =
            SessionStore::create(&root.join("sessions"), "Synthetic", "m", "s", None).unwrap();
        let id = store.snapshot().unwrap().id;
        service.lock().unwrap().view = Some(store);
        let cloud = request(&id, "cloud", "https://api.example.com/v1/");
        assert!(service.live_notes.start(&service, cloud).is_err());
        assert!(!live_root(&service, &id).exists());
        let legacy = request("legacy-0123456789abcdef", "local", "http://127.0.0.1:9/v1/");
        assert!(service.live_notes.start(&service, legacy).is_err());
        let unknown = request(&"b".repeat(32), "local", "http://127.0.0.1:9/v1/");
        assert!(service.live_notes.start(&service, unknown).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn live_notes_run_beside_a_recording_and_stopping_them_leaves_the_recording_untouched() {
        let root = fixture_root("beside-recording");
        let service = Arc::new(Service::new(root.clone()).unwrap());
        let store =
            SessionStore::create(&root.join("sessions"), "Synthetic", "m", "s", None).unwrap();
        let id = store.snapshot().unwrap().id;
        service.lock().unwrap().view = Some(store.clone());
        // Synthetic: nothing listens here, and no text exists yet, so no request is made.
        let started = service
            .live_notes
            .start(&service, request(&id, "local", "http://127.0.0.1:9/v1/"))
            .unwrap();
        assert!(started.running);
        wait_for(&service, &id, "waiting");
        assert!(service
            .live_notes
            .start(&service, request(&id, "local", "http://127.0.0.1:9/v1/"))
            .is_err());
        assert!(service.live_notes.refresh().unwrap());
        assert!(service.live_notes.set_interval(1).unwrap());
        assert_eq!(
            service
                .live_notes
                .status(&service, &id)
                .unwrap()
                .interval_ms,
            live::MIN_INTERVAL_MS
        );
        assert!(service.live_notes.stop().unwrap());
        let stopped = service.live_notes.status(&service, &id).unwrap();
        assert_eq!(stopped.status, "stopped");
        assert!(!stopped.running);
        assert_eq!(store.snapshot().unwrap().status, "recording");
        // A finished meeting without new text ends the next run by itself.
        store.set_status("complete").unwrap();
        service
            .live_notes
            .start(&service, request(&id, "local", "http://127.0.0.1:9/v1/"))
            .unwrap();
        let finished = wait_for(&service, &id, "finished");
        assert!(finished.view.is_some_and(|v| v.windows == 0));
        assert_eq!(
            live::list_runs(&live_root(&service, &id)).unwrap().0.len(),
            1
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
