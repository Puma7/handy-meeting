//! Explicit opt-in hardware diagnostic. Never called by ordinary verification.
//! Audio stays in the supplied new local session; no model or network is used.
use meeting_core::{CaptureController, CaptureOptions, SessionStore};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() -> meeting_core::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let root = args
        .next()
        .map(PathBuf::from)
        .ok_or("Supply a new local diagnostic directory")?;
    let seconds: u64 = args
        .next()
        .ok_or("Supply duration in seconds (5..60)")?
        .to_str()
        .ok_or("Invalid duration")?
        .parse()?;
    if !(5..=60).contains(&seconds) || args.next().is_some() || root.exists() {
        return Err("Use a new directory and an explicit 5..60 second duration".into());
    }
    // Fixed before capture; not adjusted to the observed measurements.
    let allowed_track_difference_ms = 500_u64;
    std::fs::create_dir_all(&root)?;
    std::fs::write(
        root.join("plan.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "version":env!("CARGO_PKG_VERSION"),"seconds":seconds,"allowedTrackDifferenceMs":allowed_track_difference_ms,
            "realHardware":true,"microphone":true,"speechRecognition":false,"cloud":false
        }))?,
    )?;
    let store = SessionStore::create(
        &root.join("sessions"),
        "Explicit local hardware diagnostic",
        "audio-only:no-asr",
        "default system output",
        Some("default microphone"),
    )?;
    let started = Instant::now();
    let mut capture = CaptureController::start(
        store.clone(),
        CaptureOptions {
            output_id: None,
            microphone_id: None,
            include_microphone: true,
        },
    )?;
    std::thread::sleep(Duration::from_secs(seconds));
    capture.stop()?;
    let elapsed_ms = started.elapsed().as_millis();
    let data = store.snapshot()?;
    let samples = |source: &str| {
        data.parts
            .iter()
            .filter(|p| p.source == source)
            .map(|p| p.confirmed_sample_count)
            .sum::<u64>()
    };
    let system_ms = samples("system") * 1000 / 16000;
    let microphone_ms = samples("microphone") * 1000 / 16000;
    let result = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"realHardware":true,"speechRecognition":false,"sessionId":data.id,"elapsedMs":elapsed_ms,"systemMs":system_ms,"microphoneMs":microphone_ms,"differenceMs":system_ms.abs_diff(microphone_ms),"trackDifferenceWithinLimit":system_ms.abs_diff(microphone_ms)<=allowed_track_difference_ms,"captureIncomplete":data.capture_incomplete,"warnings":data.warnings,"limits":"Short live-device recording only; no ASR, crash, full-disk or long-session validation."});
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
