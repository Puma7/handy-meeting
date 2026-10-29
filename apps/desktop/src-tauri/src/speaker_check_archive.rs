//! Explicit, preserving maintenance of generated speaker-capability proof folders.
use crate::{AppResult, Service};
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

pub(crate) const ARCHIVE_REQUIRED: &str = "[SPEAKER_CHECK_ARCHIVE_REQUIRED]";
pub(crate) const MAX_RECORDS: usize = 128;
const MAX_FILES_PER_RECORD: usize = 2;

/// Rejects linked/reparse entries and special files before any proof folder is moved.
fn ordinary(path: &Path) -> AppResult<fs::Metadata> {
    let meta = fs::symlink_metadata(path)
        .map_err(|_| "Prüfnachweis nicht lesbar. / Check record unavailable.")?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("Verknüpfte Prüfnachweise bleiben unverändert. / Linked check records are retained unchanged.".into());
        }
    }
    if meta.file_type().is_symlink() || !(meta.is_file() || meta.is_dir()) {
        return Err("Ungewöhnlicher Prüfnachweis bleibt unverändert. / Non-ordinary check record retained unchanged.".into());
    }
    Ok(meta)
}

/// Serializes proof reads/writes and explicit archival across app instances. The lock is
/// outside the moved directory, is never truncated, and rejects linked or nonempty files.
pub(crate) fn lock_checks(root: &Path) -> AppResult<File> {
    let root =
        meeting_core::metadata::checked_directory(root).map_err(|_| "Unsafe check storage")?;
    let path = root.join("speaker-model-checks.lock");
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            let meta = ordinary(&path)?;
            if !meta.is_file() || meta.len() != 0 {
                return Err("Unsafe speaker-check lock; original files retained".into());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Speaker-check lock unavailable".into()),
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|_| "Speaker-check lock unavailable")?;
    let current = ordinary(&path)?;
    if !current.is_file() || current.len() != 0 {
        return Err("Unsafe speaker-check lock; original files retained".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let opened = file
            .metadata()
            .map_err(|_| "Speaker-check lock unavailable")?;
        if opened.dev() != current.dev() || opened.ino() != current.ino() {
            return Err("Speaker-check lock changed; original files retained".into());
        }
    }
    file.try_lock_exclusive().map_err(|_| "Eine andere Modellprüfung läuft. Bitte warten. / Another model check is running; please wait.")?;
    Ok(file)
}

/// Moves only the bounded ordinary proof tree into a newly reserved archive parent.
/// No files are deleted or overwritten; invalid structure fails before moving any bytes.
fn archive_folder(root: &Path) -> AppResult<Option<PathBuf>> {
    let _lock = lock_checks(root)?;
    let root =
        meeting_core::metadata::checked_directory(root).map_err(|_| "Unsafe check storage")?;
    let checks = root.join("speaker-model-checks");
    match fs::symlink_metadata(&checks) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Check archive unavailable; all records retained".into()),
        Ok(_) => {}
    }
    let checks = meeting_core::metadata::checked_directory(&checks)
        .map_err(|_| "Unsafe check archive; all records retained")?;
    if checks.parent() != Some(root.as_path()) {
        return Err("Check archive escaped storage".into());
    }
    let entries = fs::read_dir(&checks)
        .map_err(|_| "Check archive unavailable")?
        .take(MAX_RECORDS + 1)
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|_| "Check archive unavailable")?;
    if entries.len() > MAX_RECORDS {
        return Err(
            "Unexpected extra records; review the check archive manually. All records retained."
                .into(),
        );
    }
    if entries.is_empty() {
        return Ok(None);
    }
    for entry in entries {
        let record = entry.path();
        let owned_name = entry
            .file_name()
            .to_str()
            .is_some_and(|id| id.len() == 32 && meeting_core::metadata::valid_id(id));
        if !owned_name || !ordinary(&record)?.is_dir() {
            return Err("Unexpected check archive entry; all records retained".into());
        }
        let files = fs::read_dir(&record)
            .map_err(|_| "Check record unavailable")?
            .take(MAX_FILES_PER_RECORD + 1)
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|_| "Check record unavailable")?;
        if files.len() > MAX_FILES_PER_RECORD {
            return Err("Oversized check record; all files retained".into());
        }
        for file in files {
            if !matches!(
                file.file_name().to_str(),
                Some("model.json" | "result.json")
            ) || !ordinary(&file.path())?.is_file()
            {
                return Err("Unexpected check record file retained unchanged".into());
            }
        }
    }
    let archives = root.join("speaker-model-check-archives");
    match fs::create_dir(&archives) { Ok(())=>{}, Err(error) if error.kind()==std::io::ErrorKind::AlreadyExists=>{}, Err(_)=>return Err("Prüfarchiv konnte nicht angelegt werden; alle Nachweise bleiben erhalten. / Could not create check archive; all records retained.".into()) }
    let archives = meeting_core::metadata::checked_directory(&archives)
        .map_err(|_| "Unsafe check archive destination; all records retained")?;
    if archives.parent() != Some(root.as_path()) {
        return Err("Check archive destination escaped storage".into());
    }
    let reserved = archives.join(meeting_notes::new_job_id());
    fs::create_dir(&reserved)
        .map_err(|_| "Could not reserve check archive; all records retained")?;
    let reserved = meeting_core::metadata::checked_directory(&reserved)
        .map_err(|_| "Unsafe check archive reservation; all records retained")?;
    if reserved.parent() != Some(archives.as_path()) {
        return Err("Check archive reservation escaped storage".into());
    }
    let destination = reserved.join("checks");
    fs::rename(&checks,&destination).map_err(|_| "Archivierung fehlgeschlagen; vorhandene Nachweise bleiben erhalten. / Archival failed; existing records are retained.")?;
    #[cfg(unix)]
    for directory in [&reserved, &archives, &root] {
        File::open(directory).and_then(|file| file.sync_all()).map_err(|_| "Prüfnachweise wurden archiviert, die Ordnersicherung konnte aber nicht bestätigt werden. Alle Dateien bleiben erhalten. / Check records were archived, but directory durability could not be confirmed. All files are retained.")?;
    }
    Ok(Some(destination))
}

