import { useEffect, useState } from "react";
import { RefreshCw, Square } from "lucide-react";
import { errorMessage } from "../api";
import { clock } from "../domain";
import type {
  DesktopApi,
  Locale,
  Meeting,
  HandyModel,
  SpeakerProgress,
  SavedSpeakerSections,
} from "../types";
export default function SpeakerExperiments({
  api,
  locale,
  meeting,
  models = [],
  recording = false,
  footer = false,
}: {
  api: DesktopApi;
  locale: Locale;
  meeting?: Meeting | null;
  models?: HandyModel[];
  recording?: boolean;
  footer?: boolean;
}) {
  const de = locale === "de";
  const [model, setModel] = useState(""),
    [job, setJob] = useState<SpeakerProgress | null>(null),
    [saved, setSaved] = useState<SavedSpeakerSections | null>(null),
    [error, setError] = useState<string | null>(null),
    [busy, setBusy] = useState(false),
    [hidden, setHidden] = useState<string | null>(null);
  useEffect(() => {
    if (!api.getSpeakerProgress) return;
    let active = true,
      polling = false;
    async function poll() {
      if (polling) return;
      polling = true;
      try {
        const next = await api.getSpeakerProgress!();
        if (active) setJob(next);
      } catch (e) {
        if (active) setError(errorMessage(e));
      } finally {
        polling = false;
      }
    }
    void poll();
    const timer = setInterval(() => void poll(), 500);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [api]);
  useEffect(() => {
    if (footer || !meeting || !api.getSpeakerSections) return;
    let active = true;
    setSaved(null);
    api
      .getSpeakerSections(meeting.id)
      .then((value) => {
        if (active) setSaved(value);
      })
      .catch((e) => {
        if (active) setError(errorMessage(e));
      });
    return () => {
      active = false;
    };
  }, [api, meeting?.id, footer, job?.status]);
  if (!api.startSpeakerExperiment) return null;
  const running = job?.status === "running";
  const label = de ? "Sprecherexperiment" : "Speaker experiment";
  async function start(mode: "live" | "post-meeting") {
    if (!meeting || !model || busy) return;
    setBusy(true);
    setError(null);
    try {
      await api.startSpeakerExperiment!(meeting.id, model, mode);
      setHidden(null);
      if (api.getSpeakerProgress) setJob(await api.getSpeakerProgress());
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }
  const status = job && (
    <>
      <div className="notes-statusbar-row">
        <strong>
          {running && <RefreshCw size={16} className="spinning" />}
          {label} ·{" "}
          {job.mode === "live"
            ? de
              ? "Live · experimentell"
              : "Live · experimental"
            : de
              ? "Audio nach dem Meeting · Beta"
              : "Post-meeting audio · beta"}{" "}
          ·{" "}
          {running
            ? job.stage === "waiting"
              ? de
                ? "wartet auf geschlossene Abschnitte"
                : "waiting for closed sections"
              : job.stage === "loading"
                ? de
                  ? "Modell wird geprüft und geladen"
                  : "checking and loading model"
                : de
                  ? "arbeitet"
                  : "working"
            : job.status === "complete"
              ? de
                ? "abgeschlossen"
                : "complete"
              : job.status === "cancelled"
                ? de
                  ? "abgebrochen"
                  : "cancelled"
                : de
                  ? "fehlgeschlagen"
                  : "failed"}
        </strong>
        <span>{clock(job.elapsedMs)}</span>
      </div>
      {running && (
        <progress
          aria-label={label}
          max={job.totalTasks || undefined}
          value={job.totalTasks ? job.confirmedTasks : undefined}
        />
      )}
      <p>
        {job.confirmedTasks}
        {job.totalTasks ? ` / ${job.totalTasks}` : ""}{" "}
        {de
          ? "bestätigt gespeicherte Audioabschnitte"
          : "confirmed saved audio sections"}
        {job.totalTasks
          ? ` · ${Math.floor((job.confirmedTasks / job.totalTasks) * 100)}%`
          : ""}
      </p>
      {(error || job.error) && <p role="alert">{error ?? job.error}</p>}
      {running && (
        <button
          className="secondary"
          disabled={job.cancelRequested}
          onClick={() =>
            void api
              .cancelSpeakerExperiment?.(job.jobId)
              .catch((e) => setError(errorMessage(e)))
          }
        >
          <Square size={14} />
          {de
            ? "Nur Sprecherexperiment abbrechen"
            : "Cancel only the speaker experiment"}
        </button>
      )}
    </>
  );
  if (footer) {
    if (!job || job.jobId === hidden) return null;
    return (
      <section
        className="notes-progress notes-statusbar"
        role="status"
        aria-label={label}
      >
        {status}
        {!running && (
          <button onClick={() => setHidden(job.jobId)}>
            {de ? "Hinweis schließen" : "Dismiss"}
          </button>
        )}
      </section>
    );
  }
  if (!meeting) return null;
  return (
    <details className="speaker-options">
      <summary>
        {de
          ? "Zusätzliche lokale Sprecherfunktionen · Beta / experimentell"
          : "Additional local speaker features · beta / experimental"}
      </summary>
      <p>
        {de
          ? "Diese beiden Optionen sind von manuellen Zuordnungen, Aufnahme und normaler Transkription getrennt. Ein passendes, bereits installiertes Handy-Modell ist nötig. Es werden keine Modellgewichte heruntergeladen oder Cloud-Dienste verwendet. Ungeeignete Modelle führen zu einer sichtbaren Fehlermeldung."
          : "These two options are separate from manual labels, recording and normal transcription. A compatible model already installed in Handy is required. No model weights are downloaded and no cloud service is used. Unsupported models produce a visible error."}
      </p>
      <label>
        {de
          ? "Zusätzliches lokales Sprechermodell"
          : "Additional local speaker model"}
        <select
          value={model}
          disabled={busy || running}
          onChange={(e) => setModel(e.target.value)}
        >
          <option value="">
            {de ? "Installiertes Modell wählen" : "Choose an installed model"}
          </option>
          {models
            .filter((m) => m.isDownloaded)
            .map((m) => (
              <option key={m.id} value={m.id}>
                {m.name}
              </option>
            ))}
        </select>
      </label>
      <div>
        <h3>
          {de
            ? "Option 2: Audio nach dem Meeting · Beta"
            : "Option 2: post-meeting audio · beta"}
        </h3>
        <p>
          {de
            ? "Prüft abgeschlossene Abschnitte auf unterschiedliche Stimmen. Originaltexte werden nicht ersetzt."
            : "Checks closed audio sections for different voices. Original transcripts are retained."}
        </p>
        <button
          className="secondary"
          disabled={
            busy ||
            running ||
            recording ||
            meeting.status !== "complete" ||
            !model
          }
          onClick={() => void start("post-meeting")}
        >
          {de
            ? "Lokale Sprechertrennung prüfen"
            : "Run local speaker separation"}
        </button>
      </div>
      <div>
        <h3>
          {de
            ? "Option 3: Während der Aufnahme · experimentell"
            : "Option 3: during recording · experimental"}
        </h3>
        <p>
          {de
            ? "Verarbeitet neue geschlossene Audioabschnitte im Hintergrund, mit entsprechender Verzögerung. Kein Eingriff in die Audiosicherung."
            : "Processes newly closed audio sections in the background, with a corresponding delay. Audio saving remains independent."}
        </p>
        <button
          className="secondary"
          disabled={busy || running || !recording || !model}
          onClick={() => void start("live")}
        >
          {de ? "Live-Stimmenabschnitte prüfen" : "Check live voice sections"}
        </button>
      </div>
      <p className="notice warning">
        {de
          ? "Stimme 1 usw. sind neutrale Modell-Entwürfe pro Abschnitt und Spur, keine Personennamen. Nummern können zwischen Abschnitten wechseln. Kein automatischer Abgleich mit Namensnennungen und keine behaupteten Wortzeiten."
          : "Voice 1 etc. are neutral model drafts per section and track, not personal identities. Numbers can change between sections. They are not automatically matched to names, and no word timing is claimed."}
      </p>
      {error && <p role="alert">{error}</p>}
      {job?.meetingId === meeting.id && status}
      {job?.meetingId === meeting.id && running && job.lastSection && (
        <div>
          <h3>
            {de
              ? "Letzter bestätigter Stimmenabschnitt"
              : "Latest confirmed voice section"}
          </h3>
          {job.lastSection.turns.length ? (
            <ul>
              {job.lastSection.turns.slice(0, 100).map((turn, i) => (
                <li key={i}>
                  {clock(job.lastSection!.startMs + turn.startMs)}–
                  {clock(job.lastSection!.startMs + turn.endMs)} ·{" "}
                  {de ? "Stimme" : "Voice"} {turn.speakerId}{" "}
                  {de ? "(nur dieser Abschnitt)" : "(this section only)"}
                </li>
              ))}
            </ul>
          ) : (
            <p>
              {de
                ? "Keine verwertbaren Stimmenzeiten in diesem Abschnitt."
                : "No usable timed voice turns in this section."}
            </p>
          )}
        </div>
      )}
      {saved && (
        <div>
          <h3>
            {de
              ? "Letzte gespeicherte Fassung · letzte 25 Abschnitte"
              : "Latest saved revision · last 25 sections"}
          </h3>
          <p>{saved.modelId}</p>
          {saved.sections.map((section) => (
            <article key={section.sourceId}>
              <strong>
                {de ? "Originalabschnitt" : "Original section"}{" "}
                {clock(section.startMs)}–
                {clock(section.startMs + section.durationMs)}
              </strong>
              {section.turns.length ? (
                <ul>
                  {section.turns.slice(0, 100).map((turn, i) => (
                    <li key={i}>
                      {clock(section.startMs + turn.startMs)}–
                      {clock(section.startMs + turn.endMs)} ·{" "}
                      {de ? "Stimme" : "Voice"} {turn.speakerId}{" "}
                      {de ? "(nur dieser Abschnitt)" : "(this section only)"}
                    </li>
                  ))}
                </ul>
              ) : (
                <p>
                  {de
                    ? "Keine verwertbaren zeitlichen Stimmenabschnitte vom Modell geliefert."
                    : "The model returned no usable timed voice sections."}
                </p>
              )}
            </article>
          ))}
        </div>
      )}
    </details>
  );
}
