//! WASAPI COM lifetime is confined to dedicated threads; no raw COM calls here.
use super::controls::CaptureControls;
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
    /// Reports the next packet's converted frame count, or zero when no packet is queued.
    fn next_frames(&self) -> Result<u32>;
    /// Consumes one packet into the byte queue and returns its device timing and flag evidence.
    fn read_packet(&self, queue: &mut VecDeque<u8>) -> Result<PacketInfo>;
}
impl PacketReader for AudioCaptureClient {
    /// Adapts an empty WASAPI packet-size response to zero without consuming audio.
    fn next_frames(&self) -> Result<u32> {
        Ok(self.get_next_packet_size()?.unwrap_or(0))
    }
    /// Copies the next WASAPI packet and translates its timing, discontinuity and silence flags.
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
    gap_silence_samples: u64,
}
impl CaptureDiagnostics {
    /// Initializes bounded diagnostic counters for one independently captured track.
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
            gap_silence_samples: 0,
        }
    }
    /// Counts every packet while retaining only the first bounded set of discontinuity/timestamp events.
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

#[derive(Default)]
struct PacketTimeline {
    origin_hns: Option<u64>,
    base_sample: u64,
}
impl PacketTimeline {
    /// Inserts explicit microphone gap silence from valid device timestamps within the monotonic capture bound.
    fn align_microphone(
        &mut self,
        info: PacketInfo,
        writer: &mut PcmWriter,
        latest_possible_sample: u64,
    ) -> Result<u64> {
        if info.timestamp_error {
            self.origin_hns = None;
            return Ok(0);
        }
        let Some(origin) = self.origin_hns else {
            self.origin_hns = Some(info.device_timestamp_hns);
            self.base_sample = writer.written_samples();
            return Ok(0);
        };
        if !info.discontinuity {
            return Ok(0);
        }
        // GetBuffer QPC timestamps are in 100 ns units, independent of the endpoint's
        // native sample rate. They locate the first converted sample in this packet.
        // Only insert an explicit silence gap; never rewrite already-confirmed audio.
        let elapsed = info
            .device_timestamp_hns
            .checked_sub(origin)
            .ok_or("Microphone clock moved backwards; saved audio is retained")?;
        let target = self
            .base_sample
            .checked_add(u64::try_from(
                u128::from(elapsed) * u128::from(SAMPLE_RATE) / 10_000_000,
            )?)
            .ok_or("Microphone timeline exceeded its safety limit")?;
        let missing = target.saturating_sub(writer.written_samples());
        if target > latest_possible_sample {
            return Err("Microphone clock jumped beyond monotonic capture time; saved audio is retained and the affected timeline requires review".into());
        }
        if missing > 0 {
            // Keep the normal bounded chunk. Closing around every gap creates two tiny
            // files per discontinuity, restarting recognition and amplifying disk stalls.
            // The exact silence range is recorded separately in capture-controls.jsonl.
            writer.pad_to(target)?;
        }
        Ok(missing)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum DrainState {
    Empty,
    More,
}

/// Drain a bounded turn before checking Stop/privacy controls again. Packets can arrive
/// while durable writes run: five seconds processed is not five seconds of queued backlog.
/// A single oversized packet is still refused before it is read or acknowledged.
fn drain_packets(
    capture: &impl PacketReader,
    queue: &mut VecDeque<u8>,
    pcm: &mut Vec<i16>,
    mut consume: impl FnMut(PacketInfo, &[i16]) -> Result<()>,
) -> Result<DrainState> {
    let mut drained = 0usize;
    loop {
        let frames = capture.next_frames()?;
        if frames == 0 {
            return Ok(DrainState::Empty);
        }
        if frames > SAMPLE_RATE * 5 {
            return Err(
                "Audio device packet exceeded five seconds; saved audio remains available".into(),
            );
        }
        if drained.saturating_add(frames as usize) > SAMPLE_RATE as usize * 5 {
            return Ok(DrainState::More);
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

#[cfg(test)]
/// Exercises packet persistence without privacy controls in synthetic WASAPI fixtures.
fn drain_track(
    capture: &impl PacketReader,
    queue: &mut VecDeque<u8>,
    pcm: &mut Vec<i16>,
    writer: &mut PcmWriter,
    diagnostics: &mut CaptureDiagnostics,
    start: Instant,
    store: &SessionStore,
) -> Result<DrainState> {
    drain_track_controlled(capture, queue, pcm, writer, diagnostics, start, store, None)
}

#[allow(clippy::too_many_arguments)]
/// Drains packet PCM through optional privacy gates and records bounded timing/discontinuity evidence.
fn drain_track_controlled(
    capture: &impl PacketReader,
    queue: &mut VecDeque<u8>,
    pcm: &mut Vec<i16>,
    writer: &mut PcmWriter,
    diagnostics: &mut CaptureDiagnostics,
    start: Instant,
    store: &SessionStore,
    mut control: Option<(&CaptureControls, u64, &mut PacketTimeline)>,
) -> Result<DrainState> {
    drain_packets(capture, queue, pcm, |info, samples| {
        diagnostics.observe(
            info,
            samples.len(),
            start.elapsed().as_millis(),
            writer.written_samples(),
        );
        // The store retains failed diagnostics in the live snapshot. Their
        // manifest writes must not prevent this packet's independent PCM write.
        if info.discontinuity && diagnostics.discontinuities == 1 {
            let _ = store.warn(&super::discontinuity_warning(
                &diagnostics.source,
                writer.written_samples(),
            ));
        }
        if info.timestamp_error && diagnostics.timestamp_errors == 1 {
            let _ = store.warn(&format!(
                "{}: audio device timestamps are uncertain; exact alignment cannot be confirmed.",
                diagnostics.source
            ));
        }
        if let Some((controls, epoch, timeline)) = control.as_mut() {
            if diagnostics.source == "microphone" && !controls.changed("microphone", *epoch) {
                // The device uses a bounded200ms buffer. Allow two seconds for clock/
                // startup uncertainty, but never let an unflagged corrupt QPC timestamp
                // fabricate minutes or hours of future audio on disk.
                let latest_possible = controls
                    .elapsed_samples()
                    .saturating_add(u64::from(SAMPLE_RATE) * 2);
                let missing = timeline.align_microphone(info, writer, latest_possible)?;
                diagnostics.gap_silence_samples += missing;
                if missing > 0 {
                    controls.record_microphone_gap(
                        writer.written_samples().saturating_sub(missing),
                        missing,
                    )?;
                }
            }
            controls.write(&diagnostics.source, *epoch, writer, samples)
        } else {
            writer.append(samples)
        }
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
    /// Initializes the calling thread's multithreaded COM apartment and propagates initialization failure.
    fn initialize() -> Result<Self> {
        initialize_mta().ok()?;
        Ok(Self)
    }
}
impl Drop for Com {
    /// Balances this thread's owned COM initialization when its guard is dropped.
    fn drop(&mut self) {
        deinitialize();
    }
}

/// Enumerates render/capture endpoints and reported defaults inside a scoped COM apartment.
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

/// Owns one WASAPI track, flushing invalidated device epochs and retaining confirmed PCM on stop or error.
pub(super) fn record(
    store: SessionStore,
    source: &str,
    device_id: Option<String>,
    stop: Arc<AtomicBool>,
    ready: SyncSender<std::result::Result<(), String>>,
    controls: Arc<CaptureControls>,
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
        writer.pad_to(controls.elapsed_samples())?;
        client.start_stream()?;
        let mut stream_running = true;
        let mut epoch = controls.epoch(source);
        let mut timeline = PacketTimeline::default();
        ready
            .send(Ok(()))
            .map_err(|_| "Capture controller was closed")?;
        started = true;
        let loop_result = (|| -> Result<()> {
            let mut drain_again = false;
            while !stop.load(Ordering::Acquire) {
                if controls.changed(source, epoch) || controls.suppressed(source) {
                    if controls.changed(source, epoch) {
                        writer.finish()?;
                        epoch = controls.epoch(source);
                    }
                    if stream_running {
                        client.stop_stream()?;
                        client.reset_stream()?;
                        stream_running = false;
                        queue.clear();
                        pcm.clear();
                        timeline = PacketTimeline::default();
                        drain_again = false;
                    }
                    writer.pad_to(controls.elapsed_samples())?;
                    writer.checkpoint_if_due()?;
                    if controls.suppressed(source) {
                        std::thread::sleep(Duration::from_millis(50));
                        continue;
                    }
                    // Reset discarded every old packet. Only this new stream epoch can
                    // write speech after Resume/Unmute; stale in-flight packets stay gated.
                    epoch = controls.epoch(source);
                    client.start_stream()?;
                    stream_running = true;
                }
                // A short timeout is normal during digital silence. Pad the system track
                // to wall time so remote speech remains aligned with microphone audio.
                if !drain_again {
                    let _ = handle.wait_for_event(100);
                }
                // A timeout alone is normal. Query the client on every tick
                // nevertheless: this also detects invalidated devices before
                // a missing event can be mistaken for continuing silence.
                drain_again = drain_track_controlled(
                    &capture,
                    &mut queue,
                    &mut pcm,
                    &mut writer,
                    &mut diagnostics,
                    start,
                    &store,
                    Some((&controls, epoch, &mut timeline)),
                )? == DrainState::More;
                if source == "system" && !drain_again {
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
        let stop_result = if stream_running {
            client.stop_stream()
        } else {
            Ok(())
        };
        let stopped_at = start.elapsed();
        // Stop prevents new packets, but already buffered frames still belong
        // to this recording and must be drained without waiting for an event.
        let tail_result = (|| -> Result<()> {
            if loop_result.is_ok() && stop_result.is_ok() && stream_running {
                // No new packets can arrive after a successful Stop. Preserve every
                // queued tail packet, across bounded turns, without waiting on events.
                while drain_track_controlled(
                    &capture,
                    &mut queue,
                    &mut pcm,
                    &mut writer,
                    &mut diagnostics,
                    start,
                    &store,
                    Some((&controls, epoch, &mut timeline)),
                )? == DrainState::More
                {}
            }
            Ok(())
        })();
        if controls.suppressed(source)
            && loop_result.is_ok()
            && stop_result.is_ok()
            && tail_result.is_ok()
        {
            writer.pad_to(controls.elapsed_samples())?;
        }
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

    #[test]
    /// Replays 100 synthetic packets with one-sample gaps, retaining every original sample,
    /// silence sample and gap record while requiring one regular audio/recognition chunk.
    fn repeated_small_device_gaps_do_not_fragment_one_audio_chunk() {
        let root = tempfile::tempdir().unwrap();
        let store =
            SessionStore::create(root.path(), "Synthetic small gaps", "m", "s", Some("mic"))
                .unwrap();
        let controls = CaptureControls::new(&store).unwrap();
        let mut writer = PcmWriter::new(store.clone(), "microphone").unwrap();
        let mut timeline = PacketTimeline::default();
        let mut expected = Vec::new();
        for index in 0..100_u64 {
            let mut packet = info(false);
            packet.discontinuity = index > 0;
            // Each packet begins one sample after the preceding packet ended.
            packet.device_timestamp_hns = 40_000_000 + index * 321 * 625;
            let missing = timeline
                .align_microphone(packet, &mut writer, 64_000)
                .unwrap();
            if index > 0 {
                assert_eq!(missing, 1);
                controls
                    .record_microphone_gap(writer.written_samples() - missing, missing)
                    .unwrap();
                expected.push(0);
            }
            controls
                .write("microphone", 0, &mut writer, &[17; 320])
                .unwrap();
            expected.extend_from_slice(&[17; 320]);
        }
        writer.finish().unwrap();
        let data = store.snapshot().unwrap();
        let mut actual = Vec::new();
        for part in &data.parts {
            actual.extend(
                hound::WavReader::open(store.audio_path(part).unwrap())
                    .unwrap()
                    .samples::<i16>()
                    .map(std::result::Result::unwrap),
            );
        }
        assert_eq!(
            actual, expected,
            "All captured samples and explicit missing-sample silence must stay in order"
        );
        assert_eq!(
            data.parts.len(),
            1,
            "Device-gap evidence must not turn one audio chunk into hundreds of ASR sessions"
        );
        let events = super::super::controls::read_capture_control_events(store.folder())
            .unwrap()
            .unwrap();
        assert_eq!(
            events
                .events
                .iter()
                .filter(|event| event.kind == "device-gap")
                .count(),
            99
        );
    }

    #[test]
    /// Drains six seconds of valid synthetic packets across bounded turns without treating
    /// cumulative work as capture failure or dropping any saved sample.
    fn valid_packets_can_be_drained_in_bounded_turns_without_stopping_capture() {
        let packets = Packets(RefCell::new(VecDeque::from_iter(
            (0..300).map(|_| (vec![23; 320], info(false))),
        )));
        let mut saved = 0;
        let mut turns = 0;
        while !packets.0.borrow().is_empty() {
            let result =
                drain_packets(&packets, &mut VecDeque::new(), &mut vec![], |_, samples| {
                    saved += samples.len();
                    Ok(())
                });
            assert!(
                result.is_ok(),
                "A cumulative drain budget is not evidence of an invalid device packet: {result:?}"
            );
            turns += 1;
            assert!(turns <= 3);
        }
        assert_eq!(saved, 96_000);
        assert_eq!(turns, 2, "Return to capture controls between bounded turns");
    }

    #[test]
    /// Checks synthetic device-clock gaps insert silence without moving confirmed microphone samples.
    fn microphone_discontinuity_inserts_device_clock_gap_without_moving_original_samples() {
        let root = tempfile::tempdir().unwrap();
        let store = SessionStore::create(
            root.path(),
            "Synthetic timestamp gap",
            "m",
            "s",
            Some("mic"),
        )
        .unwrap();
        let controls = CaptureControls::new(&store).unwrap();
        let mut writer = PcmWriter::new(store.clone(), "microphone").unwrap();
        let mut timeline = PacketTimeline::default();
        let mut first = info(false);
        first.device_timestamp_hns = 40_000_000;
        assert_eq!(
            timeline
                .align_microphone(first, &mut writer, 32_000)
                .unwrap(),
            0
        );
        controls
            .write("microphone", 0, &mut writer, &[11; 320])
            .unwrap();
        let mut after_gap = info(false);
        after_gap.discontinuity = true;
        after_gap.device_timestamp_hns = 50_000_000;
        let missing = timeline
            .align_microphone(after_gap, &mut writer, 32_000)
            .unwrap();
        assert_eq!(missing, 15_680);
        controls
            .record_microphone_gap(writer.written_samples() - missing, missing)
            .unwrap();
        controls
            .write("microphone", 0, &mut writer, &[22; 320])
            .unwrap();
        writer.finish().unwrap();
        let data = store.snapshot().unwrap();
        let mut audio = vec![];
        for part in &data.parts {
            audio.extend(
                hound::WavReader::open(store.audio_path(part).unwrap())
                    .unwrap()
                    .samples::<i16>()
                    .map(std::result::Result::unwrap),
            );
        }
        assert_eq!(data.parts.len(), 1);
        assert_eq!(data.parts[0].confirmed_sample_count, 16_320);
        assert_eq!(audio.len(), 16_320);
        assert!(audio[..320].iter().all(|s| *s == 11));
        assert!(audio[320..16_000].iter().all(|s| *s == 0));
        assert!(audio[16_000..].iter().all(|s| *s == 22));
        let events = super::super::controls::read_capture_control_events(store.folder())
            .unwrap()
            .unwrap();
        assert_eq!(events.events[1].kind, "device-gap");
        assert_eq!(events.events[1].missing_samples, Some(15_680));
        assert_eq!(events.events[1].source_start_sample, Some(320));
    }

    #[test]
    /// Checks a flagged synthetic timestamp error resets alignment instead of inventing a long gap.
    fn an_uncertain_device_timestamp_never_invents_a_long_silence_gap() {
        let root = tempfile::tempdir().unwrap();
        let store =
            SessionStore::create(root.path(), "Synthetic uncertain timestamp", "m", "s", None)
                .unwrap();
        let mut writer = PcmWriter::new(store, "microphone").unwrap();
        let mut timeline = PacketTimeline::default();
        let first = info(false);
        timeline
            .align_microphone(first, &mut writer, 32_000)
            .unwrap();
        writer.append(&[11, 12]).unwrap();
        let mut uncertain = info(false);
        uncertain.device_timestamp_hns = u64::MAX;
        uncertain.timestamp_error = true;
        uncertain.discontinuity = true;
        assert_eq!(
            timeline
                .align_microphone(uncertain, &mut writer, 32_000)
                .unwrap(),
            0
        );
        assert_eq!(writer.written_samples(), 2);
        // A later trusted packet establishes a new anchor; no earlier confirmed byte moves.
        assert_eq!(
            timeline
                .align_microphone(first, &mut writer, 32_000)
                .unwrap(),
            0
        );
        writer.finish().unwrap();
    }

    #[test]
    /// Checks an unflagged synthetic future clock jump is rejected before existing PCM or headers change.
    fn unflagged_future_device_clock_is_rejected_before_audio_or_padding_changes() {
        let root = tempfile::tempdir().unwrap();
        let store = SessionStore::create(
            root.path(),
            "Synthetic corrupt future clock",
            "m",
            "s",
            None,
        )
        .unwrap();
        let mut writer = PcmWriter::new(store.clone(), "microphone").unwrap();
        let mut timeline = PacketTimeline::default();
        timeline
            .align_microphone(info(false), &mut writer, 32_000)
            .unwrap();
        writer.append(&[11, 12, 13, 14]).unwrap();
        writer.checkpoint().unwrap();
        let part = store.snapshot().unwrap().parts[0].clone();
        let path = store.audio_path(&part).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut corrupt = info(false);
        corrupt.discontinuity = true;
        corrupt.device_timestamp_hns = 10_000_000 * 86_400;
        // Timestamp-error is false: the monotonic bound must independently reject it.
        assert!(timeline
            .align_microphone(corrupt, &mut writer, 32_000)
            .is_err());
        assert_eq!(writer.written_samples(), 4);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!store.snapshot().unwrap().parts[0].audio_closed);
        writer.finish().unwrap();
    }

    struct Packets(RefCell<VecDeque<(Vec<i16>, PacketInfo)>>);
    /// Constructs synthetic packet metadata with an explicit silence flag for drain regressions.
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
        /// Reports the next synthetic packet length without removing it from the fixture queue.
        fn next_frames(&self) -> Result<u32> {
            Ok(self.0.borrow().front().map_or(0, |p| p.0.len() as u32))
        }
        /// Serializes one synthetic sample packet into little-endian PCM and returns its fixture flags.
        fn read_packet(&self, queue: &mut VecDeque<u8>) -> Result<PacketInfo> {
            let (samples, packet) = self.0.borrow_mut().pop_front().ok_or("No packet")?;
            queue.extend(samples.into_iter().flat_map(i16::to_le_bytes));
            Ok(packet)
        }
    }

    #[test]
    fn unsaved_device_warnings_do_not_discard_flagged_or_following_pcm_packets() {
        for (discontinuity, timestamp_error) in [(true, false), (false, true), (true, true)] {
            let root = tempfile::tempdir().unwrap();
            let store =
                SessionStore::create(root.path(), "Synthetic diagnostic failure", "m", "s", None)
                    .unwrap();
            let manifest = store.folder().join("session.json");
            let original = std::fs::read(&manifest).unwrap();
            std::fs::create_dir(store.folder().join("session.json.bak")).unwrap();
            let flagged = PacketInfo {
                discontinuity,
                timestamp_error,
                ..info(false)
            };
            let packets = Packets(RefCell::new(VecDeque::from([
                (vec![41, -42, 43], flagged),
                (vec![51, -52, 53], info(false)),
            ])));
            let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
            let mut diagnostics = CaptureDiagnostics::new("system");
            drain_track(
                &packets,
                &mut VecDeque::new(),
                &mut vec![],
                &mut writer,
                &mut diagnostics,
                Instant::now(),
                &store,
            )
            .unwrap();
            writer.finish().unwrap();
            assert!(packets.0.borrow().is_empty());
            let snapshot = store.snapshot().unwrap();
            assert!(snapshot.capture_incomplete);
            assert!(snapshot
                .warnings
                .iter()
                .any(|warning| warning.contains("diagnostic could not be saved")));
            assert_eq!(snapshot.parts[0].confirmed_sample_count, 6);
            assert!(snapshot.parts[0].audio_closed);
            let samples: Vec<i16> =
                hound::WavReader::open(store.audio_path(&snapshot.parts[0]).unwrap())
                    .unwrap()
                    .samples::<i16>()
                    .map(std::result::Result::unwrap)
                    .collect();
            assert_eq!(samples, vec![41, -42, 43, 51, -52, 53]);
            assert_eq!(std::fs::read(manifest).unwrap(), original);
        }
    }

    #[test]
    fn diagnostic_fallback_still_propagates_a_real_pcm_file_failure() {
        let root = tempfile::tempdir().unwrap();
        let store =
            SessionStore::create(root.path(), "Synthetic PCM failure", "m", "s", None).unwrap();
        std::fs::create_dir(store.folder().join("session.json.bak")).unwrap();
        let blocked_audio = store.folder().join("audio/system-000000000000.wav");
        std::fs::create_dir(&blocked_audio).unwrap();
        let packets = Packets(RefCell::new(VecDeque::from([(
            vec![61, -62],
            PacketInfo {
                discontinuity: true,
                ..info(false)
            },
        )])));
        let mut writer = PcmWriter::new(store.clone(), "system").unwrap();
        let mut diagnostics = CaptureDiagnostics::new("system");
        assert!(drain_track(
            &packets,
            &mut VecDeque::new(),
            &mut vec![],
            &mut writer,
            &mut diagnostics,
            Instant::now(),
            &store
        )
        .is_err());
        let snapshot = store.snapshot().unwrap();
        assert!(
            snapshot.parts.is_empty(),
            "No failed PCM write may become confirmed audio"
        );
        assert!(snapshot.capture_incomplete);
        assert!(snapshot
            .warnings
            .iter()
            .any(|warning| warning.contains("discontinuity")));
        assert!(blocked_audio.is_dir());
    }

    #[test]
    /// Checks one synthetic capture wake drains all packets and converts silence-flagged bytes to silence.
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
    /// Checks synthetic stop draining persists the queued tail without waiting for another device event.
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
    /// Checks an oversized synthetic packet is refused before consumption or checkpoint acknowledgment.
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
    /// Checks synthetic diagnostics cap retained event details while preserving total event counters.
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
    /// Checks synthetic delayed shutdown work cannot extend system audio beyond the captured stop time.
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
    /// Checks a synthetic invalid device fails before padding, while an empty packet queue remains normal.
    fn device_query_failure_propagates_before_padding_but_empty_timeout_is_normal() {
        struct InvalidatedDevice;
        impl PacketReader for InvalidatedDevice {
            /// Injects a deterministic device-query failure without consulting a real audio endpoint.
            fn next_frames(&self) -> Result<u32> {
                Err("Synthetic device invalidated".into())
            }
            /// Fails the fixture if code tries to read audio after its synthetic device-query error.
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
