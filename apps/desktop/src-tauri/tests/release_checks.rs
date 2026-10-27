//! The real CLI must accept the same relative output used by portable packaging.
//! All meeting/audio contents here are generated fixtures; no devices or models are used.
use std::{fs, path::Path, process::Command};

#[test]
fn release_cli_accepts_relative_output_from_an_isolated_working_directory() {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("Desktop package is inside the source checkout");
    let work = checkout
        .join(".build")
        .join("cli-relative-output-tests")
        .join(meeting_notes::new_job_id());
    fs::create_dir_all(&work).unwrap();
    let work = work.canonicalize().unwrap();
    let relative = Path::new(".build").join("portable-release-checks");
    // A per-child cwd avoids racing other Rust tests or trusting caller-relative paths
    // at the renderer/native picker boundary.
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_HandyMeeting"))
            .current_dir(&work)
            .arg("--release-checks")
            .arg(&relative)
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "real CLI failed from {}: stdout={} stderr={}",
        work.display(),
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    let output = work.join(&relative);
    let evidence_path = output.join("release-checks.json");
    let evidence_bytes = fs::read(&evidence_path).unwrap();
    let evidence: serde_json::Value = serde_json::from_slice(&evidence_bytes).unwrap();
    assert_eq!(evidence["success"], true);
    assert_eq!(evidence["applicationVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(evidence["synthetic"], true);
    assert_eq!(evidence["installedMetadataQueried"], false);
    assert_eq!(evidence["realCaptureOpened"], false);
    assert_eq!(evidence["nativeRuntimeMatchesPinnedBindings"], true);
    assert_eq!(evidence["legacyOriginalUnchanged"], true);
    assert_eq!(evidence["legacyMappingSurvivedRestart"], true);
    assert_eq!(evidence["fullCopyParts"], 25);
    let manifest = output.join("legacy-fixture/session.json");
    let original = fs::read(&manifest).unwrap();
    let second = run();
    assert!(
        !second.status.success(),
        "Prior evidence must never be reused"
    );
    assert!(String::from_utf8_lossy(&second.stderr).contains("already exists"));
    assert_eq!(fs::read(&evidence_path).unwrap(), evidence_bytes);
    assert_eq!(fs::read(&manifest).unwrap(), original);
    // Keep this uniquely named synthetic evidence in the ignored build area for inspection.
}
