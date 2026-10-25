//! Bounded stdout PCM transport. It owns only the exact helper it starts.
use crate::{PcmWriter, Result, SessionStore, SAMPLE_RATE};
use std::{
    io::Read,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

const PACKET: usize = 3200;
const READY: &[u8] = b"HANDYMEETING_AUDIO_READY";
pub(super) struct HelperSpec {
    pub command: Command,
    pub readiness_marker: bool,
    pub graceful_stdin: bool,
}
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
enum Packet {
    Data(Vec<u8>),
    End,
    Failed,
}

/// Small read-only device queries: bounded output and an owned ten-second deadline.
#[cfg(any(target_os = "linux", test))]
pub(super) fn text_command(command: &mut Command) -> Result<String> {
    let mut child=OwnedChild(command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|"PulseAudio tools are missing. Install pulseaudio-utils and use a running PulseAudio or PipeWire-Pulse service.")?);
    let stdout = child
        .0
        .stdout
        .take()
        .ok_or("Device query output pipe is unavailable")?;
    let reading = thread::spawn(move || {
        let mut data = Vec::new();
        let read = stdout.take(1024 * 1024 + 1).read_to_end(&mut data);
        (read, data)
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            return Err("PulseAudio device query timed out; check the local audio service".into());
        }
        thread::sleep(Duration::from_millis(25));
    };
    let (read, data) = reading.join().map_err(|_| "Device query reader failed")?;
    read?;
    if !status.success() {
        return Err("PulseAudio device query failed; check the local audio service".into());
    }
    if data.len() > 1024 * 1024 {
        return Err("Device query exceeded its output limit".into());
    }
    Ok(String::from_utf8(data)?)
}

