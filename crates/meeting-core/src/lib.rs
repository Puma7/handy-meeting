//! Local durable recording is independent of speech recognition.
#![deny(unsafe_code)]

pub mod archive;
pub mod audio_codec;
mod capture;
pub mod library;
mod live_text;
pub mod metadata;
mod model;
mod pcm;
pub mod retention;
pub mod speakers;
mod storage;

pub use capture::{list_devices, CaptureController, CaptureOptions};
pub use model::{
    AudioDevice, AudioPart, CaptureSnapshot, SessionData, SessionProgress, TranscriptCheckpoint,
    TranscriptPage, TranscriptSlice,
};
pub use pcm::{read_pcm_samples, PcmWriter, SAMPLE_RATE};
pub use storage::{
    continues_confirmed_text, inspect_journals, inspect_manifest, list_sessions, read_closed_audio,
    read_legacy, SessionStore,
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
