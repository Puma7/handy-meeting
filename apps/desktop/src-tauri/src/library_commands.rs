use crate::{blocking, library_service, AppResult, Service};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub(crate) async fn edit_meeting_metadata(
    service: State<'_, Arc<Service>>,
    meeting_id: String,
    expected_revision: u64,
    title: Option<String>,
    pinned: bool,
) -> AppResult<meeting_core::metadata::Metadata> {
    let service = Arc::clone(&service);
    blocking(move||{
        let data=service.data(&meeting_id)?;
        let mut meta=meeting_core::metadata::edit(&service.root,&meeting_id,expected_revision,title,pinned)
            .map_err(|_|"Meeting labels could not be saved. Reload the meeting; originals are preserved. / Meetingname oder VIP-Markierung konnten nicht gespeichert werden. Bitte die Sitzung neu laden.")?;
        if data.read_only&&!data.id.starts_with("legacy-"){meta.audio_removed=true;}
        if library_service::update(&service.root,&data).is_err(){
            if let Ok(mut state)=service.lock(){state.error=Some("Meeting labels were saved. Please refresh the local search index. / Meetingbezeichnungen gespeichert. Bitte den lokalen Suchindex aktualisieren.".into());}
        }
        Ok(meta)
    }).await
}
#[tauri::command]
pub(crate) async fn search_meetings(
    service: State<'_, Arc<Service>>,
    query: String,
    offset: u64,
    limit: u32,
) -> AppResult<meeting_core::library::SearchPage> {
    let service = Arc::clone(&service);
    blocking(move || library_service::search(&service.root, &query, offset, limit)).await
}
// Runs off the UI thread: it may wait on the recording transition lock or on disk.
#[tauri::command(async)]
pub(crate) fn rebuild_search_index(service: State<'_, Arc<Service>>) -> AppResult<String> {
    let _gate = service
        .transition
        .lock()
        .map_err(|_| "Recording control unavailable")?;
    if service.lock()?.transitioning
        || service
            .maintenance_jobs
            .snapshot()
            .is_some_and(|p| p.status == "running")
    {
        return Err("Finish archive maintenance before updating the search index".into());
    }
    service.library_jobs.start_scan(service.root.clone())
}
#[tauri::command]
pub(crate) fn get_library_progress(
    service: State<'_, Arc<Service>>,
) -> Option<library_service::Progress> {
    service.library_jobs.snapshot()
}
#[tauri::command]
pub(crate) fn cancel_library_job(service: State<'_, Arc<Service>>, job_id: String) -> bool {
    service.library_jobs.cancel(&job_id)
}
