//! Audio-only archival and exact original restoration.
use super::*;

/// Reads a validated audio-archive marker's text snapshot without reopening missing PCM files.
pub fn read_archived_transcript(folder: &Path) -> Result<Option<SessionData>> {
    metadata::checked_directory(folder)?;
    let path = folder.join("audio-archive.json");
    let Some(receipt) = read_audio_marker(&path)? else {
        return Ok(None);
    };
    if receipt.restored {
        return Ok(None);
    }
    if receipt.receipt.kind != "audio-only"
        || receipt.session.id != receipt.receipt.meeting_id
        || folder.file_name().and_then(|s| s.to_str()) != Some(receipt.session.id.as_str())
        || receipt.session.status != "complete"
        || receipt.originals.is_empty()
    {
        return Err("Invalid archived transcript identity".into());
    }
    let mut seen = HashSet::new();
    for original in receipt.originals {
        if !TRANSCRIPT_FILES.contains(&original.name.as_str())
            || !seen.insert(original.name.clone())
        {
            return Err("Invalid archived transcript provenance".into());
        }
        let (bytes, hash) = digest(&folder.join(&original.name))?;
        if bytes != original.bytes || hash != original.sha256 {
            return Err(
                "Original transcript provenance changed; archived revision retained separately"
                    .into(),
            );
        }
    }
    if !same_original_text(&crate::inspect_manifest(folder)?, &receipt.session)? {
        return Err(
            "Archived transcript does not match the retained original source manifest".into(),
        );
    }
    let mut data = receipt.session;
    data.read_only = true;
    data.warnings.push("Audio ist verlustfrei lokal archiviert. Zum erneuten Erkennen zuerst Audio aus dem Archiv wiederherstellen. / Audio is archived locally without loss. Restore audio before recognition.".into());
    Ok(Some(data))
}
/// A restored flag changes availability, never the marker's path or meeting authority.
fn validate_audio_marker(marker: &AudioArchive, folder: &Path) -> Result<()> {
    if !metadata::valid_id(&marker.receipt.archive_id)
        || marker.receipt.archive_id.starts_with("legacy-")
        || !metadata::valid_id(&marker.receipt.meeting_id)
        || marker.receipt.meeting_id.starts_with("legacy-")
        || marker.receipt.kind != "audio-only"
        || marker.session.id != marker.receipt.meeting_id
        || folder.file_name().and_then(|name| name.to_str())
            != Some(marker.receipt.meeting_id.as_str())
    {
        return Err("Invalid audio archive marker identity; existing files retained".into());
    }
    Ok(())
}
/// Reads a bounded ordinary marker; only genuine absence means that no audio archive is present.
pub(super) fn read_audio_marker(path: &Path) -> Result<Option<AudioArchive>> {
    // exists() hides dangling links; they are still retained entries, not permission
    // to write a new archive or replace an unknown marker.
    match fs::symlink_metadata(path) {
        Ok(_) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    regular(path)?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("Oversized audio archive receipt".into());
    }
    let marker: AudioArchive = serde_json::from_slice(&bytes)?;
    validate_audio_marker(&marker, path.parent().ok_or("Missing audio marker folder")?)?;
    Ok(Some(marker))
}
/// Replace only verified raw copies by a verified lossless audio archive after explicit preview.
pub fn archive_audio(
    root: &Path,
    id: &str,
    expected_metadata: u64,
    cancel: &AtomicBool,
    progress: impl FnMut(u64, u64),
) -> Result<Receipt> {
    if !metadata::valid_id(id) || id.starts_with("legacy-") {
        return Err("Invalid archive request".into());
    }
    let marker_path =
        metadata::checked_directory(&root.join("sessions").join(id))?.join("audio-archive.json");
    // Checked before any ZIP is written, so a blocked request leaves no duplicate copy.
    let previous = read_audio_marker(&marker_path)?;
    if previous.as_ref().is_some_and(|old| !old.restored) {
        return Err(
            "Existing audio archive marker preserved; restore it before creating another archive"
                .into(),
        );
    }
    let receipt = create(root, id, expected_metadata, "audio-only", cancel, progress)?;
    let folder = metadata::checked_directory(&root.join("sessions").join(id))?;
    let labels = metadata::directory(root)?;
    let lock_path = labels.join(format!("{id}.lock"));
    reject_link(&lock_path)?;
    let lock = OpenOptions::new().read(true).write(true).open(lock_path)?;
    metadata::lock_soon(&lock)?;
    let meta = metadata::read(root, id)?;
    if meta.pinned || meta.revision != expected_metadata {
        return Err(
            "Meeting became protected or changed; original audio and archive retained".into(),
        );
    }
    let store = SessionStore::open(&folder)?;
    let session = store.snapshot()?;
    if session.status != "complete"
        || session.capture_incomplete
        || session
            .parts
            .iter()
            .any(|p| !p.audio_closed || p.status != "complete")
    {
        return Err("Meeting is no longer eligible; originals retained".into());
    }
    let archive = root
        .join("archives")
        .join(format!("{}.zip", receipt.archive_id));
    let manifest = verify(&archive, Some(cancel), |_, _| {})?;
    let mut originals = Vec::new();
    for name in TRANSCRIPT_FILES {
        let path = folder.join(name);
        if path.exists() {
            let (bytes, sha256) = digest(&path)?;
            originals.push(ArchivedFile {
                name: (*name).into(),
                bytes,
                sha256,
            });
        }
    }
    let marker = AudioArchive {
        receipt: receipt.clone(),
        session,
        originals,
        restored: false,
    };
    if !same_original_text(&crate::inspect_manifest(&folder)?, &marker.session)? {
        return Err("Original transcript metadata needs recovery before audio archiving; original audio retained".into());
    }
    let path = folder.join("audio-archive.json");
    let current = read_audio_marker(&path)?;
    match (&previous, &current) {
        (None, None) => (),
        (Some(old), Some(now))
            if now.restored && now.receipt.archive_id == old.receipt.archive_id =>
        {
            // Keep the restored archive's provenance beside the session; never overwrite it.
            let history = folder.join(format!(
                "audio-archive.{}.restored.json",
                old.receipt.archive_id
            ));
            if history.exists() {
                return Err("Earlier audio archive provenance preserved; original audio and new archive retained".into());
            }
            fs::rename(&path, &history)?;
        }
        _ => {
            return Err(
                "Audio archive marker changed during archiving; original audio and new archive retained"
                    .into(),
            )
        }
    }
    // The readable confirmed transcript is committed before any raw copy is removed.
    atomic_json(&path, &marker)?;
    for file in manifest.files {
        if cancel.load(Ordering::Acquire) {
            return Err("Audio archiving stopped; the verified archive, transcript and remaining originals are retained".into());
        }
        let relative = file
            .name
            .strip_prefix("session/audio/")
            .ok_or("Invalid archived audio path")?;
        let part = marker
            .session
            .parts
            .iter()
            .find(|p| p.file == relative)
            .ok_or("Unknown archived audio")?;
        let original = store.audio_path(part)?;
        let (bytes, hash) = digest(&original)?;
        if bytes != file.bytes || hash != file.sha256 {
            return Err("Raw audio changed; retained without removal".into());
        }
        fs::remove_file(original)?;
    }
    Ok(receipt)
}
/// Restores verified archived PCM without replacing existing different bytes or discarding archive evidence.
pub fn restore_audio(root: &Path, id: &str, cancel: &AtomicBool) -> Result<SessionData> {
    let _operation = operation_lock(root)?;
    if !metadata::valid_id(id) || id.starts_with("legacy-") {
        return Err("Invalid meeting identity".into());
    }
    let folder = metadata::checked_directory(&root.join("sessions").join(id))?;
    let marker_path = folder.join("audio-archive.json");
    regular(&marker_path)?;
    let lease = crate::storage::lease(&folder)?;
    let mut bytes = Vec::new();
    File::open(&marker_path)?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("Oversized audio archive receipt".into());
    }
    let mut marker: AudioArchive = serde_json::from_slice(&bytes)?;
    validate_audio_marker(&marker, &folder)?;
    if marker.restored
        || marker.receipt.meeting_id != id
        || marker.receipt.kind != "audio-only"
        || !metadata::valid_id(&marker.receipt.archive_id)
        || marker.receipt.archive_id.starts_with("legacy-")
    {
        return Err("No matching archived audio needs restoration".into());
    }
    read_archived_transcript(&folder)?.ok_or("Archived transcript not available")?;
    let archive = root
        .join("archives")
        .join(format!("{}.zip", marker.receipt.archive_id));
    let (size, hash) = digest(&archive)?;
    if size != marker.receipt.archive_bytes || hash != marker.receipt.archive_sha256 {
        return Err("Audio archive changed; restoration blocked".into());
    }
    let manifest = verify(&archive, Some(cancel), |_, _| {})?;
    if manifest.meeting_id != id || manifest.kind != "audio-only" {
        return Err("Audio archive belongs to another meeting".into());
    }
    let audio = metadata::checked_directory(&folder.join("audio"))?;
    let mut zip = zip::ZipArchive::new(File::open(archive)?)?;
    for item in manifest.files {
        if cancel.load(Ordering::Acquire) {
            return Err(
                "Audio restoration cancelled; archive and partial restored copies are retained"
                    .into(),
            );
        }
        let name = item
            .name
            .strip_prefix("session/audio/")
            .ok_or("Invalid archived audio name")?;
        if Path::new(name).components().count() != 1 {
            return Err("Invalid archived audio path".into());
        }
        let target = audio.join(name);
        if target.exists() {
            let (size, hash) = digest(&target)?;
            if size != item.bytes || hash != item.sha256 {
                return Err("Existing audio differs and was preserved".into());
            }
            continue;
        }
        let mut pending = tempfile::NamedTempFile::new_in(&audio)?;
        let mut input = zip.by_name(&item.name)?;
        let mut copied = 0u64;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err(
                    "Audio restoration cancelled; archive and restored copies retained".into(),
                );
            }
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            copied += n as u64;
            if copied > item.bytes {
                return Err("Restored audio exceeds its manifest".into());
            }
            pending.write_all(&buffer[..n])?;
            hash.update(&buffer[..n]);
        }
        pending.as_file().sync_all()?;
        if copied != item.bytes || format!("{:x}", hash.finalize()) != item.sha256 {
            return Err("Restored audio digest mismatch".into());
        }
        pending.persist_noclobber(target).map_err(|e| e.error)?;
    }
    drop(lease);
    let store = SessionStore::open(&folder)?;
    let data = store.snapshot()?;
    marker.restored = true;
    atomic_json(&marker_path, &marker)?;
    Ok(data)
}
