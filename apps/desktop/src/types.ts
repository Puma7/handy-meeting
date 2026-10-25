export type Locale = "de" | "en";
export type Theme = "light" | "dark" | "system";
export type Track = "system" | "microphone";
export interface HandyModel {
  id: string;
  name: string;
  isDownloaded: boolean;
  supportsStreaming: boolean;
  compatibilityError?: string;
}
export interface HandyStatus {
  executable: string | null;
  version: string | null;
  models: HandyModel[];
  error?: string | null;
}
export interface AudioDevice {
  id: string;
  name: string;
  source: Track;
  isDefault?: boolean;
}
export interface AudioPart {
  source: Track;
  startSample: number;
  sampleCount: number;
  confirmedSampleCount?: number;
  text: string | null;
  status: string;
  committedText?: string;
  tentativeText?: string;
  file?: string;
  processedSamples?: number;
}
export interface Meeting {
  id: string;
  title: string;
  startedAt: string;
  durationMs: number;
  status: string;
  modelId: string;
  parts: AudioPart[];
  warnings: string[];
  livePreviewLimited?: boolean;
  readOnly?: boolean;
  pendingAudioMs?: number;
  savedAudioMs?: { system: number; microphone: number };
}
export type MeetingSummary = Omit<Meeting, "parts" | "warnings">;
export interface AppState {
  version: string;
  handy: HandyStatus | null;
  activeMeeting: Meeting | null;
  recording: boolean;
  transitioning?: boolean;
  error?: string | null;
  dataDirectory?: string;
  notesRunning?: boolean;
}
export type AnalysisKind = "summary" | "decisions" | "tasks" | "analysis";
export interface AnalysisStep {
  id: string;
  name: string;
  kind: AnalysisKind;
  prompt: string;
}
export interface AnalysisTemplate {
  id: string;
  name: string;
  steps: AnalysisStep[];
}
export interface Evidence {
  sourceId: string;
  quote: string;
}
export interface NoteFact {
  text: string;
  owner: string | null;
  due: string | null;
  evidence: Evidence[];
}
export interface TranscriptSource {
  id: string;
  source: Track;
  startMs: number;
  endMs: number;
  text: string;
}
export interface AnalysisResult {
  id: string;
  meetingId: string;
  sourceHash: string;
  template: AnalysisTemplate;
  sections: {
    id: string;
    name: string;
    kind: AnalysisKind;
    items: NoteFact[];
  }[];
  sources: TranscriptSource[];
  createdAt: string;
}
export interface StartRequest {
  title: string;
  modelId: string;
  systemDeviceId: string;
  microphoneDeviceId: string | null;
}
export interface NotesRequest {
  meetingId: string;
  template: AnalysisTemplate;
  endpoint: string;
  model: string;
  mode: "local" | "cloud";
  cloudConsent: boolean;
  apiKey: string;
}
export interface DesktopApi {
  getAppState(): Promise<AppState>;
  refreshHandy(executable: string | null): Promise<HandyStatus>;
  listDevices(): Promise<AudioDevice[]>;
  startMeeting(request: StartRequest): Promise<Meeting>;
  stopMeeting(): Promise<Meeting>;
  listMeetings(): Promise<MeetingSummary[]>;
  getMeeting(id: string): Promise<Meeting>;
  resumeMeeting(id: string): Promise<Meeting>;
  importLegacy(manifest: string): Promise<Meeting>;
  generateNotes(request: NotesRequest): Promise<AnalysisResult>;
  listAnalyses(meetingId: string): Promise<AnalysisResult[]>;
  copyText(text: string): Promise<void>;
}