pub(super) fn record(
    store: SessionStore,
    source: &str,
    stop: Arc<AtomicBool>,
    ready: SyncSender<std::result::Result<(), String>>,
    mut spec: HelperSpec,
) -> Result<()> {
    let mut announced = false;
    let result = (|| -> Result<()> {
        if !["system", "microphone"].contains(&source) {
            return Err("Invalid audio track".into());
        }
        let mut child=OwnedChild(spec.command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|_|"Audio helper could not start. Check installed utilities, the complete app bundle and recording permissions.")?);
        let stdout = child
            .0
            .stdout
            .take()
            .ok_or("Audio helper output pipe unavailable")?;
        let stderr = child
            .0
            .stderr
            .take()
            .ok_or("Audio helper diagnostic pipe unavailable")?;
        let (sender, receiver) = mpsc::sync_channel(2);
        let reading = thread::spawn(move || read_packets(stdout, sender));
        let marked = Arc::new(AtomicBool::new(false));
        let overflow = Arc::new(AtomicBool::new(false));
        let marked_reader = Arc::clone(&marked);
        let overflow_reader = Arc::clone(&overflow);
        let diagnostics = thread::spawn(move || {
            let mut input = stderr;
            let mut buffer = [0_u8; 4096];
            let mut tail = Vec::with_capacity(8192);
            let mut count = 0_usize;
            while let Ok(read) = input.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                count = count.saturating_add(read);
                if count > 65536 {
                    overflow_reader.store(true, Ordering::Release);
                }
                tail.extend_from_slice(&buffer[..read]);
                if tail.windows(READY.len()).any(|value| value == READY) {
                    marked_reader.store(true, Ordering::Release);
                }
                if tail.len() > READY.len() {
                    tail.drain(..tail.len() - READY.len());
                }
            }
        });
        let start = Instant::now();
        let started_at =
            chrono::DateTime::parse_from_rfc3339(&store.preview_snapshot(0)?.started_at)?;
        let offset = chrono::Utc::now()
            .signed_duration_since(started_at)
            .num_milliseconds()
            .clamp(0, 300_000) as u64
            * 16;
        let mut writer = PcmWriter::new(store.clone(), source)?;
        store.notice("Portable audio track ranges use approximate receipt timing, not exact device or word timestamps.")?;
        let pump = (|| -> Result<()> {
            let mut carry = None;
            let mut pcm = Vec::with_capacity(PACKET / 2 + 1);
            let mut received = false;
            let mut waiting_reported = false;
            let mut late_reported = false;
            let mut stopping = None;
            loop {
                if stop.load(Ordering::Acquire) && stopping.is_none() {
                    stopping = Some(Instant::now());
                    if spec.graceful_stdin {
                        drop(child.0.stdin.take());
                    } else {
                        let _ = child.0.kill();
                    }
                }
                if stopping.is_some_and(|at| at.elapsed() > Duration::from_secs(2)) {
                    let _ = child.0.kill();
                }
                if overflow.load(Ordering::Acquire) {
                    return Err("Audio helper exceeded its diagnostic output limit".into());
                }
                match receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(Packet::Data(bytes)) => {
                        pcm.clear();
                        let mut index = 0;
                        if let Some(low) = carry.take() {
                            if let Some(high) = bytes.first() {
                                pcm.push(i16::from_le_bytes([low, *high]));
                                index = 1;
                            } else {
                                carry = Some(low);
                            }
                        }
                        while index + 1 < bytes.len() {
                            pcm.push(i16::from_le_bytes([bytes[index], bytes[index + 1]]));
                            index += 2;
                        }
                        if index < bytes.len() {
                            carry = Some(bytes[index]);
                        }
                        if !pcm.is_empty() {
                            if !received {
                                let lead = offset
                                    + (start.elapsed().as_secs_f64() * f64::from(SAMPLE_RATE))
                                        as u64;
                                let target = lead.saturating_sub(pcm.len() as u64);
                                if target > 8000 {
                                    store.warn(&format!("{source}: the source started late; leading silence is estimated and can contain missing audio."))?;
                                }
                                pad_estimated(&mut writer, target)?;
                                received = true;
                            }
                            writer.append(&pcm)?;
                            if !announced {
                                ready
                                    .send(Ok(()))
                                    .map_err(|_| "Capture controller closed")?;
                                announced = true;
                            }
                        }
                    }
                    Ok(Packet::End) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                        if stopping.is_none() {
                            return Err("Audio helper stopped unexpectedly; check the selected device and recording permissions".into());
                        }
                        break;
                    }
                    Ok(Packet::Failed) => {
                        return Err("Audio helper PCM pipe failed; saved audio is retained".into())
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if !announced
                            && ((spec.readiness_marker && marked.load(Ordering::Acquire))
                                || (!spec.readiness_marker
                                    && start.elapsed() > Duration::from_millis(250)))
                            && child.0.try_wait()?.is_none()
                        {
                            ready
                                .send(Ok(()))
                                .map_err(|_| "Capture controller closed")?;
                            announced = true;
                        }
                        if !announced && start.elapsed() > Duration::from_secs(55) {
                            return Err("Recording permission or the first audio packet did not arrive; check system permissions and start again".into());
                        }
                        if !received
                            && start.elapsed() > Duration::from_secs(2)
                            && !waiting_reported
                        {
                            store.notice(&format!("{source}: waiting for the first actual audio packet. Check permissions or play a test sound; no received audio has been confirmed yet."))?;
                            waiting_reported = true;
                        }
                        if received {
                            let target = offset
                                + ((start.elapsed().as_secs_f64() - 0.5).max(0.0)
                                    * f64::from(SAMPLE_RATE))
                                    as u64;
                            if target > writer.written_samples() {
                                if !late_reported {
                                    store.notice(&format!("{source}: gaps without packets are represented as estimated silence; late real packets are retained and timestamps can drift."))?;
                                    late_reported = true;
                                }
                                pad_estimated(&mut writer, target)?;
                            }
                        }
                    }
                }
                writer.checkpoint_if_due()?;
                if !received && child.0.try_wait()?.is_some() {
                    return Err("Audio helper exited before producing audio; check recording permissions and the selected device".into());
                }
            }
            if carry.is_some() {
                store.warn(&format!("{source}: an incomplete final PCM byte was retained only in the helper pipe and could not form an audio sample."))?;
            }
            Ok(())
        })();
        // Stop only our child; continue draining its bounded pipe before finalization.
        let _ = child.0.kill();
        let _ = child.0.wait();
        drop(receiver);
        let _ = reading.join();
        let _ = diagnostics.join();
        let finish = writer.finish();
        finish?;
        pump
    })();
    if !announced {
        if let Err(error) = &result {
            let _ = ready.try_send(Err(error.to_string()));
        }
    }
    result
}

