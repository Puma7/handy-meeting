//! Synthetic service-state regressions; ordinary tests never open audio devices.
use super::*;

#[test]
fn exclusive_reservation_blocks_annotation_policy_and_import_writes_before_worker_start() {
    for exclusive in [
        operations::Kind::Maintenance,
        operations::Kind::Installation,
        operations::Kind::Selection,
    ] {
        let root = fixture_root("exclusive-mutation");
        let service = Service::new(root.clone()).unwrap();
        let store = SessionStore::create(
            &root.join("sessions"),
            "Synthetic protected mutation",
            "m",
            "s",
            None,
        )
        .unwrap();
        let id = store.snapshot().unwrap().id;
        store.set_status("complete").unwrap();
        drop(store);
        let legacy = root.join("legacy-fixture");
        fs::create_dir(&legacy).unwrap();
        let manifest = legacy.join("session.json");
        let original=serde_json::to_vec(&serde_json::json!({"SchemaVersion":1,"Id":"0123456789abcdef0123456789abcdef","Title":"Synthetic old session","StartedAt":"2026-10-27T00:00:00Z","EndedAt":"2026-10-27T00:00:01Z","Status":"complete","ModelId":"m","Parts":[]})).unwrap();
        fs::write(&manifest, &original).unwrap();
        service
            .selections
            .approve_generated_legacy_fixture(&manifest)
            .unwrap();
        // Model the actual gap after a reservation, before start_work sets job progress.
        let reservation = service.reserve_operation(exclusive).unwrap();
        assert!(
            library_commands::edit_metadata(&service, &id, 0, Some("Must not save".into()), true)
                .is_err(),
            "metadata admitted during {exclusive:?}"
        );
        assert!(
            speaker_commands::save_labels(&service, &id, 0, false, vec![]).is_err(),
            "labels admitted during {exclusive:?}"
        );
        assert!(
            retention_commands::save_policy(&service, 0, Default::default()).is_err(),
            "policy admitted during {exclusive:?}"
        );
        assert!(
            service
                .import(manifest.to_string_lossy().into_owned())
                .is_err(),
            "import admitted during {exclusive:?}"
        );
        assert!(!root.join("user-metadata").exists());
        assert!(!root.join("speaker-labels").exists());
        assert!(!root.join("retention-settings.json").exists());
        assert_eq!(fs::read_dir(root.join("legacy-map")).unwrap().count(), 0);
        assert_eq!(fs::read(&manifest).unwrap(), original);
        drop(reservation);
        assert_eq!(
            library_commands::edit_metadata(&service, &id, 0, Some("Allowed later".into()), true)
                .unwrap()
                .revision,
            1
        );
        assert_eq!(
            speaker_commands::save_labels(&service, &id, 0, false, vec![])
                .unwrap()
                .revision,
            1
        );
        assert_eq!(
            retention_commands::save_policy(&service, 0, Default::default())
                .unwrap()
                .revision,
            1
        );
        assert!(
            service
                .import(manifest.to_string_lossy().into_owned())
                .unwrap()
                .data
                .read_only
        );
        assert_eq!(fs::read_dir(root.join("legacy-map")).unwrap().count(), 1);
        assert_eq!(fs::read(&manifest).unwrap(), original);
    }
}

#[test]
fn metadata_rename_remains_available_while_the_recording_operation_is_owned() {
    let root = fixture_root("live-mutation");
    let service = Service::new(root.clone()).unwrap();
    let store = SessionStore::create(
        &root.join("sessions"),
        "Original capture title",
        "m",
        "s",
        None,
    )
    .unwrap();
    let id = store.snapshot().unwrap().id;
    let before = fs::read(store.folder().join("session.json")).unwrap();
    let lease = service
        .reserve_operation(operations::Kind::Recording)
        .unwrap();
    let (finish, wait) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _ = wait.recv();
    });
    service.lock().unwrap().active = Some(Active {
        _operation: Some(lease),
        owner: None,
        store: store.clone(),
        capture: None,
        stop: Arc::new(AtomicBool::new(false)),
        workers: vec![worker],
    });
    let saved = library_commands::edit_metadata(
        &service,
        &id,
        0,
        Some("Renamed while running".into()),
        false,
    )
    .unwrap();
    assert_eq!(saved.title.as_deref(), Some("Renamed while running"));
    assert_eq!(
        fs::read(store.folder().join("session.json")).unwrap(),
        before,
        "The original recording manifest must remain unchanged"
    );
    assert!(service
        .reserve_operation(operations::Kind::Maintenance)
        .is_err());
    finish.send(()).unwrap();
}

#[test]
fn a_busy_operation_cannot_leave_recording_transition_stuck() {
    let root = fixture_root("operation-admission");
    let service = Service::new(root).unwrap();
    let lease = service
        .reserve_operation(operations::Kind::Maintenance)
        .unwrap();
    let result = service.start(StartRequest {
        title: "Synthetic blocked start".into(),
        model_id: "never-loaded".into(),
        system_device_id: "never-opened".into(),
        microphone_device_id: None,
    });
    assert!(result.is_err());
    assert!(!service.lock().unwrap().transitioning);
    assert!(service.lock().unwrap().active.is_none());
    drop(lease);
    assert!(service
        .reserve_operation(operations::Kind::Recording)
        .is_ok());
}

