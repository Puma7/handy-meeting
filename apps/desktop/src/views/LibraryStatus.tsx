import { useEffect, useState } from "react";
import { RefreshCw, X, Square } from "lucide-react";
import { errorMessage } from "../api";
import { clock } from "../domain";
import type { DesktopApi, LibraryProgress, Locale } from "../types";
export default function LibraryStatus({
  api,
  connected,
  locale,
}: {
  api: DesktopApi;
  connected: boolean;
  locale: Locale;
}) {
  const [job, setJob] = useState<LibraryProgress | null>(null),
    [error, setError] = useState<string | null>(null),
    [hidden, setHidden] = useState<string | null>(null);
  const de = locale === "de";
  useEffect(() => {
    if (job?.status === "complete")
      window.dispatchEvent(new Event("library-index-updated"));
  }, [job?.jobId, job?.status]);
  useEffect(() => {
    if (!connected || !api.getLibraryProgress) return;
    let disposed = false;
    async function poll() {
      try {
        const next = await api.getLibraryProgress?.();
        if (!disposed) {
          setJob(next ?? null);
          setError(null);
        }
      } catch (e) {
        if (!disposed) setError(errorMessage(e));
      }
    }
    void poll();
    const timer = setInterval(() => void poll(), 1000);
    return () => {
      disposed = true;
      clearInterval(timer);
    };
  }, [api, connected]);
  if (!job || job.jobId === hidden) return null;
  const running = job.status === "running";
  const label = de ? "Lokaler Suchindex" : "Local search index";
  return (
    <section
      className="notes-progress notes-statusbar library-status"
      data-status={job.status}
      role="status"
      aria-label={label}
    >
      <div className="notes-statusbar-row">
        <div className="notes-statusbar-summary">
          <strong>
            {running && <RefreshCw size={16} className="spinning" />}
            {label} ·{" "}
            {running
              ? de
                ? "wird aktualisiert"
                : "updating"
              : job.status === "complete"
                ? de
                  ? "aktualisiert"
                  : "updated"
                : job.status === "cancelled"
                  ? de
                    ? "abgebrochen"
                    : "cancelled"
                  : de
                    ? "fehlgeschlagen"
                    : "failed"}
          </strong>
        </div>
        <strong>{clock(job.elapsedMs)}</strong>
        {running ? (
          <button
            className="secondary"
            disabled={job.cancelRequested}
            onClick={() =>
              void api
                .cancelLibraryJob?.(job.jobId)
                .catch((e) => setError(errorMessage(e)))
            }
          >
            <Square size={14} />
            {de ? "Indexierung abbrechen" : "Cancel indexing"}
          </button>
        ) : (
          <button
            className="icon-button"
            aria-label={de ? "Indexhinweis schließen" : "Dismiss index status"}
            onClick={() => setHidden(job.jobId)}
          >
            <X size={15} />
          </button>
        )}
      </div>
      <progress
        aria-label={label}
        value={job.totalTasks ? job.confirmedTasks : undefined}
        max={job.totalTasks || undefined}
      />
      <p className="muted">
        {job.confirmedTasks}/{job.totalTasks || "?"}{" "}
        {de ? "Ordnereinträge geprüft" : "directory entries checked"}
        {job.skipped > 0
          ? ` · ${job.skipped} ${de ? "nicht lesbare Einträge unverändert übersprungen" : "unreadable entries preserved and skipped"}`
          : ""}
      </p>
      {(error || job.error) && (
        <p className="inline-error" role="alert">
          {error ?? job.error}
        </p>
      )}
    </section>
  );
}
