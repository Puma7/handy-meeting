use crate::{AudioPart, Result, SessionData, SessionProgress, TranscriptCheckpoint, SAMPLE_RATE};
use fs2::FileExt;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;
const MAX_JOURNAL_LINE: usize = 2 * 1024 * 1024;

struct Inner {
    data: SessionData,
    _lease: File,
}

/// Clones share one writer and one exclusive lease. New Rust sessions only.
#[derive(Clone)]
pub struct SessionStore {
    folder: PathBuf,
    inner: Arc<Mutex<Inner>>,
}

impl SessionStore {
    pub fn create(
        root: &Path,
        title: &str,
        model_id: &str,
        output_name: &str,
        microphone_name: Option<&str>,
    ) -> Result<Self> {
        fs::create_dir_all(root)?;
        let id = Uuid::new_v4().simple().to_string();
        let folder = root.join(&id);
        fs::create_dir(&folder)?;
        fs::create_dir(folder.join("audio"))?;
        let lease = lease(&folder)?;
        let data = SessionData {
            schema_version: 2,
            id,
            title: if title.trim().is_empty() {
                "Meeting".into()
            } else {
                title.trim().into()
            },
            started_at: now(),
            ended_at: None,
            status: "recording".into(),
            model_id: model_id.into(),
            output_name: output_name.into(),
            microphone_name: microphone_name.map(str::to_owned),
            chunk_seconds: 30,
            duration_ms: 0,
            capture_incomplete: false,
            warnings: vec![],
            parts: vec![],
            snapshot_sequence: 0,
            read_only: false,
        };
        atomic_json(&folder.join("session.json"), &data)?;
        Ok(Self {
            folder,
            inner: Arc::new(Mutex::new(Inner {
                data,
                _lease: lease,
            })),
        })
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }
    #[cfg(test)]
    pub(crate) fn test_owner_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
    pub fn snapshot(&self) -> Result<SessionData> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| "Session lock was poisoned")?
            .data
            .clone())
    }

    pub fn model_id(&self) -> Result<String> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| "Session lock was poisoned")?
            .data
            .model_id
            .clone())
    }

    pub fn next_pending_part(&self, source: &str, model_id: &str) -> Result<Option<AudioPart>> {
        let guard = self.inner.lock().map_err(|_| "Session lock was poisoned")?;
        Ok(guard
            .data
            .parts
            .iter()
            .filter(|part| {
                part.source == source
                    && !(part.status == "complete" && part.model_id.as_deref() == Some(model_id))
            })
            .min_by_key(|part| part.start_sample)
            .cloned())
    }

    pub fn recognition_complete(&self) -> Result<bool> {
        let guard = self.inner.lock().map_err(|_| "Session lock was poisoned")?;
        Ok(guard.data.parts.iter().all(|part| {
            part.audio_closed
                && part.status == "complete"
                && part.model_id.as_deref() == Some(guard.data.model_id.as_str())
        }))
    }

    /// Clone only bounded recent parts; full meeting text never enters this path.
    pub fn preview_snapshot(&self, limit: usize) -> Result<SessionData> {
        let guard = self.inner.lock().map_err(|_| "Session lock was poisoned")?;
        let data = &guard.data;
        let limit = limit.min(64);
        let mut recent: Vec<&AudioPart> = Vec::with_capacity(limit + 1);
        for part in &data.parts {
            let index = recent.partition_point(|old| {
                (old.start_sample, old.source.as_str()) <= (part.start_sample, part.source.as_str())
            });
            if recent.len() == limit && index == 0 {
                continue;
            }
            recent.insert(index, part);
            if recent.len() > limit {
                recent.remove(0);
            }
        }
        Ok(SessionData {
            schema_version: data.schema_version,
            id: data.id.clone(),
            title: data.title.clone(),
            started_at: data.started_at.clone(),
            ended_at: data.ended_at.clone(),
            status: data.status.clone(),
            model_id: data.model_id.clone(),
            output_name: data.output_name.clone(),
            microphone_name: data.microphone_name.clone(),
            chunk_seconds: data.chunk_seconds,
            duration_ms: data.duration_ms,
            capture_incomplete: data.capture_incomplete,
            warnings: data.warnings.clone(),
            parts: recent.into_iter().cloned().collect(),
            snapshot_sequence: data.snapshot_sequence,
            read_only: data.read_only,
        })
    }

    pub fn progress(&self) -> Result<SessionProgress> {
        let guard = self.inner.lock().map_err(|_| "Session lock was poisoned")?;
        let data = &guard.data;
        let mut result = SessionProgress {
            total_parts: data.parts.len(),
            complete_parts: 0,
            system_confirmed_ms: 0,
            microphone_confirmed_ms: 0,
            pending_ms: 0,
        };
        let mut pending_system = 0;
        let mut pending_microphone = 0;
        for part in &data.parts {
            let same_model = part.model_id.as_deref() == Some(data.model_id.as_str());
            if same_model && part.status == "complete" {
                result.complete_parts += 1;
            }
            let end_ms =
                (part.start_sample + part.confirmed_sample_count) * 1000 / u64::from(SAMPLE_RATE);
            let pending = if same_model {
                part.confirmed_sample_count
                    .saturating_sub(part.processed_samples)
            } else {
                part.confirmed_sample_count
            };
            if part.source == "system" {
                result.system_confirmed_ms = result.system_confirmed_ms.max(end_ms);
                pending_system += pending;
            } else {
                result.microphone_confirmed_ms = result.microphone_confirmed_ms.max(end_ms);
                pending_microphone += pending;
            }
        }
        result.pending_ms = pending_system.max(pending_microphone) * 1000 / u64::from(SAMPLE_RATE);
        Ok(result)
    }

    /// Recover a Rust v2 session. Legacy sessions are available through read_legacy only.
    pub fn open(folder: &Path) -> Result<Self> {
        reject_link(folder)?;
        let lease = lease(folder)?;
        let manifest = folder.join("session.json");
        let (mut data, backup): (SessionData, bool) = match read_json(&manifest) {
            Ok(data) => (data, false),
            Err(_) => (read_json(&folder.join("session.json.bak"))?, true),
        };
        if data.schema_version != 2 {
            return Err(
                "Use read_legacy for original C# sessions; they are never rewritten".into(),
            );
        }
        validate_data(&data)?;
        reject_link(&folder.join("audio"))?;
        let interrupted = data.ended_at.is_none();
        // The audio journal is authoritative for acknowledged sample counts. Bytes beyond
        // that point are preserved on disk but never silently advertised as confirmed.
        let mut malformed = 0;
        let mut seen_audio = std::collections::HashMap::<String, u64>::new();
        read_lines::<AudioPart, _>(
            &folder.join("audio-journal.jsonl"),
            &mut malformed,
            |part| {
                validate_part(&part)?;
                if seen_audio
                    .insert(part.file.clone(), part.confirmed_sample_count)
                    .is_some_and(|previous| part.confirmed_sample_count < previous)
                {
                    return Err("Audio journal moved backward".into());
                }
                if let Some(existing) = data.parts.iter_mut().find(|old| old.file == part.file) {
                    if part.confirmed_sample_count < existing.confirmed_sample_count {
                        return Ok(());
                    }
                    let text = existing.clone();
                    *existing = part;
                    existing.text = text.text;
                    existing.committed_text = text.committed_text;
                    existing.tentative_text = text.tentative_text;
                    existing.processed_samples = text.processed_samples;
                    existing.model_id = text.model_id;
                    existing.status = text.status;
                    existing.transcript_sequence = text.transcript_sequence;
                } else {
                    data.parts.push(part);
                }
                Ok(())
            },
        )?;
        for part in &mut data.parts {
            validate_part(part)?;
            let audio = safe_audio_path(folder, &part.file)?;
            crate::pcm::verify_confirmed(&audio, part)?;
            part.sample_count = part.confirmed_sample_count;
            if interrupted {
                part.audio_closed = true;
            }
        }
        read_lines::<TranscriptCheckpoint, _>(
            &folder.join("transcript-journal.jsonl"),
            &mut malformed,
            |checkpoint| {
                if let Some(part) = data
                    .parts
                    .iter_mut()
                    .find(|part| part.file == checkpoint.file)
                {
                    validate_checkpoint(part, &checkpoint)?;
                    if checkpoint.sequence >= part.transcript_sequence {
                        validate_continuity(part, &checkpoint)?;
                        apply_checkpoint(part, &checkpoint);
                    }
                    data.snapshot_sequence = data.snapshot_sequence.max(checkpoint.sequence);
                } else {
                    return Err("Text journal references unknown audio".into());
                }
                Ok(())
            },
        )?;
        if malformed > 0 {
            warn_data(&mut data, &format!("{malformed} unfinished journal line(s) retained; last confirmed data recovered."));
        }
        if backup {
            warn_data(
                &mut data,
                "Recovered session metadata from its backup; original damaged file retained.",
            );
        }
        if interrupted {
            warn_data(&mut data, "Recording process was interrupted. Confirmed audio and text recovered; an unconfirmed tail may be missing.");
            data.status = "recorded".into();
        }
        data.duration_ms = duration(&data.parts);
        let store = Self {
            folder: folder.to_owned(),
            inner: Arc::new(Mutex::new(Inner {
                data,
                _lease: lease,
            })),
        };
        // Recovery is journal-derived and read-only until another explicit update.
        Ok(store)
    }

    pub fn audio_path(&self, part: &AudioPart) -> Result<PathBuf> {
        validate_part(part)?;
        safe_audio_path(&self.folder, &part.file)
    }

    pub fn audio_checkpoint(&self, mut part: AudioPart) -> Result<()> {
        validate_part(&part)?;
        let mut guard = self.inner.lock().map_err(|_| "Session lock was poisoned")?;
        if let Some(existing) = guard.data.parts.iter().find(|old| old.file == part.file) {
            if part.confirmed_sample_count < existing.confirmed_sample_count {
                return Err("Confirmed audio cannot move backward".into());
            }
            part.text = existing.text.clone();
            part.committed_text = existing.committed_text.clone();
            part.tentative_text = existing.tentative_text.clone();
            part.processed_samples = existing.processed_samples;
            part.model_id = existing.model_id.clone();
            part.status = existing.status.clone();
            part.transcript_sequence = existing.transcript_sequence;
        }
        append_json(&self.folder.join("audio-journal.jsonl"), &part)?;
        guard.data.duration_ms = guard
            .data
            .duration_ms
            .max((part.start_sample + part.confirmed_sample_count) * 1000 / u64::from(SAMPLE_RATE));
        if let Some(existing) = guard
            .data
            .parts
            .iter_mut()
            .find(|old| old.file == part.file)
        {
            *existing = part;
        } else {
            guard.data.parts.push(part);
        }
        // Journal is committed before UI snapshots can acquire the mutex.
        Ok(())
    }

    pub fn append_transcript(&self, mut checkpoint: TranscriptCheckpoint) -> Result<()> {
        let mut guard = self.inner.lock().map_err(|_| "Session lock was poisoned")?;
        let part = guard
            .data
            .parts
            .iter()
            .find(|part| part.file == checkpoint.file)
            .ok_or("Unknown audio part")?;
        validate_checkpoint(part, &checkpoint)?;
        validate_continuity(part, &checkpoint)?;
        let next = guard
            .data
            .snapshot_sequence
            .checked_add(1)
            .ok_or("Sequence overflow")?;
        if checkpoint.sequence != 0 && checkpoint.sequence != next {
            return Err("Unexpected transcript sequence".into());
        }
        checkpoint.sequence = next;
        checkpoint.at = now();
        append_json(&self.folder.join("transcript-journal.jsonl"), &checkpoint)?;
        let part = guard
            .data
            .parts
            .iter_mut()
            .find(|part| part.file == checkpoint.file)
            .ok_or("Unknown audio part")?;
        apply_checkpoint(part, &checkpoint);
        guard.data.snapshot_sequence = next;
        Ok(())
    }

    pub fn set_status(&self, status: &str) -> Result<()> {
        if ![
            "recording",
            "recorded",
            "streaming",
            "catching-up",
            "complete",
            "error",
        ]
        .contains(&status)
        {
            return Err("Unknown session status".into());
        }
        let mut guard = self.inner.lock().map_err(|_| "Session lock was poisoned")?;
        let mut updated = guard.data.clone();
        updated.status = status.into();
        if status == "recorded" {
            updated.ended_at = Some(now());
        }
        atomic_json(&self.folder.join("session.json"), &updated)?;
        guard.data = updated;
        Ok(())
    }

    pub fn warn(&self, warning: &str) -> Result<()> {
        let mut guard = self.inner.lock().map_err(|_| "Session lock was poisoned")?;
        let mut updated = guard.data.clone();
        warn_data(&mut updated, warning);
        atomic_json(&self.folder.join("session.json"), &updated)?;
        guard.data = updated;
        Ok(())
    }

    /// An inference/UI warning does not imply that the audio capture was incomplete.
    pub fn notice(&self, warning: &str) -> Result<()> {
        let mut guard = self.inner.lock().map_err(|_| "Session lock was poisoned")?;
        let mut updated = guard.data.clone();
        if !updated.warnings.iter().any(|old| old == warning) {
            updated.warnings.push(warning.into());
        }
        atomic_json(&self.folder.join("session.json"), &updated)?;
        guard.data = updated;
        Ok(())
    }

    pub fn transcript_text(&self) -> Result<String> {
        Ok(render_transcript(&self.snapshot()?))
    }
}

