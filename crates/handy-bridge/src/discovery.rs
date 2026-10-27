use crate::{BridgeError, Result};
use std::path::{Path, PathBuf};

pub fn discover_handy(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return executable_path(path).ok_or(BridgeError::NotFound);
    }
    let mut candidates = Vec::new();
    if cfg!(target_os = "windows") {
        for variable in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(base) = std::env::var_os(variable) {
                candidates.push(PathBuf::from(base).join("Handy/handy.exe"));
            }
        }
    } else if cfg!(target_os = "macos") {
        candidates.push(PathBuf::from("/Applications/Handy.app"));
        if let Some(home) = home() {
            candidates.push(home.join("Applications/Handy.app"));
        }
    } else {
        candidates.extend([
            PathBuf::from("/usr/bin/handy"),
            PathBuf::from("/opt/Handy/handy"),
        ]);
        if let Some(home) = home() {
            let applications = home.join("Applications");
            // The unversioned name keeps its earlier priority; downloaded
            // release names such as `Handy_0.9.8_amd64.AppImage` follow.
            let unversioned = applications.join("Handy.AppImage");
            if is_executable(&unversioned) {
                candidates.push(unversioned);
            }
            candidates.extend(launchable_appimages(&applications, std::env::consts::ARCH));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path).filter(|p| p.is_absolute()) {
            candidates.push(directory.join(if cfg!(windows) { "handy.exe" } else { "handy" }));
        }
    }
    candidates
        .iter()
        .find_map(|p| executable_path(p))
        .ok_or(BridgeError::NotFound)
}

fn executable_path(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return path.canonicalize().ok();
    }
    if path.extension().is_some_and(|ext| ext == "app") {
        for name in ["Handy", "handy"] {
            let executable = path.join("Contents/MacOS").join(name);
            if executable.is_file() {
                return executable.canonicalize().ok();
            }
        }
    }
    None
}

/// Architecture names used in AppImage file names.
const APPIMAGE_ARCHES: [(&str, &str); 6] = [
    ("amd64", "x86_64"),
    ("x86_64", "x86_64"),
    ("aarch64", "aarch64"),
    ("arm64", "aarch64"),
    ("armhf", "arm"),
    ("i386", "x86"),
];

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct AppImageRank {
    /// Numeric components, zero-padded so `0.9` equals `0.9.0`.
    numbers: [u64; 4],
    /// A final release sorts above its pre-releases.
    release: bool,
    pre_release: Vec<PreReleaseId>,
    /// A name stating this machine's architecture is more specific.
    arch_specific: bool,
}

/// One dot-separated pre-release identifier. The derived order is SemVer precedence:
/// numbers compare numerically and below text, text compares by ASCII, and a list
/// that is a prefix of another sorts first, so `beta.10` follows `beta.9`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PreReleaseId {
    Number(u64),
    Text(String),
}

/// Parses `Handy_<version>[_<arch>].AppImage` (also `-` separators).
/// Returns `None` for other names and for another architecture.
fn appimage_rank(name: &str, machine_arch: &str) -> Option<AppImageRank> {
    let stem = name
        .len()
        .checked_sub(".AppImage".len())
        .filter(|&end| name.is_char_boundary(end))
        .filter(|&end| name[end..].eq_ignore_ascii_case(".AppImage"))
        .map(|end| &name[..end])?;
    // Byte checks: other AppImages may have non-ASCII names, and byte 5 being ASCII
    // makes index 6 a character boundary.
    let bytes = stem.as_bytes();
    if bytes.len() < 6
        || !bytes[..5].eq_ignore_ascii_case(b"handy")
        || !matches!(bytes[5], b'_' | b'-')
    {
        return None;
    }
    let rest = &stem[6..];
    let mut version = rest;
    let mut arch_specific = false;
    for (token, arch) in APPIMAGE_ARCHES {
        if let Some(prefix) = rest
            .strip_suffix(token)
            .and_then(|r| r.strip_suffix('_').or_else(|| r.strip_suffix('-')))
        {
            if arch != machine_arch {
                return None;
            }
            version = prefix;
            arch_specific = true;
            break;
        }
    }
    let (core, pre_release) = match version.split_once('-') {
        Some((_, "")) => return None,
        Some((core, pre)) => (core, pre),
        None => (version, ""),
    };
    let parts: Vec<&str> = core.split('.').collect();
    let mut numbers = [0_u64; 4];
    if parts.len() > numbers.len() {
        return None;
    }
    let mut identifiers = Vec::new();
    if !pre_release.is_empty() {
        for id in pre_release.split('.') {
            if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                return None;
            }
            identifiers.push(if id.bytes().all(|b| b.is_ascii_digit()) {
                if id.len() > 18 {
                    return None;
                }
                PreReleaseId::Number(id.parse().ok()?)
            } else {
                PreReleaseId::Text(id.into())
            });
        }
    }
    for (slot, part) in numbers.iter_mut().zip(&parts) {
        if part.is_empty() || part.len() > 9 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *slot = part.parse().ok()?;
    }
    Some(AppImageRank {
        numbers,
        release: pre_release.is_empty(),
        pre_release: identifiers,
        arch_specific,
    })
}

