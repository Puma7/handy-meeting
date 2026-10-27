//! Readable derived TXT follows confirmed journals; failures never control capture.
use crate::{
    storage::{atomic_json, reject_link, render_transcript},
    Result, SessionData,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
const HEADER:&str="HandyMeeting derived transcript v1\nJournal-backed readable copy; section times are not word timestamps.\n\n";
#[derive(Serialize, Deserialize)]
struct Receipt {
    sha256: String,
    bytes: usize,
    sequence: u64,
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn refresh(folder: &Path, data: &SessionData) -> Result<()> {
    if data.read_only {
        return Ok(());
    }
    reject_link(folder)?;
    let file = folder.join("transcript.txt");
    let receipt = folder.join("transcript.txt.receipt.json");
    let text = format!("{HEADER}{}", render_transcript(data));
    let bytes = text.as_bytes();
    if file.exists() {
        reject_link(&file)?;
        let mut previous = Vec::new();
        fs::File::open(&file)?
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut previous)?;
        if previous.len() > 32 * 1024 * 1024 {
            return Err("Existing readable text is oversized; preserved".into());
        }
        if previous != bytes {
            reject_link(&receipt)?;
            let mut content = Vec::new();
            fs::File::open(&receipt)?
                .take(4097)
                .read_to_end(&mut content)?;
            if content.len() > 4096 {
                return Err("Invalid live-text ownership receipt".into());
            }
            let old: Receipt = serde_json::from_slice(&content)?;
            if !previous.starts_with(HEADER.as_bytes())
                || old.bytes != previous.len()
                || old.sha256 != digest(&previous)
            {
                return Err(
                    "Readable text was changed outside this writer; it was preserved".into(),
                );
            }
        }
    }
    let mut pending = tempfile::NamedTempFile::new_in(folder)?;
    pending.write_all(bytes)?;
    pending.as_file().sync_all()?;
    pending.persist(&file).map_err(|e| e.error)?;
    atomic_json(
        &receipt,
        &Receipt {
            sha256: digest(bytes),
            bytes: bytes.len(),
            sequence: data.snapshot_sequence,
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PcmWriter, SessionStore, TranscriptCheckpoint};
    fn eventually(mut condition: impl FnMut() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !condition() {
            assert!(
                std::time::Instant::now() < deadline,
                "Readable text did not settle"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    fn checkpoint(store: &SessionStore, text: &str) {
        let p = store.snapshot().unwrap().parts[0].clone();
        store
            .append_transcript(TranscriptCheckpoint {
                sequence: 0,
                at: String::new(),
                file: p.file,
                model_id: "fixture-model".into(),
                full: text.into(),
                committed: text.into(),
                tentative: String::new(),
                processed_samples: p.confirmed_sample_count,
                final_text: true,
            })
            .unwrap();
    }
    #[test]
    fn readable_text_tracks_journals_and_survives_model_independent_reopen() {
        let root = tempfile::tempdir().unwrap();
        let store =
            SessionStore::create(root.path(), "Meeting", "fixture-model", "output", None).unwrap();
        let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
        writer.append(&[1; 160]).unwrap();
        writer.finish().unwrap();
        checkpoint(&store, "Original confirmed sentence.");
        let file = store.folder().join("transcript.txt");
        eventually(|| {
            fs::read_to_string(&file)
                .is_ok_and(|text| text.contains("Original confirmed sentence."))
        });
        let first = fs::read_to_string(&file).unwrap();
        assert!(first.contains("Original confirmed sentence."));
        assert!(
            fs::read_to_string(store.folder().join("transcript-journal.jsonl"))
                .unwrap()
                .contains("Original confirmed sentence.")
        );
        checkpoint(&store, "Original confirmed sentence. More confirmed words.");
        eventually(|| {
            fs::read_to_string(&file).is_ok_and(|text| text.contains("More confirmed words."))
        });
    }
    #[test]
    fn explicit_status_update_rewrites_text_recovered_from_journals_after_a_crash() {
        let root = tempfile::tempdir().unwrap();
        let store =
            SessionStore::create(root.path(), "Meeting", "fixture-model", "output", None).unwrap();
        let folder = store.folder().to_owned();
        let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
        writer.append(&[1; 160]).unwrap();
        writer.finish().unwrap();
        drop(writer);
        checkpoint(&store, "Recovered confirmed sentence.");
        let file = folder.join("transcript.txt");
        eventually(|| folder.join("transcript.txt.receipt.json").is_file());
        drop(store);
        // Simulate an exit after the journal commit but before the mirror wrote.
        fs::remove_file(&file).unwrap();
        fs::remove_file(folder.join("transcript.txt.receipt.json")).unwrap();
        let reopened = SessionStore::open(&folder).unwrap();
        assert!(!file.exists(), "opening stays read-only");
        reopened.set_status("complete").unwrap();
        eventually(|| {
            fs::read_to_string(&file)
                .is_ok_and(|text| text.contains("Recovered confirmed sentence."))
        });
    }
    #[test]
    fn user_edited_text_is_preserved_without_blocking_confirmed_journal_writes() {
        let root = tempfile::tempdir().unwrap();
        let store =
            SessionStore::create(root.path(), "Meeting", "fixture-model", "output", None).unwrap();
        let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
        writer.append(&[1; 160]).unwrap();
        writer.finish().unwrap();
        checkpoint(&store, "Original confirmed sentence.");
        let file = store.folder().join("transcript.txt");
        eventually(|| store.folder().join("transcript.txt.receipt.json").is_file());
        fs::write(&file, b"User-owned edited text").unwrap();
        checkpoint(&store, "Original confirmed sentence. More confirmed words.");
        eventually(|| {
            store
                .snapshot()
                .unwrap()
                .warnings
                .iter()
                .any(|s| s.contains("TXT"))
        });
        assert_eq!(fs::read(file).unwrap(), b"User-owned edited text");
        assert!(store.snapshot().unwrap().parts[0]
            .text
            .as_ref()
            .unwrap()
            .contains("More confirmed words."));
        assert!(store
            .snapshot()
            .unwrap()
            .warnings
            .iter()
            .any(|s| s.contains("TXT")));
        // A derived-text failure must not exclude intact audio from retention.
        assert!(!store.snapshot().unwrap().capture_incomplete);
    }
    #[test]
    fn coalesced_writes_flush_latest_text_before_releasing_the_session() {
        let root = tempfile::tempdir().unwrap();
        let store =
            SessionStore::create(root.path(), "Meeting", "fixture-model", "output", None).unwrap();
        let folder = store.folder().to_owned();
        let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
        writer.append(&[1; 160]).unwrap();
        writer.finish().unwrap();
        drop(writer);
        let mut text = String::from("Confirmed");
        for _ in 0..100 {
            text.push_str(" words");
            checkpoint(&store, &text);
        }
        drop(store);
        assert!(fs::read_to_string(folder.join("transcript.txt"))
            .unwrap()
            .contains(&text));
        let reopened = SessionStore::open(&folder).unwrap();
        assert_eq!(
            reopened.snapshot().unwrap().parts[0].text.as_deref(),
            Some(text.as_str())
        );
    }
}
