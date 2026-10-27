import { useCallback, useEffect, useRef, useState } from "react";
import { Archive } from "lucide-react";
import { errorMessage } from "../api";
import AudioCopiesPanel from "./AudioCopiesPanel";
import ArchivePanel from "./ArchivePanel";
import MaintenanceStatus from "./MaintenanceStatus";
import type {
  DesktopApi,
  Locale,
  RetentionPolicy,
  RetentionPlan,
  LibraryProgress,
  HandyModel,
} from "../types";
type Rule =
  | "audioArchiveAfterDays"
  | "archiveAfterDays"
  | "convertAfterDays"
  | "maxVisibleMeetings"
  | "deleteArchivesAfterDays";
export default function RetentionPanel({
  api,
  locale,
  models = [],
}: {
  api: DesktopApi;
  locale: Locale;
  models?: HandyModel[];
}) {
  const de = locale === "de";
  const latestJob = useRef<string | null>(null);
  // A remounted status bar reports the last finished preview; only a current one is shown.
  const policyRevision = useRef<number | null>(null);
  const [policy, setPolicy] = useState<RetentionPolicy | null>(null),
    [draft, setDraft] = useState<RetentionPolicy | null>(null),
    [plan, setPlan] = useState<RetentionPlan | null>(null),
    [job, setJob] = useState<LibraryProgress | null>(null),
    [error, setError] = useState<string | null>(null),
    [busy, setBusy] = useState(false),
    [page, setPage] = useState(0),
    [selected, setSelected] = useState<number | null>(null),
    [confirmation, setConfirmation] = useState(""),
    [accepted, setAccepted] = useState(false);
  useEffect(() => {
    if (!api.getRetentionPolicy) return;
    let active = true;
    api
      .getRetentionPolicy()
      .then((value) => {
        if (active) {
          setPolicy(value);
          setDraft(value);
        }
      })
      .catch((e) => {
        if (active) setError(errorMessage(e));
      });
    return () => {
      active = false;
    };
  }, [api]);
  useEffect(() => {
    policyRevision.current = policy?.revision ?? null;
  }, [policy]);
  const progress = useCallback(
    (value: LibraryProgress) => {
      latestJob.current = value.jobId;
      setJob(value);
      if (value.kind === "cleanup-preview" && value.status === "complete")
        void api
          .getRetentionPlan?.(value.jobId)
          .then((p) => {
            if (latestJob.current !== value.jobId || p.id !== value.jobId)
              return;
            const age = Date.now() - Date.parse(p.createdAt);
            if (
              p.policyRevision !== policyRevision.current ||
              !(age >= 0 && age <= 24 * 60 * 60 * 1000)
            )
              return;
            setSelected(null);
            setAccepted(false);
            setConfirmation("");
            setPlan(p);
            setPage(0);
          })
          .catch((e) => setError(errorMessage(e)));
    },
    [api],
  );
  if (!api.getRetentionPolicy) return null;
  const candidate = selected === null ? null : plan?.candidates[selected];
  const deleting = candidate?.action === "delete-archived-meeting";
  const expectedDelete = candidate ? `DELETE ${candidate.meetingId}` : "";
  const running = job?.status === "running";
  const changed = JSON.stringify(draft) !== JSON.stringify(policy);
  const actionNames: Record<string, string> = de
    ? {
        "archive-audio": "Nur Audio verlustfrei archivieren",
        "archive-meeting": "Ganzes Meeting archivieren",
        "convert-audio": "Zusätzliche Audiokopie umwandeln",
        "delete-archived-meeting":
          "Archiv endgültig löschen · extra Bestätigung nötig",
      }
    : {
        "archive-audio": "Archive audio only, without loss",
        "archive-meeting": "Archive the whole meeting",
        "convert-audio": "Convert an additional audio copy",
        "delete-archived-meeting":
          "Permanently delete archive · extra confirmation required",
      };
  async function save() {
    if (!policy || !draft || !api.saveRetentionPolicy) return;
    setBusy(true);
    setSelected(null);
    setAccepted(false);
    setConfirmation("");
    setError(null);
    try {
      const next = await api.saveRetentionPolicy(policy.revision, draft);
      setPolicy(next);
      setDraft(next);
      setPlan(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }
  async function preview() {
    if (!api.previewRetention) return;
    setBusy(true);
    setSelected(null);
    setAccepted(false);
    setConfirmation("");
    setError(null);
    try {
      await api.previewRetention();
      if (api.getMaintenanceProgress) {
        const value = await api.getMaintenanceProgress();
        if (value) progress(value);
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }
  async function chooseFfmpeg() {
    if (!api.chooseFfmpegExecutable || !draft || busy || running) return;
    setBusy(true);
    setError(null);
    try {
      const selected = await api.chooseFfmpegExecutable();
      if (selected) setDraft({ ...draft, ffmpegExecutable: selected });
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }
  function clearFfmpeg() {
    if (!draft || busy || running) return;
    setDraft({ ...draft, ffmpegExecutable: null });
    setError(null);
    setPlan(null);
    setSelected(null);
    setAccepted(false);
    setConfirmation("");
  }
  async function apply() {
    if (!plan || selected === null || !candidate || !api.applyRetention) return;
    setBusy(true);
    setError(null);
    try {
      await api.applyRetention(
        plan.id,
        selected,
        deleting ? confirmation : `APPLY ${plan.id} ${selected}`,
      );
      setPlan(null);
      setSelected(null);
      setConfirmation("");
      setAccepted(false);
      if (api.getMaintenanceProgress) {
        const value = await api.getMaintenanceProgress();
        if (value) progress(value);
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }
  function rule(key: Rule, label: string, initial: number) {
    if (!draft) return null;
    const number = draft[key];
    const minimum =
      key === "maxVisibleMeetings" || key === "deleteArchivesAfterDays"
        ? 1
        : draft.untouchedDays;
    return (
      <div className="retention-rule">
        <label>
          <input
            type="checkbox"
            checked={number !== null}
            disabled={busy || running}
            onChange={(e) =>
              setDraft({
                ...draft,
                [key]: e.target.checked ? Math.max(initial, minimum) : null,
              })
            }
          />
          {label}
        </label>
        {number !== null && (
          <input
            type="number"
            min={minimum}
            max={key === "maxVisibleMeetings" ? 1000000 : 36500}
            aria-label={label}
            value={number}
            disabled={busy || running}
            onChange={(e) =>
              setDraft({ ...draft, [key]: Number(e.target.value) })
            }
          />
        )}
      </div>
    );
  }
  return (
    <section className="legacy-panel retention-panel">
      <h2>
        <Archive size={19} />
        {de ? "Aufbewahrung und Archiv" : "Retention and archive"}
      </h2>
      <p>
        {de
          ? "Alle Regeln beginnen ausgeschaltet. Eine Vorschau ändert keine Dateien. VIP-, laufende, unvollständige oder nicht sicher lesbare Meetings bleiben geschützt. Das endgültige Löschen eines Archivs braucht eine zusätzliche ausdrückliche Bestätigung."
          : "Every rule starts disabled. A preview changes no files. VIP, active, incomplete and unreadable meetings are protected. Permanent archive deletion requires a separate explicit confirmation."}
      </p>
      {error && <p role="alert">{error}</p>}
      {draft && (
        <>
          <label>
            {de
              ? "Meetings zunächst unangetastet lassen · Tage"
              : "Leave meetings untouched initially · days"}
            <input
              type="number"
              min={1}
              max={36500}
              value={draft.untouchedDays}
              disabled={busy || running}
              onChange={(e) =>
                setDraft({ ...draft, untouchedDays: Number(e.target.value) })
              }
            />
          </label>
          <div className="retention-rules">
            {rule(
              "convertAfterDays",
              de
                ? "Zusätzliche Audiokopie umwandeln nach Tagen"
                : "Convert an additional audio copy after days",
              60,
            )}
            {rule(
              "audioArchiveAfterDays",
              de
                ? "Nur Audio archivieren nach Tagen · Text bleibt"
                : "Archive audio only after days · keep text",
              90,
            )}
            {rule(
              "archiveAfterDays",
              de
                ? "Ganzes Meeting archivieren nach Tagen"
                : "Archive the whole meeting after days",
              180,
            )}
            {rule(
              "maxVisibleMeetings",
              de
                ? "Höchstens so viele Meetings in der normalen Historie"
                : "Maximum meetings in normal history",
              1000,
            )}
            {rule(
              "deleteArchivesAfterDays",
              de
                ? "Endgültiges Löschen vorschlagen nach Tagen im Archiv"
                : "Propose permanent deletion after days in archive",
              365,
            )}
          </div>
          <label>
            {de ? "Optionales Audioformat" : "Optional audio format"}
            <select
              value={draft.codec}
              disabled={busy || running}
              onChange={(e) =>
                setDraft({ ...draft, codec: e.target.value as "flac" | "opus" })
              }
            >
              <option value="flac">
                FLAC · {de ? "verlustfrei" : "lossless"}
              </option>
              <option value="opus">
                Opus · {de ? "kompakter, verlustbehaftet" : "smaller, lossy"}
              </option>
            </select>
          </label>
          <p className="muted">
            {de
              ? "ZIP-Archive erhalten die Originaldateien verlustfrei. Zusätzliche FLAC-/Opus-Kopien ersetzen niemals ungeprüft die einzige Originalaufnahme. Opus kann die spätere Erkennungsqualität beeinflussen."
              : "ZIP archives preserve original files without loss. Additional FLAC/Opus copies never replace the only original without verification. Opus may affect later recognition quality."}
          </p>
          <label>
            {de
              ? "Separat installiertes FFmpeg · nur für optionale Formatumwandlung"
              : "Separately installed FFmpeg · only for optional format conversion"}
            <input value={draft.ffmpegExecutable ?? ""} readOnly />
          </label>
          <button
            className="secondary"
            disabled={busy || running || !api.chooseFfmpegExecutable}
            onClick={() => void chooseFfmpeg()}
          >
            {de
              ? "FFmpeg im Dateidialog auswählen …"
              : "Choose FFmpeg in file dialog …"}
          </button>
          <button
            className="secondary"
            disabled={busy || running || !draft.ffmpegExecutable}
            onClick={clearFfmpeg}
          >
            {de ? "FFmpeg-Auswahl entfernen" : "Clear FFmpeg selection"}
          </button>
          <p className="muted">
            {de
              ? "Das Entfernen ändert nur die Programmauswahl. Programmdatei und Aufnahmen bleiben erhalten. Übernimm die Änderung mit „Regeln speichern“, bevor du eine neue Vorschau erstellst."
              : "Clearing changes only the program selection. The program file and recordings are retained. Apply the change with Save rules before creating a new preview."}
          </p>
          <p className="muted">
            {de
              ? "Die ausgewählte Programmdatei wird geprüft. Wähle und bestätige sie nach jedem App-Neustart sowie nach einem Austausch erneut, bevor du sie verwendest. Der gespeicherte Pfad allein ist keine neue Freigabe. Abbrechen lässt die bisherige Wahl unverändert."
              : "The selected program is checked. Choose and confirm it again after each app restart or replacement before using it. The saved path alone does not grant renewed permission. Cancelling preserves the previous selection."}
          </p>
          <button
            className="secondary"
            disabled={busy || running || !changed}
            onClick={() => void save()}
          >
            {de ? "Regeln speichern" : "Save rules"}
          </button>
          <button
            className="secondary"
            disabled={busy || running || changed}
            onClick={() => void preview()}
          >
            {de ? "Aufräumvorschau erstellen" : "Create retention preview"}
          </button>
          {changed && (
            <p>
              {de
                ? "Bitte geänderte Regeln vor der Vorschau speichern."
                : "Save changed rules before creating a preview."}
            </p>
          )}
        </>
      )}
      <MaintenanceStatus api={api} locale={locale} onProgress={progress} />
      {plan && (
        <div className="retention-preview">
          <h3>
            {de
              ? "Vorschau · noch keine Dateien geändert"
              : "Preview · no files changed"}
          </h3>
          <p>
            {plan.scanned} {de ? "geprüft" : "checked"} · {plan.protected}{" "}
            {de ? "geschützt" : "protected"} · {plan.unreadable}{" "}
            {de ? "nicht sicher lesbar" : "unreadable"} ·{" "}
            {plan.candidates.length} {de ? "Vorschläge" : "proposals"}
          </p>
          {plan.limited && (
            <p>
              {de
                ? "Die Vorschau ist auf 500 Vorschläge begrenzt. Weitere Schritte benötigen eine neue Vorschau."
                : "This preview is limited to 500 proposals. Further actions need a new preview."}
            </p>
          )}
          <ul>
            {plan.candidates
              .slice(page * 25, (page + 1) * 25)
              .map((candidate, i) => (
                <li key={candidate.meetingId + candidate.action + i}>
                  <strong>{candidate.title}</strong> · {candidate.ageDays}{" "}
                  {de ? "Tage" : "days"}
                  <p>{actionNames[candidate.action]}</p>
                  {api.applyRetention && (
                    <button
                      className="secondary"
                      disabled={busy || running || changed}
                      onClick={() => {
                        setSelected(page * 25 + i);
                        setConfirmation("");
                        setAccepted(false);
                      }}
                    >
                      {de ? "Schritt prüfen" : "Review action"}
                    </button>
                  )}
                </li>
              ))}
          </ul>
          {plan.candidates.length > 25 && (
            <div className="search-pages">
              <button
                disabled={page === 0}
                onClick={() => setPage((n) => n - 1)}
              >
                {de ? "Zurück" : "Previous"}
              </button>
              <span>
                {page * 25 + 1}–
                {Math.min((page + 1) * 25, plan.candidates.length)}
              </span>
              <button
                disabled={(page + 1) * 25 >= plan.candidates.length}
                onClick={() => setPage((n) => n + 1)}
              >
                {de ? "Weiter" : "Next"}
              </button>
            </div>
          )}
        </div>
      )}
      {candidate && (
        <section
          className="retention-confirm"
          aria-label={
            de ? "Aufräumschritt bestätigen" : "Confirm retention action"
          }
        >
          <h3>{candidate.title}</h3>
          <p>{actionNames[candidate.action]}</p>
          <p>
            {deleting
              ? de
                ? "Alle zu diesem archivierten Meeting gehörenden geprüften Archivfassungen, zurückbehaltenen Quellkopien und Zuordnungen werden unwiderruflich gelöscht. Eine Wiederherstellung aus diesen Dateien ist danach unmöglich. Andere Meetings bleiben erhalten."
                : "All verified archive versions, retained source copies and annotations belonging to this archived meeting will be permanently deleted. These files cannot be restored afterwards. Other meetings are preserved."
              : candidate.action === "archive-meeting"
                ? de
                  ? "Das vollständige Meeting wird geprüft und aus der normalen Historie ins lokale Archiv verschoben. Zusätzlich zum ZIP bleibt vorerst eine Quellkopie erhalten; dadurch wird noch kein Speicher freigegeben."
                  : "The complete meeting is verified and moved from normal history into the local archive. A source copy is initially retained alongside the ZIP; this step does not yet free storage."
                : candidate.action === "archive-audio"
                  ? de
                    ? "Originalaudio wird erst nach vollständiger Prüfung im verlustfreien ZIP archiviert. Der Text bleibt in der Historie; für erneute Erkennung lässt sich das Audio wiederherstellen."
                    : "Original audio is moved to a lossless ZIP only after complete verification. Text remains in history; restore audio for recognition again."
                  : de
                    ? "Zusätzliche geprüfte Audiokopien werden mit dem gewählten lokalen Werkzeug erstellt. Originalaudio und bisherige Texte bleiben erhalten. Opus ist verlustbehaftet."
                    : "Additional verified audio copies are created with the selected local tool. Original audio and existing text are retained. Opus is lossy."}
          </p>
          {deleting ? (
            <label>
              {de
                ? "Zur Bestätigung exakt eingeben"
                : "Type exactly to confirm"}
              : <code>{expectedDelete}</code>
              <input
                aria-label={
                  de
                    ? "Endgültiges Löschen bestätigen"
                    : "Confirm permanent deletion"
                }
                value={confirmation}
                disabled={busy || running}
                onChange={(e) => setConfirmation(e.target.value)}
              />
            </label>
          ) : (
            <label>
              <input
                type="checkbox"
                checked={accepted}
                disabled={busy || running}
                onChange={(e) => setAccepted(e.target.checked)}
              />
              {de
                ? "Diesen einzelnen beschriebenen Schritt ausführen"
                : "Perform this single described action"}
            </label>
          )}
          <button
            className="secondary"
            disabled={
              busy ||
              running ||
              (deleting ? confirmation !== expectedDelete : !accepted)
            }
            onClick={() => void apply()}
          >
            {deleting
              ? de
                ? "Unwiderruflich löschen"
                : "Permanently delete"
              : de
                ? "Bestätigten Schritt ausführen"
                : "Run confirmed action"}
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
      <ArchivePanel api={api} locale={locale} job={job} />
      <AudioCopiesPanel api={api} locale={locale} models={models} job={job} />
    </section>
  );
}
