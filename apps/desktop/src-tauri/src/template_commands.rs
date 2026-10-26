use crate::{blocking, AppResult, Service};
use meeting_notes::{Template, TemplateLibrary, TemplateStore};
use std::sync::Arc;
use tauri::State;
#[tauri::command]
pub(crate) async fn list_templates(service: State<'_, Arc<Service>>) -> AppResult<TemplateLibrary> {
    let service = Arc::clone(&service);
    blocking(move || {
        TemplateStore::new(&service.root.join("templates"))
            .list()
            .map_err(|e| e.to_string())
    })
    .await
}
#[tauri::command]
pub(crate) async fn save_templates(
    service: State<'_, Arc<Service>>,
    library: TemplateLibrary,
) -> AppResult<TemplateLibrary> {
    let service = Arc::clone(&service);
    blocking(move || {
        TemplateStore::new(&service.root.join("templates"))
            .save(library)
            .map_err(|e| e.to_string())
    })
    .await
}
#[tauri::command]
pub(crate) async fn import_templates(
    service: State<'_, Arc<Service>>,
    templates: Vec<Template>,
) -> AppResult<TemplateLibrary> {
    let service = Arc::clone(&service);
    blocking(move || {
        TemplateStore::new(&service.root.join("templates"))
            .import(templates)
            .map_err(|e| e.to_string())
    })
    .await
}
