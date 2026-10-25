//! Read-only smoke test of an explicitly selected Handy installation. Does not
//! load a model, record audio, download weights or persist private metadata.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let explicit = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let bridge = handy_bridge::HandyBridge::discover(explicit.as_deref())?;
    let catalog = bridge.refresh()?;
    let installed:Vec<_>=catalog.installed_streaming_models().map(|model| {
        let resolved=bridge.resolve_streaming_model(model);
        serde_json::json!({"id":model.id,"resolved":resolved.is_ok(),"resolution":resolved.as_ref().ok().map(|r|&r.resolution),"error":resolved.err().map(|e|e.to_string())})
    }).collect();
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"modelCount":catalog.models.len(),"installedStreaming":installed,"catalogFingerprint":catalog.fingerprint_sha256})
        )?
    );
    Ok(())
}
