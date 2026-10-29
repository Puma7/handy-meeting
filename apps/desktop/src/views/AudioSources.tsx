import { Mic, Monitor, RefreshCw } from "lucide-react";
import { useId } from "react";
import type { AudioDevice, CaptureControl, DesktopApi, Locale } from "../types";
import type { MessageKey } from "../i18n";
import { useAudioLevels } from "../useAudioLevels";
import AudioLevelMeter from "./AudioLevelMeter";

type Props = {
  t: Record<MessageKey, string>;
  locale: Locale;
  devices: AudioDevice[];
  systemId: string;
  micId: string;
  micEnabled: boolean;
  recording: boolean;
  busy: boolean;
  connected: boolean;
  isStopping: boolean;
  activeMeetingId: string | null;
  displayedMeetingId: string | null;
  captureControl?: CaptureControl | null;
  getAudioLevels?: DesktopApi["getAudioLevels"];
  setSystemId: (value: string) => void;
  setMicId: (value: string) => void;
  setMicEnabled: (value: boolean) => void;
  compact?: boolean;
  /** Re-reads the device list; shown beside the system-audio choice. */
  onRefresh?: () => void;
};

/** The source controls and their meters stay visible when title/model setup collapses. */
export default function AudioSources({
  t,
  locale,
  devices,
  systemId,
  micId,
  micEnabled,
  recording,
  busy,
  connected,
  isStopping,
  activeMeetingId,
  displayedMeetingId,
  captureControl,
  getAudioLevels,
  setSystemId,
  setMicId,
  setMicEnabled,
  compact = false,
  onRefresh,
}: Props) {
  const levelExplanation = useId();
  const bound =
    recording &&
    activeMeetingId !== null &&
    activeMeetingId === displayedMeetingId;
  const levels = useAudioLevels(
    getAudioLevels,
    bound ? activeMeetingId : null,
    bound && connected && !isStopping,
    captureControl,
  );
  const commonState = !recording
    ? displayedMeetingId
      ? "stopped"
      : "idle"
    : !bound || !connected || !getAudioLevels || !levels.visible
      ? "unavailable"
      : isStopping
        ? "stopped"
        : captureControl?.paused
          ? "paused"
          : undefined;
  const microphoneState =
    commonState ??
    (captureControl?.microphoneMuted
      ? "muted"
      : captureControl?.hasMicrophone === false
        ? "unavailable"
        : undefined);
  const microphoneIncluded = recording
    ? (captureControl?.hasMicrophone ?? micEnabled)
    : micEnabled;
  const hasSystemDevice = devices.some((item) => item.source === "system");
  return (
    <div className={`audio-sources${compact ? " compact" : ""}`}>
      <div className="audio-source-grid">
        <div className="audio-source">
          <label>
            <span className="label-icon">
              <Monitor size={15} />
              {t.system}
            </span>
            {!compact && (
              <div className="inline-control">
                <select
                  value={systemId}
                  disabled={recording || busy || !hasSystemDevice}
                  onChange={(event) => setSystemId(event.target.value)}
                >
                  {!hasSystemDevice && <option value="">{t.noDevices}</option>}
                  {devices
                    .filter((item) => item.source === "system")
                    .map((item) => (
                      <option key={item.id} value={item.id}>
                        {item.name}
                      </option>
                    ))}
                </select>
                {onRefresh && (
                  <button
                    type="button"
                    className="icon-button bordered"
                    title={t.refreshDevices}
                    aria-label={t.refreshDevices}
                    disabled={busy || !connected}
                    onClick={onRefresh}
                  >
                    <RefreshCw size={17} />
                  </button>
                )}
              </div>
            )}
          </label>
          <AudioLevelMeter
            label={t.system}
            level={levels.system}
            state={commonState}
            locale={locale}
            descriptionId={levelExplanation}
          />
        </div>
        <div className="audio-source microphone-field">
          <label className="checkbox-label">
            {!compact && (
              <input
                type="checkbox"
                checked={microphoneIncluded}
                disabled={recording || busy}
                onChange={(event) => setMicEnabled(event.target.checked)}
              />
            )}
            <Mic size={15} />
            {t.microphone}
          </label>
          {!compact && (
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
          )}
          <AudioLevelMeter
            label={t.sourceMicrophone}
            level={levels.microphone}
            state={microphoneState}
            locale={locale}
            descriptionId={levelExplanation}
          />
        </div>
      </div>
      <p className={compact ? "sr-only" : "audio-level-help"}>
        {locale === "de"
          ? "Die Balken zeigen den digitalen Spitzenpegel; 0 dBFS ist das Maximum. Sprache liegt meist im grünen bis gelben Bereich, Pausen 20–40 dB darunter, das ist normal. Rot heißt knapp unter dem Maximum. „Übersteuert“ heißt, der Ton kam schon abgeschnitten an; das lässt sich nachträglich nicht reparieren. Dann den Eingangspegel in den Systemeinstellungen senken oder mehr Abstand zum Mikrofon halten."
          : "The bars show the digital peak level; 0 dBFS is the maximum. Speech is usually in the green to yellow range and pauses 20–40 dB lower, which is normal. Red means just below the maximum. “Clipping” means the audio already arrived cut off, which cannot be repaired afterwards. Then lower the input level in the system settings or keep more distance from the microphone."}
      </p>
      <p id={levelExplanation} className="sr-only">
        {locale === "de"
          ? "Digitaler Spitzenpegel in dBFS. Keine kalibrierte Lautstärkemessung und kein Nachweis, dass Audio bereits gespeichert ist."
          : "Digital peak in dBFS. Not a calibrated loudness measurement or confirmation that audio has been saved."}
      </p>
    </div>
  );
}
