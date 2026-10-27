use super::*;
use crate::{PcmWriter, TranscriptCheckpoint};
#[cfg(windows)]
use fs2::FileExt;
#[test]
fn compact_archive_protects_changes_and_vip_and_resumes_after_cancellation() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let flag = AtomicBool::new(false);
    let receipt = create(root.path(), &id, 0, "whole-meeting", &flag, |_, _| {}).unwrap();
    move_source_into_archive(root.path(), &receipt, 0, &flag).unwrap();
    let source = root
        .path()
        .join("archives")
        .join(format!("{}.source", receipt.archive_id));
    let zip = root
        .path()
        .join("archives")
        .join(format!("{}.zip", receipt.archive_id));
    let original_zip = fs::read(&zip).unwrap();
    let extra = source.join("late-file.txt");
    fs::write(&extra, b"new user data").unwrap();
    assert!(compact_source_copy(root.path(), &receipt.archive_id, &flag, |_, _| {}).is_err());
    assert_eq!(fs::read(&extra).unwrap(), b"new user data");
    fs::remove_file(extra).unwrap();
    metadata::edit(root.path(), &id, 0, None, true).unwrap();
    assert!(compact_source_copy(root.path(), &receipt.archive_id, &flag, |_, _| {}).is_err());
    metadata::edit(root.path(), &id, 1, None, false).unwrap();
    assert!(
        compact_source_copy(root.path(), &receipt.archive_id, &flag, |_, _| {
            flag.store(true, Ordering::Release);
        })
        .is_err()
    );
    assert_eq!(fs::read(&zip).unwrap(), original_zip);
    flag.store(false, Ordering::Release);
    compact_source_copy(root.path(), &receipt.archive_id, &flag, |_, _| {}).unwrap();
    assert!(!source.exists());
    assert_eq!(fs::read(&zip).unwrap(), original_zip);
    let restored = restore(root.path(), &receipt.archive_id, &flag).unwrap();
    let data = SessionStore::open(&restored).unwrap().snapshot().unwrap();
    assert_eq!(
        data.parts[0].text.as_deref(),
        Some("Original retained text")
    );
}
fn fixture(root: &Path) -> String {
    fs::create_dir_all(root.join("sessions")).unwrap();
    let store = SessionStore::create(
        &root.join("sessions"),
        "Archive fixture",
        "fixture-model",
        "system",
        None,
    )
    .unwrap();
    let id = store.snapshot().unwrap().id;
    let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
    writer.append(&[42; 1600]).unwrap();
    writer.finish().unwrap();
    drop(writer);
    let part = store.snapshot().unwrap().parts[0].clone();
    store
        .append_transcript(TranscriptCheckpoint {
            sequence: 0,
            at: String::new(),
            file: part.file,
            model_id: "fixture-model".into(),
            full: "Original retained text".into(),
            committed: "Original retained text".into(),
            tentative: String::new(),
            processed_samples: 1600,
            final_text: true,
            revision_after_restart: false,
        })
        .unwrap();
    store.set_status("recorded").unwrap();
    store.set_status("complete").unwrap();
    drop(store);
    id
}
#[test]
fn lossless_archive_roundtrip_retains_original_text_audio_and_unknown_files() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let folder = root.path().join("sessions").join(&id);
    fs::write(folder.join("user-owned.txt"), b"Retain my own bytes").unwrap();
    let receipt = create(
        root.path(),
        &id,
        0,
        "whole-meeting",
        &AtomicBool::new(false),
        |_, _| {},
    )
    .unwrap();
    let path = root
        .path()
        .join("archives")
        .join(format!("{}.zip", receipt.archive_id));
    assert!(verify(&path, None, |_, _| {}).is_ok());
    assert!(restore_new(root.path(), &receipt.archive_id, &AtomicBool::new(false)).is_err());
    let original = root.path().join("original-preserved");
    fs::rename(&folder, &original).unwrap();
    let restored = restore_new(root.path(), &receipt.archive_id, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        fs::read(restored.join("user-owned.txt")).unwrap(),
        b"Retain my own bytes"
    );
    let store = SessionStore::open(&restored).unwrap();
    assert_eq!(
        store.snapshot().unwrap().parts[0].text.as_deref(),
        Some("Original retained text")
    );
    assert!(original.join("audio").is_dir());
}
#[test]
fn pin_active_lease_and_cancellation_protect_originals() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    metadata::edit(root.path(), &id, 0, None, true).unwrap();
    assert!(create(
        root.path(),
        &id,
        1,
        "whole-meeting",
        &AtomicBool::new(false),
        |_, _| {}
    )
    .is_err());
    metadata::edit(root.path(), &id, 1, None, false).unwrap();
    let store = SessionStore::open(&root.path().join("sessions").join(&id)).unwrap();
    assert!(create(
        root.path(),
        &id,
        2,
        "whole-meeting",
        &AtomicBool::new(false),
        |_, _| {}
    )
    .is_err());
    drop(store);
    assert!(create(
        root.path(),
        &id,
        2,
        "whole-meeting",
        &AtomicBool::new(true),
        |_, _| {}
    )
    .is_err());
    assert!(root
        .path()
        .join("sessions")
        .join(id)
        .join("session.json")
        .is_file());
}
#[test]
fn audio_only_archive_keeps_readable_text_and_restores_exact_original_pcm() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let folder = root.path().join("sessions").join(&id);
    let original = fs::read(folder.join("session.json")).unwrap();
    let receipt = archive_audio(root.path(), &id, 0, &AtomicBool::new(false), |_, _| {}).unwrap();
    assert!(receipt.original_bytes > 0);
    assert_eq!(fs::read(folder.join("session.json")).unwrap(), original);
    let text = read_archived_transcript(&folder).unwrap().unwrap();
    assert!(text.read_only);
    assert_eq!(
        text.parts[0].text.as_deref(),
        Some("Original retained text")
    );
    assert!(!folder.join("audio").join(&text.parts[0].file).exists());
    let other_owner = crate::storage::lease(&folder).unwrap();
    assert!(restore_audio(root.path(), &id, &AtomicBool::new(false)).is_err());
    assert!(!folder.join("audio").join(&text.parts[0].file).exists());
    drop(other_owner);
    let restored = restore_audio(root.path(), &id, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        restored.parts[0].confirmed_sha256,
        text.parts[0].confirmed_sha256
    );
    assert!(read_archived_transcript(&folder).unwrap().is_none());
}
#[test]
fn interrupted_whole_meeting_restore_never_blocks_a_verified_retry() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let cancel = AtomicBool::new(false);
    let receipt = create(root.path(), &id, 0, "whole-meeting", &cancel, |_, _| {}).unwrap();
    move_source_into_archive(root.path(), &receipt, 0, &cancel).unwrap();
    fs::remove_dir_all(
        root.path()
            .join("archives")
            .join(format!("{}.source", receipt.archive_id)),
    )
    .unwrap();
    // Leftover of an earlier attempt that stopped mid-extraction.
    let stale = root
        .path()
        .join("archives")
        .join(format!("{}.restoring-earlier", receipt.archive_id));
    fs::create_dir_all(stale.join(&id).join("audio")).unwrap();
    fs::write(
        stale.join(&id).join("audio").join("partial.pcm"),
        b"partial",
    )
    .unwrap();
    let page = list(root.path(), 0, 25).unwrap();
    assert!(!page.items[0].session_present);
    let restored = restore_new(root.path(), &receipt.archive_id, &cancel).unwrap();
    // The storage root is canonical internally (/private/var on macOS, \\?\ on Windows).
    assert_eq!(
        restored.canonicalize().unwrap(),
        root.path()
            .join("sessions")
            .join(&id)
            .canonicalize()
            .unwrap()
    );
    assert!(restored.join("session.json").is_file());
    assert!(
        stale.join(&id).join("audio").join("partial.pcm").is_file(),
        "earlier evidence retained"
    );
    let staging: Vec<_> = fs::read_dir(root.path().join("archives"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".restoring-") && !n.ends_with("-earlier"))
        .collect();
    assert!(
        staging.is_empty(),
        "published staging folder is renamed, not copied"
    );
    assert!(restore_new(root.path(), &receipt.archive_id, &cancel).is_err());
}
#[test]
fn restored_audio_can_be_archived_again_without_duplicate_zips_or_lost_provenance() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let folder = root.path().join("sessions").join(&id);
    let cancel = AtomicBool::new(false);
    let zips = || {
        fs::read_dir(root.path().join("archives"))
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .and_then(|x| x.to_str())
                    == Some("zip")
            })
            .count()
    };
    let first = archive_audio(root.path(), &id, 0, &cancel, |_, _| {}).unwrap();
    assert_eq!(zips(), 1);
    // Still archived: rejected before another lossless copy is written.
    assert!(archive_audio(root.path(), &id, 0, &cancel, |_, _| {}).is_err());
    assert_eq!(zips(), 1);
    restore_audio(root.path(), &id, &cancel).unwrap();
    let second = archive_audio(root.path(), &id, 0, &cancel, |_, _| {}).unwrap();
    assert_ne!(first.archive_id, second.archive_id);
    assert_eq!(zips(), 2);
    let history = folder.join(format!("audio-archive.{}.restored.json", first.archive_id));
    let old: serde_json::Value = serde_json::from_slice(&fs::read(history).unwrap()).unwrap();
    assert_eq!(old["restored"], true);
    assert_eq!(old["receipt"]["archiveId"], first.archive_id.as_str());
    assert!(read_archived_transcript(&folder).unwrap().is_some());
    let restored = restore_audio(root.path(), &id, &cancel).unwrap();
    assert!(folder.join("audio").join(&restored.parts[0].file).is_file());
}
#[test]
fn invalid_restored_audio_marker_cannot_rename_provenance_outside_the_session_or_create_a_zip() {
    // Synthetic supplied/archive-restored markers, never real user sessions.
    for invalid in [
        "traversal",
        "receipt-meeting",
        "session-meeting",
        "kind",
        "legacy-id",
    ] {
        let root = tempfile::tempdir().unwrap();
        let id = fixture(root.path());
        let folder = root.path().join("sessions").join(&id);
        let original_manifest = fs::read(folder.join("session.json")).unwrap();
        let audio: Vec<_> = fs::read_dir(folder.join("audio"))
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let bytes = fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect();
        let session = crate::inspect_manifest(&folder).unwrap();
        let mut marker = AudioArchive {
            receipt: Receipt {
                archive_id: "b".repeat(32),
                meeting_id: id.clone(),
                kind: "audio-only".into(),
                archive_bytes: 0,
                original_bytes: 0,
                archive_sha256: "0".repeat(64),
                files: 0,
                title: "Synthetic retained provenance".into(),
                created_at: "2026-10-27T12:00:00Z".into(),
            },
            session,
            originals: vec![],
            restored: true,
        };
        match invalid {
            "traversal" => marker.receipt.archive_id = "x/../../../outside".into(),
            "receipt-meeting" => marker.receipt.meeting_id = "c".repeat(32),
            "session-meeting" => marker.session.id = "c".repeat(32),
            "kind" => marker.receipt.kind = "whole-meeting".into(),
            "legacy-id" => marker.receipt.archive_id = format!("legacy-{}", "b".repeat(16)),
            _ => unreachable!(),
        }
        // This ordinary directory makes the hostile history destination traversable.
        // The possible target is still inside this test's owned temporary root.
        fs::create_dir(folder.join("audio-archive.x")).unwrap();
        let marker_path = folder.join("audio-archive.json");
        let retained_marker = serde_json::to_vec_pretty(&marker).unwrap();
        fs::write(&marker_path, &retained_marker).unwrap();
        let archives = root.path().join("archives");
        let zip_count = || {
            fs::read_dir(&archives)
                .ok()
                .map(|entries| {
                    entries
                        .filter(|e| {
                            e.as_ref()
                                .unwrap()
                                .path()
                                .extension()
                                .and_then(|s| s.to_str())
                                == Some("zip")
                        })
                        .count()
                })
                .unwrap_or(0)
        };
        let before = zip_count();
        assert!(
            archive_audio(root.path(), &id, 0, &AtomicBool::new(false), |_, _| {}).is_err(),
            "Invalid {invalid} marker must be rejected before creating another archive"
        );
        assert!(
            read_archived_transcript(&folder).is_err(),
            "A restored flag must not bypass identity checks"
        );
        assert_eq!(
            zip_count(),
            before,
            "Invalid {invalid} marker must not create an extra ZIP"
        );
        assert!(!root.path().join("outside.restored.json").exists());
        assert_eq!(fs::read(&marker_path).unwrap(), retained_marker);
        assert_eq!(
            fs::read(folder.join("session.json")).unwrap(),
            original_manifest
        );
        for (path, bytes) in audio {
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        assert!(!folder
            .join(format!("audio-archive.{}.restored.json", "b".repeat(32)))
            .exists());
    }
}
#[test]
fn archive_browser_restores_verified_sources_and_purge_requires_unchanged_group_and_exact_confirmation(
) {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let cancel = AtomicBool::new(false);
    let receipt = create(root.path(), &id, 0, "whole-meeting", &cancel, |_, _| {}).unwrap();
    move_source_into_archive(root.path(), &receipt, 0, &cancel).unwrap();
    let page = list(root.path(), 0, 25).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].session_present);
    assert!(page.items[0].source_copy_retained);
    let (fingerprint, _) = deletion_fingerprint(root.path(), &id, &cancel).unwrap();
    assert!(delete_archived_meeting(
        root.path(),
        &id,
        0,
        &fingerprint,
        "DELETE",
        &cancel,
        |_, _| {}
    )
    .is_err());
    metadata::edit(root.path(), &id, 0, None, true).unwrap();
    assert!(delete_archived_meeting(
        root.path(),
        &id,
        0,
        &fingerprint,
        &format!("DELETE {id}"),
        &cancel,
        |_, _| {}
    )
    .is_err());
    metadata::edit(root.path(), &id, 1, None, false).unwrap();
    let copied = root
        .path()
        .join("archives")
        .join(format!("{}.source", receipt.archive_id));
    fs::write(copied.join("late-user-file.txt"), b"must survive").unwrap();
    assert!(deletion_fingerprint(root.path(), &id, &cancel).is_err());
    assert!(delete_archived_meeting(
        root.path(),
        &id,
        2,
        &fingerprint,
        &format!("DELETE {id}"),
        &cancel,
        |_, _| {}
    )
    .is_err());
    assert_eq!(
        fs::read(copied.join("late-user-file.txt")).unwrap(),
        b"must survive"
    );
    fs::remove_file(copied.join("late-user-file.txt")).unwrap();
    let restored = restore(root.path(), &receipt.archive_id, &cancel).unwrap();
    assert!(restored.join("session.json").is_file());
    assert!(deletion_fingerprint(root.path(), &id, &cancel).is_err());
}

