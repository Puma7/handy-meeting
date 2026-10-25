//! Local durable recording is independent of speech recognition.
#![deny(unsafe_code)]

mod capture;
mod model;
mod pcm;
mod storage;

pub use capture::{list_devices, CaptureController, CaptureOptions};
pub use model::{
    AudioDevice, AudioPart, CaptureSnapshot, SessionData, SessionProgress, TranscriptCheckpoint,
};
pub use pcm::{read_pcm_samples, PcmWriter, SAMPLE_RATE};
pub use storage::{list_sessions, read_legacy, SessionStore};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