/// Versioned Handy AppImages in `directory`, best first: highest version,
/// final before pre-release, explicit matching architecture before an
/// unspecified one, then by file name so the order never depends on the
/// directory listing.
fn versioned_appimages(directory: &Path, machine_arch: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut found: Vec<(AppImageRank, String)> = entries
        .take(10_000)
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter_map(|name| Some((appimage_rank(&name, machine_arch)?, name)))
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    found
        .into_iter()
        .map(|(_, name)| directory.join(name))
        .collect()
}

/// Versioned AppImages that can be started. A browser download is often saved without
/// the execute bit; such a file must not hide an older installation that can run. The
/// unversioned `Handy.AppImage` gets the same check.
fn launchable_appimages(directory: &Path, machine_arch: &str) -> Vec<PathBuf> {
    versioned_appimages(directory, machine_arch)
        .into_iter()
        .filter(|path| is_executable(path))
        .collect()
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

pub(crate) fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versioned_appimage_names_are_recognized_for_this_architecture_only() {
        let rank = appimage_rank("Handy_0.9.8_amd64.AppImage", "x86_64").unwrap();
        assert_eq!(rank.numbers, [0, 9, 8, 0]);
        assert!(rank.release && rank.arch_specific);
        assert!(appimage_rank("handy-1.2.0-x86_64.appimage", "x86_64").is_some());
        assert!(appimage_rank("Handy_1.2.0.AppImage", "aarch64").is_some());
        assert!(appimage_rank("Handy_0.9.8_aarch64.AppImage", "x86_64").is_none());
        assert!(appimage_rank("Handy_0.9.8_amd64.AppImage", "aarch64").is_none());
        for name in [
            "Handy.AppImage",
            "Handy_.AppImage",
            "Handy_latest.AppImage",
            "Handy_0.9.8_amd64.AppImage.zsync",
            "HandyMeeting_1.0.0_amd64.AppImage",
            "Handy_0..8_amd64.AppImage",
            "Handy_1.2.3.4.5.AppImage",
            "Handy_0.9.8_amd64 (1).AppImage",
            "Handy_0.9.8-beta/1.AppImage",
            "Handy_0.9.8-beta..1.AppImage",
            "Handy_0.9.8-beta._amd64.AppImage",
            "Handy_0.9.8-.AppImage",
            "Handy_0.9.8-beta.1234567890123456789.AppImage",
            "Handy_99999999999.AppImage",
            // Unrelated non-ASCII names must be skipped, never split mid-character.
            "中文.AppImage",
            "Handé_1.0.0.AppImage",
            "Handyé.AppImage",
            "Handy_1.0.0_ä.AppImage",
        ] {
            assert!(appimage_rank(name, "x86_64").is_none(), "{name}");
        }
    }

    #[test]
    fn pre_releases_follow_semver_precedence() {
        let ordered = [
            "Handy_0.10.0-alpha.AppImage",
            "Handy_0.10.0-alpha.1.AppImage",
            "Handy_0.10.0-alpha.beta.AppImage",
            "Handy_0.10.0-beta.AppImage",
            "Handy_0.10.0-beta.2.AppImage",
            "Handy_0.10.0-beta.9.AppImage",
            "Handy_0.10.0-beta.10.AppImage",
            "Handy_0.10.0-beta.11.AppImage",
            "Handy_0.10.0-rc.1.AppImage",
            "Handy_0.10.0.AppImage",
        ];
        // SemVer identifiers may contain hyphens.
        assert!(appimage_rank("Handy_0.10.0-rc-1.AppImage", "x86_64").is_some());
        for pair in ordered.windows(2) {
            let lower = appimage_rank(pair[0], "x86_64").unwrap();
            let higher = appimage_rank(pair[1], "x86_64").unwrap();
            assert!(lower < higher, "{} must sort below {}", pair[0], pair[1]);
        }
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "Handy_0.10.0-beta.9_amd64.AppImage",
            "Handy_0.10.0-beta.10_amd64.AppImage",
        ] {
            std::fs::write(dir.path().join(name), b"synthetic, never executed").unwrap();
        }
        assert_eq!(
            versioned_appimages(dir.path(), "x86_64")[0],
            dir.path().join("Handy_0.10.0-beta.10_amd64.AppImage")
        );
    }

    #[test]
    fn an_appimage_without_the_execute_bit_does_not_hide_one_that_runs() {
        if !cfg!(unix) {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let downloaded = dir.path().join("Handy_0.10.0-beta.10_amd64.AppImage");
        let installed = dir.path().join("Handy_0.9.8_amd64.AppImage");
        std::fs::write(&downloaded, b"synthetic, never executed").unwrap();
        std::fs::write(&installed, b"synthetic, never executed").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &Path, mode| {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap()
            };
            mode(&downloaded, 0o644);
            mode(&installed, 0o755);
        }
        assert_eq!(launchable_appimages(dir.path(), "x86_64"), [installed]);
        // The unversioned name is checked the same way before it is offered.
        let unversioned = dir.path().join("Handy.AppImage");
        std::fs::write(&unversioned, b"synthetic, never executed").unwrap();
        assert!(!is_executable(&unversioned));
        assert!(!is_executable(&dir.path().join("missing.AppImage")));
    }

    #[test]
    fn newest_most_specific_appimage_is_chosen_deterministically() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "Handy_0.9.8_amd64.AppImage",
            "Handy_0.10.0-beta.1_amd64.AppImage",
            "Handy_0.9.10.AppImage",
            "Handy_0.9.10_amd64.AppImage",
            "Handy_0.11.0_aarch64.AppImage",
            "Handy_0.9.9_amd64.AppImage.zsync",
            "notes.txt",
        ] {
            std::fs::write(dir.path().join(name), b"synthetic, never executed").unwrap();
        }
        let names: Vec<String> = versioned_appimages(dir.path(), "x86_64")
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            [
                "Handy_0.10.0-beta.1_amd64.AppImage",
                "Handy_0.9.10_amd64.AppImage",
                "Handy_0.9.10.AppImage",
                "Handy_0.9.8_amd64.AppImage",
            ]
        );
        std::fs::write(dir.path().join("Handy_0.10.0_amd64.AppImage"), b"x").unwrap();
        assert_eq!(
            versioned_appimages(dir.path(), "x86_64")[0],
            dir.path().join("Handy_0.10.0_amd64.AppImage")
        );
        assert!(versioned_appimages(&dir.path().join("missing"), "x86_64").is_empty());
    }

    #[test]
    fn first_existing_versioned_appimage_is_the_newest_for_this_machine() {
        if !cfg!(target_os = "linux") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let applications = dir.path().join("Applications");
        std::fs::create_dir(&applications).unwrap();
        let arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            other => other,
        };
        let older = applications.join(format!("Handy_0.9.8_{arch}.AppImage"));
        let newer = applications.join(format!("Handy_0.9.9_{arch}.AppImage"));
        std::fs::write(&older, b"synthetic").unwrap();
        std::fs::write(&newer, b"synthetic").unwrap();
        let candidates = versioned_appimages(&applications, std::env::consts::ARCH);
        assert_eq!(candidates, [newer.clone(), older]);
        assert_eq!(
            candidates.iter().find_map(|p| executable_path(p)),
            Some(newer.canonicalize().unwrap())
        );
    }
}
