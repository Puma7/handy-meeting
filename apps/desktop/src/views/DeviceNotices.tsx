import { X } from "lucide-react";
import type { DeviceLoss } from "../deviceWatch";
import type { MessageKey } from "../i18n";
import type { Locale } from "../types";

type Props = {
  losses: DeviceLoss[];
  locale: Locale;
  t: Record<MessageKey, string>;
  onDismiss: (loss: DeviceLoss) => void;
};

/** Explains what a disappeared audio device means for the recording or the selection. */
function message(loss: DeviceLoss, locale: Locale) {
  const de = locale === "de";
  const microphone = loss.source === "microphone";
  const kind = microphone
    ? de
      ? "Das Mikrofon"
      : "The microphone"
    : de
      ? "Die Computerton-Ausgabe"
      : "The system-audio output";
  const prefix = !loss.name
    ? kind
    : de
      ? `${kind} „${loss.name}“`
      : `${kind} “${loss.name}”`;
  if (loss.duringRecording && loss.confirmed) {
    return de
      ? `${prefix} ist während der Aufnahme verschwunden. Diese Spur nimmt nicht mehr auf; bereits gespeicherter Ton bleibt erhalten.${microphone ? " Der Computerton läuft weiter." : ""} Gerät wieder anschließen, die Aufnahme beenden und eine neue starten.`
      : `${prefix} disappeared during the recording. This track no longer records; audio saved so far is kept.${microphone ? " System audio keeps recording." : ""} Reconnect the device, end the recording and start a new one.`;
  }
  if (loss.duringRecording) {
    // Only the device list changed; the capture has not confirmed a stop (yet).
    return de
      ? `${prefix} ist während der Aufnahme aus der Geräteliste verschwunden. Diese Spur nimmt dann meist nicht mehr auf; bereits gespeicherter Ton bleibt erhalten. Den Pegel prüfen und bei Bedarf das Gerät wieder anschließen, die Aufnahme beenden und eine neue starten.`
      : `${prefix} left the device list during the recording. This track usually stops recording then; audio saved so far is kept. Check its level and, if needed, reconnect the device, end the recording and start a new one.`;
  }
  if (loss.replacement) {
    return de
      ? `${prefix} ist nicht mehr verfügbar. Ausgewählt ist jetzt „${loss.replacement}“.`
      : `${prefix} is no longer available. “${loss.replacement}” is selected now.`;
  }
  return de
    ? `${prefix} ist nicht mehr verfügbar, und es gibt kein anderes Gerät dieser Art.`
    : `${prefix} is no longer available, and no other device of this kind is listed.`;
}

/** Device-loss notices stay until dismissed; during a recording they are alerts. */
export default function DeviceNotices({ losses, locale, t, onDismiss }: Props) {
  return (
    <>
      {losses.map((loss) => (
        <div
          key={`${loss.source}:${loss.id}`}
          role={loss.duringRecording ? "alert" : "status"}
          className={`notice ${loss.duringRecording ? "error" : "warning"}`}
        >
          <span>{message(loss, locale)}</span>
          <button
            className="icon-button"
            aria-label={t.dismiss}
            onClick={() => onDismiss(loss)}
          >
            <X size={17} />
          </button>
        </div>
      ))}
    </>
  );
}
