import { useEffect, useState } from "react";
import { RefreshCw, Square } from "lucide-react";
import { clock } from "../domain";
import { errorMessage } from "../api";
import type { DesktopApi, Locale, LibraryProgress } from "../types";
export default function MaintenanceStatus({
  api,
  locale,
  onProgress,
}: {
  api: DesktopApi;
  locale: Locale;
  onProgress?: (job: LibraryProgress) => void;
}) {
  const [job, setJob] = useState<LibraryProgress | null>(null),
    [error, setError] = useState<string | null>(null),
    [hidden, setHidden] = useState<string | null>(null);
  const de = locale === "de";
  useEffect(() => {
    if (!api.getMaintenanceProgress) return;
    let active = true,
      polling = false;
    async function poll() {
      if (polling) return;
      polling = true;
      try {
        const value = await api.getMaintenanceProgress!();
        if (active) {
          setJob(value);
          setError(null);
        }
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
    if (job) onProgress?.(job);
  }, [job?.jobId, job?.status, onProgress]);
  if (!job || job.jobId === hidden) return null;
  const running = job.status === "running";
  const title = de ? "Aufbewahrung und Archiv" : "Retention and archive";
  const operation =
    (de
      ? {
          "cleanup-preview": "Vorschau",
          "retention-apply": "Bestätigter Aufräumschritt",
          "archive-restore": "Originaldateien wiederherstellen",
          "archive-compact": "Überflüssige Rohkopie entfernen",
          "prepare-transcription": "Neue Transkriptionsquelle vorbereiten",
        }
      : {
          "cleanup-preview": "Preview",
          "retention-apply": "Confirmed retention action",
          "archive-restore": "Restore original files",
          "archive-compact": "Remove redundant raw copy",
          "prepare-transcription": "Prepare new transcription source",
        })[job.kind] ?? (de ? "Aufräumschritte" : "Maintenance");
  return (
    <section
      className="notes-progress notes-statusbar"
      role="status"
      aria-label={title}
    >
      <div className="notes-statusbar-row">
        <strong>
          {running && <RefreshCw size={16} className="spinning" />}
          {title} · {operation} ·{" "}
          {running
            ? de
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
          aria-label={title}
          value={job.totalTasks ? job.confirmedTasks : undefined}
          max={job.totalTasks || undefined}
        />
      )}
      <p>
        {job.confirmedTasks} / {job.totalTasks || "?"}{" "}
        {de ? "bestätigte Prüfschritte" : "confirmed check steps"}
        {job.totalTasks
          ? ` · ${Math.floor((job.confirmedTasks / job.totalTasks) * 100)}%`
          : ""}
      </p>
      {(job.error || error) && <p role="alert">{error ?? job.error}</p>}
      {running ? (
        <button
          className="secondary"
          disabled={job.cancelRequested}
          onClick={() =>
            void api
              .cancelMaintenance?.(job.jobId)
              .catch((e) => setError(errorMessage(e)))
          }
        >
          <Square size={14} />
          {de ? "Aufbewahrungsauftrag abbrechen" : "Cancel retention job"}
        </button>
      ) : (
        <button className="text-button" onClick={() => setHidden(job.jobId)}>
          {de ? "Hinweis schließen" : "Dismiss"}
        </button>
      )}
    </section>
  );
}
