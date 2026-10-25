use meeting_core::{read_legacy, AudioPart, PcmWriter, SessionStore, TranscriptCheckpoint};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn fixture(root: &Path) -> SessionStore {
    let store = SessionStore::create(
        root,
        "Synthetic fixture",
        "synthetic-model",
        "generated system",
        Some("generated microphone"),
    )
    .unwrap();
    for (source, value) in [("system", 128_i16), ("microphone", -512_i16)] {
        let mut writer = PcmWriter::new(store.clone(), source).unwrap();
        writer.append(&vec![value; 16_000]).unwrap();
        writer.checkpoint().unwrap();
        let part = store
            .snapshot()
            .unwrap()
            .parts
            .into_iter()
            .find(|part| part.source == source)
            .unwrap();
        store
            .append_transcript(checkpoint(&part, &format!("Confirmed {source}.")))
            .unwrap();
        // No finish: simulates a currently open audio chunk. Hound drop may update
        // the header, but the independent journal still defines the confirmed prefix.
    }
    store
}

fn checkpoint(part: &AudioPart, text: &str) -> TranscriptCheckpoint {
    TranscriptCheckpoint {
        sequence: 0,
        at: String::new(),
        file: part.file.clone(),
        model_id: "synthetic-model".into(),
        full: text.into(),
        committed: text.into(),
        tentative: String::new(),
        processed_samples: part.confirmed_sample_count,
        final_text: true,
    }
}

fn hash(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

#[test]
fn confirmed_two_track_audio_and_text_recover_with_unfinished_journal_tail() {
    let root = tempfile::tempdir().unwrap();
    let store = fixture(root.path());
    let folder = store.folder().to_owned();
    let expected: Vec<_> = store
        .snapshot()
        .unwrap()
        .parts
        .iter()
        .map(|p| (p.file.clone(), hash(&store.audio_path(p).unwrap())))
        .collect();
    drop(store);
    for name in ["audio-journal.jsonl", "transcript-journal.jsonl"] {
        OpenOptions::new()
            .append(true)
            .open(folder.join(name))
            .unwrap()
            .write_all(b"{\"unfinished\":")
            .unwrap();
    }
    let audio_journal = fs::read(folder.join("audio-journal.jsonl")).unwrap();
    let recovered = SessionStore::open(&folder).unwrap();
    let snapshot = recovered.snapshot().unwrap();
    assert_eq!(snapshot.parts.len(), 2);
    assert!(snapshot.capture_incomplete);
    for part in &snapshot.parts {
        assert_eq!(part.confirmed_sample_count, 16_000);
        assert_eq!(
            part.text.as_deref(),
            Some(format!("Confirmed {}.", part.source).as_str())
        );
        assert!(part.audio_closed);
    }
    for (name, digest) in expected {
        assert_eq!(hash(&folder.join("audio").join(name)), digest);
    }
    assert_eq!(
        fs::read(folder.join("audio-journal.jsonl")).unwrap(),
        audio_journal,
        "recovery must preserve unfinished original bytes"
    );
}

#[test]
fn unconfirmed_audio_is_not_promoted_and_invalid_text_cannot_be_acknowledged() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::create(root.path(), "Fixture", "m", "s", None).unwrap();
    let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
    writer.append(&vec![7; 8000]).unwrap();
    writer.checkpoint().unwrap();
    let part = store.snapshot().unwrap().parts[0].clone();
    let mut bad = checkpoint(&part, "too much");
    bad.processed_samples += 1;
    assert!(store.append_transcript(bad).is_err());
    assert_eq!(store.snapshot().unwrap().snapshot_sequence, 0);
    writer.append(&vec![9; 4000]).unwrap();
    drop(writer);
    let folder = store.folder().to_owned();
    drop(store);
    let recovered = SessionStore::open(&folder).unwrap();
    assert_eq!(recovered.snapshot().unwrap().parts[0].sample_count, 8000);
}

