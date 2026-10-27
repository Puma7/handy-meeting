//! Confirmed section labels are versioned annotations, never rewritten transcripts.
use crate::{
    metadata,
    storage::{atomic_json, reject_link},
    Result, SessionData,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeakerDocument {
    pub revision: u64,
    pub enabled: bool,
    pub assignments: Vec<Assignment>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Assignment {
    pub source_id: String,
    pub text_sha256: String,
    pub name: String,
    pub evidence: String,
}
pub fn text_hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn directory(root: &Path) -> Result<PathBuf> {
    metadata::checked_directory(root)?;
    let path = root.join("speaker-labels");
    match fs::create_dir(&path) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    }
    metadata::checked_directory(&path)
}
pub fn read(root: &Path, id: &str) -> Result<SpeakerDocument> {
    if !metadata::valid_id(id) {
        return Err("Invalid meeting identity".into());
    }
    let folder = root.join("speaker-labels");
    if !folder.exists() {
        return Ok(Default::default());
    }
    metadata::checked_directory(&folder)?;
    let path = folder.join(format!("{id}.json"));
    if !path.exists() {
        let history = folder.join(format!("{id}-history"));
        if history.exists() {
            metadata::checked_directory(&history)?;
            if fs::read_dir(history)?.next().is_some() {
                return Err("Current speaker labels are missing but prior confirmed revisions exist; recover them explicitly".into());
            }
        }
        return Ok(Default::default());
    }
    reject_link(&path)?;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("Oversized speaker annotations; preserved".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn save(
    root: &Path,
    data: &SessionData,
    expected: u64,
    enabled: bool,
    assignments: Vec<Assignment>,
) -> Result<SpeakerDocument> {
    if !metadata::valid_id(&data.id) || assignments.len() > 10000 {
        return Err("Invalid annotation range".into());
    }
    let mut seen = HashSet::new();
    for item in &assignments {
        if !seen.insert(&item.source_id)
            || item.name.trim().is_empty()
            || item.name.chars().count() > 80
            || item.name.chars().any(char::is_control)
            || item.evidence.trim().is_empty()
            || item.evidence.len() > 8000
        {
            return Err("Invalid confirmed speaker label".into());
        }
        let original = data
            .parts
            .iter()
            .find(|p| p.file == item.source_id)
            .and_then(|p| p.text.as_deref())
            .ok_or("Unknown transcript source")?;
        if item.text_sha256 != text_hash(original) || !original.contains(&item.evidence) {
            return Err(
                "Transcript changed or evidence is not an original quote; review the section again"
                    .into(),
            );
        }
    }
    let folder = directory(root)?;
    let path = folder.join(format!("{}.lock", data.id));
    if path.exists() {
        reject_link(&path)?;
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    lock.try_lock_exclusive()
        .map_err(|_| "Speaker annotations are being edited elsewhere")?;
    let old = read(root, &data.id)?;
    if old.revision != expected {
        return Err("Speaker annotations changed; reload before saving".into());
    }
    let next = SpeakerDocument {
        revision: old.revision.checked_add(1).ok_or("Revision overflow")?,
        enabled,
        assignments,
    };
    if serde_json::to_vec(&next)?.len() > 1024 * 1024 {
        return Err("Speaker annotations are too large".into());
    }
    let history = folder.join(format!("{}-history", data.id));
    match fs::create_dir(&history) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    }
    metadata::checked_directory(&history)?;
    let mut archive = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(history.join(format!(
            "{:020}-{}.json",
            old.revision,
            uuid::Uuid::new_v4().simple()
        )))?;
    archive.write_all(&serde_json::to_vec_pretty(&old)?)?;
    archive.sync_all()?;
    atomic_json(&folder.join(format!("{}.json", data.id)), &next)?;
    Ok(next)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn data() -> SessionData {
        serde_json::from_value(serde_json::json!({"schemaVersion":2,"id":"0123456789abcdef0123456789abcdef","title":"Fixture","startedAt":"2026-10-27T00:00:00Z","modelId":"synthetic","status":"complete","parts":[{"file":"audio/system-0.wav","source":"system","startSample":0,"sampleCount":16000,"confirmedSampleCount":16000,"text":"Anna, bitte lies den Bericht.","status":"complete"}]})).unwrap()
    }
    #[test]
    fn explicit_labels_require_unchanged_source_and_original_quote() {
        let root = tempfile::tempdir().unwrap();
        let data = data();
        let original = serde_json::to_vec(&data).unwrap();
        assert!(read(root.path(), &data.id).unwrap().assignments.is_empty());
        let label = Assignment {
            source_id: data.parts[0].file.clone(),
            text_sha256: text_hash(data.parts[0].text.as_deref().unwrap()),
            name: "Manually confirmed person".into(),
            evidence: "Anna, bitte".into(),
        };
        let saved = save(root.path(), &data, 0, true, vec![label.clone()]).unwrap();
        assert_eq!(saved.revision, 1);
        assert_eq!(serde_json::to_vec(&data).unwrap(), original);
        assert!(save(root.path(), &data, 0, true, vec![]).is_err());
        let mut changed = data.clone();
        changed.parts[0].text = Some("Different original".into());
        assert!(save(root.path(), &changed, 1, true, vec![label.clone()]).is_err());
        let mut wrong = label;
        wrong.evidence = "Invented quote".into();
        assert!(save(root.path(), &data, 1, true, vec![wrong]).is_err());
        assert_eq!(read(root.path(), &data.id).unwrap().assignments.len(), 1);
    }
}
