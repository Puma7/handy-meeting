import { invoke, isTauri } from "@tauri-apps/api/core";
import type { DesktopApi } from "./types";
function desktopInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauri())
    return Promise.reject(
      "Diese Ansicht benötigt die installierte HandyMeeting-Desktop-App. / Open the installed HandyMeeting desktop application.",
    );
  return invoke<T>(command, args);
}
export const desktopApi: DesktopApi = {
  listAudioCopies: (offset) => desktopInvoke("list_audio_copies", { offset }),
  transcribeAudioCopy: (runId, modelId, confirmed) =>
    desktopInvoke("transcribe_audio_copy", { runId, modelId, confirmed }),
  compactArchive: (archiveId, confirmed) =>
    desktopInvoke("compact_archive", { archiveId, confirmed }),
  listArchives: (offset, limit) =>
    desktopInvoke("list_archives", { offset, limit }),
  restoreArchive: (archiveId, confirmed) =>
    desktopInvoke("restore_archive", { archiveId, confirmed }),
  listUnfinishedDeletions: () => desktopInvoke("list_unfinished_deletions"),
  resumePermanentDeletion: (logId, confirmation) =>
    desktopInvoke("resume_permanent_deletion", { logId, confirmation }),
  applyRetention: (planId, candidateIndex, confirmation) =>
    desktopInvoke("apply_retention", { planId, candidateIndex, confirmation }),
  getRetentionPolicy: () => desktopInvoke("get_retention_policy"),
  saveRetentionPolicy: (expectedRevision, policy) =>
    desktopInvoke("save_retention_policy", { expectedRevision, policy }),
  previewRetention: () => desktopInvoke("preview_retention"),
  getRetentionPlan: (jobId) => desktopInvoke("get_retention_plan", { jobId }),
  getMaintenanceProgress: () => desktopInvoke("get_maintenance_progress"),
  cancelMaintenance: (jobId) => desktopInvoke("cancel_maintenance", { jobId }),
  startSpeakerExperiment: (meetingId, modelId, mode) =>
    desktopInvoke("start_speaker_experiment", { meetingId, modelId, mode }),
  getSpeakerProgress: () => desktopInvoke("get_speaker_progress"),
  cancelSpeakerExperiment: (jobId) =>
    desktopInvoke("cancel_speaker_experiment", { jobId }),
  getSpeakerSections: (meetingId) =>
    desktopInvoke("get_speaker_sections", { meetingId }),
  checkForUpdate: () => desktopInvoke("check_for_update"),
  downloadUpdate: () => desktopInvoke("download_update"),
  getUpdateProgress: () => desktopInvoke("get_update_progress"),
  cancelUpdate: () => desktopInvoke("cancel_update"),
  installUpdate: () => desktopInvoke("install_update"),
  getSpeakerLabels: (meetingId) =>
    desktopInvoke("get_speaker_labels", { meetingId }),
  saveSpeakerLabels: (meetingId, expectedRevision, enabled, assignments) =>
    desktopInvoke("save_speaker_labels", {
      meetingId,
      expectedRevision,
      enabled,
      assignments,
    }),
  editMeetingMetadata: (meetingId, expectedRevision, title, pinned) =>
    desktopInvoke("edit_meeting_metadata", {
      meetingId,
      expectedRevision,
      title,
      pinned,
    }),
  searchMeetings: (query, offset, limit) =>
    desktopInvoke("search_meetings", { query, offset, limit }),
  rebuildSearchIndex: () => desktopInvoke("rebuild_search_index"),
  getLibraryProgress: () => desktopInvoke("get_library_progress"),
  cancelLibraryJob: (jobId) => desktopInvoke("cancel_library_job", { jobId }),
  getAppState: () => desktopInvoke("get_app_state"),
  refreshHandy: (executable) => desktopInvoke("refresh_handy", { executable }),
  listDevices: () => desktopInvoke("list_devices"),
  startMeeting: (request) => desktopInvoke("start_meeting", { request }),
  stopMeeting: () => desktopInvoke("stop_meeting"),
  listMeetings: () => desktopInvoke("list_meetings"),
  getMeeting: (id) => desktopInvoke("get_meeting", { id }),
  resumeMeeting: (id) => desktopInvoke("resume_meeting", { id }),
  importLegacy: (manifest) => desktopInvoke("import_legacy", { manifest }),
  generateNotes: (request) => desktopInvoke("generate_notes", { request }),
  listAnalyses: (meetingId) => desktopInvoke("list_analyses", { meetingId }),
  listUnfinishedAnalyses: (meetingId) =>
    desktopInvoke("list_unfinished_analyses", { meetingId }),
  listTextModels: (endpoint) =>
    desktopInvoke("list_text_models", { endpoint, mode: "local" }),
  getNotesProgress: () => desktopInvoke("get_notes_progress"),
  cancelNotes: (jobId) => desktopInvoke("cancel_notes", { jobId }),
  notesMarkdown: (meetingId, analysisId) =>
    desktopInvoke("notes_markdown", { meetingId, analysisId }),
  copyText: (text) => desktopInvoke("copy_text", { text }),
  openMeetingFolder: (id) => desktopInvoke("open_meeting_folder", { id }),
  exportTranscript: (id, locale) =>
    desktopInvoke("export_transcript", { id, locale }),
  listTemplates: () => desktopInvoke("list_templates"),
  saveTemplates: (library) => desktopInvoke("save_templates", { library }),
  importTemplates: (templates) =>
    desktopInvoke("import_templates", { templates }),
  credentialStatus: (endpoint, mode) =>
    desktopInvoke("credential_status", { endpoint, mode }),
  saveCredential: (request) => desktopInvoke("save_credential", { request }),
  removeCredential: (endpoint, mode, confirm) =>
    desktopInvoke("remove_credential", { endpoint, mode, confirm }),
  agentDecideRecording: (requestId, approve, request) =>
    desktopInvoke("agent_decide_recording", { requestId, approve, request }),
  agentRevoke: () => desktopInvoke("agent_revoke"),
};
export function errorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : typeof error === "string"
      ? error
      : "Die Desktop-Verbindung ist nicht verfügbar. Bitte die installierte HandyMeeting-App öffnen. / Open the installed desktop application.";
}