#[test]
fn corrupted_confirmed_audio_fails_without_rewriting_any_original_file() {
    let root = tempfile::tempdir().unwrap();
    let store = fixture(root.path());
    let folder = store.folder().to_owned();
    let part = store.snapshot().unwrap().parts[0].clone();
    let audio = store.audio_path(&part).unwrap();
    drop(store);
    let mut bytes = fs::read(&audio).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x20;
    fs::write(&audio, bytes).unwrap();
    let before = hash(&audio);
    assert!(SessionStore::open(&folder).is_err());
    assert_eq!(hash(&audio), before);
}

#[test]
fn active_session_has_one_writer_lease() {
    let root = tempfile::tempdir().unwrap();
    let store = fixture(root.path());
    assert!(SessionStore::open(store.folder()).is_err());
}

#[test]
fn legacy_read_is_byte_preserving_and_rejects_path_traversal() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("audio")).unwrap();
    let manifest = root.path().join("session.json");
    let original = br#"{"SchemaVersion":1,"Id":"legacy","Title":"Original","StartedAt":"2026-10-25T10:00:00+01:00","Status":"complete","ModelId":"m","Parts":[{"File":"system-000000000000.wav","Source":"system","StartSample":0,"SampleCount":16000,"AudioClosed":true,"Status":"complete","Text":"Legacy original"}]}"#;
    fs::write(&manifest, original).unwrap();
    fs::write(
        root.path().join("audio/system-000000000000.wav"),
        b"legacy fixture placeholder, never opened by reader",
    )
    .unwrap();
    let before: Vec<_> = fs::read_dir(root.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let parsed = read_legacy(&manifest).unwrap();
    assert!(parsed.read_only);
    assert_eq!(parsed.parts[0].text.as_deref(), Some("Legacy original"));
    assert_eq!(fs::read(&manifest).unwrap(), original);
    let after: Vec<_> = fs::read_dir(root.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after);
    fs::write(
        &manifest,
        String::from_utf8(original.to_vec())
            .unwrap()
            .replace("system-000000000000.wav", "../secret.wav"),
    )
    .unwrap();
    assert!(read_legacy(&manifest).is_err());
}

#[test]
fn model_changes_retain_prior_transcript_checkpoint_bytes() {
    let root = tempfile::tempdir().unwrap();
    let store = fixture(root.path());
    let original = fs::read(store.folder().join("transcript-journal.jsonl")).unwrap();
    let part = store.snapshot().unwrap().parts[0].clone();
    let mut update = checkpoint(&part, "New model result.");
    update.model_id = "second-model".into();
    store.append_transcript(update).unwrap();
    let current = fs::read(store.folder().join("transcript-journal.jsonl")).unwrap();
    assert!(current.starts_with(&original));
}

#[test]
fn oversized_audio_packets_are_rejected_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::create(root.path(), "Fixture", "m", "s", None).unwrap();
    let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
    assert!(writer.append(&vec![1; 80_001]).is_err());
    assert!(store.snapshot().unwrap().parts.is_empty());
}

#[test]
fn normal_finished_session_reopens_from_full_manifest_and_earlier_journal_entries() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::create(root.path(), "Finished", "m", "s", None).unwrap();
    let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
    writer.append(&vec![1; 8000]).unwrap();
    writer.checkpoint().unwrap();
    writer.append(&vec![2; 8000]).unwrap();
    writer.finish().unwrap();
    let mut text = checkpoint(&store.snapshot().unwrap().parts[0], "Complete original.");
    text.model_id = "m".into();
    store.append_transcript(text).unwrap();
    store.set_status("recorded").unwrap();
    let folder = store.folder().to_owned();
    drop(writer);
    drop(store);
    let reopened = SessionStore::open(&folder).unwrap();
    assert_eq!(
        reopened.snapshot().unwrap().parts[0].confirmed_sample_count,
        16000
    );
    assert_eq!(
        reopened
            .transcript_text()
            .unwrap()
            .matches("Complete original.")
            .count(),
        1
    );
}

