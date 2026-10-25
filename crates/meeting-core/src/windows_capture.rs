//! WASAPI COM lifetime is confined to dedicated threads; no raw COM calls here.
use crate::{AudioDevice, PcmWriter, Result, SessionStore, SAMPLE_RATE};
use serde::Serialize;
use std::{
    collections::VecDeque,
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
        Arc,
    },
    time::{Duration, Instant},
};
use wasapi::{
    deinitialize, initialize_mta, AudioCaptureClient, DeviceEnumerator, Direction, SampleType,
    StreamMode, WaveFormat,
};

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct PacketInfo {
    device_position: u64,
    device_timestamp_hns: u64,
    discontinuity: bool,
    timestamp_error: bool,
    silent: bool,
}
trait PacketReader {
    fn next_frames(&self) -> Result<u32>;
    fn read_packet(&self, queue: &mut VecDeque<u8>) -> Result<PacketInfo>;
}
impl PacketReader for AudioCaptureClient {
    fn next_frames(&self) -> Result<u32> {
        Ok(self.get_next_packet_size()?.unwrap_or(0))
    }
    fn read_packet(&self, queue: &mut VecDeque<u8>) -> Result<PacketInfo> {
        let info = self.read_from_device_to_deque(queue)?;
        Ok(PacketInfo {
            device_position: info.index,
            device_timestamp_hns: info.timestamp,
            discontinuity: info.flags.data_discontinuity,
            timestamp_error: info.flags.timestamp_error,
            silent: info.flags.silent,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureDiagnostics {
    schema_version: u32,
    source: String,
    packets: u64,
    received_samples: u64,
    first_packet: Option<PacketInfo>,
    last_packet: Option<PacketInfo>,
    discontinuities: u64,
    timestamp_errors: u64,
    first_events: Vec<serde_json::Value>,
    omitted_events: u64,
    elapsed_ms: u128,
    written_samples: u64,
}
impl CaptureDiagnostics {
    fn new(source: &str) -> Self {
        Self {
            schema_version: 1,
            source: source.into(),
            packets: 0,
            received_samples: 0,
            first_packet: None,
            last_packet: None,
            discontinuities: 0,
            timestamp_errors: 0,
            first_events: Vec::with_capacity(64),
            omitted_events: 0,
            elapsed_ms: 0,
            written_samples: 0,
        }
    }
    fn observe(&mut self, info: PacketInfo, count: usize, elapsed_ms: u128, start_sample: u64) {
        self.first_packet.get_or_insert(info);
        self.last_packet = Some(info);
        self.packets += 1;
        self.received_samples += count as u64;
        self.discontinuities += u64::from(info.discontinuity);
        self.timestamp_errors += u64::from(info.timestamp_error);
        if info.discontinuity || info.timestamp_error {
            if self.first_events.len() < 64 {
                self.first_events.push(serde_json::json!({"packet": self.packets, "elapsedMs": elapsed_ms, "savedStartSample": start_sample, "samples": count, "device": info}));
            } else {
                self.omitted_events += 1;
            }
        }
    }
}

/// Drain every currently queued packet before waiting for another event. The
/// same path runs after Stop, when no future event can announce buffered audio.
fn drain_packets(
    capture: &impl PacketReader,
    queue: &mut VecDeque<u8>,
    pcm: &mut Vec<i16>,
    mut consume: impl FnMut(PacketInfo, &[i16]) -> Result<()>,
) -> Result<()> {
    let mut drained = 0usize;
    loop {
        let frames = capture.next_frames()?;
        if frames == 0 {
            return Ok(());
        }
        if frames > SAMPLE_RATE * 5
            || drained.saturating_add(frames as usize) > SAMPLE_RATE as usize * 5
        {
            return Err(
                "Audio device backlog exceeded five seconds; saved audio remains available".into(),
            );
        }
        queue.clear();
        let info = capture.read_packet(queue)?;
        if queue.is_empty()
            || !queue.len().is_multiple_of(2)
            || queue.len() > SAMPLE_RATE as usize * 2 * 5
        {
            return Err(
                "Audio device returned an invalid packet; saved audio remains available".into(),
            );
        }
        pcm.clear();
        while let Some(lo) = queue.pop_front() {
            let hi = queue.pop_front().ok_or("PCM buffer underflow")?;
            pcm.push(if info.silent {
                0
            } else {
                i16::from_le_bytes([lo, hi])
            });
        }
        drained += pcm.len();
        consume(info, pcm)?;
    }
}

fn drain_track(
    capture: &impl PacketReader,
    queue: &mut VecDeque<u8>,
    pcm: &mut Vec<i16>,
    writer: &mut PcmWriter,
    diagnostics: &mut CaptureDiagnostics,
    start: Instant,
    store: &SessionStore,
) -> Result<()> {
    drain_packets(capture, queue, pcm, |info, samples| {
        diagnostics.observe(
            info,
            samples.len(),
            start.elapsed().as_millis(),
            writer.written_samples(),
        );
        if info.discontinuity && diagnostics.discontinuities == 1 {
            store.warn(&format!("{}: audio driver reported a discontinuity; affected recording ranges can be incomplete.", diagnostics.source))?;
        }
        if info.timestamp_error && diagnostics.timestamp_errors == 1 {
            store.warn(&format!(
                "{}: audio device timestamps are uncertain; exact alignment cannot be confirmed.",
                diagnostics.source
            ))?;
        }
        writer.append(samples)
    })
}

/// Disk syncs while draining the tail may take time after the device has
/// already stopped. Such shutdown work is not part of the recorded timeline.
fn pad_stopped_track(
    writer: &mut PcmWriter,
    source: &str,
    stopped_at: Duration,
    recording_valid: bool,
) -> Result<()> {
    if source == "system" && recording_valid {
        let final_sample =
            u64::try_from(stopped_at.as_nanos() * u128::from(SAMPLE_RATE) / 1_000_000_000)?;
        if final_sample > writer.written_samples() {
            writer.pad_to(final_sample)?;
        }
    }
    Ok(())
}

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
            // Durable file syncs can exceed a single 10/20 ms device period.
            // A bounded 200 ms device buffer leaves headroom for those stalls.
            buffer_duration_hns: period.max(2_000_000),
        };
        client.initialize_client(&desired, &Direction::Capture, &mode)?;
        let handle = client.set_get_eventhandle()?;
        let capture = client.get_audiocaptureclient()?;
        let mut writer = PcmWriter::new(store.clone(), source)?;
        let mut queue = VecDeque::with_capacity(SAMPLE_RATE as usize * 2);
        let mut pcm = Vec::with_capacity(SAMPLE_RATE as usize / 10);
        let mut diagnostics = CaptureDiagnostics::new(source);
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
                let _ = handle.wait_for_event(100);
                // A timeout alone is normal. Query the client on every tick
                // nevertheless: this also detects invalidated devices before
                // a missing event can be mistaken for continuing silence.
                drain_track(
                    &capture,
                    &mut queue,
                    &mut pcm,
                    &mut writer,
                    &mut diagnostics,
                    start,
                    &store,
                )?;
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
        let stopped_at = start.elapsed();
        // Stop prevents new packets, but already buffered frames still belong
        // to this recording and must be drained without waiting for an event.
        let tail_result = if loop_result.is_ok() {
            drain_track(
                &capture,
                &mut queue,
                &mut pcm,
                &mut writer,
                &mut diagnostics,
                start,
                &store,
            )
        } else {
            Ok(())
        };
        let pad_result = pad_stopped_track(
            &mut writer,
            source,
            stopped_at,
            loop_result.is_ok() && stop_result.is_ok() && tail_result.is_ok(),
        );
        let finish_result = writer.finish();
        diagnostics.elapsed_ms = start.elapsed().as_millis();
        diagnostics.written_samples = writer.written_samples();
        let diagnostic_result = (|| -> Result<()> {
            let path = store.folder().join(format!("capture-{source}.json"));
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(path)?;
            file.write_all(&serde_json::to_vec_pretty(&diagnostics)?)?;
            file.sync_all()?;
            Ok(())
        })();
        finish_result?;
        stop_result?;
        tail_result?;
        pad_result?;
        diagnostic_result?;
        loop_result
    })();
    if !started {
        if let Err(error) = &result {
            let _ = ready.send(Err(error.to_string()));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Packets(RefCell<VecDeque<(Vec<i16>, PacketInfo)>>);
    fn info(silent: bool) -> PacketInfo {
        PacketInfo {
            device_position: 0,
            device_timestamp_hns: 0,
            discontinuity: false,
            timestamp_error: false,
            silent,
        }
    }
    impl PacketReader for Packets {
        fn next_frames(&self) -> Result<u32> {
            Ok(self.0.borrow().front().map_or(0, |p| p.0.len() as u32))
        }
        fn read_packet(&self, queue: &mut VecDeque<u8>) -> Result<PacketInfo> {
            let (samples, packet) = self.0.borrow_mut().pop_front().ok_or("No packet")?;
            queue.extend(samples.into_iter().flat_map(i16::to_le_bytes));
            Ok(packet)
        }
    }
    #[test]
    fn one_event_drains_every_packet_and_silent_flags_never_become_false_speech() {
        let packets = Packets(RefCell::new(VecDeque::from([
            (vec![1, -2, 3], info(false)),
            (vec![999, -999], info(true)),
            (vec![4, 5], info(false)),
        ])));
        let root = tempfile::tempdir().unwrap();
        let store = SessionStore::create(
            root.path(),
            "Synthetic queued packets",
            "test",
            "test",
            None,
        )
        .unwrap();
        let mut writer = PcmWriter::new(store.clone(), "microphone").unwrap();
        let mut queue = VecDeque::new();
        let mut pcm = vec![];
        let mut diagnostic = CaptureDiagnostics::new("microphone");
        drain_track(
            &packets,
            &mut queue,
            &mut pcm,
            &mut writer,
            &mut diagnostic,
            Instant::now(),
            &store,
        )
        .unwrap();
        writer.finish().unwrap();
        let data = store.snapshot().unwrap();
        assert_eq!(data.parts[0].confirmed_sample_count, 7);
        let path = store.audio_path(&data.parts[0]).unwrap();
        let samples: Vec<_> = hound::WavReader::open(path)
            .unwrap()
            .samples::<i16>()
            .map(std::result::Result::unwrap)
            .collect();
        assert_eq!(samples, vec![1, -2, 3, 0, 0, 4, 5]);
        assert_eq!(diagnostic.packets, 3);
        assert!(packets.0.borrow().is_empty());
    }

    #[test]
    fn buffered_tail_can_be_saved_without_any_further_capture_event() {
        let packets = Packets(RefCell::new(VecDeque::from([(
            vec![21, 22, 23],
            info(false),
        )])));
        let mut saved = vec![];
        // The production stop path calls this same drain without waiting on the
        // event handle: the capture client has stopped generating events.
        drain_packets(&packets, &mut VecDeque::new(), &mut vec![], |_, samples| {
            saved.extend_from_slice(samples);
            Ok(())
        })
        .unwrap();
        assert_eq!(saved, vec![21, 22, 23]);
    }

    #[test]
    fn oversized_device_packet_is_rejected_before_reading_or_acknowledging_it() {
        let packets = Packets(RefCell::new(VecDeque::from([(
            vec![1; SAMPLE_RATE as usize * 5 + 1],
            info(false),
        )])));
        let mut saved = vec![];
        assert!(
            drain_packets(&packets, &mut VecDeque::new(), &mut vec![], |_, samples| {
                saved.extend_from_slice(samples);
                Ok(())
            })
            .is_err()
        );
        assert!(saved.is_empty());
        assert_eq!(packets.0.borrow().len(), 1);
    }

    #[test]
    fn discontinuity_evidence_is_bounded_and_keeps_total_event_counts() {
        let mut diagnostic = CaptureDiagnostics::new("microphone");
        for index in 0..100 {
            let mut packet = info(false);
            packet.device_position = index * 320;
            packet.discontinuity = true;
            diagnostic.observe(packet, 320, u128::from(index * 20), index * 320);
        }
        assert_eq!(diagnostic.discontinuities, 100);
        assert_eq!(diagnostic.first_events.len(), 64);
        assert_eq!(diagnostic.omitted_events, 36);
        assert_eq!(diagnostic.received_samples, 32000);
    }

    #[test]
    fn slow_tail_persistence_does_not_extend_stopped_system_audio_with_extra_silence() {
        let root = tempfile::tempdir().unwrap();
        let store = SessionStore::create(
            root.path(),
            "Synthetic stopped timeline",
            "test",
            "test",
            None,
        )
        .unwrap();
        let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
        let stop = Duration::from_millis(20);
        // Emulate queued samples that are persisted after the device stops.
        // Completion work must never become additional captured silence.
        std::thread::sleep(Duration::from_millis(30));
        writer.append(&[11, 12, 13, 14]).unwrap();
        pad_stopped_track(&mut writer, "system", stop, true).unwrap();
        writer.finish().unwrap();
        let data = store.snapshot().unwrap();
        assert_eq!(data.parts[0].confirmed_sample_count, 320);
        let samples: Vec<i16> = hound::WavReader::open(store.audio_path(&data.parts[0]).unwrap())
            .unwrap()
            .samples::<i16>()
            .map(std::result::Result::unwrap)
            .collect();
        assert_eq!(&samples[..4], &[11, 12, 13, 14]);
        assert!(samples[4..].iter().all(|sample| *sample == 0));
    }

    #[test]
    fn device_query_failure_propagates_before_padding_but_empty_timeout_is_normal() {
        struct InvalidatedDevice;
        impl PacketReader for InvalidatedDevice {
            fn next_frames(&self) -> Result<u32> {
                Err("Synthetic device invalidated".into())
            }
            fn read_packet(&self, _: &mut VecDeque<u8>) -> Result<PacketInfo> {
                panic!("Invalidated packets must never be read")
            }
        }
        let root = tempfile::tempdir().unwrap();
        let store = SessionStore::create(
            root.path(),
            "Synthetic timeout polling",
            "test",
            "test",
            None,
        )
        .unwrap();
        let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
        writer.append(&[1, 2, 3, 4]).unwrap();
        let mut queue = VecDeque::new();
        let mut pcm = vec![];
        let mut diagnostics = CaptureDiagnostics::new("system");
        let empty = Packets(RefCell::new(VecDeque::new()));
        drain_track(
            &empty,
            &mut queue,
            &mut pcm,
            &mut writer,
            &mut diagnostics,
            Instant::now(),
            &store,
        )
        .unwrap();
        assert_eq!(
            writer.written_samples(),
            4,
            "No packet after a timeout is an ordinary idle tick"
        );
        let tick = (|| -> Result<()> {
            drain_track(
                &InvalidatedDevice,
                &mut queue,
                &mut pcm,
                &mut writer,
                &mut diagnostics,
                Instant::now(),
                &store,
            )?;
            writer.pad_to(1600)
        })();
        let recording_valid = tick.is_ok();
        assert!(tick
            .unwrap_err()
            .to_string()
            .contains("Synthetic device invalidated"));
        assert_eq!(
            writer.written_samples(),
            4,
            "A client error must not turn into confirmed silence"
        );
        // The final shutdown path must obey the same rule, even when it runs
        // after an error rather than returning directly from the capture tick.
        pad_stopped_track(
            &mut writer,
            "system",
            Duration::from_millis(100),
            recording_valid,
        )
        .unwrap();
        writer.finish().unwrap();
        assert_eq!(store.snapshot().unwrap().parts[0].confirmed_sample_count, 4);
    }
}
