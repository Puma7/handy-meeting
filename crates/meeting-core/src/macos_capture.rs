//! Reuses the existing audio-only ScreenCaptureKit/AVAudioEngine Swift helper.
use super::portable::{self, HelperSpec};
use crate::{AudioDevice, Result, SessionStore};
use std::{
    process::Command,
    sync::{atomic::AtomicBool, mpsc::SyncSender, Arc},
};

pub(super) fn list_devices() -> Result<Vec<AudioDevice>> {
    Ok(vec![
        AudioDevice {
            id: "default".into(),
            name: "System audio · ScreenCaptureKit".into(),
            source: "system".into(),
            is_default: true,
        },
        AudioDevice {
            id: "default".into(),
            name: "Default microphone · AVAudioEngine".into(),
            source: "microphone".into(),
            is_default: true,
        },
    ])
}

pub(super) fn record(
    store: SessionStore,
    source: &str,
    device_id: Option<String>,
    stop: Arc<AtomicBool>,
    ready: SyncSender<std::result::Result<(), String>>,
) -> Result<()> {
    if device_id
        .as_deref()
        .is_some_and(|id| !id.is_empty() && id != "default")
    {
        return Err(
            "The macOS preview currently records the default microphone and system audio only"
                .into(),
        );
    }
    let executable = std::env::current_exe()?;
    let helper = executable
        .parent()
        .ok_or("Application directory is unavailable")?
        .join("HandyMeetingCapture");
    if !helper.is_file() {
        return Err(
            "The macOS audio helper is missing. Install the complete matching application bundle."
                .into(),
        );
    }
    let mut command = Command::new(helper);
    command.args(["--track", source]);
    portable::record(
        store,
        source,
        stop,
        ready,
        HelperSpec {
            command,
            readiness_marker: true,
            graceful_stdin: true,
        },
    )
}