fn read_packets(mut input: impl Read, sender: SyncSender<Packet>) {
    let mut buffer = [0_u8; PACKET];
    loop {
        match input.read(&mut buffer) {
            Ok(0) => {
                let _ = sender.send(Packet::End);
                break;
            }
            Ok(count) => {
                if sender.send(Packet::Data(buffer[..count].to_vec())).is_err() {
                    break;
                }
            }
            Err(_) => {
                let _ = sender.send(Packet::Failed);
                break;
            }
        }
    }
}
fn pad_estimated(writer: &mut PcmWriter, target: u64) -> Result<()> {
    if target.saturating_sub(writer.written_samples()) > u64::from(SAMPLE_RATE) * 86400 {
        return Err("Audio receipt clock gap exceeds one day; check the recording timeline".into());
    }
    while writer.written_samples() < target {
        writer.pad_to(target.min(writer.written_samples() + u64::from(SAMPLE_RATE) * 300))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn python() -> Command {
        Command::new(if cfg!(windows) { "python" } else { "python3" })
    }
    #[test]
    fn metadata_query_rejects_unbounded_output_and_preserves_nonzero_exit() {
        assert!(
            text_command(python().args(["-c", "import sys;sys.stdout.write('x'*1048577)"]))
                .is_err()
        );
        assert!(text_command(python().args(["-c", "raise SystemExit(9)"])).is_err());
    }
    #[test]
    fn synthetic_pipe_tracks_are_separate_and_stop_without_hardware() {
        let root = tempfile::tempdir().unwrap();
        let store = SessionStore::create(
            root.path(),
            "Synthetic portable pipes",
            "synthetic",
            "fake system",
            Some("fake microphone"),
        )
        .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let mut tracks = vec![];
        for (source, hex) in [("system", "0100"), ("microphone", "feff")] {
            let store = store.clone();
            let flag = Arc::clone(&stop);
            let (tx, rx) = mpsc::sync_channel(1);
            tracks.push(thread::spawn(move || {
                let mut cmd = python();
                cmd.args(["-c", &format!("import sys,time;sys.stdout.buffer.write(bytes.fromhex('{hex}')*16000);sys.stdout.buffer.flush();time.sleep(30)")]);
                record(store, source, flag, tx, HelperSpec { command: cmd, readiness_marker: false, graceful_stdin: false })
            }));
            rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        }
        stop.store(true, Ordering::Release);
        for track in tracks {
            track.join().unwrap().unwrap();
        }
        store.set_status("recorded").unwrap();
        let before = store.snapshot().unwrap();
        assert_eq!(before.parts.len(), 2);
        assert_ne!(
            before.parts[0].confirmed_sha256,
            before.parts[1].confirmed_sha256
        );
        let folder = store.folder().to_owned();
        drop(store);
        let reopened = SessionStore::open(&folder).unwrap();
        assert!(reopened
            .snapshot()
            .unwrap()
            .parts
            .iter()
            .all(|p| p.audio_closed && p.confirmed_sample_count >= 16000));
    }
    #[test]
    fn helper_crash_finalizes_received_pcm_and_survives_reopen() {
        let root = tempfile::tempdir().unwrap();
        let store = SessionStore::create(
            root.path(),
            "Synthetic helper crash",
            "synthetic",
            "fake",
            None,
        )
        .unwrap();
        let (ready, _received) = mpsc::sync_channel(1);
        let mut command = python();
        command.args(["-c", "import sys;sys.stdout.buffer.write(bytes.fromhex('2301')*16000);sys.stdout.buffer.flush();raise SystemExit(23)"]);
        let error = record(
            store.clone(),
            "system",
            Arc::new(AtomicBool::new(false)),
            ready,
            HelperSpec {
                command,
                readiness_marker: false,
                graceful_stdin: false,
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("stopped unexpectedly"));
        let before = store.snapshot().unwrap();
        assert_eq!(before.parts.len(), 1);
        assert!(before.parts[0].audio_closed);
        assert!(before.parts[0].confirmed_sample_count >= 16000);
        let hash = before.parts[0].confirmed_sha256.clone();
        let folder = store.folder().to_owned();
        drop(store);
        let reopened = SessionStore::open(&folder).unwrap();
        assert_eq!(reopened.snapshot().unwrap().parts[0].confirmed_sha256, hash);
    }
}
