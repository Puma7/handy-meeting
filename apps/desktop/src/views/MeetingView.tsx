import {
  AudioLines,
  Check,
  Copy,
  Mic,
  Monitor,
  RefreshCw,
  Square,
  Sparkles,
} from "lucide-react";
import type {
  AppState,
  AudioDevice,
  HandyModel,
  Meeting,
  AudioPart,
} from "../types";
import type { MessageKey, extraDe } from "../i18n";
import { clock } from "../domain";
import TranscriptBody from "./TranscriptBody";
import { useEffect, useState } from "react";
import SessionWarnings from "./SessionWarnings";
import MeetingFiles from "./MeetingFiles";
type Props = {
  t: Record<MessageKey, string>;
  x: Record<keyof typeof extraDe, string>;
  app: AppState | null;
  meeting: Meeting | null;
  recording: boolean;
  isStopping: boolean;
  busy: boolean;
  connected: boolean;
  title: string;
  defaultTitle: string;
  onAnalyse: () => void;
  onOpenFolder: () => void;
  onExport: () => void;
  savedFile: string | null;
  modelId: string;
  systemId: string;
  micId: string;
  micEnabled: boolean;
  models: HandyModel[];
  devices: AudioDevice[];
  copied: boolean;
  backlog: number | null;
  recentParts: AudioPart[];
  locale: "de" | "en";
  setTitle: (v: string) => void;
  setModelId: (v: string) => void;
  setSystemId: (v: string) => void;
  setMicId: (v: string) => void;
  setMicEnabled: (v: boolean) => void;
  refreshHandy: () => void;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  savedByTrack: (track: "system" | "microphone") => number;
  resume: () => void;
  copyFullTranscript: () => Promise<void>;
};
export default function MeetingView({
  t,
  x,
  app,
  meeting,
  recording,
  isStopping,
  busy,
  connected,
  title,
  defaultTitle,
  onAnalyse,
  onOpenFolder,
  onExport,
  savedFile,
  modelId,
  systemId,
  micId,
  micEnabled,
  models,
  devices,
  copied,
  backlog,
  recentParts,
  locale,
  setTitle,
  setModelId,
  setSystemId,
  setMicId,
  setMicEnabled,
  refreshHandy,
  start,
  stop,
  savedByTrack,
  resume,
  copyFullTranscript,
}: Props) {
  const [setupOpen, setSetupOpen] = useState(!meeting && !recording);
  useEffect(() => {
    if (meeting) setSetupOpen(false);
  }, [recording, meeting?.id]);
  return (
    <>
      {!!app?.handy?.error && (
        <div className="notice warning">{app.handy.error}</div>
      )}
      {connected && !devices.some((item) => item.source === "system") && (
        <div className="notice warning" role="status">
          {t.noSystemHelp}
        </div>
      )}
      {connected &&
        micEnabled &&
        !devices.some((item) => item.source === "microphone") && (
          <div className="notice warning" role="status">
            {t.noMicrophoneHelp}
          </div>
        )}
      <section className="setup-panel" aria-label={t.meeting}>
        <details
          className="setup-details"
          open={setupOpen}
          onToggle={(event) => setSetupOpen(event.currentTarget.open)}
        >
          <summary>
            <strong>{t.captureSetup}</strong>
            <span>
              {models.find((model) => model.id === modelId)?.name ?? t.choose}
            </span>
          </summary>
          <div className="setup-grid">
            <label className="full">
              {t.title}
              <input
                value={title}
                disabled={recording || busy}
                placeholder={defaultTitle}
                maxLength={200}
                onChange={(event) => setTitle(event.target.value)}
              />
            </label>
            <label className="full">
              {t.model}
              <div className="inline-control">
                <select
                  title={t.modelTooltip}
                  value={modelId}
                  disabled={recording || busy || !models.length}
                  onChange={(event) => setModelId(event.target.value)}
                >
                  <option value="">
                    {models.length ? t.choose : t.noModels}
                  </option>
                  {models.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.name}
                    </option>
                  ))}
                </select>
                <button
                  className="icon-button bordered"
                  title={t.refresh}
                  aria-label={t.refresh}
                  disabled={busy || recording || !connected}
                  onClick={refreshHandy}
                >
                  <RefreshCw size={17} />
                </button>
              </div>
            </label>
            <label>
              <span className="label-icon">
                <Monitor size={15} />
                {t.system}
              </span>
              <select
                value={systemId}
                disabled={
                  recording ||
                  busy ||
                  !devices.some((item) => item.source === "system")
                }
                onChange={(event) => setSystemId(event.target.value)}
              >
                {!devices.some((item) => item.source === "system") && (
                  <option value="">{t.noDevices}</option>
                )}
                {devices
                  .filter((item) => item.source === "system")
                  .map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.name}
                    </option>
                  ))}
              </select>
            </label>
            <div className="microphone-field">
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={micEnabled}
                  disabled={recording || busy}
                  onChange={(event) => setMicEnabled(event.target.checked)}
                />
                <Mic size={15} />
                {t.microphone}
              </label>
              <select
                aria-label={t.sourceMicrophone}
                value={micId}
                disabled={!micEnabled || recording || busy}
                onChange={(event) => setMicId(event.target.value)}
              >
                {!devices.some((item) => item.source === "microphone") && (
                  <option value="">{t.noDevices}</option>
                )}
                {devices
                  .filter((item) => item.source === "microphone")
                  .map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.name}
                    </option>
                  ))}
              </select>
            </div>
          </div>
        </details>
        <div className="record-controls full">
          <span className={recording ? "state recording" : "state"}>
            <span className="state-dot" />
            {isStopping
              ? t.stopping
              : app?.transitioning ||
                  (!recording &&
                    ["catching-up", "streaming"].includes(
                      app?.activeMeeting?.status ?? "",
                    ))
                ? t.busy
                : recording
                  ? t.recording
                  : connected && modelId && systemId
                    ? t.ready
                    : t.choose}
          </span>
          {recording || isStopping ? (
            <button
              className="danger"
              disabled={busy || isStopping}
              title={t.stopTooltip}
              onClick={() => void stop()}
            >
              {isStopping ? (
                <RefreshCw size={16} className="spinning" />
              ) : (
                <Square size={16} />
              )}
              {isStopping ? t.stopping : t.stop}
            </button>
          ) : (
            <button
              className="primary"
              disabled={
                !connected ||
                busy ||
                !modelId ||
                !systemId ||
                (micEnabled && !micId)
              }
              title={t.startTooltip}
              onClick={() => void start()}
            >
              <AudioLines size={18} />
              {busy ? t.busy : t.start}
            </button>
          )}
        </div>
      </section>
      {meeting && (
        <div className="session-progress">
          <div>
            <span>{t.duration}</span>
            <strong>{clock(meeting.durationMs)}</strong>
          </div>
          <div>
            <span>
              {t.saved} · {t.sourceSystem}
            </span>
            <strong>{clock(savedByTrack("system"))}</strong>
          </div>
          {meeting.parts.some((item) => item.source === "microphone") && (
            <div>
              <span>
                {t.saved} · {t.sourceMicrophone}
              </span>
              <strong>{clock(savedByTrack("microphone"))}</strong>
            </div>
          )}
          <div>
            <span>{t.lag}</span>
            <strong>{backlog === null ? "—" : clock(backlog)}</strong>
          </div>
        </div>
      )}
      {meeting && (
        <SessionWarnings warnings={meeting.warnings} locale={locale} t={t} />
      )}
      {meeting &&
        !recording &&
        meeting.status !== "complete" &&
        !meeting.readOnly && (
          <div className="recovery-actions">
            <button
              className="secondary"
              disabled={busy || !connected}
              onClick={resume}
            >
              <RefreshCw size={15} />
              {x.resume}
            </button>
          </div>
        )}
      <section className="transcript-panel">
        <div className="section-heading">
          <div>
            <h2>{meeting?.title ?? t.live}</h2>
            <p>{t.sourceTimes}</p>
          </div>
          <div className="detail-actions">
            <button
              className="secondary"
              disabled={
                busy || !meeting?.parts.some((item) => (item.text ?? "").trim())
              }
              title={t.copyTooltip}
              onClick={() => void copyFullTranscript()}
            >
              {copied ? <Check size={16} /> : <Copy size={16} />}
              <span>{copied ? t.copied : t.copy}</span>
            </button>
            {meeting?.status === "complete" && (
              <button
                className="primary"
                disabled={busy || !recentParts.length}
                onClick={onAnalyse}
              >
                <Sparkles size={16} />
                {t.analyzeMeeting}
              </button>
            )}
          </div>
        </div>
        {meeting && (
          <div className="meeting-files">
            <MeetingFiles
              t={t}
              disabled={busy || !connected}
              onOpenFolder={onOpenFolder}
              onExport={onExport}
              savedFile={savedFile}
            />
          </div>
        )}
        <TranscriptBody parts={recentParts} locale={locale} t={t} />
      </section>
    </>
  );
}
