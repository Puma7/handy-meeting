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
#[tauri::command]
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
  let _gate=service.transition.lock().map_err(|_|"Application state unavailable")?;
  {let inner=service.lock()?;if inner.active.is_some()||inner.transitioning||service.notes_jobs.running()||service.speaker_jobs.snapshot().is_some_and(|p|p.status=="running")||service.maintenance_jobs.snapshot().is_some_and(|p|p.status=="running")||service.library_jobs.snapshot().is_some_and(|p|p.status=="running"){return Err("Finish recording, recognition, analysis, speaker experiments, archive maintenance and search indexing before installing. / Bitte Aufnahme, Erkennung, Auswertung, Sprecherexperimente, Archivpflege und Indexierung vor der Installation abschließen.".into());}}
  let installer=service.updater.installer()?;
  service.lock()?.transitioning=true;
  let mut command=std::process::Command::new(&installer.path);
  command.current_dir(installer.path.parent().ok_or("Installer folder is unavailable")?);
  match command.spawn(){Ok(_)=>{app.exit(0);Ok(())},Err(_)=>{service.lock()?.transitioning=false;Err("The verified installer could not start. Your current app and recordings are unchanged. / Der geprüfte Installer konnte nicht gestartet werden; die aktuelle App bleibt geöffnet.".into())}}
 }).await
}
