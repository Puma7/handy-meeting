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