pub fn list_sessions(root: &Path) -> Result<Vec<SessionData>> {
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut result = vec![];
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() && reject_link(&path).is_ok() {
            let manifest = path.join("session.json");
            if let Ok(data) = read_json::<SessionData>(&manifest) {
                if validate_data(&data).is_ok() {
                    result.push(data);
                }
            }
        }
    }
    result.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(result)
}

/// Inspect existing v1 data without creating a lease, repairing a header or writing exports.
pub fn read_legacy(manifest: &Path) -> Result<SessionData> {
    reject_link(manifest)?;
    let folder = manifest.parent().ok_or("Session directory missing")?;
    reject_link(folder)?;
    let mut data: SessionData = read_json(manifest)?;
    if data.schema_version != 1 {
        return Err("Unsupported legacy session version".into());
    }
    validate_data(&data)?;
    for part in &mut data.parts {
        if !safe_audio_path(folder, &part.file)?.is_file() {
            return Err("Legacy audio file is missing".into());
        }
        part.confirmed_sample_count = part.sample_count;
    }
    let mut malformed = 0;
    let manifest_sequence = data.snapshot_sequence;
    let selected_model = data.model_id.clone();
    read_lines::<TranscriptCheckpoint, _>(
        &folder.join("transcript-journal.jsonl"),
        &mut malformed,
        |checkpoint| {
            if let Some(part) = data
                .parts
                .iter_mut()
                .find(|part| part.file == checkpoint.file)
            {
                validate_checkpoint(part, &checkpoint)?;
                // Batch transcription in v1 can commit a newer full manifest without
                // appending a streaming journal entry. Do not replace that result with
                // an older or another model's partial checkpoint during read-only import.
                if checkpoint.model_id != selected_model
                    || (part.status == "complete" && checkpoint.sequence <= manifest_sequence)
                    || (part.status == "complete"
                        && checkpoint.processed_samples < part.sample_count)
                    || (part.model_id.as_deref() == Some(checkpoint.model_id.as_str())
                        && checkpoint.processed_samples < part.processed_samples)
                {
                    return Ok(());
                }
                apply_checkpoint(part, &checkpoint);
            }
            Ok(())
        },
    )?;
    if malformed > 0 {
        data.warnings
            .push("Legacy journal has an unfinished line; original bytes were retained.".into());
    }
    data.read_only = true;
    Ok(data)
}

