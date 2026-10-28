import { useId } from "react";
import type { AudioLevel, Locale } from "../types";

type Props = {
  label: string;
  level: AudioLevel | null;
  state?: "idle" | "stopped" | "paused" | "muted" | "unavailable" | "waiting";
  locale: Locale;
  descriptionId?: string;
};
const segments = Array.from({ length: 24 }, (_, index) => {
  const dbfs = -60 + (index + 1) * 2.5;
  return { dbfs, zone: dbfs > -6 ? "red" : dbfs > -18 ? "yellow" : "green" };
});

/** dBFS represents the digital capture peak, not a calibrated acoustic sound level. */
export default function AudioLevelMeter({
  label,
  level,
  state,
  locale,
  descriptionId,
}: Props) {
  const description = useId();
  const de = locale === "de";
  const status = state ?? level?.status ?? "waiting";
  const active =
    status === "active" &&
    level?.peakDbfs !== null &&
    level?.peakDbfs !== undefined &&
    Number.isFinite(level.peakDbfs);
  const peak = active ? level.peakDbfs! : null;
  const rms = active ? level.rmsDbfs : null;
  const clipping = active && level.clipped;
  const silent = peak !== null && peak <= -96;
  const statusText = {
    idle: de
      ? "Bei Aufnahmebeginn verfügbar"
      : "Available when recording starts",
    stopped: de ? "Aufnahme beendet" : "Recording stopped",
    paused: de ? "Pausiert" : "Paused",
    muted: de ? "In dieser App stumm" : "Muted in this app",
    unavailable: de ? "Nicht verfügbar" : "Unavailable",
    waiting: de ? "Warte auf Audiosignal" : "Waiting for audio samples",
    stale: de ? "Kein aktueller Pegel" : "Level unavailable",
    active: clipping
      ? de
        ? "Übersteuert"
        : "Clipping"
      : silent
        ? de
          ? "Stille"
          : "Silence"
        : peak !== null && peak < -60
          ? de
            ? "Sehr leises Signal"
            : "Very quiet signal"
          : de
            ? "Signal vorhanden"
            : "Signal present",
  }[status];
  const value =
    peak === null
      ? "—"
      : peak <= -96
        ? "≤ −96"
        : peak.toFixed(1).replace("-", "−");
  const meterName = `${label} · ${de ? "Digitaler Spitzenpegel" : "Digital peak"}`;
  return (
    <div
      className="audio-level"
      data-status={status}
      data-clipped={clipping || undefined}
    >
      <div className="audio-level-reading">
        <span id={description} className="audio-level-state">
          {statusText}
        </span>
        <span className="audio-level-value">
          {value}
          <small> dBFS</small>
        </span>
      </div>
      <div
        className="audio-level-bar"
        role={active ? "meter" : "img"}
        aria-label={meterName}
        aria-valuemin={active ? -96 : undefined}
        aria-valuemax={active ? 0 : undefined}
        aria-valuenow={peak ?? undefined}
        aria-valuetext={active ? `${value} dBFS · ${statusText}` : undefined}
        aria-describedby={[
          description,
          rms !== null ? `${description}-rms` : null,
          descriptionId,
        ]
          .filter(Boolean)
          .join(" ")}
      >
        {segments.map(({ dbfs, zone }, index) => (
          <span
            key={dbfs}
            aria-hidden="true"
            className={`audio-level-segment ${zone}`}
            data-lit={
              (peak !== null && !silent && (peak >= dbfs || index === 0)) ||
              undefined
            }
          />
        ))}
      </div>
      <div className="audio-level-scale" aria-hidden="true">
        <span>−60</span>
        <span>−36</span>
        <span>−18</span>
        <span>−6</span>
        <span>0</span>
      </div>
      {rms !== null && (
        <span id={`${description}-rms`} className="sr-only">
          RMS {rms.toFixed(1)} dBFS
        </span>
      )}
    </div>
  );
}
