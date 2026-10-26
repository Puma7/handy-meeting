//! Optional installed-package metadata. Never try an unsupported Handy flag:
//! it could start a second GUI instead of answering a headless query.
use std::path::Path;

pub fn installed_version(executable: &Path) -> Option<String> {
    platform_version(executable)
}

fn clean_version(value: &str) -> Option<String> {
    let value = value.trim();
    (value.len() <= 64
        && value.as_bytes().first().is_some_and(u8::is_ascii_digit)
        && value.contains('.')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".+-".contains(&byte)))
    .then(|| value.to_owned())
}

#[cfg(windows)]
fn platform_version(executable: &Path) -> Option<String> {
    use winreg::{enums::*, RegKey};
    let executable = executable.canonicalize().ok()?;
    let parent = executable.parent()?;
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            let Ok(uninstall) = RegKey::predef(hive).open_subkey_with_flags(
                r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
                KEY_READ | view,
            ) else {
                continue;
            };
            for name in uninstall.enum_keys().take(4096).filter_map(Result::ok) {
                let Ok(package) = uninstall.open_subkey_with_flags(name, KEY_READ | view) else {
                    continue;
                };
                let display: String = package.get_value("DisplayName").unwrap_or_default();
                if !display.eq_ignore_ascii_case("Handy") {
                    continue;
                }
                let location: String = package.get_value("InstallLocation").unwrap_or_default();
                // Do not report the version of another installed copy when an
                // explicitly selected portable executable is being used.
                if location.is_empty()
                    || Path::new(&location).canonicalize().ok().as_deref() != Some(parent)
                {
                    continue;
                }
                if let Ok(version) = package.get_value::<String, _>("DisplayVersion") {
                    if let Some(version) = clean_version(&version) {
                        return Some(version);
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn platform_version(executable: &Path) -> Option<String> {
    use std::io::Read;
    let executable = executable.canonicalize().ok()?;
    let macos = executable.parent()?;
    let contents = macos.parent()?;
    if macos.file_name()? != "MacOS" || contents.file_name()? != "Contents" {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(contents.join("Info.plist"))
        .ok()?
        .take(256 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 256 * 1024 {
        return None;
    }
    let value: plist::Value = plist::from_bytes(&bytes).ok()?;
    let dictionary = value.as_dictionary()?;
    ["CFBundleShortVersionString", "CFBundleVersion"]
        .into_iter()
        .filter_map(|key| dictionary.get(key).and_then(plist::Value::as_string))
        .find_map(clean_version)
}

#[cfg(target_os = "linux")]
fn platform_version(executable: &Path) -> Option<String> {
    use crate::{process::run_query, CommandLimits};
    use std::time::Duration;
    let executable = executable.canonicalize().ok()?;
    let path = executable.to_str()?;
    let tool = Path::new("/usr/bin/dpkg-query");
    let limits = CommandLimits {
        timeout: Duration::from_secs(2),
        stdout_bytes: 8192,
        stderr_bytes: 8192,
    };
    let ownership = run_query(tool, &["--search", path], limits).ok()?;
    let ownership = std::str::from_utf8(&ownership).ok()?;
    if !ownership.lines().any(|line| {
        line.strip_prefix("handy: ") == Some(path)
            || line.strip_prefix("handy:amd64: ") == Some(path)
            || line.strip_prefix("handy:arm64: ") == Some(path)
    }) {
        return None;
    }
    let bytes = run_query(
        tool,
        &["--show", "--showformat=${Version}", "handy"],
        limits,
    )
    .ok()?;
    clean_version(std::str::from_utf8(&bytes).ok()?)
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn platform_version(_executable: &Path) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bounded_plain_version_metadata_is_displayed() {
        assert_eq!(clean_version(" 0.9.8 \n"), Some("0.9.8".into()));
        assert_eq!(
            clean_version("1.0.0-beta.2+build3"),
            Some("1.0.0-beta.2+build3".into())
        );
        for bad in [
            "",
            "Handy",
            "0.9\n8",
            "<script>0.9</script>",
            "../0.9",
            "0.9 [link](x)",
        ] {
            assert!(clean_version(bad).is_none());
        }
        assert!(clean_version(&format!("0.{}", "1".repeat(64))).is_none());
    }

    #[test]
    fn an_uninstalled_missing_executable_has_no_guessed_version() {
        let root = tempfile::tempdir().unwrap();
        assert!(installed_version(&root.path().join("missing-handy")).is_none());
    }
}
