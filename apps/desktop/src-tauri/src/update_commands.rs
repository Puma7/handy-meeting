use crate::{blocking, AppResult, Service, VERSION};
use std::sync::Arc;
use tauri::State;
#[tauri::command]
pub(crate) async fn check_for_update(
    service: State<'_, Arc<Service>>,
) -> AppResult<crate::updater::View> {
    let service = Arc::clone(&service);
    blocking(move || service.updater.check(VERSION)).await
}
#[tauri::command]
pub(crate) async fn download_update(
    service: State<'_, Arc<Service>>,
) -> AppResult<crate::updater::View> {
    let service = Arc::clone(&service);
    blocking(move || service.updater.download(&service.root)).await
}
// Runs off the UI thread: it may wait on the recording transition lock or on disk.
#[tauri::command(async)]
pub(crate) fn get_update_progress(
    service: State<'_, Arc<Service>>,
) -> AppResult<crate::updater::View> {
    service.updater.snapshot()
}
#[tauri::command]
pub(crate) fn cancel_update(service: State<'_, Arc<Service>>) {
    service.updater.cancel();
}
#[tauri::command]
pub(crate) async fn install_update(
    service: State<'_, Arc<Service>>,
    app: tauri::AppHandle,
) -> AppResult<()> {
    let service = Arc::clone(&service);
    blocking(move||{
  if !cfg!(target_os="windows"){return Err("Use the official release page for this platform".into());}
  let _lease=service.reserve_operation(crate::operations::Kind::Installation)?;
  let installer=service.updater.installer()?;
  service.lock()?.transitioning=true;
  let mut command=std::process::Command::new(&installer.path);
  command.current_dir(installer.path.parent().ok_or("Installer folder is unavailable")?);
  match command.spawn(){Ok(_)=>{app.exit(0);Ok(())},Err(_)=>{service.lock()?.transitioning=false;Err("The verified installer could not start. Your current app and recordings are unchanged. / Der geprüfte Installer konnte nicht gestartet werden; die aktuelle App bleibt geöffnet.".into())}}
 }).await
}
/// Opens only the official release page held by the updater; the WebView passes no URL.
#[tauri::command]
pub(crate) async fn open_release_page(service: State<'_, Arc<Service>>) -> AppResult<()> {
    let service = Arc::clone(&service);
    blocking(move || {
        let page = service.updater.release_page()?;
        let mut child = browser_command(&page)?.spawn().map_err(|_| {
            format!("Could not open the browser. Open {page} in your browser to see the official downloads. / Browser konnte nicht geöffnet werden; bitte die Seite manuell öffnen.")
        })?;
        // Reap the short-lived launcher without waiting for the browser window.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    })
    .await
}
fn browser_command(page: &str) -> AppResult<std::process::Command> {
    browser_command_for(std::env::consts::OS, page)
}
fn browser_command_for(platform: &str, page: &str) -> AppResult<std::process::Command> {
    let mut command = match platform {
        "windows" => {
            let base = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
            let command = std::process::Command::new(std::path::PathBuf::from(base).join("explorer.exe"));
            #[cfg(windows)]
            let command = {
                use std::os::windows::process::CommandExt;
                let mut command = command;
                command.creation_flags(0x08000000);
                command
            };
            command
        }
        "macos" => std::process::Command::new("/usr/bin/open"),
        "linux" => std::process::Command::new("xdg-open"),
        _ => return Err(format!("Opening a browser is not supported on this platform. Open the official release page manually: {page} / Browserstart wird auf dieser Plattform nicht unterstützt. Bitte die offizielle Release-Seite manuell öffnen: {page}")),
    };
    command.arg(page);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supported_browser_launchers_keep_one_literal_url_without_starting_any_process() {
        let page = "https://github.com/Puma7/meeting-for-handy/releases/tag/v1.0.4";
        for (platform, program) in [
            ("windows", "explorer.exe"),
            ("macos", "open"),
            ("linux", "xdg-open"),
        ] {
            let command = browser_command_for(platform, page).unwrap();
            assert_eq!(
                std::path::Path::new(command.get_program())
                    .file_name()
                    .unwrap(),
                program
            );
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                [std::ffi::OsStr::new(page)]
            );
        }
    }
    #[test]
    fn unsupported_browser_platforms_return_the_manual_release_action() {
        let page = "https://github.com/Puma7/meeting-for-handy/releases/tag/v1.0.4";
        for platform in ["freebsd", "android", "unknown"] {
            let error = browser_command_for(platform, page).unwrap_err();
            assert!(error.contains("not supported"));
            assert!(error.contains(page));
        }
    }
}
