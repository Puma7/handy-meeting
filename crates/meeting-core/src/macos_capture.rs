//! Reuses the existing audio-only ScreenCaptureKit/AVAudioEngine Swift helper.
//!
//! Microphones are listed by the helper's `--list-microphones` mode (Core Audio
//! input devices, stable device UIDs) and a chosen one is passed back as
//! `--track microphone --device UID`. The `default` entries keep following the
//! system's current devices. Device selection was not tested on Mac hardware
//! when it was added; see docs/RUST_PORTABLE.md.
use super::controls::CaptureControls;
use super::portable::{self, HelperSpec};
use crate::{AudioDevice, Result, SessionStore};
use serde::Deserialize;
use std::{
    collections::HashSet,
    path::PathBuf,
    process::Command,
    sync::{atomic::AtomicBool, mpsc::SyncSender, Arc},
};

const DEFAULT_ID: &str = "default";
const MISSING_HELPER: &str =
    "The macOS audio helper is missing. Install the complete matching application bundle.";

/// Provides default system and microphone choices independently of optional device enumeration.
fn default_devices() -> Vec<AudioDevice> {
    vec![
        AudioDevice {
            id: DEFAULT_ID.into(),
            name: "System audio · ScreenCaptureKit".into(),
            source: "system".into(),
            is_default: true,
        },
        AudioDevice {
            id: DEFAULT_ID.into(),
            name: "Default microphone · AVAudioEngine".into(),
            source: "microphone".into(),
            is_default: true,
        },
    ]
}

/// Locates the matching bundled capture helper beside the running executable.
fn helper() -> Result<PathBuf> {
    let helper = std::env::current_exe()?
        .parent()
        .ok_or("Application directory is unavailable")?
        .join("HandyMeetingCapture");
    if !helper.is_file() {
        return Err(MISSING_HELPER.into());
    }
    Ok(helper)
}

/// Adds validated listed microphones when available while retaining the default choices on query failure.
pub(super) fn list_devices() -> Result<Vec<AudioDevice>> {
    let mut devices = default_devices();
    // Listing is optional: when the helper cannot list devices, the default
    // entries still record exactly as before.
    if let Ok(listed) = helper()
        .and_then(|helper| {
            portable::query_command(
                Command::new(helper).arg("--list-microphones"),
                MISSING_HELPER,
            )
        })
        .and_then(|output| parse_microphones(&output))
    {
        devices.extend(listed);
    }
    Ok(devices)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedMicrophone {
    id: String,
    name: String,
}

/// Accepts a bounded explicit microphone identifier distinct from the default-device sentinel.
fn valid_device_id(id: &str) -> bool {
    !id.is_empty() && id != DEFAULT_ID && id.len() <= 4096 && !id.chars().any(char::is_control)
}

/// Parses the helper's JSON array of Core Audio input devices.
fn parse_microphones(output: &str) -> Result<Vec<AudioDevice>> {
    let listed: Vec<ListedMicrophone> = serde_json::from_str(output.trim())?;
    if listed.len() > 256 {
        return Err("The macOS microphone list exceeds its safety limit".into());
    }
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for microphone in listed {
        if !valid_device_id(&microphone.id) {
            return Err("The macOS audio helper returned an invalid microphone identifier".into());
        }
        if !seen.insert(microphone.id.clone()) {
            continue;
        }
        let name: String = microphone
            .name
            .chars()
            .filter(|c| !c.is_control())
            .take(200)
            .collect();
        result.push(AudioDevice {
            name: if name.trim().is_empty() {
                "Microphone".into()
            } else {
                name
            },
            id: microphone.id,
            source: "microphone".into(),
            // The `default` entry represents the current system default.
            is_default: false,
        });
    }
    Ok(result)
}

/// Helper arguments for one track. Only the microphone accepts a chosen device.
fn track_arguments(source: &str, device_id: Option<&str>) -> Result<Vec<String>> {
    let mut arguments = vec!["--track".to_string(), source.to_string()];
    match device_id.filter(|id| !id.is_empty() && *id != DEFAULT_ID) {
        None => {}
        Some(_) if source != "microphone" => {
            return Err("The macOS preview records the default system output only".into())
        }
        Some(id) if valid_device_id(id) => {
            arguments.extend(["--device".to_string(), id.to_string()]);
        }
        Some(_) => return Err("Invalid macOS microphone identifier".into()),
    }
    Ok(arguments)
}

/// Starts the bundled track helper with readiness signaling and portable privacy-aware PCM handling.
pub(super) fn record(
    store: SessionStore,
    source: &str,
    device_id: Option<String>,
    stop: Arc<AtomicBool>,
    ready: SyncSender<std::result::Result<(), String>>,
    controls: Arc<CaptureControls>,
) -> Result<()> {
    let arguments = track_arguments(source, device_id.as_deref())?;
    let mut command = Command::new(helper()?);
    command.args(arguments);
    portable::record_controlled(
        store,
        source,
        stop,
        ready,
        HelperSpec {
            command,
            readiness_marker: true,
            graceful_stdin: true,
        },
        controls,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Checks synthetic microphone listings reject unsafe IDs and retain separate default choices.
    fn listed_microphones_are_validated_deduplicated_and_never_replace_defaults() {
        let output = r#"[{"id":"BuiltInMicrophoneDevice","name":"MacBook Pro Microphone","isDefault":true},
            {"id":"AppleUSBAudioEngine:Vendor:Mic:1","name":"USB\u0007 Mic","isDefault":false},
            {"id":"BuiltInMicrophoneDevice","name":"Duplicate","isDefault":false}]"#;
        let devices = parse_microphones(output).unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "BuiltInMicrophoneDevice");
        assert_eq!(devices[1].name, "USB Mic");
        assert!(devices
            .iter()
            .all(|d| d.source == "microphone" && !d.is_default));
        for invalid in [
            r#"[{"id":"default","name":"x"}]"#,
            r#"[{"id":"","name":"x"}]"#,
            r#"[{"id":"a\nb","name":"x"}]"#,
            r#"{"id":"a","name":"x"}"#,
            "not json",
        ] {
            assert!(parse_microphones(invalid).is_err(), "{invalid}");
        }
        assert_eq!(default_devices().len(), 2);
        assert!(default_devices().iter().all(|d| d.id == DEFAULT_ID));
    }

    #[test]
    /// Checks synthetic arguments forward explicit microphone IDs and reject nondefault system outputs.
    fn only_a_chosen_microphone_is_passed_to_the_helper() {
        assert_eq!(
            track_arguments("microphone", None).unwrap(),
            ["--track", "microphone"]
        );
        assert_eq!(
            track_arguments("microphone", Some("default")).unwrap(),
            ["--track", "microphone"]
        );
        assert_eq!(
            track_arguments("system", Some("")).unwrap(),
            ["--track", "system"]
        );
        assert_eq!(
            track_arguments("microphone", Some("AppleUSBAudioEngine:1")).unwrap(),
            ["--track", "microphone", "--device", "AppleUSBAudioEngine:1"]
        );
        assert!(track_arguments("system", Some("AppleUSBAudioEngine:1")).is_err());
        assert!(track_arguments("microphone", Some("bad\u{0}id")).is_err());
    }
}
