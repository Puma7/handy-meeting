//! Native human choices create in-memory, purpose-bound file grants. IPC may use
//! the selected canonical file, but cannot approve a path or replacement bytes.
use crate::{blocking, operations, AppResult, Service};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::State;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Purpose {
    Handy,
    Ffmpeg,
    Legacy,
}
impl Purpose {
    /// Return purpose-specific native confirmation text without treating a supplied path as
    /// approval.
    fn description(self) -> &'static str {
        match self {
            Self::Handy => "Use this Handy program for local model discovery? / Dieses Handy-Programm für die lokale Modellabfrage verwenden?",
            Self::Ffmpeg => "Use this FFmpeg program for optional local audio conversion? / Dieses FFmpeg-Programm für optionale lokale Audiokonvertierung verwenden?",
            Self::Legacy => "Read this original session manifest? Originals will stay unchanged. / Diese ursprüngliche Sitzungsdatei lesen? Originale bleiben unverändert.",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Grant {
    path: PathBuf,
    bytes: u64,
    sha256: String,
}
#[derive(Default)]
pub(crate) struct Selections {
    native: Mutex<HashMap<Purpose, Grant>>,
    // Origin only: an OS-discovered installation can be rediscovered after an
    // update. It is never a grant for a renderer-supplied explicit path.
    discovered_handy: Mutex<Option<PathBuf>>,
}

/// Validate an absolute ordinary local file and bind canonical path, size and SHA-256 to a
/// purpose. Linked final files are rejected; inspection grants no renderer authority by itself.
fn inspect(purpose: Purpose, path: &Path) -> AppResult<Grant> {
    if !path.is_absolute() || path.to_string_lossy().chars().any(char::is_control) {
        return Err("Choose a local file through the native picker. / Bitte eine lokale Datei im Systemdialog wählen.".into());
    }
    let path = if purpose == Purpose::Handy && path.extension().is_some_and(|ext| ext == "app") {
        ["Handy", "handy"]
            .into_iter()
            .map(|name| path.join("Contents/MacOS").join(name))
            .find(|p| p.is_file())
            .ok_or("Handy bundle has no program")?
    } else {
        path.to_owned()
    };
    let metadata = fs::symlink_metadata(&path).map_err(|_| "The selected file is unavailable")?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Linked selected files are rejected".into());
        }
    }
    let limit = if purpose == Purpose::Legacy {
        32 * 1024 * 1024
    } else {
        2 * 1024 * 1024 * 1024
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > limit
    {
        return Err("The selected file is not a supported ordinary local file".into());
    }
    if purpose == Purpose::Handy && !crate::plausible_handy_path(&path) {
        return Err("Choose the Handy program itself".into());
    }
    if purpose == Purpose::Ffmpeg
        && !path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            n.eq_ignore_ascii_case("ffmpeg") || n.eq_ignore_ascii_case("ffmpeg.exe")
        })
    {
        return Err("Choose the FFmpeg program itself".into());
    }
    if purpose == Purpose::Legacy && path.file_name().is_none_or(|n| n != "session.json") {
        return Err("Choose the original session.json manifest".into());
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| "The selected file cannot be resolved")?;
    let mut file = File::open(&canonical).map_err(|_| "The selected file cannot be opened")?;
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "The selected file cannot be verified")?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        if bytes > limit {
            return Err("The selected file exceeds the verification limit".into());
        }
        hash.update(&buffer[..count]);
    }
    if bytes != metadata.len() {
        return Err("The selected file changed during verification; choose it again".into());
    }
    Ok(Grant {
        path: canonical,
        bytes,
        sha256: format!("{:x}", hash.finalize()),
    })
}
impl Selections {
    /// Require an existing native grant before reading an unrelated renderer path; reinspection
    /// rejects purpose reuse, changed bytes and different canonical files.
    pub(crate) fn verify(&self, purpose: Purpose, path: &Path) -> AppResult<()> {
        let approved = self.native.lock().map_err(|_| "Native file selections are unavailable")?.get(&purpose).cloned().ok_or("Choose and confirm this file in the native system dialog first. / Bitte diese Datei zuerst im Systemdialog auswählen und bestätigen.")?;
        // Refuse an unrelated renderer path before opening or hashing its contents.
        if path.canonicalize().ok().as_ref() != Some(&approved.path) {
            return Err("This path was not selected for this operation. Choose it in the native system dialog first. / Dieser Pfad wurde für diesen Vorgang nicht ausgewählt. Bitte zuerst den Systemdialog verwenden.".into());
        }
        let actual = inspect(purpose, path)?;
        if actual != approved {
            return Err("The selected path or file changed. Choose and confirm it again; nothing was started. / Die gewählte Datei wurde geändert. Bitte erneut im Systemdialog bestätigen; es wurde nichts gestartet.".into());
        }
        Ok(())
    }
    /// Retain a verified purpose-bound grant for this Service lifetime and return its canonical
    /// display path.
    fn remember(&self, purpose: Purpose, selected: Grant) -> AppResult<String> {
        let path = selected.path.to_string_lossy().into_owned();
        self.native
            .lock()
            .map_err(|_| "Native file selections are unavailable")?
            .insert(purpose, selected);
        Ok(path)
    }
    /// Leave denial unapproved and rehash accepted bytes after human confirmation; remember only
    /// an unchanged purpose-bound file.
    fn finish_native_choice(
        &self,
        purpose: Purpose,
        grant: Grant,
        approved: bool,
    ) -> AppResult<Option<String>> {
        if !approved {
            return Ok(None);
        }
        if inspect(purpose, &grant.path)? != grant {
            return Err("The file changed during confirmation; choose it again".into());
        }
        self.remember(purpose, grant).map(Some)
    }
    /// Record an inspected OS-discovered origin without granting permission for explicit renderer
    /// paths.
    pub(crate) fn remember_discovered_handy(&self, path: &Path) -> AppResult<()> {
        let verified = inspect(Purpose::Handy, path)?;
        *self
            .discovered_handy
            .lock()
            .map_err(|_| "Handy discovery state is unavailable")? = Some(verified.path);
        Ok(())
    }
    /// Rediscover OS installations but strictly revalidate native choices. Invalid explicit
    /// requests clear discovery fallback instead of reusing earlier authority.
    pub(crate) fn handy_discovery_path(
        &self,
        explicit: Option<&Path>,
        cached: Option<&Path>,
    ) -> AppResult<Option<PathBuf>> {
        if let Some(path) = explicit {
            // An explicit request, even an invalid one, must not fall back to
            // an earlier automatically discovered executable on the next call.
            *self
                .discovered_handy
                .lock()
                .map_err(|_| "Handy discovery state is unavailable")? = None;
            self.verify(Purpose::Handy, path)?;
            return Ok(Some(path.to_owned()));
        }
        let Some(path) = cached else {
            return Ok(None);
        };
        let discovered = self
            .discovered_handy
            .lock()
            .map_err(|_| "Handy discovery state is unavailable")?
            .as_deref()
            == Some(path);
        if discovered {
            // Search the known installation locations again rather than reusing
            // an old fingerprint/path when Handy has installed an update.
            Ok(None)
        } else {
            self.verify(Purpose::Handy, path)?;
            Ok(Some(path.to_owned()))
        }
    }
    /// Only internal synthetic CLI fixtures use this; this is not an IPC command.
    pub(crate) fn approve_generated_legacy_fixture(&self, path: &Path) -> AppResult<()> {
        self.remember(Purpose::Legacy, inspect(Purpose::Legacy, path)?)?;
        Ok(())
    }
}

