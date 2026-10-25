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
  copyText: (text) => desktopInvoke("copy_text", { text }),
};
export function errorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : typeof error === "string"
      ? error
      : "Die Desktop-Verbindung ist nicht verfügbar. Bitte die installierte HandyMeeting-App öffnen. / Open the installed desktop application.";
}
