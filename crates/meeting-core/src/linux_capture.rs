//! Reuses the installed PulseAudio-compatible utilities, including PipeWire-Pulse.
use super::portable::{self, HelperSpec};
use crate::{AudioDevice, Result, SessionStore};
use serde_json::Value;
use std::{
    collections::HashSet,
    process::Command,
    sync::{atomic::AtomicBool, mpsc::SyncSender, Arc},
};

fn query(arguments: &[&str]) -> Result<String> {
    portable::text_command(Command::new("pactl").args(arguments))
}

pub(super) fn list_devices() -> Result<Vec<AudioDevice>> {
    let sinks: Value = serde_json::from_str(&query(&["--format=json", "list", "sinks"])?)?;
    let sources: Value = serde_json::from_str(&query(&["--format=json", "list", "sources"])?)?;
    let default_sink = query(&["get-default-sink"])?;
    let default_source = query(&["get-default-source"])?;
    parse_devices(&sinks, &sources, default_sink.trim(), default_source.trim())
}

fn identifier(value: &Value, field: &str) -> Result<String> {
    let text = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or("PulseAudio returned an unsupported device description")?;
    if text.is_empty() || text.len() > 4096 || text.chars().any(char::is_control) {
        return Err("Invalid PulseAudio device identifier".into());
    }
    Ok(text.into())
}

fn parse_devices(
    sinks: &Value,
    sources: &Value,
    default_sink: &str,
    default_source: &str,
) -> Result<Vec<AudioDevice>> {
    let sinks = sinks
        .as_array()
        .ok_or("PulseAudio sink list is not an array")?;
    let sources = sources
        .as_array()
        .ok_or("PulseAudio source list is not an array")?;
    if sinks.len() + sources.len() > 2048 {
        return Err("PulseAudio device list exceeds its safety limit".into());
    }
    let mut result = vec![];
    let mut monitors = HashSet::new();
    for sink in sinks {
        let name = identifier(sink, "name")?;
        // The server supplies the actual monitor name. Do not guess a suffix.
        let monitor = identifier(sink, "monitor_source_name")?;
        monitors.insert(monitor.clone());
        result.push(AudioDevice {
            id: monitor,
            name: sink
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or(&name)
                .into(),
            source: "system".into(),
            is_default: name == default_sink,
        });
    }
    for source in sources {
        let name = identifier(source, "name")?;
        if monitors.contains(&name) || name.ends_with(".monitor") {
            continue;
        }
        result.push(AudioDevice {
            id: name.clone(),
            name: source
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or(&name)
                .into(),
            source: "microphone".into(),
            is_default: name == default_source,
        });
    }
    Ok(result)
}

pub(super) fn record(
    store: SessionStore,
    source: &str,
    device_id: Option<String>,
    stop: Arc<AtomicBool>,
    ready: SyncSender<std::result::Result<(), String>>,
) -> Result<()> {
    let device = device_id.filter(|id| !id.is_empty()).unwrap_or_else(|| {
        if source == "system" {
            "@DEFAULT_MONITOR@"
        } else {
            "@DEFAULT_SOURCE@"
        }
        .into()
    });
    if device.len() > 4096 || device.chars().any(char::is_control) {
        return Err("Invalid PulseAudio recording source".into());
    }
    let mut command = Command::new("parec");
    command
        .args([
            "--raw",
            "--format=s16le",
            "--rate=16000",
            "--channels=1",
            "--latency-msec=100",
        ])
        .arg(format!("--device={device}"));
    portable::record(
        store,
        source,
        stop,
        ready,
        HelperSpec {
            command,
            readiness_marker: false,
            graceful_stdin: false,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_monitor_names_and_defaults_are_retained_without_reclassifying_loopback_as_mic() {
        let sinks = serde_json::json!([{"name":"headphones","description":"Headphones","monitor_source_name":"custom.remote.capture"}]);
        let sources = serde_json::json!([{"name":"custom.remote.capture"},{"name":"desk-microphone","description":"Desk microphone"}]);
        let devices = parse_devices(&sinks, &sources, "headphones", "desk-microphone").unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "custom.remote.capture");
        assert!(devices.iter().all(|d| d.is_default));
        assert_eq!(devices[1].source, "microphone");
        assert!(parse_devices(
            &serde_json::json!([{"name":"missing-monitor"}]),
            &sources,
            "",
            ""
        )
        .is_err());
    }
}
