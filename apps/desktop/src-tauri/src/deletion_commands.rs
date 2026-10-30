//! Explicit per-meeting erasure. Previews are bounded in-memory metadata, never content copies.
use crate::{blocking, AppResult, Service};
use meeting_core::archive::{ErasurePreview, PendingErasure};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::State;

struct Cached {
    preview: ErasurePreview,
    at: Instant,
}
#[derive(Default)]
pub(crate) struct Previews(Mutex<HashMap<String, Cached>>);
impl Previews {
    /// Stores a bounded, short-lived preview capability containing no meeting text.
    fn put(&self, id: &str, preview: ErasurePreview) -> AppResult<()> {
        let mut rows = self.0.lock().map_err(|_| "Löschvorschau ist gesperrt.")?;
        rows.retain(|_, row| row.at.elapsed() < Duration::from_secs(900));
        if rows.len() >= 16 {
            rows.clear();
        }
        rows.insert(
            id.into(),
            Cached {
                preview,
                at: Instant::now(),
            },
        );
        Ok(())
    }
    /// Reads only the exact unexpired preview capability selected by its native job identity.
    fn get(&self, id: &str) -> AppResult<ErasurePreview> {
        crate::validate_id(id)?;
        let rows = self.0.lock().map_err(|_| "Löschvorschau ist gesperrt.")?;
        rows.get(id).filter(|row|row.at.elapsed()<Duration::from_secs(900)).map(|row|row.preview.clone()).ok_or_else(||"Löschvorschau ist abgelaufen. Bitte erneut prüfen. / Deletion preview expired; create another preview.".into())
    }
    /// Consumes one matching confirmed capability once; a different meeting or word leaves it intact.
    fn take(&self, id: &str, meeting: &str, confirmation: &str) -> AppResult<ErasurePreview> {
        let preview = self.get(id)?;
        if preview.meeting_id != meeting || !["LÖSCHEN", "DELETE"].contains(&confirmation) {
            return Err(
                "Bestätige genau das ausgewählte Meeting. / Confirm exactly the selected meeting."
                    .into(),
            );
        }
        self.0
            .lock()
            .map_err(|_| "Löschvorschau ist gesperrt.")?
            .remove(id)
            .ok_or("Löschvorschau wurde bereits verwendet.")?;
        Ok(preview)
    }
}

/// Starts a cancellable read-only inventory under exclusive operation admission.
#[tauri::command(async)]
pub(crate) fn preview_meeting_erasure(
    service: State<'_, Arc<Service>>,
    meeting_id: String,
) -> AppResult<String> {
    crate::validate_id(&meeting_id)?;
    if crate::valid_legacy_id(&meeting_id) {
        return Err("Importierte Originale liegen außerhalb dieser App-Ablage. / Imported originals are outside app-owned storage.".into());
    }
    let owned = Arc::clone(&service);
    let guard = crate::retention_commands::reserve(&owned)?;
    service
        .maintenance_jobs
        .start_work("meeting-erasure-preview", move |job, jobs, cancel| {
            let _guard = guard;
            jobs.update_work(job, 0, 1);
            let preview = meeting_core::archive::preview_erasure(&owned.root, &meeting_id, cancel)?;
            if cancel.load(std::sync::atomic::Ordering::Acquire) {
                return Err("Löschvorschau abgebrochen. / Deletion preview cancelled.".into());
            }
            owned.deletion_previews.put(job, preview)?;
            jobs.update_work(job, 1, 1);
            Ok(())
        })
}
/// Returns only the exact original preview's count/identity/hash, without local file paths.
#[tauri::command]
pub(crate) fn get_meeting_erasure_preview(
    service: State<'_, Arc<Service>>,
    job_id: String,
) -> AppResult<ErasurePreview> {
    service.deletion_previews.get(&job_id)
}

/// Consumes one confirmed preview; any changed content requires another explicit preview.
#[tauri::command(async)]
pub(crate) fn erase_meeting_now(
    service: State<'_, Arc<Service>>,
    preview_id: String,
    meeting_id: String,
    confirmation: String,
) -> AppResult<String> {
    let owned = Arc::clone(&service);
    // Validate confirmation before changing cached view or reserving destructive work.
    let preview = service.deletion_previews.get(&preview_id)?;
    if preview.meeting_id != meeting_id || !["LÖSCHEN", "DELETE"].contains(&confirmation.as_str())
    {
        return Err("Bestätigung passt nicht zum ausgewählten Meeting. / Confirmation does not match the selected meeting.".into());
    }
    let guard = crate::retention_commands::reserve(&owned)?;
    let preview = service
        .deletion_previews
        .take(&preview_id, &meeting_id, &confirmation)?;
    service
        .maintenance_jobs
        .start_work("meeting-erasure", move |job, jobs, cancel| {
            let _guard = guard;
            // All writers have ended before maintenance admission. Forget their finished
            // in-memory copies after explicit confirmation and before any file removal.
            owned.lock()?.error = None;
            owned.live_notes.forget(&meeting_id)?;
            owned.notes_jobs.forget(&meeting_id)?;
            owned.speaker_jobs.forget(&meeting_id)?;
            meeting_core::archive::erase_meeting(
                &owned.root,
                &meeting_id,
                &preview.fingerprint,
                &format!("DELETE {meeting_id}"),
                cancel,
                |n, total| jobs.update_work(job, n, total),
            )?;
            Ok(())
        })
}
/// Minimal interruption markers allow a new confirmation after restarting the application.
#[tauri::command]
pub(crate) async fn list_pending_erasures(
    service: State<'_, Arc<Service>>,
) -> AppResult<Vec<PendingErasure>> {
    let owned = Arc::clone(&service);
    blocking(move||meeting_core::archive::pending_erasures(&owned.root).map_err(|_|"Unterbrochene Löschungen konnten nicht geprüft werden. / Interrupted deletions could not be checked.".into())).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn previews_bind_confirmation_to_one_meeting_and_are_single_use() {
        let rows = Previews::default();
        let job = "a".repeat(32);
        let meeting = "b".repeat(32);
        rows.put(
            &job,
            ErasurePreview {
                meeting_id: meeting.clone(),
                fingerprint: "synthetic".into(),
                files: 1,
                bytes: 2,
                rebuild_search: true,
            },
        )
        .unwrap();
        assert!(rows.take(&job, &"c".repeat(32), "LÖSCHEN").is_err());
        assert!(rows.take(&job, &meeting, "yes").is_err());
        assert!(rows.get(&job).is_ok());
        assert!(rows.take(&job, &meeting, "LÖSCHEN").is_ok());
        assert!(rows.take(&job, &meeting, "LÖSCHEN").is_err());
    }
}
