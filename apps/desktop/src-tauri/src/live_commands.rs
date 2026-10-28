//! Commands for live notes during a meeting and for the local text model setup.
use crate::live_notes::{LiveNotesRequest, LiveNotesStatus};
use crate::local_models::{LocalModelSetup, PullProgress};
use crate::{blocking, AppResult, Service};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub(crate) async fn start_live_notes(
    service: State<'_, Arc<Service>>,
    request: LiveNotesRequest,
) -> AppResult<LiveNotesStatus> {
    let service = Arc::clone(&service);
    blocking(move || service.live_notes.start(&service, request)).await
}

#[tauri::command]
pub(crate) fn stop_live_notes(service: State<'_, Arc<Service>>) -> AppResult<bool> {
    service.live_notes.stop()
}

#[tauri::command]
pub(crate) fn refresh_live_notes(service: State<'_, Arc<Service>>) -> AppResult<bool> {
    service.live_notes.refresh()
}

#[tauri::command]
pub(crate) fn set_live_notes_interval(
    service: State<'_, Arc<Service>>,
    interval_ms: u64,
) -> AppResult<bool> {
    service.live_notes.set_interval(interval_ms)
}

#[tauri::command]
pub(crate) async fn get_live_notes(
    service: State<'_, Arc<Service>>,
    meeting_id: String,
) -> AppResult<LiveNotesStatus> {
    let service = Arc::clone(&service);
    blocking(move || service.live_notes.status(&service, &meeting_id)).await
}

#[tauri::command]
pub(crate) async fn local_model_setup() -> AppResult<LocalModelSetup> {
    blocking(|| Ok(crate::local_models::setup())).await
}

#[tauri::command]
pub(crate) fn pull_ollama_model(
    service: State<'_, Arc<Service>>,
    model: String,
) -> AppResult<PullProgress> {
    service.model_pulls.start(model)
}

#[tauri::command]
pub(crate) fn get_model_pull_progress(
    service: State<'_, Arc<Service>>,
) -> AppResult<Option<PullProgress>> {
    Ok(service.model_pulls.snapshot())
}

#[tauri::command]
pub(crate) fn cancel_model_pull(service: State<'_, Arc<Service>>) -> AppResult<bool> {
    Ok(service.model_pulls.cancel())
}
