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
pub(crate) struct Selections(Mutex<HashMap<Purpose, Grant>>);

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
    pub(crate) fn verify(&self, purpose: Purpose, path: &Path) -> AppResult<()> {
        let approved = self.0.lock().map_err(|_| "Native file selections are unavailable")?.get(&purpose).cloned().ok_or("Choose and confirm this file in the native system dialog first. / Bitte diese Datei zuerst im Systemdialog auswählen und bestätigen.")?;
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
    fn remember(&self, purpose: Purpose, selected: Grant) -> AppResult<String> {
        let path = selected.path.to_string_lossy().into_owned();
        self.0
            .lock()
            .map_err(|_| "Native file selections are unavailable")?
            .insert(purpose, selected);
        Ok(path)
    }
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
    pub(crate) fn remember_discovered_handy(&self, path: &Path) -> AppResult<()> {
        self.remember(Purpose::Handy, inspect(Purpose::Handy, path)?)?;
        Ok(())
    }
    /// Only internal synthetic CLI fixtures use this; this is not an IPC command.
    pub(crate) fn approve_generated_legacy_fixture(&self, path: &Path) -> AppResult<()> {
        self.remember(Purpose::Legacy, inspect(Purpose::Legacy, path)?)?;
        Ok(())
    }
}

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

#[tauri::command]
pub(crate) async fn choose_handy_executable(
    service: State<'_, Arc<Service>>,
    app: tauri::AppHandle,
) -> AppResult<Option<String>> {
    choose(Arc::clone(&service), app, Purpose::Handy).await
}
#[tauri::command]
pub(crate) async fn choose_ffmpeg_executable(
    service: State<'_, Arc<Service>>,
    app: tauri::AppHandle,
) -> AppResult<Option<String>> {
    choose(Arc::clone(&service), app, Purpose::Ffmpeg).await
}
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
