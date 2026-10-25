//! Synthetic two-track data, accelerated by default. Opens no audio device.
use meeting_core::{PcmWriter, SessionStore, TranscriptCheckpoint, SAMPLE_RATE};
use std::{env, path::Path};

fn main() -> meeting_core::Result<()> {
    let args: Vec<_> = env::args().collect();
    let root = args
        .get(1)
        .ok_or("Usage: synthetic_capture OUTPUT_ROOT [seconds]")?;
    let seconds: u32 = args.get(2).map(String::as_str).unwrap_or("2").parse()?;
    if !(1..=300).contains(&seconds) {
        return Err("Synthetic fixture duration must be 1..300 seconds".into());
    }
    let store = SessionStore::create(
        Path::new(root),
        "Synthetic two-track fixture",
        "synthetic-no-model",
        "generated-system",
        Some("generated-microphone"),
    )?;
    for (source, frequency) in [("system", 440.0), ("microphone", 660.0)] {
        let mut writer = PcmWriter::new(store.clone(), source)?;
        for second in 0..seconds {
            let samples: Vec<_> = (0..SAMPLE_RATE)
                .map(|index| {
                    ((std::f64::consts::TAU * frequency * f64::from(second * SAMPLE_RATE + index)
                        / f64::from(SAMPLE_RATE))
                    .sin()
                        * 4096.0) as i16
                })
                .collect();
            writer.append(&samples)?;
        }
        writer.finish()?;
        for part in store
            .snapshot()?
            .parts
            .iter()
            .filter(|part| part.source == source)
        {
            store.append_transcript(TranscriptCheckpoint {
                sequence: 0,
                at: String::new(),
                file: part.file.clone(),
                model_id: "synthetic-no-model".into(),
                full: format!("Synthetic {source} checkpoint."),
                committed: format!("Synthetic {source} checkpoint."),
                tentative: String::new(),
                processed_samples: part.confirmed_sample_count,
                final_text: true,
            })?;
        }
    }
    store.set_status("recorded")?;
    println!("{}", store.folder().display());
    Ok(())
}
