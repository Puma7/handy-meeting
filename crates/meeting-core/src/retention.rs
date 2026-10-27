//! Disabled-by-default retention policy and read-only proposals. Applying a plan is separate.
use crate::{
    archive,
    audio_codec::Codec,
    metadata,
    storage::{atomic_json, reject_link},
    Result,
};
use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy {
    pub revision: u64,
    pub untouched_days: u32,
    pub audio_archive_after_days: Option<u32>,
    pub archive_after_days: Option<u32>,
    pub convert_after_days: Option<u32>,
    pub max_visible_meetings: Option<u32>,
    pub delete_archives_after_days: Option<u32>,
    pub codec: Codec,
    pub ffmpeg_executable: Option<PathBuf>,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            revision: 0,
            untouched_days: 30,
            audio_archive_after_days: None,
            archive_after_days: None,
            convert_after_days: None,
            max_visible_meetings: None,
            delete_archives_after_days: None,
            codec: Codec::Flac,
            ffmpeg_executable: None,
        }
    }
}
impl Policy {
    pub fn validate(&self) -> Result<()> {
        if !(1..=36500).contains(&self.untouched_days)
            || [
                self.audio_archive_after_days,
                self.archive_after_days,
                self.convert_after_days,
            ]
            .into_iter()
            .flatten()
            .any(|d| d < self.untouched_days || d > 36500)
            || self
                .delete_archives_after_days
                .is_some_and(|d| !(1..=36500).contains(&d))
            || self
                .max_visible_meetings
                .is_some_and(|n| !(1..=1000000).contains(&n))
            || self
                .ffmpeg_executable
                .as_ref()
                .is_some_and(|p| !p.is_absolute())
        {
            return Err("Invalid retention policy; untouched period and absolute local tool path must be respected".into());
        }
        Ok(())
    }
}
fn json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    reject_link(path)?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Oversized maintenance record; retained".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn read_policy(root: &Path) -> Result<Policy> {
    metadata::checked_directory(root)?;
    let path = root.join("retention-settings.json");
    if !path.exists() {
        let history = root.join("retention-settings-history");
        if history.exists() {
            metadata::checked_directory(&history)?;
            if fs::read_dir(history)?.next().is_some() {
                return Err("Current retention settings are missing; recover retained revisions before saving".into());
            }
        }
        return Ok(Default::default());
    }
    let value: Policy = json(&path)?;
    value.validate()?;
    Ok(value)
}
pub fn save_policy(root: &Path, expected: u64, mut next: Policy) -> Result<Policy> {
    next.validate()?;
    metadata::checked_directory(root)?;
    let lock_path = root.join("retention-settings.lock");
    if lock_path.exists() {
        reject_link(&lock_path)?;
    }
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    lock.try_lock_exclusive()?;
    let old = read_policy(root)?;
    if old.revision != expected || next.revision != expected {
        return Err("Retention policy changed; reload before saving".into());
    }
    let history = root.join("retention-settings-history");
    match fs::create_dir(&history) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    }
    metadata::checked_directory(&history)?;
    let mut backup = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(history.join(format!(
            "{:020}-{}.json",
            old.revision,
            uuid::Uuid::new_v4().simple()
        )))?;
    backup.write_all(&serde_json::to_vec_pretty(&old)?)?;
    backup.sync_all()?;
    next.revision = expected.checked_add(1).ok_or("Policy revision overflow")?;
    atomic_json(&root.join("retention-settings.json"), &next)?;
    Ok(next)
}
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    ArchiveAudio,
    ArchiveMeeting,
    ConvertAudio,
    DeleteArchivedMeeting,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Candidate {
    pub meeting_id: String,
    pub title: String,
    pub action: Action,
    pub archive_id: Option<String>,
    pub metadata_revision: u64,
    pub original_manifest_sha256: String,
    pub age_days: u64,
    pub reason: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub id: String,
    pub policy_revision: u64,
    pub created_at: String,
    pub scanned: u64,
    pub protected: u64,
    pub unreadable: u64,
    pub candidates: Vec<Candidate>,
    pub limited: bool,
}
pub fn manifest_fingerprint(folder: &Path) -> Result<String> {
    metadata::checked_directory(folder)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    for name in [
        "session.json",
        "session.json.bak",
        "audio-journal.jsonl",
        "transcript-journal.jsonl",
        "audio-archive.json",
    ] {
        let path = folder.join(name);
        hash.update(name.as_bytes());
        if !path.exists() {
            hash.update(b"missing");
            continue;
        }
        reject_link(&path)?;
        let mut file = File::open(path)?;
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
    }
    let mut entries = Vec::new();
    tree_stamp(folder, folder, &mut entries)?;
    entries.sort();
    hash.update(serde_json::to_vec(&entries)?);
    Ok(format!("{:x}", hash.finalize()))
}
fn tree_stamp(
    folder: &Path,
    base: &Path,
    entries: &mut Vec<(String, bool, u64, String)>,
) -> Result<()> {
    metadata::checked_directory(folder)?;
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let path = entry.path();
        reject_link(&path)?;
        let meta = entry.metadata()?;
        let relative = path
            .strip_prefix(base)?
            .to_str()
            .ok_or("Invalid maintenance input path")?
            .replace('\\', "/");
        if relative == "session.lock" {
            continue;
        }
        if !meta.is_file() && !meta.is_dir() {
            return Err("Unsupported maintenance input type; retained".into());
        }
        entries.push((
            relative,
            meta.is_dir(),
            if meta.is_dir() { 0 } else { meta.len() },
            if meta.is_dir() {
                String::new()
            } else {
                format!("{:?}", meta.modified()?)
            },
        ));
        if entries.len() > 200000 {
            return Err("Maintenance inventory exceeds safe bound; retained".into());
        }
        if meta.is_dir() {
            tree_stamp(&path, base, entries)?;
        }
    }
    Ok(())
}
fn age(now: DateTime<Utc>, at: &str) -> Option<u64> {
    let date = DateTime::parse_from_rfc3339(at).ok()?.with_timezone(&Utc);
    (date <= now).then_some((now - date).num_days() as u64)
}
/// Preview reads metadata only. It never starts tools or removes session files.
pub fn preview(
    root: &Path,
    now: DateTime<Utc>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<Plan> {
    let policy = read_policy(root)?;
    let sessions = metadata::checked_directory(&root.join("sessions"))?;
    let total = fs::read_dir(&sessions)?.count() as u64;
    let mut plan = Plan {
        id: uuid::Uuid::new_v4().simple().to_string(),
        policy_revision: policy.revision,
        created_at: now.to_rfc3339(),
        scanned: 0,
        protected: 0,
        unreadable: 0,
        candidates: Vec::new(),
        limited: false,
    };
    let mut eligible = Vec::new();
    let mut visible = 0u64;
    for entry in fs::read_dir(sessions)? {
        if cancel.load(Ordering::Acquire) {
            return Err("Cleanup preview cancelled; all recordings are unchanged".into());
        }
        plan.scanned += 1;
        progress(plan.scanned, total);
        let entry = match entry {
            Ok(v) => v,
            Err(_) => {
                plan.unreadable += 1;
                continue;
            }
        };
        let id = entry.file_name().to_string_lossy().into_owned();
        if !metadata::valid_id(&id) || id.starts_with("legacy-") {
            continue;
        }
        visible += 1;
        let one = (|| -> Result<_> {
            let folder = metadata::checked_directory(&entry.path())?;
            let data = match archive::read_archived_transcript(&folder)? {
                Some(data) => data,
                None => crate::inspect_manifest(&folder)?,
            };
            let labels = metadata::read(root, &id)?;
            Ok((folder, data, labels))
        })();
        let (folder, data, labels) = match one {
            Ok(v) => v,
            Err(_) => {
                plan.unreadable += 1;
                plan.protected += 1;
                continue;
            }
        };
        let Some(days) = age(now, data.ended_at.as_deref().unwrap_or(&data.started_at)) else {
            plan.protected += 1;
            continue;
        };
        if labels.pinned
            || days < u64::from(policy.untouched_days)
            || data.status != "complete"
            || data.capture_incomplete
            || data
                .parts
                .iter()
                .any(|p| !p.audio_closed || p.status != "complete")
        {
            plan.protected += 1;
            continue;
        }
        eligible.push((days, id, labels, folder, data.title, data.read_only));
    }
    eligible.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut excess = policy
        .max_visible_meetings
        .map_or(0, |max| visible.saturating_sub(u64::from(max)));
    for (days, id, labels, folder, title, archived_audio) in eligible {
        let (action, reason) = if policy
            .archive_after_days
            .is_some_and(|d| days >= u64::from(d))
            || excess > 0
        {
            (Some(Action::ArchiveMeeting), "age or visible-meeting limit")
        } else if policy
            .audio_archive_after_days
            .is_some_and(|d| days >= u64::from(d))
            && !archived_audio
        {
            (
                Some(Action::ArchiveAudio),
                "audio-only age limit; transcript retained",
            )
        } else if policy
            .convert_after_days
            .is_some_and(|d| days >= u64::from(d))
            && !archived_audio
        {
            (
                Some(Action::ConvertAudio),
                "optional local audio conversion; originals retained",
            )
        } else {
            (None, "")
        };
        if let Some(action) = action {
            if plan.candidates.len() >= 500 {
                plan.limited = true;
                break;
            }
            let fingerprint = match manifest_fingerprint(&folder) {
                Ok(value) => value,
                Err(_) => {
                    plan.protected += 1;
                    plan.unreadable += 1;
                    continue;
                }
            };
            if action == Action::ArchiveMeeting {
                excess = excess.saturating_sub(1);
            }
            plan.candidates.push(Candidate {
                meeting_id: id,
                title: labels.title.unwrap_or(title),
                action,
                archive_id: None,
                metadata_revision: labels.revision,
                original_manifest_sha256: fingerprint,
                age_days: days,
                reason: reason.into(),
            });
        }
    }
    let archives = root.join("archives");
    let mut proposed = std::collections::HashSet::new();
    if let Some(delete_days) = policy.delete_archives_after_days {
        if archives.exists() {
            metadata::checked_directory(&archives)?;
            for entry in fs::read_dir(&archives)? {
                if cancel.load(Ordering::Acquire) {
                    return Err("Cleanup preview cancelled; all data retained".into());
                }
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if !name.ends_with(".receipt.json") {
                    continue;
                }
                let receipt: archive::Receipt = match json(&entry.path()) {
                    Ok(v) => v,
                    Err(_) => {
                        plan.unreadable += 1;
                        plan.protected += 1;
                        continue;
                    }
                };
                if !metadata::valid_id(&receipt.meeting_id)
                    || receipt.meeting_id.starts_with("legacy-")
                    || !metadata::valid_id(&receipt.archive_id)
                    || receipt.archive_id.starts_with("legacy-")
                    || name != format!("{}.receipt.json", receipt.archive_id)
                {
                    plan.protected += 1;
                    continue;
                }
                if receipt.kind != "whole-meeting"
                    || root.join("sessions").join(&receipt.meeting_id).exists()
                    || !proposed.insert(receipt.meeting_id.clone())
                {
                    continue;
                }
                let labels = match metadata::read(root, &receipt.meeting_id) {
                    Ok(v) => v,
                    Err(_) => {
                        plan.protected += 1;
                        continue;
                    }
                };
                let (group_hash, newest) =
                    match archive::deletion_fingerprint(root, &receipt.meeting_id, cancel) {
                        Ok(v) => v,
                        Err(_) => {
                            plan.protected += 1;
                            plan.unreadable += 1;
                            continue;
                        }
                    };
                let Some(days) = age(now, &newest) else {
                    plan.protected += 1;
                    continue;
                };
                if labels.pinned || days < u64::from(delete_days) {
                    plan.protected += 1;
                    continue;
                }
                if plan.candidates.len() >= 500 {
                    plan.limited = true;
                    break;
                }
                plan.candidates.push(Candidate{meeting_id:receipt.meeting_id.clone(),title:labels.title.unwrap_or_else(||if receipt.title.is_empty(){receipt.meeting_id.clone()}else{receipt.title.clone()}),action:Action::DeleteArchivedMeeting,archive_id:Some(receipt.archive_id),metadata_revision:labels.revision,original_manifest_sha256:group_hash,age_days:days,reason:"all owned archive versions, retained source copies and annotations; separate explicit permanent-delete confirmation required".into()});
            }
        }
    }
    Ok(plan)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn manifest(root: &Path, id: &str, when: &str, status: &str) {
        let folder = root.join("sessions").join(id);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("session.json"),serde_json::to_vec(&serde_json::json!({"schemaVersion":2,"id":id,"title":"Synthetic retained meeting","modelId":"fixture","startedAt":when,"status":status,"parts":[]})).unwrap()).unwrap();
    }
    #[test]
    fn rules_start_disabled_and_bad_settings_are_never_silently_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let policy = read_policy(root.path()).unwrap();
        assert!(
            policy.archive_after_days.is_none()
                && policy.audio_archive_after_days.is_none()
                && policy.convert_after_days.is_none()
                && policy.delete_archives_after_days.is_none()
                && policy.max_visible_meetings.is_none()
        );
        fs::write(
            root.path().join("retention-settings.json"),
            b"broken retained policy",
        )
        .unwrap();
        assert!(save_policy(root.path(), 0, Policy::default()).is_err());
        assert_eq!(
            fs::read(root.path().join("retention-settings.json")).unwrap(),
            b"broken retained policy"
        );
    }
    #[test]
    fn preview_fingerprint_detects_a_late_unknown_file_and_missing_policy_cannot_reset_history() {
        let root = tempfile::tempdir().unwrap();
        let id = "12345678901234567890123456789012";
        manifest(root.path(), id, "2024-01-01T00:00:00Z", "complete");
        let folder = root.path().join("sessions").join(id);
        let before = manifest_fingerprint(&folder).unwrap();
        fs::write(folder.join("user-note.txt"), b"preserve this").unwrap();
        assert_ne!(manifest_fingerprint(&folder).unwrap(), before);
        save_policy(root.path(), 0, Policy::default()).unwrap();
        fs::remove_file(root.path().join("retention-settings.json")).unwrap();
        assert!(save_policy(root.path(), 0, Policy::default()).is_err());
    }
    #[test]
    fn preview_protects_vip_fresh_incomplete_and_corrupt_labels_without_deleting() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("sessions")).unwrap();
        let ids: Vec<_> = (0..5).map(|n| format!("{n:032x}")).collect();
        for id in &ids {
            manifest(root.path(), id, "2024-01-01T00:00:00Z", "complete");
        }
        metadata::edit(root.path(), &ids[1], 0, None, true).unwrap();
        manifest(root.path(), &ids[2], "2026-10-26T00:00:00Z", "complete");
        manifest(root.path(), &ids[3], "2024-01-01T00:00:00Z", "recording");
        let dir = metadata::directory(root.path()).unwrap();
        fs::write(dir.join(format!("{}.json", ids[4])), b"corrupt pin state").unwrap();
        let policy = Policy {
            archive_after_days: Some(365),
            ..Default::default()
        };
        save_policy(root.path(), 0, policy).unwrap();
        let plan = preview(
            root.path(),
            DateTime::parse_from_rfc3339("2026-10-27T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap();
        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].meeting_id, ids[0]);
        assert_eq!(plan.protected, 4);
        assert_eq!(
            fs::read_dir(root.path().join("sessions")).unwrap().count(),
            5
        );
    }
    #[test]
    fn later_audio_archive_tier_takes_precedence_over_an_earlier_conversion_rule() {
        let root = tempfile::tempdir().unwrap();
        let id = "abcdef0123456789abcdef0123456789";
        manifest(root.path(), id, "2026-07-01T00:00:00Z", "complete");
        save_policy(
            root.path(),
            0,
            Policy {
                convert_after_days: Some(60),
                audio_archive_after_days: Some(90),
                ..Default::default()
            },
        )
        .unwrap();
        let plan = preview(
            root.path(),
            DateTime::parse_from_rfc3339("2026-10-27T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap();
        assert_eq!(plan.candidates[0].action, Action::ArchiveAudio);
    }
}
