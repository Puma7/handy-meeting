//! WASAPI COM lifetime is confined to dedicated threads; no raw COM calls here.
use crate::{AudioDevice, PcmWriter, Result, SessionStore, SAMPLE_RATE};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
        Arc,
    },
    time::Instant,
};
use wasapi::{
    deinitialize, initialize_mta, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat,
};

struct Com;
impl Com {
    fn initialize() -> Result<Self> {
        initialize_mta().ok()?;
        Ok(Self)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        deinitialize();
    }
}

pub(super) fn list_devices() -> Result<Vec<AudioDevice>> {
    let _com = Com::initialize()?;
    let enumerator = DeviceEnumerator::new()?;
    let mut devices = vec![];
    for (direction, source) in [
        (Direction::Render, "system"),
        (Direction::Capture, "microphone"),
    ] {
        let default = enumerator
            .get_default_device(&direction)
            .and_then(|device| device.get_id())
            .ok();
        let collection = enumerator.get_device_collection(&direction)?;
        for index in 0..collection.get_nbr_devices()? {
            let device = collection.get_device_at_index(index)?;
            let id = device.get_id()?;
            devices.push(AudioDevice {
                is_default: default.as_ref() == Some(&id),
                id,
                name: device.get_friendlyname()?,
                source: source.into(),
            });
        }
    }
    Ok(devices)
}

pub(super) fn record(
    store: SessionStore,
    source: &str,
    device_id: Option<String>,
    stop: Arc<AtomicBool>,
    ready: SyncSender<std::result::Result<(), String>>,
) -> Result<()> {
    let mut started = false;
    let result = (|| -> Result<()> {
        let _com = Com::initialize()?;
        let enumerator = DeviceEnumerator::new()?;
        let direction = if source == "system" {
            Direction::Render
        } else {
            Direction::Capture
        };
        let device = match device_id {
            Some(id) if !id.is_empty() => enumerator.get_device(&id)?,
            _ => enumerator.get_default_device(&direction)?,
        };
        let mut client = device.get_iaudioclient()?;
        let desired = WaveFormat::new(16, 16, &SampleType::Int, SAMPLE_RATE as usize, 1, None);
        let (period, _) = client.get_device_period()?;
        let mode = StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: period.max(200_000),
        };
        client.initialize_client(&desired, &Direction::Capture, &mode)?;
        let handle = client.set_get_eventhandle()?;
        let capture = client.get_audiocaptureclient()?;
        let mut writer = PcmWriter::new(store.clone(), source)?;
        let mut queue = VecDeque::with_capacity(SAMPLE_RATE as usize * 2);
        let mut pcm = Vec::with_capacity(SAMPLE_RATE as usize / 10);
        let start = Instant::now();
        client.start_stream()?;
        ready
            .send(Ok(()))
            .map_err(|_| "Capture controller was closed")?;
        started = true;
        let loop_result = (|| -> Result<()> {
            while !stop.load(Ordering::Acquire) {
                // A short timeout is normal during digital silence. Pad the system track
                // to wall time so remote speech remains aligned with microphone audio.
                if handle.wait_for_event(100).is_ok() {
                    if capture.get_next_packet_size()?.unwrap_or(0) > SAMPLE_RATE * 5 {
                        return Err(
                            "Audio driver reported an oversized packet; saved audio is retained"
                                .into(),
                        );
                    }
                    let info = capture.read_from_device_to_deque(&mut queue)?;
                    if info.flags.data_discontinuity {
                        store.warn(&format!("{source}: audio driver reported a discontinuity; affected recording ranges can be incomplete."))?;
                    }
                    if queue.len() > SAMPLE_RATE as usize * 2 * 5 {
                        return Err("Audio device buffer exceeded five seconds; capture stopped visibly instead of growing memory".into());
                    }
                    pcm.clear();
                    while queue.len() >= 2 && pcm.len() < SAMPLE_RATE as usize {
                        let lo = queue.pop_front().ok_or("PCM buffer underflow")?;
                        let hi = queue.pop_front().ok_or("PCM buffer underflow")?;
                        pcm.push(i16::from_le_bytes([lo, hi]));
                    }
                    if !pcm.is_empty() {
                        writer.append(&pcm)?;
                    }
                }
                if source == "system" && queue.is_empty() {
                    // Keep one buffer of headroom; events already received are written first.
                    let elapsed = start.elapsed().as_secs_f64();
                    let target = ((elapsed - 0.2).max(0.0) * f64::from(SAMPLE_RATE)) as u64;
                    if target > writer.written_samples() {
                        writer.pad_to(target)?;
                    }
                }
                writer.checkpoint_if_due()?;
            }
            Ok(())
        })();
        let stop_result = client.stop_stream();
        let pad_result = if source == "system" {
            let final_sample = (start.elapsed().as_secs_f64() * f64::from(SAMPLE_RATE)) as u64;
            if final_sample > writer.written_samples() {
                writer.pad_to(final_sample)
            } else {
                Ok(())
            }
        } else {
            Ok(())
        };
        let finish_result = writer.finish();
        finish_result?;
        stop_result?;
        pad_result?;
        loop_result
    })();
    if !started {
        if let Err(error) = &result {
            let _ = ready.send(Err(error.to_string()));
        }
    }
    result
}