#[test]
fn active_or_stale_control_and_unapproved_import_requests_never_touch_devices() {
    let root = fixture_root("capture-control-identity");
    let service = Service::new(root.clone()).unwrap();
    assert!(service
        .set_capture_control("stale", Some(false), None)
        .is_err());
    let path = root.join("session.json");
    fs::write(&path, b"synthetic manifest; deliberately never parsed").unwrap();
    let error = service
        .import(path.to_string_lossy().into_owned())
        .err()
        .unwrap();
    assert!(error.contains("Systemdialog"));
    assert_eq!(
        fs::read(&path).unwrap(),
        b"synthetic manifest; deliberately never parsed"
    );
    let store = SessionStore::create(
        &root.join("sessions"),
        "Synthetic agent session",
        "m",
        "s",
        None,
    )
    .unwrap();
    let id = store.snapshot().unwrap().id;
    service.lock().unwrap().active = Some(Active {
        _operation: None,
        owner: Some("synthetic-owner".into()),
        store,
        capture: None,
        stop: Arc::new(AtomicBool::new(false)),
        workers: vec![],
    });
    assert!(service
        .set_capture_control("other-meeting", Some(false), None)
        .is_err());
    let error = service
        .set_capture_control(&id, None, Some(false))
        .err()
        .unwrap();
    assert!(error.contains("Agenten"));
}

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
        _operation: None,
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
    // Synthetic: the worker ended without recognising the open part, so the catch-up
    // must give way to a resumable state instead of blocking new recordings.
    let data = store.snapshot().unwrap();
    assert_eq!(data.status, "recorded");
    assert!(data
        .warnings
        .iter()
        .any(|w| w.contains("resume to process them")));
}

#[test]
fn a_failed_catch_up_worker_leaves_a_resumable_session() {
    let root = fixture_root("catch-up-panic");
    let store = SessionStore::create(
        &root.join("sessions"),
        "Synthetic failed catch-up",
        "fixture-model",
        "fixture-output",
        None,
    )
    .unwrap();
    let mut writer = meeting_core::PcmWriter::new(store.clone(), "system").unwrap();
    writer.append(&[1_i16, 2, 3, 4]).unwrap();
    writer.finish().unwrap();
    drop(writer);
    store.set_status("catching-up").unwrap();
    let service = Service::new(root.clone()).unwrap();
    // Synthetic: a recognition worker that panics after the user stopped.
    let worker = std::thread::spawn(|| panic!("synthetic recognition failure"));
    while !worker.is_finished() {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    service.lock().unwrap().active = Some(Active {
        _operation: None,
        owner: None,
        store: store.clone(),
        capture: None,
        stop: Arc::new(AtomicBool::new(true)),
        workers: vec![worker],
    });
    service.reap_finished().unwrap();
    assert!(service.lock().unwrap().active.is_none());
    let data = store.snapshot().unwrap();
    assert_eq!(data.status, "recorded");
    assert!(data.warnings.iter().any(|w| w.contains("unterbrochen")));
    // Audio is untouched.
    assert_eq!(data.parts[0].confirmed_sample_count, 4);
}

#[test]
fn a_failed_catch_up_status_write_is_retried_and_an_index_failure_is_reported() {
    let root = fixture_root("catch-up-retry");
    let store = SessionStore::create(
        &root.join("sessions"),
        "Synthetic blocked status write",
        "fixture-model",
        "fixture-output",
        None,
    )
    .unwrap();
    let mut writer = meeting_core::PcmWriter::new(store.clone(), "system").unwrap();
    writer.append(&[1_i16, 2, 3, 4]).unwrap();
    writer.finish().unwrap();
    drop(writer);
    store.set_status("catching-up").unwrap();
    let service = Service::new(root.clone()).unwrap();
    let worker = std::thread::spawn(|| {});
    while !worker.is_finished() {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    service.lock().unwrap().active = Some(Active {
        _operation: None,
        owner: None,
        store: store.clone(),
        capture: None,
        stop: Arc::new(AtomicBool::new(true)),
        workers: vec![worker],
    });
    // Polling leaves the session alone while a transition holds the gate.
    {
        let _gate = service.transition.lock().unwrap();
        service.reap_when_idle().unwrap();
        assert!(service.lock().unwrap().active.is_some());
    }
    // Synthetic: a directory in place of session.json makes the status write fail.
    let manifest = store.folder().join("session.json");
    let saved = std::fs::read(&manifest).unwrap();
    std::fs::remove_file(&manifest).unwrap();
    std::fs::create_dir(&manifest).unwrap();
    assert!(service.reap_when_idle().is_err());
    assert!(service.lock().unwrap().active.is_some());
    assert_eq!(store.snapshot().unwrap().status, "catching-up");
    std::fs::remove_dir(&manifest).unwrap();
    std::fs::write(&manifest, saved).unwrap();
    // Synthetic: a file in place of the index folder makes the search refresh fail.
    let index = root.join("library-index");
    assert!(!index.exists());
    std::fs::write(&index, b"synthetic, blocks the search index").unwrap();
    service.reap_when_idle().unwrap();
    assert!(service.lock().unwrap().active.is_none());
    let data = store.snapshot().unwrap();
    assert_eq!(data.status, "recorded");
    assert_eq!(data.parts[0].confirmed_sample_count, 4);
    assert!(data.warnings.iter().any(|w| w.contains("Suchindex")));
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
                revision_after_restart: false,
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
