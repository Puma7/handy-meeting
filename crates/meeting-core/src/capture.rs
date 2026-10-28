use crate::{AudioDevice, CaptureSnapshot, Result, SessionStore};
use serde::{Deserialize, Serialize};
#[path = "capture_controls.rs"]
mod controls;
use controls::CaptureControls;
pub use controls::{
    read_capture_control_events, CaptureControlEvent, CaptureControlLog, CaptureControlSnapshot,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use std::{sync::mpsc, thread, time::Duration};

#[cfg(windows)]
#[path = "windows_capture.rs"]
mod platform;

#[cfg(target_os = "linux")]
#[path = "linux_capture.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "macos_capture.rs"]
mod platform;
#[cfg(any(target_os = "linux", target_os = "macos", test))]
#[path = "portable_capture.rs"]
mod portable;
// Type-checks and unit-tests the macOS adapter's pure parsing/argument code on
// other build hosts; it never starts the Swift helper there.
#[cfg(all(test, not(target_os = "macos")))]
#[path = "macos_capture.rs"]
#[allow(dead_code)]
mod macos_adapter_checks;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureOptions {
    pub output_id: Option<String>,
    pub microphone_id: Option<String>,
    pub include_microphone: bool,
}

/// A track owns its capture client and disk writer on one dedicated thread.
/// There is no unbounded queue between the device and durable storage.
pub struct CaptureController {
    stop: Arc<AtomicBool>,
    tracks: Vec<(&'static str, JoinHandle<()>)>,
    store: SessionStore,
    controls: Arc<CaptureControls>,
}

impl CaptureController {
    /// Starts independent system/microphone writers; a microphone startup failure leaves system capture running.
    pub fn start(store: SessionStore, options: CaptureOptions) -> Result<Self> {
        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        {
            let _ = (store, options);
            Err("This operating system has no supported audio capture adapter.".into())
        }
        #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
        {
            let stop = Arc::new(AtomicBool::new(false));
            let controls = Arc::new(CaptureControls::new(&store)?);
            let mut controller = Self {
                stop,
                tracks: vec![],
                store,
                controls,
            };
            if let Err(error) = controller.start_track("system", options.output_id) {
                controller.stop()?;
                return Err(error);
            }
            if options.include_microphone {
                // A microphone startup failure must not terminate a working system track.
                if let Err(error) = controller.start_track("microphone", options.microphone_id) {
                    controller.store.warn(&format!(
                        "Microphone could not start: {error}. System audio continues."
                    ))?;
                } else {
                    controller.controls.set_has_microphone(true);
                }
            }
            Ok(controller)
        }
    }

    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    /// Starts one owned capture thread and waits for its bounded readiness acknowledgment.
    fn start_track(&mut self, source: &'static str, device_id: Option<String>) -> Result<()> {
        let stop = Arc::clone(&self.stop);
        let store = self.store.clone();
        let controls = Arc::clone(&self.controls);
        let (ready, result) = mpsc::sync_channel(1);
        let track = thread::Builder::new().name(format!("meeting-{source}-capture")).spawn(move || {
            if let Err(error) = platform::record(store.clone(), source, device_id, stop, ready, controls) {
                let _ = store.warn(&format!("{source}: capture stopped: {error}. Saved audio is retained; check the device before recording again."));
            }
        })?;
        self.tracks.push((source, track));
        let timeout = if cfg!(target_os = "macos") { 60 } else { 10 };
        match result.recv_timeout(Duration::from_secs(timeout)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error.into()),
            Err(_) => Err("Audio device did not become ready in time".into()),
        }
    }

    /// Returns capture liveness and a bounded session preview without cloning the full transcript.
    pub fn snapshot(&self) -> Result<CaptureSnapshot> {
        Ok(CaptureSnapshot {
            running: self.is_running(),
            session: self.store.preview_snapshot(16)?,
        })
    }

    /// Reports whether an owned track is still alive before an explicit stop request.
    pub fn is_running(&self) -> bool {
        !self.stop.load(Ordering::Acquire)
            && self.tracks.iter().any(|(_, track)| !track.is_finished())
    }

    /// Returns pause/mute state and derives microphone availability from its live capture thread.
    pub fn controls(&self) -> CaptureControlSnapshot {
        let mut snapshot = self.controls.snapshot();
        snapshot.has_microphone = !self.stop.load(Ordering::Acquire)
            && self
                .tracks
                .iter()
                .any(|(source, track)| *source == "microphone" && !track.is_finished());
        snapshot
    }

    /// Changes both track gates only while capture runs; successful return excludes old-epoch speech writes.
    pub fn set_paused(&self, paused: bool) -> Result<()> {
        if !self.is_running() {
            return Err("No capture is running".into());
        }
        self.controls.set_paused(paused)
    }

    /// Changes only the live microphone gate; system capture continues independently.
    pub fn set_microphone_muted(&self, muted: bool) -> Result<()> {
        if !self.is_running() || self.stopped_tracks().contains(&"microphone") {
            return Err("No microphone capture is running".into());
        }
        self.controls.set_microphone_muted(muted)
    }

    /// Tracks ("system", "microphone") whose capture ended before an explicit stop, while
    /// another track may still record. A partly failed capture must stay visible.
    pub fn stopped_tracks(&self) -> Vec<&'static str> {
        if self.stop.load(Ordering::Acquire) {
            return Vec::new();
        }
        self.tracks
            .iter()
            .filter(|(_, track)| track.is_finished())
            .map(|(source, _)| *source)
            .collect()
    }

    /// Requests owned tracks to stop, joins their writers and marks the retained session recorded.
    pub fn stop(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Release);
        for (_, track) in self.tracks.drain(..) {
            if track.join().is_err() {
                self.store.warn(
                    "A recording thread stopped unexpectedly. Confirmed audio is retained.",
                )?;
            }
        }
        self.store.set_status("recorded")
    }
}

impl Drop for CaptureController {
    /// Stops only this controller's owned capture threads when the controller is released.
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Enumerates supported platform devices on a separate thread without starting a recording.
pub fn list_devices() -> Result<Vec<AudioDevice>> {
    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    {
        thread::spawn(platform::list_devices)
            .join()
            .map_err(|_| "Device enumeration stopped unexpectedly")?
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Err("This operating system has no supported audio device adapter.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Checks synthetic thread liveness reports one failed track while the other remains active.
    fn a_track_that_ends_early_is_reported_while_the_other_keeps_recording() {
        let root = tempfile::tempdir().unwrap();
        let store = SessionStore::create(root.path(), "Synthetic", "m", "s", None).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let keep = Arc::clone(&stop);
        let system = std::thread::spawn(move || {
            while !keep.load(Ordering::Acquire) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        });
        let microphone = std::thread::spawn(|| {});
        let mut controller = CaptureController {
            stop,
            tracks: vec![("system", system), ("microphone", microphone)],
            controls: Arc::new(CaptureControls::new(&store).unwrap()),
            store,
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while controller.stopped_tracks().is_empty() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(controller.is_running(), "system audio still records");
        assert_eq!(controller.stopped_tracks(), vec!["microphone"]);
        controller.stop().unwrap();
        assert!(controller.stopped_tracks().is_empty());
    }
}