fn legacy_receipt_fixture(root: &Path) -> (String, Receipt, PathBuf) {
    legacy_receipt_fixture_with_dates(root, "2000-01-01T00:00:00Z", "2000-01-01T00:00:01Z")
}
fn legacy_receipt_fixture_with_dates(
    root: &Path,
    started: &str,
    ended: &str,
) -> (String, Receipt, PathBuf) {
    let id = fixture(root);
    let manifest_path = root.join("sessions").join(&id).join("session.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    // An old meeting newly archived today must not become deletion-eligible
    // merely because the old receipt omitted its archive creation date.
    manifest["startedAt"] = serde_json::json!(started);
    manifest["endedAt"] = serde_json::json!(ended);
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let cancel = AtomicBool::new(false);
    let receipt = create(root, &id, 0, "whole-meeting", &cancel, |_, _| {}).unwrap();
    move_source_into_archive(root, &receipt, 0, &cancel).unwrap();
    let path = root
        .join("archives")
        .join(format!("{}.receipt.json", receipt.archive_id));
    let mut legacy: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    legacy.as_object_mut().unwrap().remove("createdAt");
    fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    (id, receipt, path)
}

#[test]
fn legacy_archive_with_newer_start_date_never_ages_from_an_earlier_meeting_end() {
    let root = tempfile::tempdir().unwrap();
    let now = chrono::Utc::now();
    let started = now + chrono::Duration::days(365 * 25);
    let ended = now - chrono::Duration::days(365 * 25);
    // Synthetic imported/clock-rollback metadata: archive verification does not
    // assert that start <= end, so deletion must consider every known timestamp.
    let (id, receipt, path) =
        legacy_receipt_fixture_with_dates(root.path(), &started.to_rfc3339(), &ended.to_rfc3339());
    let cancel = AtomicBool::new(false);
    let before = fs::read(&path).unwrap();
    let zip = root
        .path()
        .join("archives")
        .join(format!("{}.zip", receipt.archive_id));
    let original_zip = fs::read(&zip).unwrap();
    let (_, newest) = deletion_fingerprint(root.path(), &id, &cancel).unwrap();
    assert!(
        chrono::DateTime::parse_from_rfc3339(&newest).unwrap() >= started,
        "A newer known start must extend protection, never be discarded in favour of an older end"
    );
    crate::retention::save_policy(
        root.path(),
        0,
        crate::retention::Policy {
            delete_archives_after_days: Some(30),
            ..Default::default()
        },
    )
    .unwrap();
    let preview = crate::retention::preview(
        root.path(),
        now + chrono::Duration::days(31),
        &cancel,
        |_, _| {},
    )
    .unwrap();
    assert!(preview.candidates.is_empty());
    assert!(preview.protected >= 1);
    assert_eq!(preview.unreadable, 0);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::read(&zip).unwrap(), original_zip);
    assert!(!root.path().join("maintenance-log").exists());
}

