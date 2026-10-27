//! Explicit official-release downloads. No automatic installation or shell command strings.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, String>;
const LATEST: &str = "https://api.github.com/repos/Puma7/handy-meeting/releases/latest";
const MAX_INSTALLER: u64 = 1024 * 1024 * 1024;
#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct View {
    pub status: String,
    pub version: Option<String>,
    pub release_url: Option<String>,
    pub received_bytes: u64,
    pub total_bytes: u64,
    pub elapsed_ms: u64,
    pub error: Option<String>,
    /// Increases with every check or download, so the UI can scope a dismissal to one attempt.
    pub attempt: u64,
}
#[derive(Clone)]
struct Candidate {
    version: String,
    url: String,
    sha256: String,
    bytes: u64,
}
#[derive(Default)]
struct Inner {
    view: View,
    candidate: Option<Candidate>,
    ready: Option<PathBuf>,
    started: Option<Instant>,
}
#[derive(Default)]
pub(super) struct Updater {
    inner: Mutex<Inner>,
    cancel: AtomicBool,
}
pub(super) struct InstallerLease {
    pub path: PathBuf,
    _file: fs::File,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    html_url: String,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    size: u64,
    digest: Option<String>,
    browser_download_url: String,
}
fn version(value: &str) -> Result<(u32, u32, u32)> {
    let parts = value
        .split('.')
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| "Unsupported release version")?;
    if parts.len() != 3 {
        return Err("Unsupported release version".into());
    }
    Ok((parts[0], parts[1], parts[2]))
}
fn candidate(release: Release, current: &str) -> Result<Option<Candidate>> {
    let tag = release
        .tag_name
        .strip_prefix('v')
        .ok_or("Unexpected release tag")?;
    let newer = version(tag)? > version(current)?;
    if release.draft
        || release.prerelease
        || release.html_url
            != format!(
                "https://github.com/Puma7/handy-meeting/releases/tag/{}",
                release.tag_name
            )
    {
        return Err("Unexpected official release metadata".into());
    }
    if !newer {
        return Ok(None);
    }
    let name = format!("HandyMeeting-{tag}-windows-x64-setup.exe");
    let mut matches = release.assets.into_iter().filter(|a| a.name == name);
    let asset = matches.next().ok_or(
        "The latest release has no Windows setup yet. Try again after its package checks finish.",
    )?;
    if matches.next().is_some() || asset.size == 0 || asset.size > MAX_INSTALLER {
        return Err("Conflicting or oversized release installer".into());
    }
    let expected_url = format!(
        "https://github.com/Puma7/handy-meeting/releases/download/{}/{}",
        release.tag_name, name
    );
    if asset.browser_download_url != expected_url {
        return Err("Unexpected installer origin".into());
    }
    let digest = asset
        .digest
        .and_then(|s| s.strip_prefix("sha256:").map(str::to_owned))
        .ok_or("GitHub did not provide an installer SHA256; download is blocked")?;
    if digest.len() != 64 || !digest.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid installer digest".into());
    }
    Ok(Some(Candidate {
        version: tag.into(),
        url: expected_url,
        sha256: digest.to_ascii_lowercase(),
        bytes: asset.size,
    }))
}
fn allowed(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some("github.com" | "release-assets.githubusercontent.com")
        )
}
fn client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 5 && allowed(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("Unexpected installer redirect")
            }
        }))
        .user_agent("HandyMeeting release updater")
        .build()
        .map_err(|_| "Could not initialize the update connection".into())
}
impl Updater {
    pub fn snapshot(&self) -> Result<View> {
        let state = self.inner.lock().map_err(|_| "Update state unavailable")?;
        let mut view = state.view.clone();
        if matches!(view.status.as_str(), "checking" | "downloading") {
            view.elapsed_ms = state.started.map_or(0, |s| s.elapsed().as_millis() as u64);
        }
        Ok(view)
    }
    fn begin(&self, status: &str) -> Result<()> {
        let mut s = self.inner.lock().map_err(|_| "Update state unavailable")?;
        if matches!(
            s.view.status.as_str(),
            "checking" | "downloading" | "launched"
        ) {
            return Err("An update operation is already active".into());
        }
        s.view.status = status.into();
        s.view.error = None;
        s.view.elapsed_ms = 0;
        s.view.attempt += 1;
        s.started = Some(Instant::now());
        self.cancel.store(false, Ordering::Release);
        Ok(())
    }
    fn finish<T>(&self, result: Result<T>) -> Result<T> {
        if let Err(error) = &result {
            if let Ok(mut s) = self.inner.lock() {
                s.view.status = if self.cancel.load(Ordering::Acquire) {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                s.view.error = Some(error.clone());
                s.view.elapsed_ms = s.started.map_or(0, |v| v.elapsed().as_millis() as u64);
            }
        }
        result
    }
    pub fn check(&self, current: &str) -> Result<View> {
        self.begin("checking")?;
        let result = (|| {
            let response=client()?.get(LATEST).timeout(Duration::from_secs(30)).header("Accept","application/vnd.github+json").send().and_then(reqwest::blocking::Response::error_for_status).map_err(|_|"The official update check failed. Check your internet connection and retry. / Die offizielle Updateprüfung ist fehlgeschlagen. Verbindung prüfen und erneut versuchen.")?;
            let mut bytes = Vec::new();
            response
                .take(2 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Could not read release metadata")?;
            if bytes.len() > 2 * 1024 * 1024 {
                return Err("Release metadata exceeds the limit".into());
            }
            let release: Release =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid release metadata")?;
            let found = candidate(release, current)?;
            let mut state = self.inner.lock().map_err(|_| "Update state unavailable")?;
            state.ready = None;
            state.view.received_bytes = 0;
            state.view.total_bytes = found.as_ref().map_or(0, |c| c.bytes);
            state.view.version = found.as_ref().map(|c| c.version.clone());
            state.view.release_url = found.as_ref().map(|c| {
                format!(
                    "https://github.com/Puma7/handy-meeting/releases/tag/v{}",
                    c.version
                )
            });
            state.view.status = if found.is_some() {
                "available"
            } else {
                "current"
            }
            .into();
            state.candidate = found;
            state.view.elapsed_ms = state.started.map_or(0, |v| v.elapsed().as_millis() as u64);
            Ok(state.view.clone())
        })();
        self.finish(result)
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
    pub fn download(&self, root: &Path) -> Result<View> {
        if !cfg!(target_os = "windows") {
            return Err("In-app installer downloads currently support Windows; use the official release page on this platform".into());
        }
        self.begin("downloading")?;
        let result = (|| {
            let candidate = self
                .inner
                .lock()
                .map_err(|_| "Update state unavailable")?
                .candidate
                .clone()
                .ok_or("Check for an official update first")?;
            meeting_core::metadata::checked_directory(root)
                .map_err(|_| "Update storage is unavailable")?;
            let folder = root.join("updates");
            match fs::create_dir(&folder) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(_) => return Err("Could not create update folder".into()),
            }
            meeting_core::metadata::checked_directory(&folder)
                .map_err(|_| "Linked update folders are rejected")?;
            let file = folder.join(format!(
                "HandyMeeting-{}-{}-setup.exe",
                candidate.version,
                meeting_notes::new_job_id()
            ));
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&file)
                .map_err(|_| "Could not reserve a new installer file")?;
            // Partial downloads use unique files and are never executed or overwritten.
            let mut response = client()?
                .get(&candidate.url)
                .send()
                .and_then(reqwest::blocking::Response::error_for_status)
                .map_err(|_| "Installer download failed; retry from the update check")?;
            let mut received = 0u64;
            let mut hasher = Sha256::new();
            let mut buffer = [0u8; 65536];
            loop {
                if self.cancel.load(Ordering::Acquire) {
                    return Err("Installer download cancelled. Recordings and installed versions are unchanged. / Download abgebrochen; Aufnahmen und installierte Versionen bleiben erhalten.".into());
                }
                let count = response.read(&mut buffer).map_err(|_| {
                    "Installer connection was interrupted; the partial file will not be installed"
                })?;
                if count == 0 {
                    break;
                }
                received = received
                    .checked_add(count as u64)
                    .ok_or("Download size overflow")?;
                if received > candidate.bytes {
                    return Err("Installer size exceeds the verified release size".into());
                }
                output
                    .write_all(&buffer[..count])
                    .map_err(|_| "Could not save installer download")?;
                hasher.update(&buffer[..count]);
                if let Ok(mut state) = self.inner.lock() {
                    state.view.received_bytes = received;
                }
            }
            output
                .sync_all()
                .map_err(|_| "Could not confirm saved installer")?;
            if received != candidate.bytes || format!("{:x}", hasher.finalize()) != candidate.sha256
            {
                return Err("Installer size or SHA256 did not match GitHub. Installation is blocked. / Installerprüfung fehlgeschlagen; Installation gesperrt.".into());
            }
            let mut state = self.inner.lock().map_err(|_| "Update state unavailable")?;
            state.ready = Some(file);
            state.view.status = "ready".into();
            state.view.elapsed_ms = state.started.map_or(0, |v| v.elapsed().as_millis() as u64);
            Ok(state.view.clone())
        })();
        self.finish(result)
    }
    pub fn installer(&self) -> Result<InstallerLease> {
        let state = self.inner.lock().map_err(|_| "Update state unavailable")?;
        if state.view.status != "ready" {
            return Err("No verified installer is ready".into());
        }
        let candidate = state.candidate.as_ref().ok_or("Missing verified release")?;
        let path = state.ready.as_ref().ok_or("Missing verified installer")?;
        let meta = fs::symlink_metadata(path).map_err(|_| "Verified installer is missing")?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return Err("Linked installer rejected".into());
            }
        }
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != candidate.bytes {
            return Err("Installer changed; check and download again".into());
        }
        meeting_core::metadata::checked_directory(path.parent().ok_or("Missing installer folder")?)
            .map_err(|_| "Installer folder changed")?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1);
        }
        let mut input = options
            .open(path)
            .map_err(|_| "Installer is unavailable or being changed")?;
        let mut hash = Sha256::new();
        let mut bytes = [0u8; 65536];
        loop {
            let n = input
                .read(&mut bytes)
                .map_err(|_| "Installer recheck failed")?;
            if n == 0 {
                break;
            }
            hash.update(&bytes[..n]);
        }
        if format!("{:x}", hash.finalize()) != candidate.sha256 {
            return Err("Installer changed; installation blocked".into());
        }
        Ok(InstallerLease {
            path: path.clone(),
            _file: input,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn release() -> Release {
        Release{tag_name:"v1.0.2".into(),draft:false,prerelease:false,html_url:"https://github.com/Puma7/handy-meeting/releases/tag/v1.0.2".into(),assets:vec![Asset{name:"HandyMeeting-1.0.2-windows-x64-setup.exe".into(),size:123,digest:Some(format!("sha256:{}","a".repeat(64))),browser_download_url:"https://github.com/Puma7/handy-meeting/releases/download/v1.0.2/HandyMeeting-1.0.2-windows-x64-setup.exe".into()}]}
    }
    #[test]
    fn release_selection_rejects_drafts_bad_digests_and_foreign_origins() {
        assert!(candidate(release(), "1.0.1").unwrap().is_some());
        assert!(candidate(release(), "1.0.2").unwrap().is_none());
        let mut r = release();
        r.draft = true;
        assert!(candidate(r, "1.0.1").is_err());
        let mut r = release();
        r.assets[0].digest = None;
        assert!(candidate(r, "1.0.1").is_err());
        let mut r = release();
        r.assets[0].browser_download_url = "https://example.com/program.exe".into();
        assert!(candidate(r, "1.0.1").is_err());
    }
    #[test]
    fn redirects_cannot_escape_https_official_asset_hosts() {
        for url in [
            "http://github.com/x",
            "https://evil.example/x",
            "https://github.com.evil.example/x",
            "https://user@github.com/x",
            "https://github.com:444/x",
        ] {
            assert!(!allowed(&url::Url::parse(url).unwrap()));
        }
        assert!(allowed(
            &url::Url::parse("https://release-assets.githubusercontent.com/file").unwrap()
        ));
    }
    #[test]
    fn install_recheck_blocks_modified_bytes() {
        let root = std::env::temp_dir().join(meeting_notes::new_job_id());
        fs::create_dir(&root).unwrap();
        let file = root.join("synthetic-not-executable.exe");
        fs::write(&file, b"old").unwrap();
        let updater = Updater::default();
        {
            let mut s = updater.inner.lock().unwrap();
            s.view.status = "ready".into();
            s.candidate = Some(Candidate {
                version: "1.0.2".into(),
                url: String::new(),
                sha256: format!("{:x}", Sha256::digest(b"old")),
                bytes: 3,
            });
            s.ready = Some(file.clone());
        }
        assert_eq!(updater.installer().unwrap().path, file);
        fs::write(&file, b"new").unwrap();
        assert!(updater.installer().is_err());
        fs::remove_file(file).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