pub fn render_transcript(data: &SessionData) -> String {
    let mut text = format!(
        "{}\n{} · {}\nSection times are recording ranges, not exact word times.\n",
        data.title, data.started_at, data.model_id
    );
    let mut parts: Vec<_> = data.parts.iter().collect();
    parts.sort_by_key(|part| (part.start_sample, part.source.clone()));
    for part in parts {
        if let Some(value) = &part.text {
            if !value.trim().is_empty() {
                text.push_str(&format!(
                    "\n[{}–{}] {}\n{}\n",
                    time(part.start_sample),
                    time(part.start_sample + part.sample_count),
                    part.source,
                    value
                ));
            }
        }
    }
    text
}

fn time(sample: u64) -> String {
    let seconds = sample / u64::from(SAMPLE_RATE);
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        (seconds / 60) % 60,
        seconds % 60
    )
}
fn duration(parts: &[AudioPart]) -> u64 {
    parts
        .iter()
        .map(|p| (p.start_sample + p.confirmed_sample_count) * 1000 / u64::from(SAMPLE_RATE))
        .max()
        .unwrap_or(0)
}
pub(crate) fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn warn_data(data: &mut SessionData, text: &str) {
    data.capture_incomplete = true;
    if !data.warnings.iter().any(|old| old == text) {
        data.warnings.push(text.into());
    }
}

