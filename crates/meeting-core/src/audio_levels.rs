//! Best-effort, bounded telemetry of effective mono PCM at the capture write gate.
//! These levels are neither disk checkpoints nor recognition-quality evidence.
use serde::Serialize;
use std::sync::Mutex;

const BUCKET_MS: u64 = 25;
const BUCKETS: usize = 12;
const STALE_MS: u64 = 1_000;
const FULL_SCALE: f64 = 32_768.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioLevelStatus {
    Active,
    Waiting,
    Stale,
    Paused,
    Muted,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioLevel {
    pub status: AudioLevelStatus,
    pub peak_dbfs: Option<f64>,
    pub rms_dbfs: Option<f64>,
    pub clipped: bool,
    pub age_ms: Option<u64>,
}

impl AudioLevel {
    fn empty(status: AudioLevelStatus, age_ms: Option<u64>) -> Self {
        Self {
            status,
            peak_dbfs: None,
            rms_dbfs: None,
            clipped: false,
            age_ms,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureAudioLevels {
    pub meeting_id: String,
    pub system: AudioLevel,
    pub microphone: AudioLevel,
}

#[derive(Clone, Copy, Default)]
struct Energy {
    peak: u32,
    squares: u64,
    samples: u64,
    clipped: bool,
}

impl Energy {
    fn from_pcm(samples: &[i16]) -> Self {
        let mut result = Self::default();
        for &sample in samples {
            let value = i32::from(sample);
            result.peak = result.peak.max(value.unsigned_abs());
            // The write path accepts at most five seconds per packet. Both the
            // i16 minimum and its square fit after widening, without abs overflow.
            result.squares = result
                .squares
                .saturating_add((i64::from(value).pow(2)) as u64);
            result.clipped |= sample == i16::MIN || sample == i16::MAX;
        }
        result.samples = samples.len() as u64;
        result
    }

    fn merge(&mut self, other: Self) {
        self.peak = self.peak.max(other.peak);
        self.squares = self.squares.saturating_add(other.squares);
        self.samples = self.samples.saturating_add(other.samples);
        self.clipped |= other.clipped;
    }

    fn levels(self, age_ms: u64) -> AudioLevel {
        let peak = f64::from(self.peak) / FULL_SCALE;
        let rms = ((self.squares as f64) / (self.samples as f64)).sqrt() / FULL_SCALE;
        AudioLevel {
            status: AudioLevelStatus::Active,
            peak_dbfs: Some(dbfs(peak)),
            rms_dbfs: Some(dbfs(rms)),
            clipped: self.clipped,
            age_ms: Some(age_ms),
        }
    }
}

fn dbfs(amplitude: f64) -> f64 {
    if amplitude > 0.0 {
        (20.0 * amplitude.log10()).clamp(-96.0, 0.0)
    } else {
        -96.0
    }
}

#[derive(Clone, Copy, Default)]
struct Bucket {
    tick: u64,
    energy: Energy,
}

struct State {
    epoch: u64,
    last_ms: u64,
    last: Energy,
    buckets: [Bucket; BUCKETS],
}

/// One track uses twelve 25 ms receipt-time buckets (no retained PCM). A sample
/// peak/clipping indication remains visible for 275–300 ms while new PCM arrives.
/// RMS weights the actual mono samples in those same buckets, not elapsed time.
/// If packets stop, retain the last real packet's values until stale at 1000 ms;
/// never fabricate silence from missing packets, device liveness or disk padding.
#[derive(Default)]
pub(super) struct AudioMeter(Mutex<Option<State>>);

impl AudioMeter {
    /// Called only after an effective PCM append succeeds. Meter lock contention
    /// or poisoning drops telemetry, never blocks or fails capture persistence.
    pub fn observe(&self, epoch: u64, samples: &[i16], now_ms: u64) {
        if samples.is_empty() {
            return;
        }
        let energy = Energy::from_pcm(samples);
        let Ok(mut state) = self.0.try_lock() else {
            return;
        };
        let state = state.get_or_insert_with(|| State {
            epoch,
            last_ms: now_ms,
            last: energy,
            buckets: [Bucket::default(); BUCKETS],
        });
        if state.epoch != epoch {
            state.epoch = epoch;
            state.buckets = [Bucket::default(); BUCKETS];
        }
        state.last_ms = now_ms;
        state.last = energy;
        let tick = now_ms / BUCKET_MS;
        let bucket = &mut state.buckets[(tick % BUCKETS as u64) as usize];
        if bucket.tick != tick {
            *bucket = Bucket {
                tick,
                energy: Energy::default(),
            };
        }
        bucket.energy.merge(energy);
    }

    /// Availability and acknowledged privacy gates take priority over telemetry.
    /// An epoch mismatch hides every old level after Resume/Unmute until new PCM.
    pub fn snapshot(
        &self,
        epoch: u64,
        now_ms: u64,
        available: bool,
        paused: bool,
        muted: bool,
    ) -> AudioLevel {
        for (matches, status) in [
            (!available, AudioLevelStatus::Unavailable),
            (paused, AudioLevelStatus::Paused),
            (muted, AudioLevelStatus::Muted),
        ] {
            if matches {
                return AudioLevel::empty(status, None);
            }
        }
        let Ok(guard) = self.0.try_lock() else {
            return AudioLevel::empty(AudioLevelStatus::Unavailable, None);
        };
        let Some(state) = guard.as_ref().filter(|state| state.epoch == epoch) else {
            return AudioLevel::empty(AudioLevelStatus::Waiting, None);
        };
        let age = now_ms.saturating_sub(state.last_ms);
        if age >= STALE_MS {
            return AudioLevel::empty(AudioLevelStatus::Stale, Some(age));
        }
        let tick = now_ms / BUCKET_MS;
        let mut energy = Energy::default();
        for bucket in &state.buckets {
            if tick.saturating_sub(bucket.tick) < BUCKETS as u64 {
                energy.merge(bucket.energy);
            }
        }
        if energy.samples == 0 {
            energy = state.last;
        }
        energy.levels(age)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(meter: &AudioMeter, time: u64) -> AudioLevel {
        meter.snapshot(0, time, true, false, false)
    }

    #[test]
    fn synthetic_fullscale_rms_clipping_and_silence_are_finite_and_distinguish_missing() {
        let meter = AudioMeter::default();
        assert_eq!(level(&meter, 0).status, AudioLevelStatus::Waiting);
        meter.observe(0, &[i16::MIN, 0], 0);
        let signal = level(&meter, 0);
        assert_eq!(signal.peak_dbfs, Some(0.0));
        assert!((signal.rms_dbfs.unwrap() - -3.010_299_956_6).abs() < 0.000_001);
        assert!(signal.clipped);
        meter.observe(0, &[0, 0], 300);
        let silence = level(&meter, 300);
        assert_eq!(silence.status, AudioLevelStatus::Active);
        assert_eq!(
            (silence.peak_dbfs, silence.rms_dbfs),
            (Some(-96.0), Some(-96.0))
        );
        assert!(!silence.clipped);
        meter.observe(0, &[i16::MAX], 600);
        let positive = level(&meter, 600);
        assert!(positive.clipped);
        assert!(positive.peak_dbfs.unwrap().is_finite());
        assert!((positive.peak_dbfs.unwrap() - -0.000_265_076).abs() < 0.000_001);
        assert_eq!(
            meter.snapshot(0, 600, false, false, false).status,
            AudioLevelStatus::Unavailable
        );
    }

    #[test]
    fn synthetic_buckets_hold_brief_peaks_without_inventing_silent_or_fresh_packets() {
        let meter = AudioMeter::default();
        meter.observe(0, &[16_384], 24);
        meter.observe(0, &[0; 3], 25);
        let held = level(&meter, 100);
        assert!((held.peak_dbfs.unwrap() - -6.020_599_913).abs() < 0.000_001);
        assert!((held.rms_dbfs.unwrap() - -12.041_199_827).abs() < 0.000_001);
        assert!(level(&meter, 299).peak_dbfs.unwrap() > -96.0);
        assert_eq!(level(&meter, 300).peak_dbfs, Some(-96.0));
        // Missing packets do not become an invented digital-silence observation.
        meter.observe(0, &[8_192], 400);
        assert_eq!(level(&meter, 1_399).age_ms, Some(999));
        assert!(level(&meter, 1_399).peak_dbfs.unwrap() > -96.0);
        let stale = level(&meter, 1_400);
        assert_eq!(stale.status, AudioLevelStatus::Stale);
        assert_eq!((stale.peak_dbfs, stale.rms_dbfs), (None, None));
        assert_eq!(stale.age_ms, Some(1_000));
    }

    #[test]
    fn synthetic_epochs_and_unavailable_telemetry_clear_levels_without_blocking() {
        let meter = AudioMeter::default();
        meter.observe(0, &[i16::MIN], 10);
        for (paused, muted, status) in [
            (true, false, AudioLevelStatus::Paused),
            (false, true, AudioLevelStatus::Muted),
        ] {
            let gated = meter.snapshot(1, 20, true, paused, muted);
            assert_eq!(gated.status, status);
            assert_eq!(
                (gated.peak_dbfs, gated.rms_dbfs, gated.age_ms),
                (None, None, None)
            );
            assert!(!gated.clipped);
        }
        assert_eq!(
            meter.snapshot(1, 20, true, false, false).status,
            AudioLevelStatus::Waiting
        );
        let held = meter.0.lock().unwrap();
        meter.observe(1, &[i16::MIN], 30); // Must return, never wait behind a reader.
        assert_eq!(level(&meter, 30).status, AudioLevelStatus::Unavailable);
        drop(held);
        assert_eq!(
            meter.snapshot(1, 30, true, false, false).status,
            AudioLevelStatus::Waiting
        );
        meter.observe(1, &[0], 40);
        assert_eq!(
            meter.snapshot(1, 40, true, false, false).peak_dbfs,
            Some(-96.0)
        );
        assert_eq!(
            level(&AudioMeter::default(), 40).status,
            AudioLevelStatus::Waiting
        );
    }

    #[test]
    fn serialized_meter_contract_contains_only_scalar_levels_and_identity() {
        let meter = AudioMeter::default();
        meter.observe(0, &[0], 0);
        let value = serde_json::to_value(CaptureAudioLevels {
            meeting_id: "synthetic".into(),
            system: level(&meter, 0),
            microphone: meter.snapshot(0, 0, false, false, false),
        })
        .unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "meetingId": "synthetic",
                "system": {"status":"active", "peakDbfs":-96.0, "rmsDbfs":-96.0, "clipped":false, "ageMs":0},
                "microphone": {"status":"unavailable", "peakDbfs":null, "rmsDbfs":null, "clipped":false, "ageMs":null}
            })
        );
    }

    #[test]
    fn poisoned_meter_drops_only_telemetry_without_panicking_or_waiting() {
        let meter = AudioMeter::default();
        let poisoned = std::panic::catch_unwind(|| {
            let _guard = meter.0.lock().unwrap();
            panic!("Synthetic meter fault");
        });
        assert!(poisoned.is_err());
        meter.observe(0, &[i16::MIN], 10);
        assert_eq!(level(&meter, 10).status, AudioLevelStatus::Unavailable);
    }
}
