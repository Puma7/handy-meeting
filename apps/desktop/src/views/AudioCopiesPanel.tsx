import { useCallback, useEffect, useState } from "react";
import { errorMessage } from "../api";
import type {
  AudioCopy,
  DesktopApi,
  HandyModel,
  LibraryProgress,
  Locale,
} from "../types";
export default function AudioCopiesPanel({
  api,
  locale,
  models,
  job,
}: {
  api: DesktopApi;
  locale: Locale;
  models: HandyModel[];
  job: LibraryProgress | null;
}) {
  const de = locale === "de";
  const [copies, setCopies] = useState<AudioCopy[]>([]),
    [offset, setOffset] = useState(0),
    [selected, setSelected] = useState<AudioCopy | null>(null),
    [model, setModel] = useState(""),
    [accepted, setAccepted] = useState(false),
    [error, setError] = useState<string | null>(null),
    [busy, setBusy] = useState(false);
  const load = useCallback(async () => {
    if (!api.listAudioCopies) return;
    try {
      setCopies(await api.listAudioCopies(offset));
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [api, offset]);
  useEffect(() => {
    void load();
  }, [load, job?.jobId, job?.status]);
  if (!api.listAudioCopies) return null;
  const running = job?.status === "running";
  async function start() {
    if (!selected || !model || !accepted || !api.transcribeAudioCopy) return;
    setBusy(true);
    setError(null);
    try {
      await api.transcribeAudioCopy(selected.runId, model, true);
      setSelected(null);
      setAccepted(false);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="retention-preview">
      <h3>
        {de
          ? "Kompakte Audiokopien erneut transkribieren"
          : "Transcribe compact audio copies again"}
      </h3>
      <p>
        {de
          ? "Dafür wird ein neues Meeting mit eigener Modellfassung angelegt. Originaltexte und frühere Auswertungen bleiben erhalten. Bei archivierten ganzen Meetings zuerst die Sitzung wiederherstellen. Opus ist verlustbehaftet und kann Erkennungsfehler verursachen."
          : "This creates a new meeting with its own model snapshot. Original transcripts and earlier analyses are retained. Restore a whole archived meeting before using its copies. Opus is lossy and may affect recognition."}
      </p>
      {error && <p role="alert">{error}</p>}
      <button
        className="text-button"
        disabled={busy || running}
        onClick={() => void load()}
      >
        {de ? "Audiokopien aktualisieren" : "Refresh audio copies"}
      </button>
      <ul>
        {copies.map((copy) => (
          <li key={copy.runId}>
            <strong>{copy.title}</strong> · {copy.codec.toUpperCase()} ·{" "}
            {copy.sections} {de ? "Abschnitte" : "sections"}
            <button
              className="secondary"
              disabled={busy || running}
              onClick={() => {
                setSelected(copy);
                setAccepted(false);
                setModel("");
              }}
            >
              {de
                ? "Neue Transkription vorbereiten"
                : "Prepare new transcription"}
            </button>
          </li>
        ))}
      </ul>
      {!copies.length && (
        <p>
          {de
            ? "Keine fertigen Audiokopien auf dieser Seite."
            : "No completed audio copies on this page."}
        </p>
      )}
      <div className="search-pages">
        <button
          disabled={!offset || busy || running}
          onClick={() => setOffset((n) => Math.max(0, n - 25))}
        >
          {de ? "Zurück" : "Previous"}
        </button>
        <button
          disabled={copies.length < 25 || busy || running}
          onClick={() => setOffset((n) => n + 25)}
        >
          {de ? "Weiter" : "Next"}
        </button>
      </div>
      {selected && (
        <section className="retention-confirm">
          <h4>{selected.title}</h4>
          <label>
            {de
              ? "Modell für die neue Transkription"
              : "Model for the new transcription"}
            <select
              value={model}
              disabled={busy || running}
              onChange={(e) => setModel(e.target.value)}
            >
              <option value="">
                {de ? "Installiertes Modell wählen" : "Choose installed model"}
              </option>
              {models
                .filter((m) => m.isDownloaded && m.supportsStreaming)
                .map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name}
                  </option>
                ))}
            </select>
          </label>
          <label>
            <input
              type="checkbox"
              checked={accepted}
              disabled={busy || running}
              onChange={(e) => setAccepted(e.target.checked)}
            />
            {de
              ? "Geprüfte Audiokopien in einem neuen Meeting transkribieren; Originale behalten"
              : "Transcribe verified audio copies in a new meeting; keep originals"}
          </label>
          <button
            className="secondary"
            disabled={busy || running || !accepted || !model}
            onClick={() => void start()}
          >
            {de
              ? "Neue lokale Transkription starten"
              : "Start new local transcription"}
          </button>
          <button
            className="text-button"
            disabled={busy || running}
            onClick={() => setSelected(null)}
          >
            {de ? "Abbrechen" : "Cancel"}
          </button>
        </section>
      )}
    </section>
  );
}