fn lease(folder: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(folder.join("session.lock"))?;
    file.try_lock_exclusive()
        .map_err(|_| "This session is already active in another process")?;
    Ok(file)
}
fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    reject_link(path)?;
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_MANIFEST_BYTES {
        return Err("Metadata exceeds safety limit".into());
    }
    Ok(serde_json::from_reader(file)?)
}

pub(crate) fn validate_part(part: &AudioPart) -> Result<()> {
    let (source, suffix) = part.file.split_once('-').ok_or("Invalid audio file name")?;
    let number = suffix
        .strip_suffix(".wav")
        .ok_or("Invalid audio file suffix")?;
    if !["system", "microphone"].contains(&source)
        || source != part.source
        || number.len() < 12
        || !number.bytes().all(|b| b.is_ascii_digit())
        || number.parse::<u64>()? != part.start_sample
        || part.sample_count > u64::from(SAMPLE_RATE) * 300
        || part.confirmed_sample_count > part.sample_count
    {
        return Err("Unsafe or inconsistent audio part".into());
    }
    part.start_sample
        .checked_add(part.sample_count)
        .ok_or("Audio timeline overflow")?;
    Ok(())
}
fn validate_data(data: &SessionData) -> Result<()> {
    if ![1, 2].contains(&data.schema_version)
        || !(1..=300).contains(&data.chunk_seconds)
        || data.parts.len() > 100_000
    {
        return Err("Unsupported or oversized session".into());
    }
    for part in &data.parts {
        validate_part(part)?;
    }
    Ok(())
}
fn validate_checkpoint(part: &AudioPart, checkpoint: &TranscriptCheckpoint) -> Result<()> {
    if checkpoint.model_id.is_empty()
        || checkpoint.processed_samples > part.confirmed_sample_count
        || checkpoint.full.len() > 1024 * 1024
        || checkpoint.committed.len() > 1024 * 1024
        || checkpoint.tentative.len() > 1024 * 1024
    {
        return Err("Transcript does not match confirmed audio".into());
    }
    Ok(())
}

