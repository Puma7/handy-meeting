//! Lossless local archives. Verification and restoration never overwrite existing sessions.
use crate::{
    metadata,
    storage::{atomic_json, reject_link},
    Result, SessionData, SessionStore,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
const MAX_FILES: usize = 100000;
const MAX_TOTAL: u64 = 64 * 1024 * 1024 * 1024;
const MAX_MANIFEST: u64 = 64 * 1024 * 1024;
fn operation_lock(root: &Path) -> Result<File> {
    metadata::checked_directory(root)?;
    let file = metadata::open_lock_file(&root.join("archive-maintenance.lock"))?;
    metadata::lock_soon(&file)?;
    Ok(file)
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArchivedFile {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub meeting_id: String,
    pub kind: String,
    pub session: SessionData,
    pub files: Vec<ArchivedFile>,
    #[serde(default)]
    pub directories: Vec<String>,
    #[serde(default)]
    pub retained_sidecars: Vec<Sidecar>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sidecar {
    pub name: String,
    pub bytes: Vec<u8>,
    pub sha256: String,
}
fn sidecar_name(name: &str, id: &str) -> bool {
    let parts: Vec<_> = name.split('/').collect();
    if !matches!(
        parts.first().copied(),
        Some("user-metadata" | "speaker-labels")
    ) {
        return false;
    }
    if parts.len() == 2 {
        return parts[1] == format!("{id}.json");
    }
    if parts.len() != 3 || parts[1] != format!("{id}-history") {
        return false;
    }
    let file = parts[2];
    file.len() == 58
        && file.as_bytes()[..20].iter().all(u8::is_ascii_digit)
        && file.as_bytes()[20] == b'-'
        && file.as_bytes()[21..53].iter().all(u8::is_ascii_hexdigit)
        && &file[53..] == ".json"
}
fn sidecars(root: &Path, id: &str) -> Result<Vec<Sidecar>> {
    let mut paths = Vec::new();
    for parent in ["user-metadata", "speaker-labels"] {
        let folder = root.join(parent);
        if !folder.exists() {
            continue;
        }
        metadata::checked_directory(&folder)?;
        let current = folder.join(format!("{id}.json"));
        if current.exists() {
            paths.push(current);
        }
        let history = folder.join(format!("{id}-history"));
        if history.exists() {
            metadata::checked_directory(&history)?;
            for entry in fs::read_dir(history)? {
                paths.push(entry?.path());
                if paths.len() > MAX_FILES {
                    return Err("Too many retained annotation revisions".into());
                }
            }
        }
    }
    paths.sort();
    let mut total = 0u64;
    let mut actual = 0usize;
    let mut output = Vec::new();
    for path in paths {
        let name = path
            .strip_prefix(root)?
            .to_str()
            .ok_or("Invalid annotation path")?
            .replace('\\', "/");
        if !sidecar_name(&name, id) {
            return Err(
                "Unknown annotation history file was preserved; archive requires review".into(),
            );
        }
        let meta = regular(&path)?;
        total = total
            .checked_add(meta.len())
            .ok_or("Annotation size overflow")?;
        if total > 16 * 1024 * 1024 {
            return Err("Annotation history exceeds archive bound; originals retained".into());
        }
        let mut bytes = Vec::new();
        File::open(path)?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Annotation changed beyond its archive bound; preserved".into());
        }
        actual = actual
            .checked_add(bytes.len())
            .ok_or("Annotation size overflow")?;
        if actual > 16 * 1024 * 1024 {
            return Err("Annotation history exceeds archive bound; originals retained".into());
        }
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        output.push(Sidecar {
            name,
            bytes,
            sha256,
        });
    }
    Ok(output)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    pub archive_id: String,
    pub meeting_id: String,
    pub kind: String,
    pub archive_bytes: u64,
    pub original_bytes: u64,
    pub archive_sha256: String,
    pub files: usize,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub created_at: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AudioArchive {
    receipt: Receipt,
    session: SessionData,
    originals: Vec<ArchivedFile>,
    restored: bool,
}
const TRANSCRIPT_FILES: &[&str] = &[
    "session.json",
    "session.json.bak",
    "audio-journal.jsonl",
    "transcript-journal.jsonl",
];
fn same_original_text(a: &SessionData, b: &SessionData) -> Result<bool> {
    let encode = |value: &SessionData| -> Result<Vec<u8>> {
        let mut data = value.clone();
        data.warnings.clear();
        data.read_only = false;
        data.parts.sort_by(|x, y| x.file.cmp(&y.file));
        Ok(serde_json::to_vec(&data)?)
    };
    Ok(encode(a)? == encode(b)?)
}
fn regular(path: &Path) -> Result<fs::Metadata> {
    reject_link(path)?;
    let meta = fs::metadata(path)?;
    if !meta.is_file() {
        return Err("Archive input must be an ordinary file".into());
    }
    Ok(meta)
}
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['\\', ':', '\0', '<', '>', '"', '|', '?', '*'])
        && name.split('/').all(|part| {
            let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with(['.', ' '])
                && !part.chars().any(char::is_control)
                && ![
                    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
                    "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7",
                    "LPT8", "LPT9",
                ]
                .contains(&stem.as_str())
        })
        && name.starts_with("session/")
}
fn digest(path: &Path) -> Result<(u64, String)> {
    digest_cancellable(path, None)
}
fn digest_cancellable(path: &Path, cancel: Option<&AtomicBool>) -> Result<(u64, String)> {
    regular(path)?;
    let mut input = File::open(path)?;
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            return Err("Archive hashing cancelled; existing data retained".into());
        }
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes = bytes.checked_add(n as u64).ok_or("Archive size overflow")?;
        if bytes > MAX_TOTAL {
            return Err("Archive input exceeds size limit".into());
        }
        hash.update(&buffer[..n]);
    }
    Ok((bytes, format!("{:x}", hash.finalize())))
}
fn inventory(
    folder: &Path,
    base: &Path,
    files: &mut Vec<(String, PathBuf)>,
    directories: &mut Vec<String>,
) -> Result<()> {
    metadata::checked_directory(folder)?;
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let path = entry.path();
        reject_link(&path)?;
        let meta = entry.metadata()?;
        if meta.is_dir() {
            let relative = path
                .strip_prefix(base)?
                .to_str()
                .ok_or("Non-Unicode archive directory")?
                .replace('\\', "/");
            let name = format!("session/{relative}");
            if !safe_name(&name) {
                return Err("Unsafe archive directory".into());
            }
            directories.push(name);
            if directories.len() > MAX_FILES {
                return Err("Too many archive directories".into());
            }
            inventory(&path, base, files, directories)?;
        } else if meta.is_file() {
            let relative = path
                .strip_prefix(base)?
                .to_str()
                .ok_or("Non-Unicode archive input")?
                .replace('\\', "/");
            if relative == "session.lock" {
                continue;
            }
            let name = format!("session/{relative}");
            if !safe_name(&name) {
                return Err("Unsafe archive input path".into());
            }
            files.push((name, path));
            if files.len() > MAX_FILES {
                return Err("Too many archive input files".into());
            }
        } else {
            return Err("Unsupported archive input type".into());
        }
    }
    Ok(())
}
fn safe_directories(base: &Path, relative: &Path) -> Result<()> {
    let mut current = metadata::checked_directory(base)?;
    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err("Unsafe restore directory component".into());
        }
        current.push(component);
        match fs::create_dir(&current) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.into()),
        }
        metadata::checked_directory(&current)?;
    }
    Ok(())
}
pub fn create(
    root: &Path,
    id: &str,
    expected_metadata: u64,
    kind: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<Receipt> {
    let _operation = operation_lock(root)?;
    if !metadata::valid_id(id)
        || id.starts_with("legacy-")
        || !["whole-meeting", "audio-only"].contains(&kind)
    {
        return Err("Invalid archive request".into());
    }
    let labels = metadata::directory(root)?;
    let lock = metadata::open_lock_file(&labels.join(format!("{id}.lock")))?;
    metadata::lock_soon(&lock)?;
    let meta = metadata::read(root, id)?;
    if meta.revision != expected_metadata || meta.pinned {
        return Err("Meeting labels changed or this meeting is VIP protected".into());
    }
    let _speaker_lock = if kind == "whole-meeting" {
        let folder = root.join("speaker-labels");
        match fs::create_dir(&folder) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.into()),
        }
        metadata::checked_directory(&folder)?;
        let lock = metadata::open_lock_file(&folder.join(format!("{id}.lock")))?;
        metadata::lock_soon(&lock)?;
        Some(lock)
    } else {
        None
    };
    let sessions = metadata::checked_directory(&root.join("sessions"))?;
    let folder = metadata::checked_directory(&sessions.join(id))?;
    let store = SessionStore::open(&folder)?;
    let data = store.snapshot()?;
    if data.id != id
        || data.read_only
        || data.status != "complete"
        || data.capture_incomplete
        || data
            .parts
            .iter()
            .any(|p| !p.audio_closed || p.status != "complete")
    {
        return Err("Only complete, unpinned, inactive meetings may be archived".into());
    }
    let mut input = Vec::new();
    let mut directories = Vec::new();
    inventory(&folder, &folder, &mut input, &mut directories)?;
    if kind == "audio-only" {
        let names: HashSet<_> = data
            .parts
            .iter()
            .map(|p| format!("session/audio/{}", p.file))
            .collect();
        input.retain(|(name, _)| names.contains(name));
        directories.retain(|name| name == "session/audio");
        if input.len() != names.len() {
            return Err("Original audio inventory is incomplete".into());
        }
    }
    input.sort_by(|a, b| a.0.cmp(&b.0));
    let total = input.len() as u64;
    let archives = root.join("archives");
    match fs::create_dir(&archives) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    }
    metadata::checked_directory(&archives)?;
    let archive_id = uuid::Uuid::new_v4().simple().to_string();
    // Written before the ZIP so an interrupted copy stays attributable to its meeting,
    // and permanent deletion can find it even without a receipt.
    atomic_json(
        &archives.join(format!("{archive_id}.pending.json")),
        &serde_json::json!({"meetingId": id, "kind": kind}),
    )?;
    let path = archives.join(format!("{archive_id}.zip"));
    let output = OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(&path)?;
    let mut writer = zip::ZipWriter::new(output);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(3))
        .large_file(true);
    let mut files = Vec::new();
    let mut original_bytes = 0u64;
    let mut seen = HashSet::new();
    directories.sort();
    for name in &directories {
        if !seen.insert(name.to_lowercase()) {
            return Err("Archive directory names collide".into());
        }
        writer.add_directory(format!("{name}/"), options)?;
    }
    for (number, (name, file)) in input.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return Err(
                "Archiving cancelled; original files and partial evidence are retained".into(),
            );
        }
        if !seen.insert(name.to_lowercase()) {
            return Err("Archive paths collide across platforms".into());
        }
        regular(file)?;
        writer.start_file(name, options)?;
        let mut stream = File::open(file)?;
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err("Archiving cancelled; originals are retained".into());
            }
            let n = stream.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            bytes += n as u64;
            original_bytes = original_bytes
                .checked_add(n as u64)
                .ok_or("Archive size overflow")?;
            if original_bytes > MAX_TOTAL {
                return Err("Archive exceeds size limit".into());
            }
            writer.write_all(&buffer[..n])?;
            hash.update(&buffer[..n]);
        }
        files.push(ArchivedFile {
            name: name.clone(),
            bytes,
            sha256: format!("{:x}", hash.finalize()),
        });
        progress(number as u64 + 1, total);
    }
    let manifest = Manifest {
        schema: 1,
        meeting_id: id.into(),
        kind: kind.into(),
        session: data,
        files,
        directories,
        retained_sidecars: if kind == "whole-meeting" {
            sidecars(root, id)?
        } else {
            vec![]
        },
    };
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("Archive manifest exceeds size limit".into());
    }
    writer.start_file("ARCHIVE_MANIFEST.json", options)?;
    writer.write_all(&bytes)?;
    let output = writer.finish()?;
    output.sync_all()?;
    drop(output);
    verify(&path, Some(cancel), |_, _| {})?;
    // A concurrent external edit must not become eligible for later source removal.
    for entry in &manifest.files {
        let relative = entry
            .name
            .strip_prefix("session/")
            .ok_or("Invalid archive name")?;
        let (len, hash) = digest_cancellable(&folder.join(relative), Some(cancel))?;
        if len != entry.bytes || hash != entry.sha256 {
            return Err(
                "Original files changed while archiving; originals and archive are retained".into(),
            );
        }
    }
    let (archive_bytes, archive_sha256) = digest_cancellable(&path, Some(cancel))?;
    let receipt = Receipt {
        archive_id,
        meeting_id: id.into(),
        kind: kind.into(),
        archive_bytes,
        original_bytes,
        archive_sha256,
        files: manifest.files.len(),
        title: manifest.session.title.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    atomic_json(
        &archives.join(format!("{}.receipt.json", receipt.archive_id)),
        &receipt,
    )?;
    Ok(receipt)
}
pub fn verify(
    path: &Path,
    cancel: Option<&AtomicBool>,
    mut progress: impl FnMut(u64, u64),
) -> Result<Manifest> {
    regular(path)?;
    let mut reader = zip::ZipArchive::new(File::open(path)?)?;
    if reader.len() > MAX_FILES * 2 + 1 {
        return Err("Too many archive entries".into());
    }
    let manifest: Manifest = {
        let file = reader.by_name("ARCHIVE_MANIFEST.json")?;
        if file.size() > MAX_MANIFEST {
            return Err("Oversized archive manifest".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_MANIFEST + 1).read_to_end(&mut bytes)?;
        serde_json::from_slice(&bytes)?
    };
    if manifest.schema != 1
        || !metadata::valid_id(&manifest.meeting_id)
        || manifest.session.id != manifest.meeting_id
        || manifest.files.len() > MAX_FILES
        || manifest.directories.len() > MAX_FILES
        || reader.len() != manifest.files.len() + manifest.directories.len() + 1
        || !["whole-meeting", "audio-only"].contains(&manifest.kind.as_str())
    {
        return Err("Archive manifest identity or inventory mismatch".into());
    }
    let mut seen = HashSet::new();
    let mut sidecar_names = HashSet::new();
    let mut sidecar_bytes = 0usize;
    for sidecar in &manifest.retained_sidecars {
        sidecar_bytes = sidecar_bytes
            .checked_add(sidecar.bytes.len())
            .ok_or("Annotation archive overflow")?;
        if sidecar_bytes > 16 * 1024 * 1024
            || !sidecar_name(&sidecar.name, &manifest.meeting_id)
            || !sidecar_names.insert(sidecar.name.to_lowercase())
            || format!("{:x}", Sha256::digest(&sidecar.bytes)) != sidecar.sha256
        {
            return Err("Invalid retained annotation evidence".into());
        }
    }
    for directory in &manifest.directories {
        if !safe_name(directory) || !seen.insert(directory.to_lowercase()) {
            return Err("Unsafe or duplicate archive directory".into());
        }
        let entry = reader.by_name(&format!("{directory}/"))?;
        if !entry.is_dir()
            || entry.size() != 0
            || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            return Err("Invalid archive directory entry".into());
        }
    }
    let mut total = 0u64;
    for (number, item) in manifest.files.iter().enumerate() {
        if cancel.is_some_and(|c| c.load(Ordering::Acquire)) {
            return Err("Archive verification cancelled; originals retained".into());
        }
        if !safe_name(&item.name)
            || !seen.insert(item.name.to_lowercase())
            || item.sha256.len() != 64
        {
            return Err("Unsafe or duplicate archive path".into());
        }
        total = total
            .checked_add(item.bytes)
            .ok_or("Archive size overflow")?;
        if total > MAX_TOTAL {
            return Err("Archive decoded size exceeds limit".into());
        }
        let mut input = reader.by_name(&item.name)?;
        if input.size() != item.bytes
            || input.is_dir()
            || input.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            return Err("Archive entry type or size mismatch".into());
        }
        let mut hasher = Sha256::new();
        let mut bytes = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            if cancel.is_some_and(|c| c.load(Ordering::Acquire)) {
                return Err("Archive verification cancelled; existing data retained".into());
            }
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            bytes += n as u64;
            if bytes > item.bytes {
                return Err("Archive expanded beyond manifest".into());
            }
            hasher.update(&buffer[..n]);
        }
        if bytes != item.bytes || format!("{:x}", hasher.finalize()) != item.sha256 {
            return Err("Archive content SHA256 mismatch".into());
        }
        progress(number as u64 + 1, manifest.files.len() as u64);
    }
    Ok(manifest)
}
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
    let source = metadata::checked_directory(&sessions.join(&receipt.meeting_id))?;
    let store = SessionStore::open(&source)?;
    if !same_original_text(&store.snapshot()?, &manifest.session)? {
        return Err("Session changed after archiving; original source retained".into());
    }
    let mut files = Vec::new();
    let mut directories = Vec::new();
    inventory(&source, &source, &mut files, &mut directories)?;
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
    let destination = archives.join(format!("{}.source", receipt.archive_id));
    let placement = archives.join(format!("{}.placement.json", receipt.archive_id));
    retire_interrupted_placement(&archives, receipt, &placement, &destination)?;
    let mut record = serde_json::json!({"archiveId":receipt.archive_id,"meetingId":receipt.meeting_id,"phase":"prepared","sourceCopyRetained":true,"archiveSHA256":receipt.archive_sha256});
    atomic_json(&placement, &record)?;
    if cancel.load(Ordering::Acquire) {
        return Err("Archive move cancelled; original source retained".into());
    }
    let relocation_guard = store.into_relocation_guard()?;
    fs::rename(&source, &destination)?;
    record["phase"] = serde_json::json!("archived");
    atomic_json(&placement, &record)?;
    drop(relocation_guard);
    Ok(destination)
}
/// Retained records of earlier move attempts that stopped after `prepared`.
const MAX_PLACEMENT_ATTEMPTS: usize = 100;
fn placement_attempt(archives: &Path, archive_id: &str, number: usize) -> PathBuf {
    archives.join(format!("{archive_id}.placement.attempt-{number}.json"))
}
/// True only when nothing, not even a dangling link, exists at `path`.
fn absent(path: &Path) -> Result<bool> {
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
fn retire_interrupted_placement(
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
    {
        return Err(blocked().into());
    }
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

pub fn read_archived_transcript(folder: &Path) -> Result<Option<SessionData>> {
    metadata::checked_directory(folder)?;
    let path = folder.join("audio-archive.json");
    if !path.exists() {
        return Ok(None);
    }
    regular(&path)?;
    let mut bytes = Vec::new();
    File::open(&path)?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("Oversized audio archive receipt".into());
    }
    let receipt: AudioArchive = serde_json::from_slice(&bytes)?;
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
fn read_audio_marker(path: &Path) -> Result<Option<AudioArchive>> {
    if !path.exists() {
        return Ok(None);
    }
    regular(path)?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("Oversized audio archive receipt".into());
    }
    Ok(Some(serde_json::from_slice(&bytes)?))
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
/// Receipt browsing is paged and does not decompress audio. Restoration verifies every byte.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePage {
    pub items: Vec<ArchiveEntry>,
    pub more: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntry {
    pub archive_id: String,
    /// Absent when the receipt itself could not be read; its files are still preserved.
    pub receipt: Option<Receipt>,
    pub title: String,
    pub session_present: bool,
    pub source_copy_retained: bool,
    /// Set when part of this entry could not be read. Such entries offer no actions.
    pub unreadable: Option<Unreadable>,
}
/// Which record of an archive entry could not be read. All files stay unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Unreadable {
    /// Missing, linked, oversized, corrupt or from a newer version (unknown fields).
    Receipt,
    /// The meeting's saved labels could not be read.
    Labels,
}
fn receipt_at(folder: &Path, id: &str) -> Result<Receipt> {
    if !metadata::valid_id(id) || id.starts_with("legacy-") {
        return Err("Invalid archive identity".into());
    }
    let path = folder.join(format!("{id}.receipt.json"));
    regular(&path)?;
    let mut bytes = Vec::new();
    File::open(path)?.take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err("Oversized archive receipt".into());
    }
    let value: Receipt = serde_json::from_slice(&bytes)?;
    if value.archive_id != id
        || !metadata::valid_id(&value.meeting_id)
        || value.meeting_id.starts_with("legacy-")
        || !["whole-meeting", "audio-only"].contains(&value.kind.as_str())
    {
        return Err("Archive receipt identity mismatch".into());
    }
    Ok(value)
}
pub fn list(root: &Path, offset: usize, limit: usize) -> Result<ArchivePage> {
    if !(1..=50).contains(&limit) || offset > 1000000 {
        return Err("Invalid archive page".into());
    }
    metadata::checked_directory(root)?;
    let folder = root.join("archives");
    if !folder.exists() {
        return Ok(ArchivePage {
            items: Vec::new(),
            more: false,
        });
    }
    metadata::checked_directory(&folder)?;
    let mut ids = Vec::new();
    for entry in fs::read_dir(&folder)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if let Some(id) = name.strip_suffix(".receipt.json") {
            if metadata::valid_id(id) && !id.starts_with("legacy-") {
                ids.push(id.to_owned());
            }
        }
        if ids.len() > 1000000 {
            return Err("Archive catalog exceeds browsing bound".into());
        }
    }
    ids.sort();
    let more = ids.len() > offset.saturating_add(limit);
    let mut items = Vec::new();
    for id in ids.iter().skip(offset).take(limit) {
        // One unreadable entry (e.g. a receipt from a newer version) must not hide the page.
        let source_copy_retained = folder.join(format!("{id}.source")).exists();
        let Ok(receipt) = receipt_at(&folder, id) else {
            items.push(ArchiveEntry {
                archive_id: id.clone(),
                receipt: None,
                title: id.clone(),
                session_present: false,
                source_copy_retained,
                unreadable: Some(Unreadable::Receipt),
            });
            continue;
        };
        let fallback = if receipt.title.is_empty() {
            receipt.meeting_id.clone()
        } else {
            receipt.title.clone()
        };
        let (title, unreadable) = match metadata::read(root, &receipt.meeting_id) {
            Ok(labels) => (labels.title.unwrap_or(fallback), None),
            Err(_) => (fallback, Some(Unreadable::Labels)),
        };
        items.push(ArchiveEntry {
            archive_id: id.clone(),
            title,
            session_present: root.join("sessions").join(&receipt.meeting_id).exists(),
            source_copy_retained,
            receipt: Some(receipt),
            unreadable,
        });
    }
    Ok(ArchivePage { items, more })
}
pub fn restore(root: &Path, archive_id: &str, cancel: &AtomicBool) -> Result<PathBuf> {
    let folder = metadata::checked_directory(&root.join("archives"))?;
    let receipt = receipt_at(&folder, archive_id)?;
    let zip = folder.join(format!("{archive_id}.zip"));
    let (size, hash) = digest_cancellable(&zip, Some(cancel))?;
    if size != receipt.archive_bytes || hash != receipt.archive_sha256 {
        return Err("Archive changed; restoration blocked".into());
    }
    if receipt.kind == "whole-meeting" {
        restore_new(root, archive_id, cancel)
    } else {
        restore_audio(root, &receipt.meeting_id, cancel)?;
        Ok(root.join("sessions").join(receipt.meeting_id))
    }
}
/// Reclaim the redundant raw copy only after fully verifying its retained lossless ZIP.
/// Missing files from an interrupted previous compaction are safe; unknown/changed files block it.
pub fn compact_source_copy(
    root: &Path,
    archive_id: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<()> {
    let _operation = operation_lock(root)?;
    let archives = metadata::checked_directory(&root.join("archives"))?;
    let receipt = receipt_at(&archives, archive_id)?;
    if receipt.kind != "whole-meeting" {
        return Err("Only whole-meeting source copies can be compacted".into());
    }
    let labels = metadata::directory(root)?;
    let lock = metadata::open_lock_file(&labels.join(format!("{}.lock", receipt.meeting_id)))?;
    metadata::lock_soon(&lock)?;
    if metadata::read(root, &receipt.meeting_id)?.pinned {
        return Err("VIP meeting is protected from archive compaction".into());
    }
    let zip = archives.join(format!("{archive_id}.zip"));
    let (size, hash) = digest_cancellable(&zip, Some(cancel))?;
    if size != receipt.archive_bytes || hash != receipt.archive_sha256 {
        return Err("Archive changed; raw copy retained".into());
    }
    let manifest = verify(&zip, Some(cancel), |_, _| {})?;
    if manifest.meeting_id != receipt.meeting_id || manifest.kind != "whole-meeting" {
        return Err("Archive identity mismatch".into());
    }
    let source = metadata::checked_directory(&archives.join(format!("{archive_id}.source")))?;
    let mut actual = Vec::new();
    let mut directories = Vec::new();
    inventory(&source, &source, &mut actual, &mut directories)?;
    if directories
        .iter()
        .any(|d| !manifest.directories.contains(d))
    {
        return Err("Unknown directory in retained source; raw copy preserved".into());
    }
    let mut selected = Vec::new();
    for (name, path) in actual {
        let expected = manifest
            .files
            .iter()
            .find(|f| f.name == name)
            .ok_or("New user file in retained copy; preserved")?;
        let (bytes, sha) = digest_cancellable(&path, Some(cancel))?;
        if bytes != expected.bytes || sha != expected.sha256 {
            return Err("Retained source changed; raw copy preserved".into());
        }
        selected.push((path, bytes, sha));
    }
    let session_lock = source.join("session.lock");
    if session_lock.exists() {
        let (bytes, sha) = digest_cancellable(&session_lock, Some(cancel))?;
        if bytes != 0 {
            return Err("Changed lease file in retained copy; preserved".into());
        }
        selected.push((session_lock, bytes, sha));
    }
    let logs = root.join("maintenance-log");
    match fs::create_dir(&logs) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    };
    metadata::checked_directory(&logs)?;
    let log = logs.join(format!("compact-{}.json", uuid::Uuid::new_v4().simple()));
    let mut record = serde_json::json!({"archiveId":archive_id,"meetingId":receipt.meeting_id,"archiveSHA256":receipt.archive_sha256,"phase":"verified","removedDuplicateFiles":0,"totalDuplicateFiles":selected.len(),"originalsInLosslessZIP":true});
    atomic_json(&log, &record)?;
    for (n, (path, bytes, sha)) in selected.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return Err("Compaction cancelled; the complete lossless ZIP and remaining raw copies are retained. Repeat compaction to finish.".into());
        }
        let mut parent = source.clone();
        for part in path
            .strip_prefix(&source)?
            .parent()
            .ok_or("Invalid compaction parent")?
            .components()
        {
            parent.push(part);
            metadata::checked_directory(&parent)?;
        }
        let (current_size, current_hash) = digest_cancellable(path, Some(cancel))?;
        if current_size != *bytes || current_hash != *sha {
            return Err(
                "Raw copy changed during compaction; remaining files and complete ZIP retained"
                    .into(),
            );
        }
        fs::remove_file(path)?;
        record["removedDuplicateFiles"] = serde_json::json!(n + 1);
        atomic_json(&log, &record)?;
        progress(n as u64 + 1, selected.len() as u64);
    }
    directories.sort_by_key(|p| std::cmp::Reverse(p.split('/').count()));
    for directory in directories {
        fs::remove_dir(
            source.join(
                directory
                    .strip_prefix("session/")
                    .ok_or("Invalid source directory")?,
            ),
        )?;
    }
    fs::remove_dir(&source)?;
    let placement = archives.join(format!("{archive_id}.placement.json"));
    if placement.exists() {
        regular(&placement)?;
        atomic_json(
            &placement,
            &serde_json::json!({"archiveId":archive_id,"meetingId":receipt.meeting_id,"phase":"compacted","sourceCopyRetained":false,"archiveSHA256":receipt.archive_sha256}),
        )?;
    }
    record["phase"] = serde_json::json!("complete");
    atomic_json(&log, &record)?;
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteFile {
    name: String,
    bytes: u64,
    sha256: String,
}
struct DeleteInventory {
    files: Vec<DeleteFile>,
    directories: Vec<PathBuf>,
    newest: String,
}
fn deletion_inventory(root: &Path, id: &str, cancel: &AtomicBool) -> Result<DeleteInventory> {
    if !metadata::valid_id(id)
        || id.starts_with("legacy-")
        || root.join("sessions").join(id).exists()
    {
        return Err(
            "Only a fully archived meeting absent from normal history can be permanently deleted"
                .into(),
        );
    }
    let archives = metadata::checked_directory(&root.join("archives"))?;
    let mut files = Vec::new();
    let mut directories = Vec::new();
    let mut newest = String::new();
    let mut whole = false;
    for entry in fs::read_dir(&archives)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(aid) = name.strip_suffix(".receipt.json") else {
            continue;
        };
        // Any unreadable catalog entry blocks deletion: it might be another copy of this meeting.
        let receipt = receipt_at(&archives, aid)?;
        if receipt.meeting_id != id {
            continue;
        }
        whole |= receipt.kind == "whole-meeting";
        let at = chrono::DateTime::parse_from_rfc3339(&receipt.created_at)?;
        let at = at.with_timezone(&chrono::Utc).to_rfc3339();
        if at > newest {
            newest = at;
        }
        let zip = archives.join(format!("{aid}.zip"));
        let (size, hash) = digest_cancellable(&zip, Some(cancel))?;
        if size != receipt.archive_bytes || hash != receipt.archive_sha256 {
            return Err("Archive changed; permanent deletion blocked".into());
        }
        let manifest = verify(&zip, Some(cancel), |_, _| {})?;
        if manifest.meeting_id != id || manifest.kind != receipt.kind {
            return Err("Archive identity mismatch; retained".into());
        }
        for path in [zip, entry.path()] {
            push_delete_file(root, &path, cancel, &mut files)?;
        }
        // atomic_json keeps the previous placement phase as a .bak next to the record.
        for placement in [
            archives.join(format!("{aid}.placement.json")),
            archives.join(format!("{aid}.placement.json.bak")),
        ] {
            if placement.exists() {
                push_delete_file(root, &placement, cancel, &mut files)?;
            }
        }
        for number in 1..=MAX_PLACEMENT_ATTEMPTS {
            let attempt = placement_attempt(&archives, aid, number);
            if !absent(&attempt)? {
                push_delete_file(root, &attempt, cancel, &mut files)?;
            }
        }
        let source = archives.join(format!("{aid}.source"));
        if source.exists() {
            metadata::checked_directory(&source)?;
            let mut actual = Vec::new();
            let mut dirs = Vec::new();
            inventory(&source, &source, &mut actual, &mut dirs)?;
            let mut expected_dirs = manifest.directories.clone();
            expected_dirs.sort();
            dirs.sort();
            if actual.len() != manifest.files.len() || dirs != expected_dirs {
                return Err("Retained source inventory changed; permanent deletion blocked".into());
            }
            for (name, path) in actual {
                let expected = manifest
                    .files
                    .iter()
                    .find(|f| f.name == name)
                    .ok_or("Unarchived file in retained source")?;
                let (size, hash) = digest_cancellable(&path, Some(cancel))?;
                if size != expected.bytes || hash != expected.sha256 {
                    return Err("Retained source changed; permanent deletion blocked".into());
                }
                push_delete_file(root, &path, cancel, &mut files)?;
            }
            let lease = source.join("session.lock");
            if lease.exists() {
                push_delete_file(root, &lease, cancel, &mut files)?;
            }
            for dir in dirs {
                directories.push(
                    source.join(
                        dir.strip_prefix("session/")
                            .ok_or("Invalid archive directory")?,
                    ),
                );
            }
            directories.push(source);
        }
    }
    if !whole {
        return Err("No complete whole-meeting archive; permanent deletion blocked".into());
    }
    // Copies outside the receipt catalog: interrupted archive writes and restore staging.
    for entry in fs::read_dir(&archives)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(aid) = name.strip_suffix(".pending.json") {
            if pending_meeting(&entry.path())? != id {
                continue;
            }
            push_delete_file(root, &entry.path(), cancel, &mut files)?;
            let zip = archives.join(format!("{aid}.zip"));
            if zip.exists() && !archives.join(format!("{aid}.receipt.json")).exists() {
                regular(&zip)?;
                push_delete_file(root, &zip, cancel, &mut files)?;
            }
        } else if let Some(aid) = name.strip_suffix(".zip") {
            if archives.join(format!("{aid}.receipt.json")).exists()
                || archives.join(format!("{aid}.pending.json")).exists()
            {
                continue;
            }
            // Older or foreign copy without attribution: only its own manifest can identify it.
            match verify(&entry.path(), Some(cancel), |_, _| {}) {
                Ok(manifest) if manifest.meeting_id != id => (),
                Ok(_) => push_delete_file(root, &entry.path(), cancel, &mut files)?,
                Err(_) => {
                    return Err(format!(
                        "Unidentified archive copy archives/{name} found; permanent deletion blocked until it is inspected"
                    )
                    .into())
                }
            }
        } else if name.contains(".restoring-") {
            let staging = metadata::checked_directory(&entry.path())?;
            let inner = staging.join(id);
            if !inner.exists() {
                continue;
            }
            metadata::checked_directory(&inner)?;
            let mut found = Vec::new();
            let mut dirs = Vec::new();
            inventory(&inner, &inner, &mut found, &mut dirs)?;
            for (_, path) in found {
                push_delete_file(root, &path, cancel, &mut files)?;
            }
            for dir in dirs {
                directories.push(
                    inner.join(
                        dir.strip_prefix("session/")
                            .ok_or("Invalid staging directory")?,
                    ),
                );
            }
            directories.push(inner);
            directories.push(staging);
        }
    }
    for sidecar in sidecars(root, id)? {
        push_delete_file(root, &root.join(sidecar.name), cancel, &mut files)?;
    }
    // atomic_json keeps the previous label/speaker revision as .json.bak; it can hold the
    // title, speaker names and quotes, so it belongs to the confirmed deletion too.
    for parent in ["user-metadata", "speaker-labels"] {
        let backup = root.join(parent).join(format!("{id}.json.bak"));
        if fs::symlink_metadata(&backup).is_ok() {
            regular(&backup)?;
            push_delete_file(root, &backup, cancel, &mut files)?;
        }
    }
    let copies = root.join("audio-copies");
    if copies.exists() {
        metadata::checked_directory(&copies)?;
        for entry in fs::read_dir(&copies)? {
            let path = entry?.path();
            regular(&path)?;
            let mut bytes = Vec::new();
            File::open(&path)?.take(65537).read_to_end(&mut bytes)?;
            if bytes.len() > 65536 {
                return Err("Unreadable audio-copy catalog; deletion blocked".into());
            }
            let value: serde_json::Value = serde_json::from_slice(&bytes)?;
            if value["meetingId"].as_str() == Some(id) {
                let run = value["runId"]
                    .as_str()
                    .ok_or("Invalid audio-copy identity")?;
                if !metadata::valid_id(run)
                    || path.file_name().and_then(|s| s.to_str())
                        != Some(format!("{run}.json").as_str())
                {
                    return Err("Invalid audio-copy catalog; deletion blocked".into());
                }
                push_delete_file(root, &path, cancel, &mut files)?;
            }
        }
    }
    for parent in ["user-metadata", "speaker-labels"] {
        let history = root.join(parent).join(format!("{id}-history"));
        if history.exists() {
            metadata::checked_directory(&history)?;
            directories.push(history);
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    directories.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    Ok(DeleteInventory {
        files,
        directories,
        newest,
    })
}
fn pending_meeting(path: &Path) -> Result<String> {
    regular(path)?;
    let mut bytes = Vec::new();
    File::open(path)?.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err("Oversized pending archive record".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let id = value["meetingId"]
        .as_str()
        .ok_or("Invalid pending archive record")?;
    if !metadata::valid_id(id) {
        return Err("Invalid pending archive record".into());
    }
    Ok(id.to_owned())
}
fn push_delete_file(
    root: &Path,
    path: &Path,
    cancel: &AtomicBool,
    files: &mut Vec<DeleteFile>,
) -> Result<()> {
    let (bytes, sha256) = digest_cancellable(path, Some(cancel))?;
    let canonical = path.canonicalize()?;
    let base = root.canonicalize()?;
    let name = canonical
        .strip_prefix(base)?
        .to_str()
        .ok_or("Invalid deletion path")?
        .replace('\\', "/");
    files.push(DeleteFile {
        name,
        bytes,
        sha256,
    });
    if files.len() > 500000 {
        return Err("Deletion inventory exceeds bound; data retained".into());
    }
    Ok(())
}
/// Fingerprint covers every owned archive version, retained source and user annotation.
pub fn deletion_fingerprint(
    root: &Path,
    id: &str,
    cancel: &AtomicBool,
) -> Result<(String, String)> {
    let inventory = deletion_inventory(root, id, cancel)?;
    Ok((
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&inventory.files)?)
        ),
        inventory.newest,
    ))
}
/// Label and speaker-annotation locks for one meeting, held across a destructive operation.
fn annotation_locks(root: &Path, id: &str) -> Result<Vec<File>> {
    let metadata_dir = metadata::directory(root)?;
    let mut locks = Vec::new();
    for parent in [metadata_dir, root.join("speaker-labels")] {
        if !parent.exists() {
            fs::create_dir(&parent)?;
        }
        metadata::checked_directory(&parent)?;
        let lock = metadata::open_lock_file(&parent.join(format!("{id}.lock")))?;
        metadata::lock_soon(&lock)?;
        locks.push(lock);
    }
    Ok(locks)
}
pub fn delete_archived_meeting(
    root: &Path,
    id: &str,
    expected_meta: u64,
    expected_hash: &str,
    confirmation: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<()> {
    if !metadata::valid_id(id) || id.starts_with("legacy-") {
        return Err("Invalid archived meeting identity".into());
    }
    let _operation = operation_lock(root)?;
    if confirmation != format!("DELETE {id}") {
        return Err("Permanent deletion requires the exact meeting confirmation".into());
    }
    let _locks = annotation_locks(root, id)?;
    let labels = metadata::read(root, id)?;
    if labels.pinned || labels.revision != expected_meta {
        return Err("VIP protection or labels changed; permanent deletion blocked".into());
    }
    let mut inventory = deletion_inventory(root, id, cancel)?;
    let hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&inventory.files)?)
    );
    if hash != expected_hash {
        return Err("Archive inventory changed since preview; create a new preview".into());
    }
    let logs = root.join("maintenance-log");
    match fs::create_dir(&logs) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    }
    metadata::checked_directory(&logs)?;
    let log = logs.join(format!("delete-{}.json", uuid::Uuid::new_v4().simple()));
    inventory.files.sort_by_key(|item| {
        (
            item.name.starts_with("user-metadata/") || item.name.starts_with("speaker-labels/"),
            item.name.ends_with(".receipt.json"),
        )
    });
    let mut record = serde_json::json!({"meetingId":id,"confirmation":confirmation,"at":chrono::Utc::now().to_rfc3339(),"inventorySHA256":hash,"inventory":inventory.files,"totalFiles":inventory.files.len(),"deletedFiles":0,"phase":"confirmed"});
    atomic_json(&log, &record)?;
    for (n, item) in inventory.files.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return Err("Permanent deletion stopped. Some confirmed files may already be deleted; consult the local maintenance log.".into());
        }
        let path = root.join(&item.name);
        let mut parent = root.to_path_buf();
        for part in Path::new(&item.name)
            .parent()
            .ok_or("Invalid deletion parent")?
            .components()
        {
            parent.push(part);
            metadata::checked_directory(&parent)?;
        }
        let (size, hash) = digest_cancellable(&path, Some(cancel))?;
        if size != item.bytes || hash != item.sha256 {
            return Err("A file changed during deletion; remaining files retained. Consult the maintenance log.".into());
        }
        fs::remove_file(path)?;
        record["deletedFiles"] = serde_json::json!(n + 1);
        record["phase"] = serde_json::json!("deleting");
        atomic_json(&log, &record)?;
        progress(n as u64 + 1, inventory.files.len() as u64);
    }
    // Never recursively remove directories: late or unknown files must survive.
    for directory in inventory.directories {
        fs::remove_dir(directory)?;
    }
    record["phase"] = serde_json::json!("complete");
    atomic_json(&log, &record)?;
    Ok(())
}

