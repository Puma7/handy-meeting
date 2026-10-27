//! Explicit installed-tool check. Invented tones only; no microphone, user audio or model.
use meeting_core::{
    audio_codec::{AudioTool, Codec},
    PcmWriter, SessionStore,
};
use std::{fs, path::PathBuf, sync::atomic::AtomicBool, time::Instant};
fn main() -> meeting_core::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let executable = PathBuf::from(args.next().ok_or("Provide installed FFmpeg path")?);
    let root = PathBuf::from(args.next().ok_or("Provide a NEW check folder")?);
    if root.exists() {
        return Err("Check folder must be new; earlier evidence retained".into());
    }
    fs::create_dir_all(&root)?;
    let started = Instant::now();
    let cancel = AtomicBool::new(false);
    let tool = AudioTool::inspect(&executable, &cancel)?;
    let store = SessionStore::create(
        &root.join("sessions"),
        "Synthetic codec reference",
        "no-recognition-model",
        "synthetic",
        None,
    )?;
    let mut writer = PcmWriter::new(store.clone(), "system")?;
    let pcm: Vec<i16> = (0..48000)
        .map(|n| {
            let t = n as f64 / 16000.0;
            (8000.0 * (std::f64::consts::TAU * 220.0 * t).sin()
                + 2000.0 * (std::f64::consts::TAU * 880.0 * t).sin()) as i16
        })
        .collect();
    writer.append(&pcm)?;
    writer.finish()?;
    drop(writer);
    let part = store.snapshot()?.parts[0].clone();
    let source = store.audio_path(&part)?;
    let original = fs::read(&source)?;
    let mut proofs = Vec::new();
    for (name, codec) in [
        ("lossless-flac", Codec::Flac),
        ("compact-opus", Codec::Opus),
    ] {
        let output = root.join(name);
        fs::create_dir(&output)?;
        let proof = tool.convert(store.folder(), &part, &output, codec, &cancel)?;
        let decoded = tool.decode_copy(&output, &proof, &part, &cancel)?;
        assert_eq!(decoded.len(), pcm.len());
        if codec == Codec::Flac {
            assert_eq!(decoded, pcm);
        }
        let mut mismatched = proof.clone();
        mismatched.original_pcm_sha256 = "0".repeat(64);
        assert!(tool
            .decode_copy(&output, &mismatched, &part, &cancel)
            .is_err());
        let encoded = output.join(&proof.encoded_file);
        let saved = fs::read(&encoded)?;
        fs::write(&encoded, b"changed synthetic copy")?;
        assert!(tool.decode_copy(&output, &proof, &part, &cancel).is_err());
        fs::write(&encoded, saved)?;
        assert_eq!(fs::read(&source)?, original);
        assert!(proof.original_retained);
        if codec == Codec::Flac {
            assert_eq!(proof.decoded_pcm_sha256, proof.original_pcm_sha256);
        }
        proofs.push(proof);
    }
    let report = serde_json::json!({"success":true,"version":env!("CARGO_PKG_VERSION"),"actualInstalledFfmpeg":true,"verifiedCopyReimport":true,"changedCopyAndSourceRejected":true,"syntheticToneSeconds":3,"userAudioRead":false,"audioDevicesOpened":false,"asrModelUsed":false,"elapsedSeconds":started.elapsed().as_secs_f64(),"proofs":proofs,"limits":"PCM/codec/duration round-trip and verified reimport only. Opus is lossy. This does not establish speech intelligibility, recognition accuracy, real meeting compression ratio, hardware capture or endurance."});
    fs::write(
        root.join("reference.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
