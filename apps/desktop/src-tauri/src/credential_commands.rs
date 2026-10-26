use crate::{blocking, credential_service, AppResult};
use credential_service::{CredentialStatus, SaveCredential};
#[tauri::command]
pub(crate) async fn credential_status(
    endpoint: String,
    mode: String,
) -> AppResult<CredentialStatus> {
    blocking(move || credential_service::credential_status(&endpoint, &mode)).await
}
#[tauri::command]
pub(crate) async fn save_credential(request: SaveCredential) -> AppResult<CredentialStatus> {
    blocking(move || credential_service::save_credential(&request)).await
}
#[tauri::command]
pub(crate) async fn remove_credential(
    endpoint: String,
    mode: String,
    confirm: bool,
) -> AppResult<CredentialStatus> {
    blocking(move || credential_service::remove_credential(&endpoint, &mode, confirm)).await
}
