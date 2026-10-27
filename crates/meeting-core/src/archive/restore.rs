//! Verified restoration and interrupted source placement.
use super::*;

pub fn restore_new(root: &Path, archive_id: &str, cancel: &AtomicBool) -> Result<PathBuf> {
    let _operation = operation_lock(root)?;
    if !metadata::valid_id(archive_id) || archive_id.starts_with("legacy-") {
        return Err("Invalid archive identity".into());
    }
    let archives = metadata::checked_directory(&root.join("archives"))?;
    let path = archives.join(format!("{archive_id}.zip"));
    let manifest = verify(&path, Some(cancel), |_, _| {})?;
    if manifest.kind != "whole-meeting" {
        return Err(
            "This archive contains only audio; use the dedicated audio restoration action".into(),
        );
    }
    let sessions = metadata::checked_directory(&root.join("sessions"))?;
    let destination = sessions.join(&manifest.meeting_id);
    if destination.exists() {
        return Err("A session folder with this identity already exists and was preserved".into());
    }
    // Extract into an owned staging folder; the session appears only after every byte verified.
    // A cancelled or failed attempt keeps its staging folder and never blocks a later retry.
    let staging = archives.join(format!(
        "{archive_id}.restoring-{}",
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir(&staging)?;
    let target = staging.join(&manifest.meeting_id);
    fs::create_dir(&target)?;
    let mut reader = zip::ZipArchive::new(File::open(&path)?)?;
    let mut files = manifest.files.clone();
    for directory in &manifest.directories {
        let relative = directory
            .strip_prefix("session/")
            .ok_or("Unsafe archive directory")?;
        safe_directories(&target, Path::new(relative))?;
    }
    files.sort_by_key(|f| f.name == "session/session.json" || f.name == "session/session.json.bak");
    for item in files {
        if cancel.load(Ordering::Acquire) {
            return Err("Restoration cancelled; archive and partial new folder retained".into());
        }
        let relative = item
            .name
            .strip_prefix("session/")
            .ok_or("Unsafe restore path")?;
        let output = target.join(relative);
        let parent = output.parent().ok_or("Invalid restore path")?;
        let relative_parent = parent.strip_prefix(&target)?;
        safe_directories(&target, relative_parent)?;
        // Check every created parent, including against externally inserted links.
        let mut check = target.clone();
        for part in Path::new(relative)
            .parent()
            .unwrap_or(Path::new(""))
            .components()
        {
            check.push(part);
            metadata::checked_directory(&check)?;
        }
        let mut input = reader.by_name(&item.name)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)?;
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err(
                    "Restoration cancelled; archive and partial new folder retained".into(),
                );
            }
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            bytes += n as u64;
            if bytes > item.bytes {
                return Err("Restore size mismatch".into());
            }
            file.write_all(&buffer[..n])?;
            hash.update(&buffer[..n]);
        }
        file.sync_all()?;
        if bytes != item.bytes || format!("{:x}", hash.finalize()) != item.sha256 {
            return Err("Restore content mismatch; partial new folder retained".into());
        }
    }
    drop(reader);
    if crate::inspect_manifest(&target)?.id != manifest.meeting_id {
        return Err("Restored session identity mismatch; staging folder retained".into());
    }
    if destination.exists() {
        return Err("A session folder with this identity appeared and was preserved".into());
    }
    fs::rename(&target, &destination)?;
    // Only the now-empty staging parent is removed; never a recursive delete.
    let _ = fs::remove_dir(&staging);
    let target = destination;
    let store = SessionStore::open(&target)?;
    let mut conflicts = Vec::new();
    for sidecar in &manifest.retained_sidecars {
        let path = root.join(&sidecar.name);
        let parent = path.parent().ok_or("Invalid annotation restore path")?;
        safe_directories(root, parent.strip_prefix(root)?)?;
        if path.exists() {
            let (_, hash) = digest(&path)?;
            if hash != sidecar.sha256 {
                conflicts.push(sidecar.name.clone());
            }
            continue;
        }
        if sidecar
            .name
            .ends_with(&format!("/{}.json", manifest.meeting_id))
        {
            let history = parent.join(format!("{}-history", manifest.meeting_id));
            let mut newer = false;
            if history.exists() {
                metadata::checked_directory(&history)?;
                for entry in fs::read_dir(history)? {
                    let entry = entry?;
                    let relative = entry
                        .path()
                        .strip_prefix(root)?
                        .to_str()
                        .ok_or("Invalid retained history path")?
                        .replace('\\', "/");
                    let (_, hash) = digest(&entry.path())?;
                    if !manifest
                        .retained_sidecars
                        .iter()
                        .any(|s| s.name == relative && s.sha256 == hash)
                    {
                        newer = true;
                        break;
                    }
                }
            }
            if newer {
                conflicts.push(sidecar.name.clone());
                continue;
            }
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        output.write_all(&sidecar.bytes)?;
        output.sync_all()?;
    }
    if !conflicts.is_empty() {
        let file = target.join(format!(
            "annotation-restore-conflicts-{}.json",
            uuid::Uuid::new_v4().simple()
        ));
        atomic_json(&file, &conflicts)?;
    }
    if store.snapshot()?.id != manifest.meeting_id {
        return Err("Restored session identity mismatch".into());
    }
    Ok(target)
}
/// Move a verified complete source into the archive area; never remove its original bytes.
pub fn move_source_into_archive(
    root: &Path,
    receipt: &Receipt,
    expected_metadata: u64,
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    move_source_with_placement_writer(root, receipt, expected_metadata, cancel, |path, record| {
        atomic_json(path, record)
    })
}
pub(super) fn move_source_with_placement_writer(
    root: &Path,
    receipt: &Receipt,
    expected_metadata: u64,
    cancel: &AtomicBool,
    mut write_placement: impl FnMut(&Path, &serde_json::Value) -> Result<()>,
) -> Result<PathBuf> {
    let _operation = operation_lock(root)?;
    if receipt.kind != "whole-meeting"
        || !metadata::valid_id(&receipt.meeting_id)
        || receipt.meeting_id.starts_with("legacy-")
        || !metadata::valid_id(&receipt.archive_id)
        || receipt.archive_id.starts_with("legacy-")
    {
        return Err("Invalid whole-meeting archive identity".into());
    }
    let labels = metadata::directory(root)?;
    let lock_path = labels.join(format!("{}.lock", receipt.meeting_id));
    reject_link(&lock_path)?;
    let lock = OpenOptions::new().read(true).write(true).open(lock_path)?;
    metadata::lock_soon(&lock)?;
    let meta = metadata::read(root, &receipt.meeting_id)?;
    if meta.pinned || meta.revision != expected_metadata {
        return Err("Meeting became protected or labels changed; original source retained".into());
    }
    let archives = metadata::checked_directory(&root.join("archives"))?;
    let zip = archives.join(format!("{}.zip", receipt.archive_id));
    let (size, hash) = digest_cancellable(&zip, Some(cancel))?;
    if size != receipt.archive_bytes || hash != receipt.archive_sha256 {
        return Err("Archive changed; source folder will not move".into());
    }
    let manifest = verify(&zip, Some(cancel), |_, _| {})?;
    if manifest.meeting_id != receipt.meeting_id || manifest.kind != "whole-meeting" {
        return Err("Archive source identity mismatch".into());
    }
    let sessions = metadata::checked_directory(&root.join("sessions"))?;
    let source = sessions.join(&receipt.meeting_id);
    let destination = archives.join(format!("{}.source", receipt.archive_id));
    let placement = archives.join(format!("{}.placement.json", receipt.archive_id));
    if !absent(&destination)? {
        // A failed final record write may leave the only source at its new location.
        // Recover that narrow state before requiring the old session directory.
        let mut record = prepared_placement(&placement, receipt)?;
        if !absent(&source)? {
            return Err(
                "Both session and retained archive source exist; both were preserved".into(),
            );
        }
        let retained = metadata::checked_directory(&destination)?;
        // Do not create a missing or follow a replaced internal lease in a retained copy.
        if regular(&retained.join("session.lock"))?.len() != 0 {
            return Err(
                "Changed lease file in retained copy; source and placement preserved".into(),
            );
        }
        // Holds both the external relocation lease and the internal session lease until
        // promotion, as well as the operation and label locks acquired above.
        let _store = verified_source_copy(&retained, &manifest, cancel)?;
        if cancel.load(Ordering::Acquire) {
            return Err("Archive move cancelled; retained source and placement preserved".into());
        }
        if !absent(&source)? || prepared_placement(&placement, receipt)? != record {
            return Err(
                "Archive placement changed during verification; all sources preserved".into(),
            );
        }
        record["phase"] = serde_json::json!("archived");
        write_placement(&placement, &record)?;
        return Ok(destination);
    }
    let source = metadata::checked_directory(&source)?;
    let store = verified_source_copy(&source, &manifest, cancel)?;
    retire_interrupted_placement(&archives, receipt, &placement, &destination)?;
    let mut record = serde_json::json!({"archiveId":receipt.archive_id,"meetingId":receipt.meeting_id,"phase":"prepared","sourceCopyRetained":true,"archiveSHA256":receipt.archive_sha256});
    write_placement(&placement, &record)?;
    if cancel.load(Ordering::Acquire) {
        return Err("Archive move cancelled; original source retained".into());
    }
    let relocation_guard = store.into_relocation_guard()?;
    fs::rename(&source, &destination)?;
    record["phase"] = serde_json::json!("archived");
    write_placement(&placement, &record)?;
    drop(relocation_guard);
    Ok(destination)
}

