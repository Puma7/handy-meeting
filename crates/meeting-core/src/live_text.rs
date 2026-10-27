//! Readable derived TXT follows confirmed journals; failures never control capture.
use crate::{
    storage::{atomic_json, exists_no_link, reject_link, render_transcript},
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
/// Ownership record for the derived TXT. `sha256`/`bytes` name the content this writer
/// last completed. While a replacement is in flight, `pending` names it as well, so a
/// crash between replacing the TXT and finalizing the receipt keeps the file owned.
#[derive(Serialize, Deserialize)]
struct Receipt {
    sha256: String,
    bytes: usize,
    sequence: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pending: Option<Content>,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct Content {
    sha256: String,
    bytes: usize,
}
impl Receipt {
    fn owns(&self, content: &Content) -> bool {
        (self.sha256 == content.sha256 && self.bytes == content.bytes)
            || self.pending.as_ref() == Some(content)
    }
}
/// Durable steps of one refresh; tests stop after a step to simulate a crash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    ReceiptPrepared,
    TextReplaced,
}
fn digest(bytes: &[u8]) -> Content {
    Content {
        sha256: format!("{:x}", Sha256::digest(bytes)),
        bytes: bytes.len(),
    }
}
fn read_receipt(path: &Path) -> Result<Receipt> {
    reject_link(path)?;
    let mut content = Vec::new();
    fs::File::open(path)?.take(4097).read_to_end(&mut content)?;
    if content.len() > 4096 {
        return Err("Invalid live-text ownership receipt".into());
    }
    Ok(serde_json::from_slice(&content)?)
}
pub(crate) fn refresh(folder: &Path, data: &SessionData) -> Result<()> {
    write(folder, data, &mut |_| Ok(()))
}
fn write(
    folder: &Path,
    data: &SessionData,
    after: &mut dyn FnMut(Step) -> Result<()>,
) -> Result<()> {
    if data.read_only {
        return Ok(());
    }
    reject_link(folder)?;
    let file = folder.join("transcript.txt");
    let receipt = folder.join("transcript.txt.receipt.json");
    let text = format!("{HEADER}{}", render_transcript(data));
    let bytes = text.as_bytes();
    let next = digest(bytes);
    let mut current = next.clone();
    if exists_no_link(&file)? {
        let mut previous = Vec::new();
        fs::File::open(&file)?
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut previous)?;
        if previous.len() > 32 * 1024 * 1024 {
            return Err("Existing readable text is oversized; preserved".into());
        }
        current = digest(&previous);
        if previous != bytes
            && (!previous.starts_with(HEADER.as_bytes()) || !read_receipt(&receipt)?.owns(&current))
        {
            return Err("Readable text was changed outside this writer; it was preserved".into());
        }
    }
    // Record both the owned current content and the pending one before replacing it.
    atomic_json(
        &receipt,
        &Receipt {
            sha256: current.sha256,
            bytes: current.bytes,
            sequence: data.snapshot_sequence,
            pending: Some(next.clone()),
        },
    )?;
    after(Step::ReceiptPrepared)?;
    let mut pending = tempfile::NamedTempFile::new_in(folder)?;
    pending.write_all(bytes)?;
    pending.as_file().sync_all()?;
    pending.persist(&file).map_err(|e| e.error)?;
    after(Step::TextReplaced)?;
    atomic_json(
        &receipt,
        &Receipt {
            sha256: next.sha256,
            bytes: next.bytes,
            sequence: data.snapshot_sequence,
            pending: None,
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
    /// The derived writer finished a refresh: final receipt naming the current TXT.
    fn settled(folder: &Path) -> bool {
        let Ok(receipt) = read_receipt(&folder.join("transcript.txt.receipt.json")) else {
            return false;
        };
        fs::read(folder.join("transcript.txt"))
            .is_ok_and(|text| receipt.pending.is_none() && receipt.owns(&digest(&text)))
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
                revision_after_restart: false,
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
    fn index_inspection_reads_confirmed_text_that_only_the_journal_holds() {
        let root = tempfile::tempdir().unwrap();
        let store =
            SessionStore::create(root.path(), "Meeting", "fixture-model", "output", None).unwrap();
        let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
        writer.append(&[1; 160]).unwrap();
        writer.finish().unwrap();
        checkpoint(&store, "Journal-only confirmed sentence.");
        let folder = store.folder().to_owned();
        let manifest = crate::inspect_manifest(&folder).unwrap();
        assert!(manifest.parts.iter().all(|p| p.text.is_none()));
        let replayed = crate::inspect_journals(&folder).unwrap();
        assert_eq!(
            replayed.parts[0].text.as_deref(),
            Some("Journal-only confirmed sentence.")
        );
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
        eventually(|| settled(&folder));
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
        eventually(|| settled(store.folder()));
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
    fn crash_between_text_replacement_and_receipt_keeps_the_text_owned() {
        // Synthetic crash points (#21): the refresh stops after each durable step.
        for crash in [Step::ReceiptPrepared, Step::TextReplaced] {
            let root = tempfile::tempdir().unwrap();
            let store =
                SessionStore::create(root.path(), "Meeting", "fixture-model", "output", None)
                    .unwrap();
            let folder = store.folder().to_owned();
            let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
            writer.append(&[1; 160]).unwrap();
            writer.finish().unwrap();
            drop(writer);
            checkpoint(&store, "First sentence.");
            let mut data = store.snapshot().unwrap();
            // Dropping the last owner flushes the derived writer.
            drop(store);
            let file = folder.join("transcript.txt");
            assert!(fs::read_to_string(&file)
                .unwrap()
                .contains("First sentence."));

            data.parts[0].text = Some("First sentence. Second.".into());
            let stopped = write(&folder, &data, &mut |step| {
                if step == crash {
                    Err("Synthetic crash".into())
                } else {
                    Ok(())
                }
            });
            assert!(stopped.is_err());
            assert_eq!(
                fs::read_to_string(&file).unwrap().contains("Second."),
                crash == Step::TextReplaced
            );

            // The next refresh after a restart still recognizes its own file.
            data.parts[0].text = Some("First sentence. Second. Third.".into());
            refresh(&folder, &data).unwrap();
            assert!(fs::read_to_string(&file).unwrap().contains("Third."));
            let receipt = read_receipt(&folder.join("transcript.txt.receipt.json")).unwrap();
            assert!(receipt.pending.is_none());

            // A genuine outside edit is still preserved.
            fs::write(&file, b"User-owned edited text").unwrap();
            data.parts[0].text = Some("First sentence. Second. Third. Fourth.".into());
            assert!(refresh(&folder, &data).is_err());
            assert_eq!(fs::read(&file).unwrap(), b"User-owned edited text");
        }
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