#[test]
fn durable_manifest_prevents_downgrade_when_latest_journal_tails_are_damaged() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::create(root.path(), "Tail damage", "m", "s", None).unwrap();
    let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
    writer.append(&vec![1; 8000]).unwrap();
    writer.checkpoint().unwrap();
    let mut first = checkpoint(&store.snapshot().unwrap().parts[0], "Earlier text.");
    first.model_id = "m".into();
    store.append_transcript(first).unwrap();
    writer.append(&vec![2; 8000]).unwrap();
    writer.finish().unwrap();
    let mut last = checkpoint(
        &store.snapshot().unwrap().parts[0],
        "Earlier text. Latest confirmed text.",
    );
    last.model_id = "m".into();
    store.append_transcript(last).unwrap();
    store.set_status("recorded").unwrap();
    let folder = store.folder().to_owned();
    drop(writer);
    drop(store);
    for name in ["audio-journal.jsonl", "transcript-journal.jsonl"] {
        let path = folder.join(name);
        let text = fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = text.lines().collect();
        fs::write(
            path,
            format!(
                "{}\n{{\"damagedTail\":",
                lines[..lines.len() - 1].join("\n")
            ),
        )
        .unwrap();
    }
    let recovered = SessionStore::open(&folder).unwrap();
    let part = &recovered.snapshot().unwrap().parts[0];
    assert_eq!(part.confirmed_sample_count, 16000);
    assert_eq!(
        part.text.as_deref(),
        Some("Earlier text. Latest confirmed text.")
    );
    assert_eq!(part.transcript_sequence, 2);
}

