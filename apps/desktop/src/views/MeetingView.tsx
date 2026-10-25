import {
  AudioLines,
  Check,
  Copy,
  Mic,
  Monitor,
  RefreshCw,
  Square,
} from "lucide-react";
import type {
  AppState,
  AudioDevice,
  HandyModel,
  Meeting,
  AudioPart,
} from "../types";
import type { MessageKey, extraDe } from "../i18n";
import { clock, trackName } from "../domain";
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
  return (
    <>
      {!!app?.handy?.error && (
        <div className="notice warning">{app.handy.error}</div>
      )}
      <section className="setup-panel" aria-label={t.meeting}>
        <label className="full">
          {t.title}
          <input
            value={title}
            disabled={recording || busy}
            placeholder={t.emptyTitle}
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
              <option value="">{models.length ? t.choose : t.noModels}</option>
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
      {!!meeting?.warnings.length && (
        <div className="notice warning">
          <div>
            <strong>{t.warnings}</strong>
            {meeting.warnings.map((warning, index) => (
              <p key={index}>{warning}</p>
            ))}
          </div>
        </div>
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
        </div>
        <div className="transcript-content" aria-live="polite">
          {recentParts.length ? (
            recentParts.map((part, index) => (
              <article
                className="transcript-part"
                key={part.file ?? part.source + part.startSample + index}
              >
                <div className="part-meta">
                  <span>
                    {clock(part.startSample / 16)}–
                    {clock((part.startSample + part.sampleCount) / 16)}
                  </span>
                  <span>{trackName(part.source, locale)}</span>
                  {part.status !== "complete" && <span>{t.pending}</span>}
                </div>
                <p>{part.text}</p>
              </article>
            ))
          ) : (
            <div className="empty">
              <AudioLines size={30} />
              <p>{t.emptyTranscript}</p>
            </div>
          )}
        </div>
      </section>
    </>
  );
}