/// Recheck the exact original inventory under a session lease without rewriting it.
fn verified_source_copy(
    source: &Path,
    manifest: &Manifest,
    cancel: &AtomicBool,
) -> Result<SessionStore> {
    let store = SessionStore::open(source)?;
    if !same_original_text(&store.snapshot()?, &manifest.session)? {
        return Err("Session changed after archiving; original source retained".into());
    }
    let mut files = Vec::new();
    let mut directories = Vec::new();
    inventory(source, source, &mut files, &mut directories)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    directories.sort();
    let mut expected_dirs = manifest.directories.clone();
    expected_dirs.sort();
    if files.len() != manifest.files.len() || directories != expected_dirs {
        return Err("Source inventory changed; original source and archive retained".into());
    }
    for (name, path) in &files {
        if cancel.load(Ordering::Acquire) {
            return Err("Archive move cancelled; original source retained".into());
        }
        let expected = manifest
            .files
            .iter()
            .find(|f| &f.name == name)
            .ok_or("Source file was not archived")?;
        let (size, hash) = digest_cancellable(path, Some(cancel))?;
        if size != expected.bytes || hash != expected.sha256 {
            return Err("Source file changed; source folder retained".into());
        }
    }
    Ok(store)
}
/// Retained records of earlier move attempts that stopped after `prepared`.
pub(super) const MAX_PLACEMENT_ATTEMPTS: usize = 100;
pub(super) fn placement_attempt(archives: &Path, archive_id: &str, number: usize) -> PathBuf {
    archives.join(format!("{archive_id}.placement.attempt-{number}.json"))
}
/// True only when nothing, not even a dangling link, exists at `path`.
pub(super) fn absent(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e.into()),
    }
}
/// A move that stopped between its `prepared` record and the rename (crash or cancellation)
/// leaves the source in normal history and no `.source` folder. That record is kept as a
/// numbered attempt file, never overwritten, so the verified move can be repeated. Any other
/// existing placement or retained source blocks the move.
pub(super) fn retire_interrupted_placement(
    archives: &Path,
    receipt: &Receipt,
    placement: &Path,
    destination: &Path,
) -> Result<()> {
    let blocked = || "Existing archive placement preserved; inspect its previous operation";
    if !absent(destination)? {
        return Err(blocked().into());
    }
    if absent(placement)? {
        return Ok(());
    }
    prepared_placement(placement, receipt)?;
    for number in 1..=MAX_PLACEMENT_ATTEMPTS {
        let retained = placement_attempt(archives, &receipt.archive_id, number);
        if absent(&retained)? {
            fs::rename(placement, &retained)?;
            #[cfg(unix)]
            File::open(archives)?.sync_all()?;
            return Ok(());
        }
    }
    Err("Too many interrupted archive moves are retained; inspect them before moving again".into())
}

fn prepared_placement(placement: &Path, receipt: &Receipt) -> Result<serde_json::Value> {
    let blocked = || "Existing archive placement preserved; inspect its previous operation";
    regular(placement)?;
    let mut bytes = Vec::new();
    File::open(placement)?.take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err(blocked().into());
    }
    let record: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| blocked())?;
    if record["phase"].as_str() != Some("prepared")
        || record["archiveId"].as_str() != Some(receipt.archive_id.as_str())
        || record["meetingId"].as_str() != Some(receipt.meeting_id.as_str())
        || record["archiveSHA256"].as_str() != Some(receipt.archive_sha256.as_str())
        || record["sourceCopyRetained"].as_bool() != Some(true)
    {
        return Err(blocked().into());
    }
    Ok(record)
}
