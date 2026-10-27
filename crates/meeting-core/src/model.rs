use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    /// `system` means a render endpoint captured in loopback mode.
    pub source: String,
    pub is_default: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioPart {
    #[serde(alias = "File")]
    pub file: String,
    #[serde(alias = "Source")]
    pub source: String,
    #[serde(alias = "StartSample")]
    pub start_sample: u64,
    #[serde(alias = "SampleCount")]
    pub sample_count: u64,
    #[serde(default)]
    pub confirmed_sample_count: u64,
    #[serde(default, alias = "HasSignal")]
    pub has_signal: bool,
    #[serde(default, alias = "AudioClosed")]
    pub audio_closed: bool,
    #[serde(default = "pending", alias = "Status")]
    pub status: String,
    #[serde(default, alias = "Text")]
    pub text: Option<String>,
    #[serde(default, alias = "CommittedText")]
    pub committed_text: String,
    #[serde(default, alias = "TentativeText")]
    pub tentative_text: String,
    #[serde(default, alias = "ProcessedSamples")]
    pub processed_samples: u64,
    #[serde(default, alias = "ModelId")]
    pub model_id: Option<String>,
    #[serde(default, alias = "Error")]
    pub error: Option<String>,
    #[serde(default)]
    pub confirmed_sha256: String,
    #[serde(default)]
    pub transcript_sequence: u64,
}

fn pending() -> String {
    "pending".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionData {
    #[serde(alias = "SchemaVersion")]
    pub schema_version: u32,
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Title")]
    pub title: String,
    #[serde(alias = "StartedAt")]
    pub started_at: String,
    #[serde(default, alias = "EndedAt")]
    pub ended_at: Option<String>,
    #[serde(alias = "Status")]
    pub status: String,
    #[serde(default, alias = "ModelId")]
    pub model_id: String,
    #[serde(default, alias = "OutputName")]
    pub output_name: String,
    #[serde(default, alias = "MicrophoneName")]
    pub microphone_name: Option<String>,
    #[serde(default = "chunk_seconds", alias = "ChunkSeconds")]
    pub chunk_seconds: u32,
    #[serde(default, alias = "DurationMs")]
    pub duration_ms: u64,
    #[serde(default, alias = "CaptureIncomplete")]
    pub capture_incomplete: bool,
    #[serde(default, alias = "Warnings")]
    pub warnings: Vec<String>,
    #[serde(default, alias = "Parts")]
    pub parts: Vec<AudioPart>,
    #[serde(default, alias = "SnapshotSequence")]
    pub snapshot_sequence: u64,
    #[serde(default)]
    pub read_only: bool,
}

fn chunk_seconds() -> u32 {
    30
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSlice {
    pub source: String,
    pub start_sample: u64,
    pub end_sample: u64,
    pub sequence: u64,
    pub status: String,
    pub text: String,
    pub offset: usize,
    pub next_offset: usize,
    pub total_bytes: usize,
    pub committed_bytes: usize,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptPage {
    pub next_sequence: u64,
    pub more: bool,
    pub updates: Vec<TranscriptSlice>,
    pub untrusted_data: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptCheckpoint {
    #[serde(default, alias = "Sequence")]
    pub sequence: u64,
    #[serde(default, alias = "At")]
    pub at: String,
    #[serde(alias = "File")]
    pub file: String,
    #[serde(alias = "ModelId")]
    pub model_id: String,
    #[serde(alias = "Full")]
    pub full: String,
    #[serde(default, alias = "Committed")]
    pub committed: String,
    #[serde(default, alias = "Tentative")]
    pub tentative: String,
    #[serde(alias = "ProcessedSamples")]
    pub processed_samples: u64,
    #[serde(rename = "final", alias = "Final")]
    pub final_text: bool,
    /// Explicit new text version (#20): a native stream reconstructed after a
    /// recognition restart caught up with the durable progress of its part, but its
    /// confirmed text does not continue the stored confirmed prefix (e.g. GPU to CPU
    /// replacement). Only such a marked checkpoint may replace a confirmed prefix; the
    /// earlier checkpoints stay unchanged in the append-only journal as the previous
    /// version. Omitted when false, so older records keep their exact bytes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub revision_after_restart: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSnapshot {
    pub running: bool,
    pub session: SessionData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionProgress {
    pub total_parts: usize,
    pub complete_parts: usize,
    pub system_confirmed_ms: u64,
    pub microphone_confirmed_ms: u64,
    pub pending_ms: u64,
}