#[test]
#[ignore = "Dedicated child entry point; invoked only by hard_killed_child_recovers_both_confirmed_tracks"]
fn crash_writer_child() {
    let root = std::env::var_os("MEETING_CORE_CRASH_ROOT").expect("dedicated test child only");
    let store = if std::env::var_os("MEETING_CORE_REPLAY_CHILD").is_some() {
        let store = SessionStore::create(
            Path::new(&root),
            "Synthetic interrupted replay",
            "synthetic-model",
            "generated",
            None,
        )
        .unwrap();
        let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
        for _ in 0..20 {
            writer.append(&vec![9; 16000]).unwrap();
        }
        writer.finish().unwrap();
        let part = store.snapshot().unwrap().parts[0].clone();
        store
            .append_transcript(checkpoint(&part, "Confirmed twenty-second original."))
            .unwrap();
        let mut earlier = checkpoint(&part, "partial replay");
        earlier.processed_samples = 8000;
        assert!(store.append_transcript(earlier).is_err());
        store
    } else {
        fixture(Path::new(&root))
    };
    let folder = store.folder().to_owned();
    let mut ready = File::create(Path::new(&root).join("ready.txt")).unwrap();
    write!(ready, "{}", folder.display()).unwrap();
    ready.sync_all().unwrap();
    // Keep lease and in-memory state live until parent force-kills this own process.
    let _store = store;
    loop {
        thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn hard_killed_child_recovers_both_confirmed_tracks() {
    let root = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_writer_child", "--ignored", "--nocapture"])
        .env("MEETING_CORE_CRASH_ROOT", root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let ready = root.path().join("ready.txt");
    while !ready.exists() {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("Synthetic capture child exited before ready: {status}");
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("Synthetic capture child readiness timed out");
        }
        thread::sleep(Duration::from_millis(20));
    }
    let folder = std::path::PathBuf::from(fs::read_to_string(ready).unwrap());
    let expected: Vec<_> = fs::read_dir(folder.join("audio"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.clone(), hash(&path))
        })
        .collect();
    child.kill().unwrap();
    child.wait().unwrap();
    let store = SessionStore::open(&folder).unwrap();
    let data = store.snapshot().unwrap();
    assert_eq!(data.parts.len(), 2);
    assert!(data
        .parts
        .iter()
        .all(|p| p.confirmed_sample_count == 16_000 && p.text.is_some()));
    for (path, digest) in expected {
        assert_eq!(hash(&path), digest);
    }
}

#[test]
fn second_process_crash_during_replay_retains_twenty_second_checkpoint() {
    let root = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_writer_child", "--ignored", "--nocapture"])
        .env("MEETING_CORE_CRASH_ROOT", root.path())
        .env("MEETING_CORE_REPLAY_CHILD", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let ready = root.path().join("ready.txt");
    let deadline = Instant::now() + Duration::from_secs(15);
    while !ready.exists() {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("Synthetic replay child exited: {status}");
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("Synthetic replay child timed out");
        }
        thread::sleep(Duration::from_millis(20));
    }
    let folder = std::path::PathBuf::from(fs::read_to_string(ready).unwrap());
    let original = fs::read(folder.join("transcript-journal.jsonl")).unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    let store = SessionStore::open(&folder).unwrap();
    let part = &store.snapshot().unwrap().parts[0];
    assert_eq!(part.processed_samples, 320000);
    assert_eq!(
        part.text.as_deref(),
        Some("Confirmed twenty-second original.")
    );
    assert_eq!(
        fs::read(folder.join("transcript-journal.jsonl")).unwrap(),
        original
    );
}

#[test]
fn committed_text_and_progress_cannot_regress_in_same_model() {
    let root = tempfile::tempdir().unwrap();
    let store = fixture(root.path());
    let part = store.snapshot().unwrap().parts[0].clone();
    let before = fs::read(store.folder().join("transcript-journal.jsonl")).unwrap();
    let mut earlier = checkpoint(&part, "partial replay");
    earlier.processed_samples = 8000;
    assert!(store.append_transcript(earlier).is_err());
    assert!(store
        .append_transcript(checkpoint(&part, "Changed committed text"))
        .is_err());
    assert_eq!(store.snapshot().unwrap().parts[0].text, part.text);
    assert_eq!(
        fs::read(store.folder().join("transcript-journal.jsonl")).unwrap(),
        before
    );
}

#[test]
fn legacy_final_batch_manifest_beats_older_stream_and_other_model_journal() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("audio")).unwrap();
    let manifest = root.path().join("session.json");
    let audio = root.path().join("audio/system-000000000000.wav");
    let journal = root.path().join("transcript-journal.jsonl");
    fs::write(&manifest,br#"{"SchemaVersion":1,"Id":"legacy","Title":"Original","StartedAt":"2026-10-25T10:00:00+01:00","Status":"complete","ModelId":"new","SnapshotSequence":10,"Parts":[{"File":"system-000000000000.wav","Source":"system","StartSample":0,"SampleCount":16000,"ProcessedSamples":16000,"ModelId":"new","AudioClosed":true,"Status":"complete","Text":"New complete batch result."}]}"#).unwrap();
    fs::write(&audio, b"preserved legacy fixture").unwrap();
    fs::write(&journal,b"{\"Sequence\":9,\"File\":\"system-000000000000.wav\",\"ModelId\":\"new\",\"Full\":\"Old partial\",\"Committed\":\"Old\",\"ProcessedSamples\":8000,\"Final\":false}\n{\"Sequence\":11,\"File\":\"system-000000000000.wav\",\"ModelId\":\"old\",\"Full\":\"Other model\",\"ProcessedSamples\":16000,\"Final\":true}\n").unwrap();
    let before = [hash(&manifest), hash(&audio), hash(&journal)];
    let data = read_legacy(&manifest).unwrap();
    assert_eq!(
        data.parts[0].text.as_deref(),
        Some("New complete batch result.")
    );
    assert_eq!(data.parts[0].status, "complete");
    assert_eq!(data.parts[0].model_id.as_deref(), Some("new"));
    assert_eq!([hash(&manifest), hash(&audio), hash(&journal)], before);
    assert!(!root.path().join("session.lock").exists());
}
