use crate::{blocking, validate_id, AppResult, Service};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub(crate) async fn list_unfinished_analyses(
    service: State<'_, Arc<Service>>,
    meeting_id: String,
) -> AppResult<meeting_notes::PendingListing> {
    let service = Arc::clone(&service);
    blocking(move || crate::notes_service::pending_analyses(&service, &meeting_id)).await
}

#[tauri::command]
pub(crate) async fn list_text_models(endpoint: String, mode: String) -> AppResult<Vec<String>> {
    if mode != "local" {
        return Err("Automatische Textmodellerkennung ist ausschließlich lokal verfügbar. Cloud-Modelle bitte ausdrücklich eingeben.".into());
    }
    blocking(move || meeting_notes::local_model_ids(&endpoint).map_err(|e| e.to_string())).await
}
#[tauri::command]
pub(crate) fn get_notes_progress(
    service: State<'_, Arc<Service>>,
) -> AppResult<Option<super::notes_service::NotesProgress>> {
    service.notes_jobs.snapshot()
}
#[tauri::command]
pub(crate) fn cancel_notes(service: State<'_, Arc<Service>>, job_id: String) -> AppResult<bool> {
    service.notes_jobs.cancel(&job_id)
}
#[tauri::command]
pub(crate) async fn notes_markdown(
    service: State<'_, Arc<Service>>,
    meeting_id: String,
    analysis_id: String,
) -> AppResult<String> {
    let service = Arc::clone(&service);
    blocking(move || {
        validate_id(&meeting_id)?;
        validate_id(&analysis_id)?;
        let analysis=meeting_notes::read_analysis(&service.root.join("sessions").join(&meeting_id).join("analyses").join(&analysis_id).join("result.json")).map_err(|_|"Die gespeicherte Auswertung besteht die Quellenprüfung nicht. Originaldateien bleiben erhalten.")?;
        if analysis.id!=analysis_id||analysis.meeting_id!=meeting_id {
            return Err("Die gespeicherte Auswertung gehört nicht zur ausgewählten Sitzung.".into());
        }
        meeting_notes::render_markdown(&analysis).map_err(|_|"Die Auswertung konnte nicht sicher als Markdown ausgegeben werden.".into())
    }).await
}