fn validate_continuity(part: &AudioPart, checkpoint: &TranscriptCheckpoint) -> Result<()> {
    if part.model_id.as_deref() == Some(checkpoint.model_id.as_str()) {
        if checkpoint.processed_samples < part.processed_samples {
            return Err(
                "Confirmed transcript progress cannot move backward during recognition restart"
                    .into(),
            );
        }
        if !part.committed_text.is_empty()
            && (!checkpoint.committed.starts_with(&part.committed_text)
                || !checkpoint.full.starts_with(&part.committed_text))
        {
            return Err("Confirmed transcript prefix changed during recognition restart; previous confirmed text remains visible".into());
        }
    }
    Ok(())
}
fn apply_checkpoint(part: &mut AudioPart, checkpoint: &TranscriptCheckpoint) {
    part.text = Some(checkpoint.full.clone());
    part.committed_text = checkpoint.committed.clone();
    part.tentative_text = checkpoint.tentative.clone();
    part.processed_samples = checkpoint.processed_samples;
    part.model_id = Some(checkpoint.model_id.clone());
    part.transcript_sequence = checkpoint.sequence;
    part.status = if checkpoint.final_text
        && part.audio_closed
        && checkpoint.processed_samples == part.confirmed_sample_count
    {
        "complete"
    } else {
        "pending"
    }
    .into();
}

pub(crate) fn safe_audio_path(folder: &Path, name: &str) -> Result<PathBuf> {
    if Path::new(name).components().count() != 1 || name.contains(['/', '\\', ':']) {
        return Err("Unsafe audio path".into());
    }
    let audio = folder.join("audio");
    reject_link(&audio)?;
    let path = audio.join(name);
    if path.exists() {
        reject_link(&path)?;
    }
    Ok(path)
}
pub(crate) fn reject_link(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err("Links are not allowed in session storage".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("Reparse points are not allowed in session storage".into());
        }
    }
    Ok(())
}

