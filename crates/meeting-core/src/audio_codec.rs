//! Optional external audio conversion. Originals are never replaced by this module.
use crate::{
    metadata,
    storage::{atomic_json, reject_link},
    AudioPart, Result,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
const OUTPUT_LIMIT: u64 = 1024 * 1024;
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    Flac,
    Opus,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversionProof {
    pub source_id: String,
    pub codec: Codec,
    pub lossy: bool,
    pub original_retained: bool,
    pub original_pcm_sha256: String,
    pub original_samples: u64,
    pub encoded_file: String,
    pub encoded_sha256: String,
    pub encoded_bytes: u64,
    pub decoded_pcm_sha256: String,
    pub decoded_samples: u64,
    pub sample_rate: u32,
    pub tool_sha256: String,
    pub tool_version: String,
    pub parameters: Vec<String>,
    pub decoded_duration_tolerance_samples: u64,
}
pub struct AudioTool {
    path: PathBuf,
    sha256: String,
    version: String,
}
struct OwnedProcess {
    child: Child,
    readers: Vec<JoinHandle<std::io::Result<Vec<u8>>>>,
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
    }
}
fn run(path: &Path, args: &[OsString], cancel: &AtomicBool) -> Result<Vec<u8>> {
    let mut command = Command::new(path);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().ok_or("Audio tool output unavailable")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("Audio tool diagnostics unavailable")?;
    let fault = Arc::new(AtomicBool::new(false));
    let out_fault = fault.clone();
    let err_fault = fault.clone();
    let readers = vec![
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout.take(OUTPUT_LIMIT + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > OUTPUT_LIMIT {
                out_fault.store(true, Ordering::Release);
            }
            Ok(bytes)
        }),
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stderr.take(OUTPUT_LIMIT + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > OUTPUT_LIMIT {
                err_fault.store(true, Ordering::Release);
            }
            Ok(bytes)
        }),
    ];
    let mut owned = OwnedProcess { child, readers };
    let started = Instant::now();
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err("Audio conversion cancelled; original recordings retained".into());
        }
        if fault.load(Ordering::Acquire) || started.elapsed() > Duration::from_secs(120) {
            return Err(
                "Local audio tool exceeded its time or output limit; originals retained".into(),
            );
        }
        if let Some(status) = owned.child.try_wait()? {
            if !status.success() {
                return Err("The local audio tool could not convert or decode this section; originals retained".into());
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut outputs = Vec::new();
    for reader in owned.readers.drain(..) {
        outputs.push(reader.join().map_err(|_| "Audio output reader stopped")??);
    }
    if fault.load(Ordering::Acquire) {
        return Err("Audio tool output exceeded limit".into());
    }
    Ok(outputs.remove(0))
}
fn file_digest(path: &Path) -> Result<(u64, String)> {
    reject_link(path)?;
    if !path.is_file() {
        return Err("Expected an ordinary audio/tool file".into());
    }
    let mut file = File::open(path)?;
    let mut bytes = 0u64;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        hash.update(&buffer[..n]);
    }
    Ok((bytes, format!("{:x}", hash.finalize())))
}
fn pcm_digest(path: &Path) -> Result<(u64, String)> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.sample_rate != 16000
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err("Decoded archive audio is not the expected PCM format".into());
    }
    let mut hash = Sha256::new();
    let mut count = 0u64;
    for sample in reader.samples::<i16>() {
        count += 1;
        if count > 16000 * 121 {
            return Err("Decoded audio exceeds section limit".into());
        }
        hash.update(sample?.to_le_bytes());
    }
    Ok((count, format!("{:x}", hash.finalize())))
}
impl AudioTool {
    /// Decode only a previously verified app-owned copy against the immutable source section.
    /// The caller writes a NEW session, never an older source or transcript journal.
    pub fn decode_copy(
        &self,
        folder: &Path,
        proof: &ConversionProof,
        part: &AudioPart,
        cancel: &AtomicBool,
    ) -> Result<Vec<i16>> {
        metadata::checked_directory(folder)?;
        let extension = if proof.codec == Codec::Flac {
            "flac"
        } else {
            "opus"
        };
        let expected = format!("{}.{extension}", part.file.trim_end_matches(".wav"));
        if proof.source_id != part.file
            || proof.encoded_file != expected
            || Path::new(&expected).components().count() != 1
            || proof.original_pcm_sha256 != part.confirmed_sha256
            || proof.original_samples != part.confirmed_sample_count
            || !part.audio_closed
            || part.confirmed_sample_count == 0
            || part.confirmed_sample_count > 16000 * 120
            || proof.sample_rate != 16000
            || proof.lossy != (proof.codec == Codec::Opus)
        {
            return Err("Converted copy does not match its original confirmed section".into());
        }
        let input = folder.join(expected);
        let (size, hash) = file_digest(&input)?;
        if size != proof.encoded_bytes || hash != proof.encoded_sha256 {
            return Err("Converted audio changed; new transcription blocked".into());
        }
        let temporary = tempfile::tempdir_in(folder)?;
        let output = temporary.path().join("decoded.wav");
        let mut args: Vec<OsString> = [
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        args.push(input.as_os_str().into());
        args.extend(
            [
                "-map",
                "0:a:0",
                "-vn",
                "-sn",
                "-dn",
                "-ac",
                "1",
                "-ar",
                "16000",
                "-c:a",
                "pcm_s16le",
                "-threads",
                "1",
                "-n",
            ]
            .into_iter()
            .map(Into::into),
        );
        args.push(output.as_os_str().into());
        run(&self.path, &args, cancel)?;
        let (samples, sha) = pcm_digest(&output)?;
        // Same duration contract as `convert`: the proof's tolerance (0 for FLAC, at most one
        // 20 ms Opus frame), with the hash checked on the untrimmed decoder output.
        let allowed = if proof.codec == Codec::Flac { 0 } else { 320 };
        if samples != proof.decoded_samples
            || sha != proof.decoded_pcm_sha256
            || proof.decoded_duration_tolerance_samples > allowed
            || samples.abs_diff(part.confirmed_sample_count)
                > proof.decoded_duration_tolerance_samples
            || proof.codec == Codec::Flac && sha != part.confirmed_sha256
        {
            return Err("Decoded copy failed its saved PCM or exact duration check; original session retained".into());
        }
        let mut reader = hound::WavReader::open(output)?;
        let pcm: std::result::Result<Vec<i16>, _> = reader.samples::<i16>().collect();
        let mut pcm = pcm?;
        // Section times stay those of the confirmed original.
        pcm.resize(part.confirmed_sample_count as usize, 0);
        Ok(pcm)
    }
    pub fn inspect(path: &Path, cancel: &AtomicBool) -> Result<Self> {
        let path = path.canonicalize()?;
        let (_, sha256) = file_digest(&path)?;
        let bytes = run(&path, &["-version".into()], cancel)?;
        let text = String::from_utf8(bytes)?;
        let version = text
            .lines()
            .next()
            .filter(|s| s.starts_with("ffmpeg version ") && s.len() < 512)
            .ok_or("This tool is not a recognized FFmpeg installation")?
            .to_owned();
        Ok(Self {
            path,
            sha256,
            version,
        })
    }
    pub fn convert(
        &self,
        folder: &Path,
        part: &AudioPart,
        output: &Path,
        codec: Codec,
        cancel: &AtomicBool,
    ) -> Result<ConversionProof> {
        metadata::checked_directory(folder)?;
        metadata::checked_directory(output)?;
        crate::read_closed_audio(folder, part)?;
        let source = crate::storage::safe_audio_path(folder, &part.file)?;
        let extension = if codec == Codec::Flac { "flac" } else { "opus" };
        let encoded_file = format!("{}.{extension}", part.file.trim_end_matches(".wav"));
        let encoded = output.join(&encoded_file);
        if encoded.exists() {
            return Err("Existing converted audio preserved; choose a new revision".into());
        }
        let parameters: Vec<String> = if codec == Codec::Flac {
            vec!["-c:a", "flac", "-compression_level", "5"]
        } else {
            vec![
                "-c:a",
                "libopus",
                "-b:a",
                "32k",
                "-vbr",
                "on",
                "-application",
                "voip",
            ]
        }
        .into_iter()
        .map(str::to_owned)
        .collect();
        let mut args: Vec<OsString> = [
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        args.push(source.as_os_str().into());
        args.extend(
            [
                "-map", "0:a:0", "-vn", "-sn", "-dn", "-ac", "1", "-ar", "16000", "-threads", "1",
            ]
            .into_iter()
            .map(OsString::from),
        );
        args.extend(parameters.iter().map(OsString::from));
        args.push("-n".into());
        args.push(encoded.as_os_str().into());
        run(&self.path, &args, cancel)?;
        let (encoded_bytes, encoded_sha256) = file_digest(&encoded)?;
        if encoded_bytes > part.confirmed_sample_count * 2 + 1024 * 1024 {
            return Err("Converted section exceeds expected size; original retained".into());
        }
        let temporary = tempfile::tempdir_in(output)?;
        let decoded = temporary.path().join("decoded.wav");
        let mut args: Vec<OsString> = [
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        args.push(encoded.as_os_str().into());
        args.extend(
            [
                "-map",
                "0:a:0",
                "-vn",
                "-ac",
                "1",
                "-ar",
                "16000",
                "-c:a",
                "pcm_s16le",
                "-threads",
                "1",
                "-n",
            ]
            .into_iter()
            .map(OsString::from),
        );
        args.push(decoded.as_os_str().into());
        run(&self.path, &args, cancel)?;
        let (decoded_samples, decoded_pcm_sha256) = pcm_digest(&decoded)?;
        // Friendly external edits during conversion must not be attributed to an older checkpoint.
        crate::read_closed_audio(folder, part)?;
        let tolerance = if codec == Codec::Flac { 0 } else { 320 };
        if decoded_samples.abs_diff(part.confirmed_sample_count) > tolerance
            || codec == Codec::Flac && decoded_pcm_sha256 != part.confirmed_sha256
        {
            return Err("Converted audio did not pass its declared PCM/duration round-trip check; original retained".into());
        }
        let proof = ConversionProof {
            source_id: part.file.clone(),
            codec,
            lossy: codec == Codec::Opus,
            original_retained: true,
            original_pcm_sha256: part.confirmed_sha256.clone(),
            original_samples: part.confirmed_sample_count,
            encoded_file: encoded_file.clone(),
            encoded_sha256,
            encoded_bytes,
            decoded_pcm_sha256,
            decoded_samples,
            sample_rate: 16000,
            tool_sha256: self.sha256.clone(),
            tool_version: self.version.clone(),
            parameters,
            decoded_duration_tolerance_samples: tolerance,
        };
        atomic_json(&output.join(format!("{encoded_file}.proof.json")), &proof)?;
        Ok(proof)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn open_or_changed_audio_cannot_be_sent_to_a_conversion_tool() {
        let root = tempfile::tempdir().unwrap();
        let store =
            crate::SessionStore::create(root.path(), "Synthetic", "fixture-model", "system", None)
                .unwrap();
        let mut writer = crate::PcmWriter::new(store.clone(), "system").unwrap();
        writer.append(&[42; 16000]).unwrap();
        writer.checkpoint().unwrap();
        let part = store.snapshot().unwrap().parts[0].clone();
        let out = root.path().join("converted");
        fs::create_dir(&out).unwrap();
        let tool = AudioTool {
            path: PathBuf::from("must-not-run"),
            sha256: String::new(),
            version: String::new(),
        };
        assert!(tool
            .convert(
                store.folder(),
                &part,
                &out,
                Codec::Flac,
                &AtomicBool::new(false)
            )
            .is_err());
        writer.finish().unwrap();
        let mut part = store.snapshot().unwrap().parts[0].clone();
        part.confirmed_sha256 = "0".repeat(64);
        assert!(tool
            .convert(
                store.folder(),
                &part,
                &out,
                Codec::Opus,
                &AtomicBool::new(false)
            )
            .is_err());
        assert_eq!(fs::read_dir(out).unwrap().count(), 0);
    }
}