#[test]
fn legacy_receipts_keep_all_files_and_conservative_archive_age_before_explicit_deletion() {
    let root = tempfile::tempdir().unwrap();
    let before_creation = chrono::Utc::now() - chrono::Duration::seconds(2);
    let (id, receipt, receipt_path) = legacy_receipt_fixture(root.path());
    let cancel = AtomicBool::new(false);
    let zip = root
        .path()
        .join("archives")
        .join(format!("{}.zip", receipt.archive_id));
    let source = root
        .path()
        .join("archives")
        .join(format!("{}.source", receipt.archive_id));
    let zip_bytes = fs::read(&zip).unwrap();
    let receipt_bytes = fs::read(&receipt_path).unwrap();
    let (hash, newest) = deletion_fingerprint(root.path(), &id, &cancel).unwrap();
    assert!(chrono::DateTime::parse_from_rfc3339(&newest).unwrap() >= before_creation);
    let inventory = deletion::deletion_inventory(root.path(), &id, &cancel).unwrap();
    assert!(inventory
        .files
        .iter()
        .any(|f| f.name == format!("archives/{}.zip", receipt.archive_id)));
    assert!(inventory
        .files
        .iter()
        .any(|f| f.name == format!("archives/{}.receipt.json", receipt.archive_id)));
    assert!(inventory.files.iter().any(|f| f
        .name
        .starts_with(&format!("archives/{}.source/", receipt.archive_id))));
    let policy = crate::retention::Policy {
        delete_archives_after_days: Some(30),
        ..Default::default()
    };
    crate::retention::save_policy(root.path(), 0, policy).unwrap();
    let now = chrono::Utc::now();
    let too_recent = crate::retention::preview(
        root.path(),
        now + chrono::Duration::hours(1),
        &cancel,
        |_, _| {},
    )
    .unwrap();
    assert!(too_recent.candidates.is_empty());
    assert!(too_recent.protected >= 1);
    assert_eq!(
        too_recent.unreadable, 0,
        "A valid legacy receipt is supported, not classified as damaged"
    );
    // Synthetic calendar advancement, not a real thirty-one-day waiting test.
    let eligible = crate::retention::preview(
        root.path(),
        now + chrono::Duration::days(31),
        &cancel,
        |_, _| {},
    )
    .unwrap();
    assert_eq!(eligible.candidates.len(), 1);
    assert_eq!(
        eligible.candidates[0].action,
        crate::retention::Action::DeleteArchivedMeeting
    );
    assert_eq!(eligible.candidates[0].original_manifest_sha256, hash);
    assert!(
        delete_archived_meeting(root.path(), &id, 0, &hash, "DELETE", &cancel, |_, _| {}).is_err()
    );
    assert!(delete_archived_meeting(
        root.path(),
        &id,
        0,
        &hash,
        &format!("DELETE {id}"),
        &AtomicBool::new(true),
        |_, _| {}
    )
    .is_err());
    assert_eq!(fs::read(&zip).unwrap(), zip_bytes);
    assert_eq!(fs::read(&receipt_path).unwrap(), receipt_bytes);
    assert!(source.is_dir());
    // The explicit low-level deletion API still requires the exact current inventory
    // and confirmation; preview itself never mutates or silently migrates receipts.
    delete_archived_meeting(
        root.path(),
        &id,
        0,
        &hash,
        &format!("DELETE {id}"),
        &cancel,
        |_, _| {},
    )
    .unwrap();
    assert!(!zip.exists());
    assert!(!receipt_path.exists());
    assert!(!source.exists());
}