fn append_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if path.exists() {
        reject_link(path)?;
    }
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_JOURNAL_LINE {
        return Err("Journal entry exceeds safety limit".into());
    }
    let mut output = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)?;
    // Retain an unfinished crash tail, but separate it from the next complete record.
    if output.metadata()?.len() > 0 {
        use std::io::{Seek, SeekFrom};
        output.seek(SeekFrom::End(-1))?;
        let mut last = [0];
        output.read_exact(&mut last)?;
        if last[0] != b'\n' {
            output.write_all(b"\n")?;
        }
    }
    output.write_all(&bytes)?;
    output.write_all(b"\n")?;
    output.sync_all()?;
    Ok(())
}

fn read_lines<T: DeserializeOwned, F: FnMut(T) -> Result<()>>(
    path: &Path,
    malformed: &mut usize,
    mut consume: F,
) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    reject_link(path)?;
    let mut reader = BufReader::new(File::open(path)?);
    let mut line = vec![];
    // Replay one bounded line at a time, regardless of meeting duration.
    loop {
        line.clear();
        let bytes = reader
            .by_ref()
            .take((MAX_JOURNAL_LINE + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if bytes == 0 {
            break;
        }
        if bytes > MAX_JOURNAL_LINE {
            return Err("Oversized journal entry".into());
        }
        if !line.ends_with(b"\n") {
            *malformed += 1;
            break;
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice(&line) {
            Ok(value) => consume(value)?,
            Err(_) => *malformed += 1,
        }
    }
    Ok(())
}

fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path.parent().ok_or("Metadata directory missing")?;
    let bytes = serde_json::to_vec_pretty(value)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("Metadata exceeds safety limit".into());
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    if path.exists() {
        reject_link(path)?;
        let backup = path.with_extension("json.bak");
        if backup.exists() {
            reject_link(&backup)?;
        }
        fs::copy(path, &backup)?;
        OpenOptions::new().write(true).open(backup)?.sync_all()?;
    }
    temporary.persist(path).map_err(|error| error.error)?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_hour_metadata_preview_is_bounded_but_old_pending_audio_remains_selectable() {
        let root = tempfile::tempdir().unwrap();
        let store = SessionStore::create(
            root.path(),
            "Two-hour metadata fixture",
            "m",
            "synthetic",
            Some("synthetic"),
        )
        .unwrap();
        {
            let mut guard = store.inner.lock().unwrap();
            // Accelerated metadata-only fixture, not a real-time recording test.
            for window in (0..240_u64).rev() {
                for source in ["system", "microphone"] {
                    let start = window * 480_000;
                    guard.data.parts.push(AudioPart {
                        file: format!("{source}-{start:012}.wav"),
                        source: source.into(),
                        start_sample: start,
                        sample_count: 480_000,
                        confirmed_sample_count: 480_000,
                        has_signal: true,
                        audio_closed: true,
                        status: if window == 0 && source == "system" {
                            "pending"
                        } else {
                            "complete"
                        }
                        .into(),
                        text: Some("x".repeat(8192)),
                        committed_text: String::new(),
                        tentative_text: String::new(),
                        processed_samples: 480_000,
                        model_id: Some("m".into()),
                        error: None,
                        confirmed_sha256: String::new(),
                        transcript_sequence: 0,
                    });
                }
            }
            guard.data.duration_ms = 7_200_000;
        }
        let preview = store.preview_snapshot(16).unwrap();
        assert_eq!(preview.parts.len(), 16);
        assert_eq!(preview.duration_ms, 7_200_000);
        assert!(preview
            .parts
            .iter()
            .all(|part| part.start_sample >= 232 * 480_000));
        assert!(serde_json::to_vec(&preview).unwrap().len() < 150_000);
        assert!(store.preview_snapshot(0).unwrap().parts.is_empty());
        assert_eq!(
            store
                .next_pending_part("system", "m")
                .unwrap()
                .unwrap()
                .start_sample,
            0
        );
        assert_eq!(store.progress().unwrap().total_parts, 480);
        assert_eq!(store.progress().unwrap().complete_parts, 479);
        assert!(!store.recognition_complete().unwrap());
    }
}
