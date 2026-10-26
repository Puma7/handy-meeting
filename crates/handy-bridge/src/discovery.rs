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
            candidates.push(home.join("Applications/Handy.AppImage"));
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

pub(crate) fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}