/// Hold exclusive selection admission through the native picker and confirmation, then recheck
/// bytes before remembering a purpose-bound grant.
async fn choose(
    service: Arc<Service>,
    app: tauri::AppHandle,
    purpose: Purpose,
) -> AppResult<Option<String>> {
    blocking(move || {
        let _lease = service.reserve_operation(operations::Kind::Selection)?;
        // blocking_pick_file and blocking_show run on this worker, never the UI thread.
        let selected = app.dialog().file().set_title(purpose.description()).blocking_pick_file();
        let Some(selected) = selected else {return Ok(None);};
        let path = selected.into_path().map_err(|_| "Only a local filesystem choice is supported")?;
        let grant = inspect(purpose,&path)?;
        let approved = app.dialog().message(format!("{}\n\n{}\n\nSHA-256: {}\n\n{}",purpose.description(),grant.path.display(),grant.sha256,"Only continue if you trust this local file. / Nur fortfahren, wenn Sie dieser lokalen Datei vertrauen.")).title("Meeting for Handy").kind(MessageDialogKind::Warning).buttons(MessageDialogButtons::OkCancel).blocking_show();
        // A file replaced while the human reads the native prompt is not approved.
        service.selections.finish_native_choice(purpose,grant,approved)
    }).await
}

/// Offer native confirmation of a Handy executable; cancellation returns no selected path.
#[tauri::command]
pub(crate) async fn choose_handy_executable(
    service: State<'_, Arc<Service>>,
    app: tauri::AppHandle,
) -> AppResult<Option<String>> {
    choose(Arc::clone(&service), app, Purpose::Handy).await
}
/// Offer native confirmation of separately installed FFmpeg for optional local audio conversion.
#[tauri::command]
pub(crate) async fn choose_ffmpeg_executable(
    service: State<'_, Arc<Service>>,
    app: tauri::AppHandle,
) -> AppResult<Option<String>> {
    choose(Arc::clone(&service), app, Purpose::Ffmpeg).await
}
/// Offer native confirmation of an original session manifest for read-only import.
#[tauri::command]
pub(crate) async fn choose_legacy_manifest(
    service: State<'_, Arc<Service>>,
    app: tauri::AppHandle,
) -> AppResult<Option<String>> {
    choose(Arc::clone(&service), app, Purpose::Legacy).await
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Verify relative program and manifest paths fail inspection and cannot supply native
    /// authority.
    #[test]
    fn relative_paths_never_become_native_file_grants() {
        for (purpose, path) in [
            (Purpose::Handy, "Handy.exe"),
            (Purpose::Ffmpeg, "ffmpeg"),
            (Purpose::Legacy, "legacy-fixture/session.json"),
        ] {
            assert!(inspect(purpose, Path::new(path)).is_err());
            assert!(Selections::default()
                .verify(purpose, Path::new(path))
                .is_err());
        }
    }
    /// Verify synthetic discovered updates trigger rediscovery while explicit replacements
    /// require new approval.
    #[test]
    fn discovered_handy_updates_rediscover_without_becoming_native_path_grants() {
        let root = std::env::temp_dir().join(format!(
            "meeting-discovered-update-{}",
            meeting_notes::new_job_id()
        ));
        fs::create_dir(&root).unwrap();
        let path = root.join("Handy.exe");
        fs::write(&path, b"synthetic installed v1; never executed").unwrap();
        let path = path.canonicalize().unwrap();
        let selections = Selections::default();
        selections.remember_discovered_handy(&path).unwrap();
        assert_eq!(
            selections.handy_discovery_path(None, Some(&path)).unwrap(),
            None
        );
        assert!(
            selections.verify(Purpose::Handy, &path).is_err(),
            "Auto discovery is not native consent"
        );
        fs::write(&path, b"synthetic installed v2; never executed").unwrap();
        assert_eq!(selections.handy_discovery_path(None,Some(&path)).unwrap(),None,"Updated discovered bytes must trigger fresh OS discovery, not a stale native hash check");
        selections.remember_discovered_handy(&path).unwrap();
        assert!(
            selections
                .handy_discovery_path(Some(&path), Some(&path))
                .is_err(),
            "Explicit renderer paths still require real native approval"
        );
        assert!(
            selections.handy_discovery_path(None, Some(&path)).is_err(),
            "An invalid explicit choice cannot silently revert to automatic trust"
        );
        let grant = inspect(Purpose::Handy, &path).unwrap();
        selections
            .finish_native_choice(Purpose::Handy, grant, true)
            .unwrap();
        assert_eq!(
            selections
                .handy_discovery_path(Some(&path), Some(&path))
                .unwrap(),
            Some(path.clone())
        );
        assert_eq!(
            selections.handy_discovery_path(None, Some(&path)).unwrap(),
            Some(path.clone())
        );
        fs::write(&path, b"synthetic installed v3; never executed").unwrap();
        assert!(
            selections.handy_discovery_path(None, Some(&path)).is_err(),
            "A native-confirmed file replacement still needs new consent"
        );
    }
    /// Verify a removed discovered binary does not block rediscovery or authorize an explicit new
    /// path.
    #[test]
    fn removed_discovered_handy_path_can_be_rediscovered_at_a_new_installation() {
        let root = std::env::temp_dir().join(format!(
            "meeting-discovered-move-{}",
            meeting_notes::new_job_id()
        ));
        fs::create_dir(&root).unwrap();
        let old = root.join("Handy-old.exe");
        fs::write(&old, b"synthetic old install").unwrap();
        let old = old.canonicalize().unwrap();
        let selections = Selections::default();
        selections.remember_discovered_handy(&old).unwrap();
        fs::remove_file(&old).unwrap();
        assert_eq!(
            selections.handy_discovery_path(None, Some(&old)).unwrap(),
            None,
            "Missing old binary must not block fresh system discovery"
        );
        let new = root.join("Handy-new.exe");
        fs::write(&new, b"synthetic newer install").unwrap();
        let new = new.canonicalize().unwrap();
        selections.remember_discovered_handy(&new).unwrap();
        assert_eq!(
            selections.handy_discovery_path(None, Some(&new)).unwrap(),
            None
        );
        assert!(
            selections.verify(Purpose::Handy, &new).is_err(),
            "Discovery must not confer renderer path authority"
        );
    }
    /// Verify denial and replacement during simulated confirmation leave execution authority
    /// absent.
    #[test]
    fn native_denial_or_change_while_confirming_never_creates_a_grant() {
        let root = std::env::temp_dir().join(format!(
            "meeting-denied-choice-{}",
            meeting_notes::new_job_id()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("ffmpeg.exe");
        fs::write(&path, b"synthetic original").unwrap();
        let selections = Selections::default();
        let grant = inspect(Purpose::Ffmpeg, &path).unwrap();
        assert!(selections
            .finish_native_choice(Purpose::Ffmpeg, grant, false)
            .unwrap()
            .is_none());
        assert!(selections.verify(Purpose::Ffmpeg, &path).is_err());
        let grant = inspect(Purpose::Ffmpeg, &path).unwrap();
        fs::write(&path, b"synthetic replacement").unwrap();
        assert!(selections
            .finish_native_choice(Purpose::Ffmpeg, grant, true)
            .is_err());
        assert!(selections.verify(Purpose::Ffmpeg, &path).is_err());
    }
    /// Verify synthetic grants reject missing approval, purpose reuse and replaced bytes without
    /// executing programs.
    #[test]
    fn absent_or_wrong_purpose_and_replaced_bytes_never_authorize_execution() {
        let root = std::env::temp_dir().join(format!(
            "meeting-selection-{}-{}",
            std::process::id(),
            meeting_notes::new_job_id()
        ));
        fs::create_dir(&root).unwrap();
        let handy = root.join("Handy.exe");
        fs::write(&handy, b"synthetic first program; never executed").unwrap();
        let selections = Selections::default();
        assert!(selections.verify(Purpose::Handy, &handy).is_err());
        selections
            .remember(Purpose::Handy, inspect(Purpose::Handy, &handy).unwrap())
            .unwrap();
        assert!(selections.verify(Purpose::Handy, &handy).is_ok());
        assert!(selections.verify(Purpose::Ffmpeg, &handy).is_err());
        fs::write(&handy, b"synthetic replaced program; never executed").unwrap();
        assert!(selections.verify(Purpose::Handy, &handy).is_err());
        assert_eq!(
            fs::read(&handy).unwrap(),
            b"synthetic replaced program; never executed"
        );
    }
    /// Verify synthetic manifest approval is limited to its canonical file and current selection
    /// store.
    #[test]
    fn an_unconfirmed_legacy_path_is_not_read_and_a_grant_is_not_transferable() {
        let root = std::env::temp_dir().join(format!(
            "meeting-import-choice-{}",
            meeting_notes::new_job_id()
        ));
        fs::create_dir_all(&root).unwrap();
        let first = root.join("session.json");
        fs::write(&first, b"synthetic original; never imported").unwrap();
        let other = root.join("other");
        fs::create_dir(&other).unwrap();
        let second = other.join("session.json");
        fs::write(&second, b"synthetic original; never imported").unwrap();
        let selections = Selections::default();
        assert!(selections.verify(Purpose::Legacy, &first).is_err());
        selections
            .remember(Purpose::Legacy, inspect(Purpose::Legacy, &first).unwrap())
            .unwrap();
        assert!(selections.verify(Purpose::Legacy, &first).is_ok());
        assert!(selections.verify(Purpose::Legacy, &second).is_err());
        assert!(
            Selections::default()
                .verify(Purpose::Legacy, &first)
                .is_err(),
            "Grants are session-only"
        );
    }
}