/// Runs only after an explicit renderer action and exclusive native admission; recordings,
/// analyses and other model checks cannot overlap this maintenance operation.
pub(crate) fn archive(service: &Service) -> AppResult<bool> {
    let _lease = service.reserve_operation(crate::operations::Kind::Selection)?;
    Ok(archive_folder(&service.root)?.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Foreign files or an unrecognized record name stop the complete move before any
    /// evidence changes location; even a normal regular file must have an owned role.
    #[test]
    fn unknown_check_entries_never_move_as_generated_proofs() {
        for unknown_folder in [false, true] {
            let root = std::env::temp_dir()
                .join(format!("hm-proof-unknown-{}", meeting_notes::new_job_id()));
            let record = root.join("speaker-model-checks").join(if unknown_folder {
                "foreign-folder".to_owned()
            } else {
                "a".repeat(32)
            });
            fs::create_dir_all(&record).unwrap();
            fs::write(record.join("model.json"), b"retained partial identity").unwrap();
            if !unknown_folder {
                fs::write(record.join("user-recording.wav"), b"foreign bytes").unwrap();
            }
            assert!(archive_folder(&root).is_err());
            assert_eq!(
                fs::read(record.join("model.json")).unwrap(),
                b"retained partial identity"
            );
            if !unknown_folder {
                assert_eq!(
                    fs::read(record.join("user-recording.wav")).unwrap(),
                    b"foreign bytes"
                );
            }
            assert!(!root.join("speaker-model-check-archives").exists());
        }
    }
    /// A full set of partial/complete proofs remains byte-identical after explicit archival;
    /// another process's proof lock and an active recording admission both prevent the move.
    #[test]
    fn explicit_archive_preserves_all_records_and_respects_both_owners() {
        let root =
            std::env::temp_dir().join(format!("hm-proof-archive-{}", meeting_notes::new_job_id()));
        let service = Service::new(root.clone()).unwrap();
        let recordings = root.join("sessions");
        fs::create_dir_all(&recordings).unwrap();
        fs::write(
            recordings.join("synthetic-audio.bin"),
            b"recording remains in place",
        )
        .unwrap();
        let checks = root.join("speaker-model-checks");
        fs::create_dir(&checks).unwrap();
        for i in 0..MAX_RECORDS {
            let folder = checks.join(format!("{i:032x}"));
            fs::create_dir(&folder).unwrap();
            fs::write(folder.join("model.json"), format!("retained proof {i}")).unwrap();
            if i % 2 == 0 {
                fs::write(folder.join("result.json"), b"retained result").unwrap();
            }
        }
        let lock = lock_checks(&root).unwrap();
        assert!(archive(&service).is_err());
        drop(lock);
        let recording = service
            .operations
            .try_acquire(crate::operations::Kind::Recording)
            .unwrap();
        assert!(archive(&service).is_err());
        drop(recording);
        let destination = archive_folder(&root).unwrap().unwrap();
        assert!(!checks.exists());
        assert_eq!(
            fs::read(recordings.join("synthetic-audio.bin")).unwrap(),
            b"recording remains in place"
        );
        for i in 0..MAX_RECORDS {
            assert_eq!(
                fs::read_to_string(destination.join(format!("{i:032x}")).join("model.json"))
                    .unwrap(),
                format!("retained proof {i}")
            );
            if i % 2 == 0 {
                assert_eq!(
                    fs::read(destination.join(format!("{i:032x}")).join("result.json")).unwrap(),
                    b"retained result"
                );
            }
        }
        fs::create_dir(&checks).unwrap();
        assert!(archive_folder(&root).unwrap().is_none());
        assert_eq!(
            fs::read_dir(root.join("speaker-model-check-archives"))
                .unwrap()
                .count(),
            1
        );
    }
    /// An occupied archive destination and unexpected nested entries fail without moving,
    /// replacing or deleting either the proofs or the conflicting user-supplied entry.
    #[test]
    fn unsafe_or_conflicting_archives_leave_the_proof_tree_unchanged() {
        let root =
            std::env::temp_dir().join(format!("hm-proof-conflict-{}", meeting_notes::new_job_id()));
        fs::create_dir(&root).unwrap();
        let record = root.join("speaker-model-checks").join("a".repeat(32));
        fs::create_dir_all(&record).unwrap();
        fs::write(record.join("model.json"), b"preserve").unwrap();
        let conflict = root.join("speaker-model-check-archives");
        fs::write(&conflict, b"foreign").unwrap();
        assert!(archive_folder(&root).is_err());
        assert_eq!(fs::read(&conflict).unwrap(), b"foreign");
        assert_eq!(fs::read(record.join("model.json")).unwrap(), b"preserve");
        fs::create_dir(record.join("nested")).unwrap();
        assert!(archive_folder(&root).is_err());
        assert_eq!(fs::read(record.join("model.json")).unwrap(), b"preserve");
    }
}
