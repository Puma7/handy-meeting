use crate::{blocking, AppResult, Service};
use std::sync::Arc;
use tauri::State;
#[tauri::command]
pub(crate) fn start_speaker_experiment(
    service: State<'_, Arc<Service>>,
    meeting_id: String,
    model_id: String,
    mode: String,
) -> AppResult<String> {
    let _gate = service
        .transition
        .lock()
        .map_err(|_| "Recording control unavailable")?;
    if service.lock()?.transitioning {
        return Err("Finish the pending archive or recording operation first".into());
    }
    service
        .speaker_jobs
        .start(Arc::clone(&service), meeting_id, model_id, mode)
}
#[tauri::command]
pub(crate) fn get_speaker_progress(
    service: State<'_, Arc<Service>>,
) -> Option<crate::speaker_jobs::Progress> {
    service.speaker_jobs.snapshot()
}
#[tauri::command]
pub(crate) fn cancel_speaker_experiment(service: State<'_, Arc<Service>>, job_id: String) -> bool {
    service.speaker_jobs.cancel(&job_id)
}
#[tauri::command]
pub(crate) async fn get_speaker_sections(
    service: State<'_, Arc<Service>>,
    meeting_id: String,
) -> AppResult<Option<crate::speaker_jobs::SavedSections>> {
    let service = Arc::clone(&service);
    blocking(move || crate::speaker_jobs::saved(&service, &meeting_id)).await
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpeakerView {
    #[serde(flatten)]
    document: meeting_core::speakers::SpeakerDocument,
    sources: std::collections::BTreeMap<String, String>,
}
#[tauri::command]
pub(crate) async fn get_speaker_labels(
    service: State<'_, Arc<Service>>,
    meeting_id: String,
) -> AppResult<SpeakerView> {
    let service = Arc::clone(&service);
    blocking(move||{let data=service.data(&meeting_id)?;let document=meeting_core::speakers::read(&service.root,&meeting_id).map_err(|_|"Speaker annotations could not be read; prior revisions are preserved. / Sprecherzuordnungen konnten nicht gelesen werden; bisherige Fassungen bleiben erhalten.")?;Ok(SpeakerView{document,sources:data.parts.iter().filter_map(|p|p.text.as_ref().map(|t|(p.file.clone(),meeting_core::speakers::text_hash(t)))).collect()})}).await
}
#[tauri::command]
pub(crate) async fn save_speaker_labels(
    service: State<'_, Arc<Service>>,
    meeting_id: String,
    expected_revision: u64,
    enabled: bool,
    assignments: Vec<meeting_core::speakers::Assignment>,
) -> AppResult<meeting_core::speakers::SpeakerDocument> {
    let service = Arc::clone(&service);
    blocking(move||{let data=service.data(&meeting_id)?;meeting_core::speakers::save(&service.root,&data,expected_revision,enabled,assignments).map_err(|_|"Speaker labels could not be saved. Reload and check the original section and quote. / Sprecherzuordnung nicht gespeichert. Bitte neu laden und Originalabschnitt sowie Zitat prüfen.".into())}).await
}
