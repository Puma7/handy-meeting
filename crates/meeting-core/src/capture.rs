use crate::{AudioDevice, CaptureSnapshot, Result, SessionStore};
use serde::{Deserialize, Serialize};
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
    tracks: Vec<JoinHandle<()>>,
    store: SessionStore,
}

impl CaptureController {
    pub fn start(store: SessionStore, options: CaptureOptions) -> Result<Self> {
        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        {
            let _ = (store, options);
            Err("This operating system has no supported audio capture adapter.".into())
        }
        #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
        {
            let stop = Arc::new(AtomicBool::new(false));
            let mut controller = Self {
                stop,
                tracks: vec![],
                store,
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
                }
            }
            Ok(controller)
        }
    }

    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    fn start_track(&mut self, source: &'static str, device_id: Option<String>) -> Result<()> {
        let stop = Arc::clone(&self.stop);
        let store = self.store.clone();
        let (ready, result) = mpsc::sync_channel(1);
        let track = thread::Builder::new().name(format!("meeting-{source}-capture")).spawn(move || {
            if let Err(error) = platform::record(store.clone(), source, device_id, stop, ready) {
                let _ = store.warn(&format!("{source}: capture stopped: {error}. Saved audio is retained; check the device before recording again."));
            }
        })?;
        self.tracks.push(track);
        let timeout = if cfg!(target_os = "macos") { 60 } else { 10 };
        match result.recv_timeout(Duration::from_secs(timeout)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error.into()),
            Err(_) => Err("Audio device did not become ready in time".into()),
        }
    }

    pub fn snapshot(&self) -> Result<CaptureSnapshot> {
        Ok(CaptureSnapshot {
            running: self.is_running(),
            session: self.store.preview_snapshot(16)?,
        })
    }

    pub fn is_running(&self) -> bool {
        !self.stop.load(Ordering::Acquire) && self.tracks.iter().any(|track| !track.is_finished())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Release);
        for track in self.tracks.drain(..) {
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
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

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