/// A permanent deletion that stopped before completion, from its own maintenance log.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnfinishedDeletion {
    pub log_id: String,
    pub meeting_id: String,
    pub deleted_files: u64,
    pub total_files: u64,
    pub confirmed_at: String,
}
fn deletion_log(root: &Path, log_id: &str) -> Result<(PathBuf, serde_json::Value)> {
    if !log_id
        .strip_prefix("delete-")
        .is_some_and(metadata::valid_id)
    {
        return Err("Invalid deletion log identity".into());
    }
    let logs = metadata::checked_directory(&root.join("maintenance-log"))?;
    let path = logs.join(format!("{log_id}.json"));
    regular(&path)?;
    let mut bytes = Vec::new();
    File::open(&path)?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("Oversized deletion log".into());
    }
    Ok((path, serde_json::from_slice(&bytes)?))
}
/// Lists deletions whose log ended before `complete` (cancelled, failed or interrupted).
pub fn unfinished_deletions(root: &Path) -> Result<Vec<UnfinishedDeletion>> {
    let logs = root.join("maintenance-log");
    if !logs.exists() {
        return Ok(Vec::new());
    }
    metadata::checked_directory(&logs)?;
    let mut result = Vec::new();
    for entry in fs::read_dir(&logs)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        let Some(log_id) = name.strip_suffix(".json") else {
            continue;
        };
        if !log_id.starts_with("delete-") {
            continue;
        }
        let Ok((_, record)) = deletion_log(root, log_id) else {
            continue;
        };
        if !matches!(record["phase"].as_str(), Some("confirmed" | "deleting")) {
            continue;
        }
        let Some(meeting_id) = record["meetingId"]
            .as_str()
            .filter(|m| metadata::valid_id(m))
        else {
            continue;
        };
        result.push(UnfinishedDeletion {
            log_id: log_id.to_owned(),
            meeting_id: meeting_id.to_owned(),
            deleted_files: record["deletedFiles"].as_u64().unwrap_or(0),
            total_files: record["totalFiles"].as_u64().unwrap_or(0),
            confirmed_at: record["at"].as_str().unwrap_or_default().to_owned(),
        });
        if result.len() > 1000 {
            break;
        }
    }
    result.sort_by(|a, b| b.confirmed_at.cmp(&a.confirmed_at));
    Ok(result)
}
/// Continue a confirmed permanent deletion from its own log. Only the logged files are touched:
/// absent ones count as done, present ones must still match their logged size and SHA-256.
pub fn resume_deletion(
    root: &Path,
    log_id: &str,
    confirmation: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<String> {
    let _operation = operation_lock(root)?;
    let (log, mut record) = deletion_log(root, log_id)?;
    if !matches!(record["phase"].as_str(), Some("confirmed" | "deleting")) {
        return Err("This deletion is not unfinished".into());
    }
    let id = record["meetingId"]
        .as_str()
        .filter(|m| metadata::valid_id(m) && !m.starts_with("legacy-"))
        .ok_or("Invalid deletion log identity")?
        .to_owned();
    if confirmation != format!("DELETE {id}") {
        return Err("Permanent deletion requires the exact meeting confirmation".into());
    }
    if root.join("sessions").join(&id).exists() {
        return Err("The meeting is back in normal history; deletion stays stopped".into());
    }
    let _locks = annotation_locks(root, &id)?;
    let files: Vec<DeleteFile> = serde_json::from_value(record["inventory"].clone())?;
    let labels = format!("user-metadata/{id}.json");
    match metadata::read(root, &id) {
        Ok(meta) if meta.pinned => {
            return Err("VIP protection was set; deletion stays stopped".into());
        }
        Ok(_) => (),
        // The confirmed run already removed the current labels; their history may remain.
        Err(_) if !root.join(&labels).exists() && files.iter().any(|f| f.name == labels) => (),
        Err(e) => return Err(e),
    }
    let base = root.canonicalize()?;
    let total = files.len() as u64;
    let mut parents = HashSet::new();
    for (n, item) in files.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return Err("Permanent deletion stopped again. Remaining confirmed files are retained; it can be continued.".into());
        }
        let relative = Path::new(&item.name);
        if relative.components().count() < 2
            || !relative
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
        {
            return Err("Invalid path in deletion log; remaining files retained".into());
        }
        let mut parent = root.to_path_buf();
        let mut missing_parent = false;
        for part in relative
            .parent()
            .ok_or("Invalid deletion parent")?
            .components()
        {
            parent.push(part);
            if !parent.exists() {
                missing_parent = true;
                break;
            }
            metadata::checked_directory(&parent)?;
        }
        let path = root.join(relative);
        if !missing_parent && fs::symlink_metadata(&path).is_ok() {
            regular(&path)?;
            if !path.canonicalize()?.starts_with(&base) {
                return Err("Deletion path left the storage folder; retained".into());
            }
            let (size, hash) = digest_cancellable(&path, Some(cancel))?;
            if size != item.bytes || hash != item.sha256 {
                return Err("A file changed since the deletion was confirmed; remaining files retained. Consult the maintenance log.".into());
            }
            fs::remove_file(&path)?;
        }
        let mut dir = relative.parent();
        while let Some(d) = dir.filter(|d| d.components().count() > 1) {
            parents.insert(d.to_path_buf());
            dir = d.parent();
        }
        record["deletedFiles"] = serde_json::json!(n + 1);
        record["phase"] = serde_json::json!("deleting");
        record["resumedAt"] = serde_json::json!(chrono::Utc::now().to_rfc3339());
        atomic_json(&log, &record)?;
        progress(n as u64 + 1, total);
    }
    // Only now-empty folders below the top-level storage folders; never a recursive delete.
    let mut parents: Vec<_> = parents.into_iter().collect();
    parents.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for dir in parents {
        let _ = fs::remove_dir(root.join(dir));
    }
    record["phase"] = serde_json::json!("complete");
    atomic_json(&log, &record)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
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
        let restored =
            restore_new(root.path(), &receipt.archive_id, &AtomicBool::new(false)).unwrap();
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
        let receipt =
            archive_audio(root.path(), &id, 0, &AtomicBool::new(false), |_, _| {}).unwrap();
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
        let record: serde_json::Value =
            serde_json::from_slice(&fs::read(&logs[0]).unwrap()).unwrap();
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
            serde_json::to_vec(&serde_json::json!({"meetingId": id, "kind": "whole-meeting"}))
                .unwrap(),
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
        assert!(
            resume_deletion(root.path(), &open[0].log_id, "DELETE", &cancel, |_, _| {}).is_err()
        );
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
        let receipt =
            archive_audio(root.path(), &id, 0, &AtomicBool::new(false), |_, _| {}).unwrap();
        let path = folder.join("audio-archive.json");
        let mut marker: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
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
        let restored =
            restore_new(fresh.path(), &receipt.archive_id, &AtomicBool::new(false)).unwrap();
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
        assert!(
            move_source_into_archive(root.path(), &receipt, 0, &AtomicBool::new(false)).is_err()
        );
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
        assert!(
            move_source_into_archive(root.path(), &current, 0, &AtomicBool::new(false)).is_err()
        );
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
        let now: serde_json::Value =
            serde_json::from_slice(&fs::read(&placement).unwrap()).unwrap();
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
}