#[test]
fn malformed_nonempty_archive_dates_remain_protected_without_deleting_any_original() {
    let root = tempfile::tempdir().unwrap();
    let (id, receipt, path) = legacy_receipt_fixture(root.path());
    let cancel = AtomicBool::new(false);
    let (prior_hash, _) = deletion_fingerprint(root.path(), &id, &cancel).unwrap();
    let zip = root
        .path()
        .join("archives")
        .join(format!("{}.zip", receipt.archive_id));
    let zip_bytes = fs::read(&zip).unwrap();
    crate::retention::save_policy(
        root.path(),
        0,
        crate::retention::Policy {
            delete_archives_after_days: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    for invalid in [" ", "not-a-date", "2026-13-45T00:00:00Z"] {
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["createdAt"] = serde_json::json!(invalid);
        let bad = serde_json::to_vec(&value).unwrap();
        fs::write(&path, &bad).unwrap();
        assert!(deletion_fingerprint(root.path(), &id, &cancel).is_err());
        let preview = crate::retention::preview(
            root.path(),
            chrono::Utc::now() + chrono::Duration::days(365),
            &cancel,
            |_, _| {},
        )
        .unwrap();
        assert!(preview.candidates.is_empty());
        assert!(preview.protected >= 1);
        assert!(preview.unreadable >= 1);
        assert!(delete_archived_meeting(
            root.path(),
            &id,
            0,
            &prior_hash,
            &format!("DELETE {id}"),
            &cancel,
            |_, _| {}
        )
        .is_err());
        assert_eq!(fs::read(&path).unwrap(), bad);
        assert_eq!(fs::read(&zip).unwrap(), zip_bytes);
    }
    assert!(!root.path().join("maintenance-log").exists());
}

#[test]
fn changed_legacy_archive_age_provenance_invalidates_a_byte_identical_old_preview() {
    let root = tempfile::tempdir().unwrap();
    let (id, receipt, path) = legacy_receipt_fixture(root.path());
    let cancel = AtomicBool::new(false);
    let (old_hash, old_date) = deletion_fingerprint(root.path(), &id, &cancel).unwrap();
    let before = fs::read(&path).unwrap();
    // Synthetic filesystem timestamp change, with all archive and receipt bytes intact.
    let touched = std::time::SystemTime::now() + std::time::Duration::from_secs(3 * 86400);
    OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(touched))
        .unwrap();
    let (new_hash, new_date) = deletion_fingerprint(root.path(), &id, &cancel).unwrap();
    assert_ne!(new_hash, old_hash);
    assert!(
        chrono::DateTime::parse_from_rfc3339(&new_date).unwrap()
            > chrono::DateTime::parse_from_rfc3339(&old_date).unwrap()
    );
    assert!(delete_archived_meeting(
        root.path(),
        &id,
        0,
        &old_hash,
        &format!("DELETE {id}"),
        &cancel,
        |_, _| {}
    )
    .is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(root
        .path()
        .join("archives")
        .join(format!("{}.zip", receipt.archive_id))
        .is_file());
    assert!(!root.path().join("maintenance-log").exists());
}
#[test]
fn permanent_deletion_removes_all_verified_archive_copies_and_annotations_but_preserves_other_meetings(
) {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let cancel = AtomicBool::new(false);
    metadata::edit(root.path(), &id, 0, Some("Earlier title".into()), false).unwrap();
    metadata::edit(root.path(), &id, 1, Some("Archived fixture".into()), false).unwrap();
    let backup = root
        .path()
        .join("user-metadata")
        .join(format!("{id}.json.bak"));
    assert!(backup.is_file(), "a second label save leaves a backup");
    let first = create(root.path(), &id, 2, "whole-meeting", &cancel, |_, _| {}).unwrap();
    let second = create(root.path(), &id, 2, "whole-meeting", &cancel, |_, _| {}).unwrap();
    move_source_into_archive(root.path(), &second, 2, &cancel).unwrap();
    let other = fixture(root.path());
    let (hash, _) = deletion_fingerprint(root.path(), &id, &cancel).unwrap();
    assert!(delete_archived_meeting(
        root.path(),
        &id,
        2,
        &hash,
        &format!("DELETE {id}"),
        &AtomicBool::new(true),
        |_, _| {}
    )
    .is_err());
    delete_archived_meeting(
        root.path(),
        &id,
        2,
        &hash,
        &format!("DELETE {id}"),
        &cancel,
        |_, _| {},
    )
    .unwrap();
    for aid in [&first.archive_id, &second.archive_id] {
        assert!(!root
            .path()
            .join("archives")
            .join(format!("{aid}.zip"))
            .exists());
        assert!(!root
            .path()
            .join("archives")
            .join(format!("{aid}.receipt.json"))
            .exists());
    }
    assert!(!root
        .path()
        .join("user-metadata")
        .join(format!("{id}.json"))
        .exists());
    assert!(
        !backup.exists(),
        "label backups are deleted with the meeting"
    );
    assert!(root
        .path()
        .join("sessions")
        .join(other)
        .join("session.json")
        .is_file());
    // atomic_json keeps a .json.bak of the previous phase; directory order is filesystem-specific.
    let logs: Vec<_> = fs::read_dir(root.path().join("maintenance-log"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    assert_eq!(logs.len(), 1);
    let record: serde_json::Value = serde_json::from_slice(&fs::read(&logs[0]).unwrap()).unwrap();
    assert_eq!(record["phase"], "complete");
    assert_eq!(record["deletedFiles"], record["totalFiles"]);
}
#[test]
fn permanent_deletion_includes_interrupted_copies_and_blocks_on_unidentified_zips() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let cancel = AtomicBool::new(false);
    let receipt = create(root.path(), &id, 0, "whole-meeting", &cancel, |_, _| {}).unwrap();
    move_source_into_archive(root.path(), &receipt, 0, &cancel).unwrap();
    let archives = root.path().join("archives");
    // An archive write that stopped before its receipt, and a stopped restore.
    fs::write(
        archives.join("0123456789abcdef0123456789abcdef.pending.json"),
        serde_json::to_vec(&serde_json::json!({"meetingId": id, "kind": "whole-meeting"})).unwrap(),
    )
    .unwrap();
    fs::write(
        archives.join("0123456789abcdef0123456789abcdef.zip"),
        b"partial copy",
    )
    .unwrap();
    let staging = archives.join(format!("{}.restoring-stopped", receipt.archive_id));
    fs::create_dir_all(staging.join(&id).join("audio")).unwrap();
    fs::write(staging.join(&id).join("audio").join("part.pcm"), b"pcm").unwrap();
    let unknown = archives.join("fedcba9876543210fedcba9876543210.zip");
    fs::write(&unknown, b"not attributable").unwrap();
    assert!(deletion_fingerprint(root.path(), &id, &cancel).is_err());
    fs::remove_file(&unknown).unwrap();
    let (hash, _) = deletion_fingerprint(root.path(), &id, &cancel).unwrap();
    delete_archived_meeting(
        root.path(),
        &id,
        0,
        &hash,
        &format!("DELETE {id}"),
        &cancel,
        |_, _| {},
    )
    .unwrap();
    assert!(!archives
        .join("0123456789abcdef0123456789abcdef.zip")
        .exists());
    assert!(!archives
        .join("0123456789abcdef0123456789abcdef.pending.json")
        .exists());
    assert!(!staging.exists());
    let left: Vec<_> = fs::read_dir(&archives)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(left.is_empty(), "{left:?}");
}
#[test]
fn stopped_permanent_deletion_can_be_continued_from_its_log() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let cancel = AtomicBool::new(false);
    metadata::edit(root.path(), &id, 0, Some("To delete".into()), false).unwrap();
    let receipt = create(root.path(), &id, 1, "whole-meeting", &cancel, |_, _| {}).unwrap();
    move_source_into_archive(root.path(), &receipt, 1, &cancel).unwrap();
    let (hash, _) = deletion_fingerprint(root.path(), &id, &cancel).unwrap();
    let stop = AtomicBool::new(false);
    let zip = root
        .path()
        .join("archives")
        .join(format!("{}.zip", receipt.archive_id));
    // Stop right after the ZIP itself was removed: the copy is now incomplete.
    assert!(delete_archived_meeting(
        root.path(),
        &id,
        1,
        &hash,
        &format!("DELETE {id}"),
        &stop,
        |_, _| if !zip.exists() {
            stop.store(true, Ordering::Release)
        }
    )
    .is_err());
    assert!(!zip.exists());
    assert!(
        deletion_fingerprint(root.path(), &id, &cancel).is_err(),
        "a fresh preview cannot cover the remaining files"
    );
    let open = unfinished_deletions(root.path()).unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].meeting_id, id);
    assert!(open[0].deleted_files < open[0].total_files);
    assert!(resume_deletion(root.path(), &open[0].log_id, "DELETE", &cancel, |_, _| {}).is_err());
    // A file changed after confirmation is never deleted by the continuation.
    let receipt_file = root
        .path()
        .join("archives")
        .join(format!("{}.receipt.json", receipt.archive_id));
    let original = fs::read(&receipt_file).unwrap();
    fs::write(&receipt_file, b"changed later").unwrap();
    assert!(resume_deletion(
        root.path(),
        &open[0].log_id,
        &format!("DELETE {id}"),
        &cancel,
        |_, _| {}
    )
    .is_err());
    assert_eq!(fs::read(&receipt_file).unwrap(), b"changed later");
    fs::write(&receipt_file, original).unwrap();
    resume_deletion(
        root.path(),
        &open[0].log_id,
        &format!("DELETE {id}"),
        &cancel,
        |_, _| {},
    )
    .unwrap();
    assert!(unfinished_deletions(root.path()).unwrap().is_empty());
    let left: Vec<_> = fs::read_dir(root.path().join("archives"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(left.is_empty(), "{left:?}");
    assert!(!root
        .path()
        .join("user-metadata")
        .join(format!("{id}.json"))
        .exists());
    assert!(resume_deletion(
        root.path(),
        &open[0].log_id,
        &format!("DELETE {id}"),
        &cancel,
        |_, _| {}
    )
    .is_err());
}
#[test]
fn hostile_archive_paths_are_rejected_before_any_restore_destination_is_created() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let store = SessionStore::open(&root.path().join("sessions").join(&id)).unwrap();
    let data = store.snapshot().unwrap();
    drop(store);
    let archives = root.path().join("archives");
    fs::create_dir(&archives).unwrap();
    let archive_id = uuid::Uuid::new_v4().simple().to_string();
    let path = archives.join(format!("{archive_id}.zip"));
    let item = ArchivedFile {
        name: "session/../../outside.txt".into(),
        bytes: 6,
        sha256: format!("{:x}", Sha256::digest(b"unsafe")),
    };
    let manifest = Manifest {
        schema: 1,
        meeting_id: id,
        kind: "whole-meeting".into(),
        session: data,
        files: vec![item.clone()],
        directories: vec![],
        retained_sidecars: vec![],
    };
    let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    writer.start_file(&item.name, options).unwrap();
    writer.write_all(b"unsafe").unwrap();
    writer.start_file("ARCHIVE_MANIFEST.json", options).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer.finish().unwrap();
    assert!(verify(&path, None, |_, _| {}).is_err());
    assert!(restore_new(root.path(), &archive_id, &AtomicBool::new(false)).is_err());
    assert!(!root.path().join("outside.txt").exists());
}
#[test]
fn changed_archived_text_cannot_be_presented_as_its_original_journal_evidence() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let folder = root.path().join("sessions").join(&id);
    let original = fs::read(folder.join("session.json")).unwrap();
    let receipt = archive_audio(root.path(), &id, 0, &AtomicBool::new(false), |_, _| {}).unwrap();
    let path = folder.join("audio-archive.json");
    let mut marker: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    marker["session"]["parts"][0]["text"] = serde_json::json!("Invented replacement text");
    atomic_json(&path, &marker).unwrap();
    assert!(read_archived_transcript(&folder).is_err());
    assert!(restore_audio(root.path(), &id, &AtomicBool::new(false)).is_err());
    assert_eq!(fs::read(folder.join("session.json")).unwrap(), original);
    assert!(verify(
        &root
            .path()
            .join("archives")
            .join(format!("{}.zip", receipt.archive_id)),
        None,
        |_, _| {}
    )
    .is_ok());
}
#[test]
fn full_archive_preserves_empty_folders_and_annotation_versions_without_resetting_newer_pins() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let folder = root.path().join("sessions").join(&id);
    fs::create_dir_all(folder.join("empty-user-folder/nested")).unwrap();
    metadata::edit(
        root.path(),
        &id,
        0,
        Some("Earlier custom title".into()),
        true,
    )
    .unwrap();
    metadata::edit(
        root.path(),
        &id,
        1,
        Some("Archived custom title".into()),
        false,
    )
    .unwrap();
    let store = SessionStore::open(&folder).unwrap();
    let data = store.snapshot().unwrap();
    drop(store);
    let part = &data.parts[0];
    crate::speakers::save(
        root.path(),
        &data,
        0,
        true,
        vec![crate::speakers::Assignment {
            source_id: part.file.clone(),
            text_sha256: crate::speakers::text_hash(part.text.as_deref().unwrap()),
            name: "Manually confirmed label".into(),
            evidence: "Original retained text".into(),
        }],
    )
    .unwrap();
    let receipt = create(
        root.path(),
        &id,
        2,
        "whole-meeting",
        &AtomicBool::new(false),
        |_, _| {},
    )
    .unwrap();
    let zip = root
        .path()
        .join("archives")
        .join(format!("{}.zip", receipt.archive_id));
    let manifest = verify(&zip, None, |_, _| {}).unwrap();
    assert!(manifest.retained_sidecars.len() >= 5);
    let fresh = tempfile::tempdir().unwrap();
    fs::create_dir(fresh.path().join("sessions")).unwrap();
    fs::create_dir(fresh.path().join("archives")).unwrap();
    fs::copy(
        &zip,
        fresh
            .path()
            .join("archives")
            .join(format!("{}.zip", receipt.archive_id)),
    )
    .unwrap();
    let restored = restore_new(fresh.path(), &receipt.archive_id, &AtomicBool::new(false)).unwrap();
    assert!(restored.join("empty-user-folder/nested").is_dir());
    assert_eq!(
        metadata::read(fresh.path(), &id).unwrap().title.as_deref(),
        Some("Archived custom title")
    );
    assert_eq!(
        crate::speakers::read(fresh.path(), &id)
            .unwrap()
            .assignments
            .len(),
        1
    );
    metadata::edit(root.path(), &id, 2, Some("Newer VIP title".into()), true).unwrap();
    fs::rename(&folder, root.path().join("original-source-preserved")).unwrap();
    restore_new(root.path(), &receipt.archive_id, &AtomicBool::new(false)).unwrap();
    let current = metadata::read(root.path(), &id).unwrap();
    assert!(current.pinned);
    assert_eq!(current.title.as_deref(), Some("Newer VIP title"));
}
#[test]
fn verified_source_moves_to_archive_but_a_new_file_or_pin_blocks_a_stale_move() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let source = root.path().join("sessions").join(&id);
    let receipt = create(
        root.path(),
        &id,
        0,
        "whole-meeting",
        &AtomicBool::new(false),
        |_, _| {},
    )
    .unwrap();
    fs::write(source.join("late-user-file.txt"), b"Preserve my later edit").unwrap();
    assert!(move_source_into_archive(root.path(), &receipt, 0, &AtomicBool::new(false)).is_err());
    assert_eq!(
        fs::read(source.join("late-user-file.txt")).unwrap(),
        b"Preserve my later edit"
    );
    let current = create(
        root.path(),
        &id,
        0,
        "whole-meeting",
        &AtomicBool::new(false),
        |_, _| {},
    )
    .unwrap();
    metadata::edit(root.path(), &id, 0, None, true).unwrap();
    assert!(move_source_into_archive(root.path(), &current, 0, &AtomicBool::new(false)).is_err());
    metadata::edit(root.path(), &id, 1, None, false).unwrap();
    let current = create(
        root.path(),
        &id,
        2,
        "whole-meeting",
        &AtomicBool::new(false),
        |_, _| {},
    )
    .unwrap();
    let archive =
        move_source_into_archive(root.path(), &current, 2, &AtomicBool::new(false)).unwrap();
    assert!(!source.exists());
    assert_eq!(
        fs::read(archive.join("late-user-file.txt")).unwrap(),
        b"Preserve my later edit"
    );
    assert!(restore_new(root.path(), &current.archive_id, &AtomicBool::new(false)).is_ok());
}
#[test]
fn archive_list_shows_unreadable_receipts_and_labels_as_preserved_entries() {
    let root = tempfile::tempdir().unwrap();
    let cancel = AtomicBool::new(false);
    let id = fixture(root.path());
    let readable = create(root.path(), &id, 0, "whole-meeting", &cancel, |_, _| {}).unwrap();
    let other = fixture(root.path());
    let labels_broken =
        create(root.path(), &other, 0, "whole-meeting", &cancel, |_, _| {}).unwrap();
    let labels = root
        .path()
        .join("user-metadata")
        .join(format!("{other}.json"));
    fs::write(&labels, b"broken retained labels").unwrap();
    let archives = root.path().join("archives");
    // A receipt written by a newer version with a field this version does not know.
    let newer_id = "0".repeat(32);
    let mut newer = serde_json::to_value(&readable).unwrap();
    newer["archiveId"] = serde_json::json!(newer_id);
    newer["fieldFromNewerVersion"] = serde_json::json!(true);
    let newer_path = archives.join(format!("{newer_id}.receipt.json"));
    let newer_bytes = serde_json::to_vec(&newer).unwrap();
    fs::write(&newer_path, &newer_bytes).unwrap();
    let corrupt_id = "f".repeat(32);
    let corrupt_path = archives.join(format!("{corrupt_id}.receipt.json"));
    fs::write(&corrupt_path, b"corrupt retained receipt").unwrap();
    let page = list(root.path(), 0, 25).unwrap();
    assert_eq!(page.items.len(), 4);
    assert!(!page.more);
    let entry = |aid: &str| page.items.iter().find(|e| e.archive_id == aid).unwrap();
    for aid in [&newer_id, &corrupt_id] {
        assert!(entry(aid).receipt.is_none());
        assert_eq!(entry(aid).unreadable, Some(Unreadable::Receipt));
    }
    let fine = entry(&readable.archive_id);
    assert_eq!(fine.unreadable, None);
    assert_eq!(fine.receipt.as_ref().unwrap().meeting_id, id);
    let broken = entry(&labels_broken.archive_id);
    assert_eq!(broken.unreadable, Some(Unreadable::Labels));
    assert_eq!(broken.title, "Archive fixture");
    assert_eq!(broken.receipt.as_ref().unwrap().meeting_id, other);
    // The page stays bounded and listing never changes the unreadable files.
    let first = list(root.path(), 0, 1).unwrap();
    assert_eq!(first.items.len(), 1);
    assert!(first.more);
    assert!(list(root.path(), 0, 51).is_err());
    assert_eq!(fs::read(&newer_path).unwrap(), newer_bytes);
    assert_eq!(
        fs::read(&corrupt_path).unwrap(),
        b"corrupt retained receipt"
    );
    assert_eq!(fs::read(&labels).unwrap(), b"broken retained labels");
    assert!(restore(root.path(), &newer_id, &cancel).is_err());
    assert_eq!(fs::read(&newer_path).unwrap(), newer_bytes);
}
#[test]
fn source_move_interrupted_after_prepared_record_can_be_repeated_and_keeps_that_record() {
    let root = tempfile::tempdir().unwrap();
    let cancel = AtomicBool::new(false);
    let id = fixture(root.path());
    let receipt = create(root.path(), &id, 0, "whole-meeting", &cancel, |_, _| {}).unwrap();
    let archives = root.path().join("archives");
    let source = root.path().join("sessions").join(&id);
    let placement = archives.join(format!("{}.placement.json", receipt.archive_id));
    let destination = archives.join(format!("{}.source", receipt.archive_id));
    let attempt = archives.join(format!("{}.placement.attempt-1.json", receipt.archive_id));
    let record = |phase: &str| {
        serde_json::to_vec_pretty(&serde_json::json!({"archiveId":receipt.archive_id,"meetingId":receipt.meeting_id,"phase":phase,"sourceCopyRetained":true,"archiveSHA256":receipt.archive_sha256})).unwrap()
    };
    // A completed placement record is never replaced.
    fs::write(&placement, record("archived")).unwrap();
    assert!(move_source_into_archive(root.path(), &receipt, 0, &cancel).is_err());
    assert_eq!(fs::read(&placement).unwrap(), record("archived"));
    // State after a crash between the `prepared` record and the rename.
    fs::write(&placement, record("prepared")).unwrap();
    fs::create_dir(&destination).unwrap();
    assert!(move_source_into_archive(root.path(), &receipt, 0, &cancel).is_err());
    fs::remove_dir(&destination).unwrap();
    fs::write(source.join("late-user-file.txt"), b"Preserve my later edit").unwrap();
    assert!(move_source_into_archive(root.path(), &receipt, 0, &cancel).is_err());
    assert_eq!(fs::read(&placement).unwrap(), record("prepared"));
    assert!(!attempt.exists());
    fs::remove_file(source.join("late-user-file.txt")).unwrap();
    let moved = move_source_into_archive(root.path(), &receipt, 0, &cancel).unwrap();
    assert!(!source.exists());
    assert!(moved.join("session.json").is_file());
    assert_eq!(
        fs::read(&attempt).unwrap(),
        record("prepared"),
        "evidence kept"
    );
    let now: serde_json::Value = serde_json::from_slice(&fs::read(&placement).unwrap()).unwrap();
    assert_eq!(now["phase"], "archived");
    // The retained attempt belongs to the meeting's confirmed permanent deletion.
    let (hash, _) = deletion_fingerprint(root.path(), &id, &cancel).unwrap();
    delete_archived_meeting(
        root.path(),
        &id,
        0,
        &hash,
        &format!("DELETE {id}"),
        &cancel,
        |_, _| {},
    )
    .unwrap();
    let left: Vec<_> = fs::read_dir(&archives)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(left.is_empty(), "{left:?}");
}
#[cfg(unix)]
#[test]
fn a_dangling_audio_archive_receipt_link_is_refused_not_read_as_missing() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let folder = root.path().join("sessions").join(&id);
    assert!(read_archived_transcript(&folder).unwrap().is_none());
    let receipt = folder.join("audio-archive.json");
    std::os::unix::fs::symlink(outside.path().join("elsewhere.json"), &receipt).unwrap();
    assert!(!receipt.exists(), "the link dangles");
    assert!(read_archived_transcript(&folder).is_err());
    assert!(audio::read_audio_marker(&receipt).is_err());
    assert!(fs::symlink_metadata(&receipt)
        .unwrap()
        .file_type()
        .is_symlink());
}
#[cfg(unix)]
#[test]
fn dangling_archive_lock_links_are_rejected_without_creating_their_targets() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let cancel = AtomicBool::new(false);
    let id = fixture(root.path());
    let target = outside.path().join("created-through-link");
    let maintenance = root.path().join("archive-maintenance.lock");
    std::os::unix::fs::symlink(&target, &maintenance).unwrap();
    assert!(!maintenance.exists(), "the link dangles");
    assert!(create(root.path(), &id, 0, "whole-meeting", &cancel, |_, _| {}).is_err());
    assert!(!target.exists(), "no file is created through the link");
    fs::remove_file(&maintenance).unwrap();
    for parent in ["user-metadata", "speaker-labels"] {
        let folder = root.path().join(parent);
        fs::create_dir_all(&folder).unwrap();
        let lock = folder.join(format!("{id}.lock"));
        std::os::unix::fs::symlink(&target, &lock).unwrap();
        assert!(create(root.path(), &id, 0, "whole-meeting", &cancel, |_, _| {}).is_err());
        assert!(
            !target.exists(),
            "no {parent} lock is created through the link"
        );
        fs::remove_file(&lock).unwrap();
    }
    assert!(
        !root.path().join("archives").exists() || {
            fs::read_dir(root.path().join("archives")).unwrap().count() == 0
        }
    );
    create(root.path(), &id, 0, "whole-meeting", &cancel, |_, _| {}).unwrap();
}
#[test]
fn relocation_guard_keeps_other_new_writers_out_after_releasing_windows_internal_handle() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let folder = root.path().join("sessions").join(&id);
    let store = SessionStore::open(&folder).unwrap();
    let guard = store.into_relocation_guard().unwrap();
    assert!(SessionStore::open(&folder).is_err());
    let destination = root.path().join("moved-source");
    fs::rename(&folder, &destination).unwrap();
    assert!(destination.join("session.json").is_file());
    drop(guard);
}
#[cfg(windows)]
#[test]
fn legacy_windows_writer_lease_blocks_a_move_during_the_transition() {
    let root = tempfile::tempdir().unwrap();
    let id = fixture(root.path());
    let folder = root.path().join("sessions").join(&id);
    let store = SessionStore::open(&folder).unwrap();
    let guard = store.into_relocation_guard().unwrap();
    let legacy = OpenOptions::new()
        .read(true)
        .write(true)
        .open(folder.join("session.lock"))
        .unwrap();
    legacy.try_lock_exclusive().unwrap();
    let destination = root.path().join("moved-source");
    assert!(fs::rename(&folder, &destination).is_err());
    assert!(folder.join("session.json").is_file());
    drop(legacy);
    fs::rename(&folder, &destination).unwrap();
    assert!(destination.join("session.json").is_file());
    drop(guard);
}
