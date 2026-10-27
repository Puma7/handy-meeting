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
  userMetadata?: MeetingMetadata;
  metadataError?: string | null;
}
export interface MeetingMetadata {
  revision: number;
  title: string | null;
  pinned: boolean;
  audioRemoved: boolean;
}
export interface SearchHit {
  id: string;
  title: string;
  startedAt: string;
  durationMs: number;
  status: string;
  modelId: string;
  pinned: boolean;
  audioRemoved: boolean;
  metadataRevision: number;
  sourceId: string | null;
  startMs: number | null;
  endMs: number | null;
  excerpt: string;
}
export interface SearchPage {
  items: SearchHit[];
  total: number;
  offset: number;
  limit: number;
}
export interface LibraryProgress {
  jobId: string;
  kind: string;
  status: "running" | "complete" | "cancelled" | "failed";
  confirmedTasks: number;
  totalTasks: number;
  elapsedMs: number;
  skipped: number;
  error: string | null;
  cancelRequested: boolean;
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
  agent?: AgentState | null;
}
export interface AgentState {
  connected: boolean;
  label: string;
  ownerId: string;
  revoked: boolean;
  pending: {
    requestId: string;
    title: string;
    includeMicrophone: boolean;
    maxDurationSeconds: number;
    status: "pending" | "starting";
  } | null;
  ownedSessionId: string | null;
  expiresAtUnixMs: number | null;
  activeCapture?: boolean;
  lastOutcome?: string | null;
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
export interface TemplateLibrary {
  revision: number;
  templates: AnalysisTemplate[];
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
export interface PendingAnalysis {
  id: string;
  meetingId: string;
  sourceHash: string;
  template: AnalysisTemplate;
  createdAt: string;
  model: string;
  mode: "local" | "cloud";
  endpoint: string;
  confirmedTasks: number;
  totalTasks: number;
}
export interface NotesRequest {
  jobId?: string;
  resumeRunId?: string;
  meetingId: string;
  template: AnalysisTemplate;
  endpoint: string;
  model: string;
  mode: "local" | "cloud";
  cloudConsent: boolean;
  apiKey: string;
  useSavedKey?: boolean;
}
export interface CredentialStatus {
  state: "missing" | "stored" | "unavailable";
  provider: string;
  message: string | null;
}
export interface NotesProgress {
  jobId: string;
  runId: string | null;
  meetingId: string;
  status: "running" | "complete" | "cancelled" | "failed";
  stage: "preparing" | "requesting" | "validating" | "saving" | "finished";
  stepName: string | null;
  stepIndex: number;
  totalSteps: number;
  batchIndex: number;
  totalBatches: number;
  confirmedTasks: number;
  totalTasks: number;
  elapsedMs: number;
  error: string | null;
  cancelRequested: boolean;
}
export interface DesktopApi {
  listAudioCopies?(offset: number): Promise<AudioCopy[]>;
  transcribeAudioCopy?(
    runId: string,
    modelId: string,
    confirmed: boolean,
  ): Promise<string>;
  compactArchive?(archiveId: string, confirmed: boolean): Promise<string>;
  listArchives?(offset: number, limit: number): Promise<ArchivePage>;
  restoreArchive?(archiveId: string, confirmed: boolean): Promise<string>;
  listUnfinishedDeletions?(): Promise<UnfinishedDeletion[]>;
  resumePermanentDeletion?(
    logId: string,
    confirmation: string,
  ): Promise<string>;
  applyRetention?(
    planId: string,
    candidateIndex: number,
    confirmation: string,
  ): Promise<string>;
  getRetentionPolicy?(): Promise<RetentionPolicy>;
  saveRetentionPolicy?(
    expectedRevision: number,
    policy: RetentionPolicy,
  ): Promise<RetentionPolicy>;
  previewRetention?(): Promise<string>;
  getRetentionPlan?(jobId: string): Promise<RetentionPlan>;
  getMaintenanceProgress?(): Promise<LibraryProgress | null>;
  cancelMaintenance?(jobId: string): Promise<boolean>;
  startSpeakerExperiment?(
    meetingId: string,
    modelId: string,
    mode: "live" | "post-meeting",
  ): Promise<string>;
  getSpeakerProgress?(): Promise<SpeakerProgress | null>;
  cancelSpeakerExperiment?(jobId: string): Promise<boolean>;
  getSpeakerSections?(meetingId: string): Promise<SavedSpeakerSections | null>;
  checkForUpdate?(): Promise<UpdateProgress>;
  downloadUpdate?(): Promise<UpdateProgress>;
  getUpdateProgress?(): Promise<UpdateProgress>;
  cancelUpdate?(): Promise<void>;
  installUpdate?(): Promise<void>;
  getSpeakerLabels?(
    meetingId: string,
  ): Promise<SpeakerDocument & { sources: Record<string, string> }>;
  saveSpeakerLabels?(
    meetingId: string,
    expectedRevision: number,
    enabled: boolean,
    assignments: SpeakerAssignment[],
  ): Promise<SpeakerDocument>;
  editMeetingMetadata?(
    meetingId: string,
    expectedRevision: number,
    title: string | null,
    pinned: boolean,
  ): Promise<MeetingMetadata>;
  searchMeetings?(
    query: string,
    offset: number,
    limit: number,
  ): Promise<SearchPage>;
  rebuildSearchIndex?(): Promise<string>;
  getLibraryProgress?(): Promise<LibraryProgress | null>;
  cancelLibraryJob?(jobId: string): Promise<boolean>;
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
  listUnfinishedAnalyses(meetingId: string): Promise<PendingAnalysis[]>;
  listTextModels(endpoint: string): Promise<string[]>;
  getNotesProgress(): Promise<NotesProgress | null>;
  cancelNotes(jobId: string): Promise<boolean>;
  notesMarkdown(meetingId: string, analysisId: string): Promise<string>;
  copyText(text: string): Promise<void>;
  openMeetingFolder(id: string): Promise<void>;
  exportTranscript(id: string, locale: Locale): Promise<string>;
  listTemplates(): Promise<TemplateLibrary>;
  saveTemplates(library: TemplateLibrary): Promise<TemplateLibrary>;
  importTemplates(templates: AnalysisTemplate[]): Promise<TemplateLibrary>;
  credentialStatus(
    endpoint: string,
    mode: "local" | "cloud",
  ): Promise<CredentialStatus>;
  saveCredential(request: {
    endpoint: string;
    mode: "local" | "cloud";
    apiKey: string;
    replaceExisting: boolean;
  }): Promise<CredentialStatus>;
  removeCredential(
    endpoint: string,
    mode: "local" | "cloud",
    confirm: boolean,
  ): Promise<CredentialStatus>;
  agentDecideRecording(
    requestId: string,
    approve: boolean,
    request?: StartRequest,
  ): Promise<void>;
  agentRevoke(): Promise<void>;
}
export interface SpeakerAssignment {
  sourceId: string;
  textSha256: string;
  name: string;
  evidence: string;
}
export interface RetentionPolicy {
  revision: number;
  untouchedDays: number;
  audioArchiveAfterDays: number | null;
  archiveAfterDays: number | null;
  convertAfterDays: number | null;
  maxVisibleMeetings: number | null;
  deleteArchivesAfterDays: number | null;
  codec: "flac" | "opus";
  ffmpegExecutable: string | null;
}
export interface RetentionCandidate {
  meetingId: string;
  title: string;
  action:
    | "archive-audio"
    | "archive-meeting"
    | "convert-audio"
    | "delete-archived-meeting";
  archiveId: string | null;
  metadataRevision: number;
  originalManifestSha256: string;
  ageDays: number;
  reason: string;
}
export interface RetentionPlan {
  id: string;
  policyRevision: number;
  createdAt: string;
  scanned: number;
  protected: number;
  unreadable: number;
  candidates: RetentionCandidate[];
  limited: boolean;
}
export interface SpeakerTurn {
  startMs: number;
  endMs: number;
  speakerId: number;
  confidence: number | null;
}
export interface SpeakerSection {
  sourceId: string;
  startMs: number;
  durationMs: number;
  audioSha256: string;
  turns: SpeakerTurn[];
}
export interface SpeakerProgress {
  jobId: string;
  meetingId: string;
  mode: string;
  status: string;
  stage: string;
  modelId: string;
  confirmedTasks: number;
  totalTasks: number;
  elapsedMs: number;
  error: string | null;
  cancelRequested: boolean;
  lastSection: SpeakerSection | null;
}
export interface SavedSpeakerSections {
  runId: string;
  mode: string;
  modelId: string;
  sections: SpeakerSection[];
}
export interface UpdateProgress {
  status: string;
  version: string | null;
  releaseUrl: string | null;
  receivedBytes: number;
  totalBytes: number;
  elapsedMs: number;
  error: string | null;
  attempt?: number;
}
export interface SpeakerDocument {
  revision: number;
  enabled: boolean;
  assignments: SpeakerAssignment[];
}

export interface ArchiveReceipt {
  archiveId: string;
  meetingId: string;
  kind: string;
  archiveBytes: number;
  originalBytes: number;
  createdAt: string;
}
export interface ArchiveEntry {
  archiveId: string;
  /** Null when the receipt could not be read; its files are still preserved. */
  receipt: ArchiveReceipt | null;
  title: string;
  sessionPresent: boolean;
  sourceCopyRetained: boolean;
  /** Which record could not be read. Such entries offer no restore or compaction. */
  unreadable: "receipt" | "labels" | null;
}
export interface UnfinishedDeletion {
  logId: string;
  meetingId: string;
  deletedFiles: number;
  totalFiles: number;
  confirmedAt: string;
}
export interface ArchivePage {
  items: ArchiveEntry[];
  more: boolean;
}

export interface AudioCopy {
  runId: string;
  meetingId: string;
  title: string;
  codec: "flac" | "opus";
  createdAt: string;
  sections: number;
}
