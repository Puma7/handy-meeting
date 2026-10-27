import { useState, useEffect } from "react";
import { RefreshCw, Download } from "lucide-react";
import { errorMessage } from "../api";
import { clock } from "../domain";
import type { DesktopApi, Locale, UpdateProgress } from "../types";
export default function UpdatePanel({
  api,
  locale,
  footer = false,
  blocked = false,
}: {
  api: DesktopApi;
  locale: Locale;
  footer?: boolean;
  blocked?: boolean;
}) {
  const de = locale === "de";
  const [job, setJob] = useState<UpdateProgress | null>(null),
    [error, setError] = useState<string | null>(null),
    [request, setRequest] = useState(false),
    [jobsRunning, setJobsRunning] = useState(false),
    [dismissed, setDismissed] = useState<string | null>(null);
  useEffect(() => {
    if (!api.getUpdateProgress) return;
    let active = true,
      polling = false;
    async function poll() {
      if (polling) return;
      polling = true;
      try {
        const value = await api.getUpdateProgress!();
        // Archive maintenance and indexing also block installation in the backend.
        const others = footer
          ? []
          : await Promise.all([
              api.getMaintenanceProgress?.() ?? null,
              api.getLibraryProgress?.() ?? null,
            ]);
        if (active) {
          setJob(value);
          setJobsRunning(others.some((p) => p?.status === "running"));
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
  }, [api, footer]);
  if (!api.checkForUpdate) return null;
  const installBlocked = blocked || jobsRunning;
  const running = job?.status === "checking" || job?.status === "downloading";
  async function action(run: () => Promise<unknown>) {
    setRequest(true);
    setError(null);
    try {
      await run();
      if (api.getUpdateProgress) setJob(await api.getUpdateProgress());
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setRequest(false);
    }
  }
  const title = de ? "App aktualisieren" : "Update app";
  const labels: Record<string, string> = de
    ? {
        "": "Updates noch nicht geprüft",
        idle: "Updates noch nicht geprüft",
        checking: "Offizielles Update wird geprüft",
        downloading: "Windows-Installer wird geladen",
        ready: "Installer vollständig geladen und geprüft",
        available: "Neue Version verfügbar",
        current: "Du verwendest die aktuelle veröffentlichte Version",
        failed: "Update fehlgeschlagen",
        cancelled: "Download abgebrochen",
      }
    : {
        "": "Updates not checked yet",
        idle: "Updates not checked yet",
        checking: "Checking the official release",
        downloading: "Downloading the Windows installer",
        ready: "Installer downloaded and verified",
        available: "New version available",
        current: "You have the current published version",
        failed: "Update failed",
        cancelled: "Download cancelled",
      };
  // A cancelled download was the user's own choice; a failure stays until dismissed.
  const failureKey = `${job?.attempt ?? ""}:${job?.version ?? ""}:${job?.error ?? ""}`;
  if (
    footer &&
    !running &&
    (job?.status !== "failed" || dismissed === failureKey)
  )
    return null;
  const content = (
    <>
      <div className="notes-statusbar-row">
        <strong>
          {running && <RefreshCw className="spinning" size={16} />}{" "}
          {job ? labels[job.status] : title}
          {job?.version ? ` · ${job.version}` : ""}
        </strong>
        {running && <span>{clock(job?.elapsedMs ?? 0)}</span>}
      </div>
      {running && (
        <progress
          aria-label={title}
          value={job?.status === "downloading" ? job.receivedBytes : undefined}
          max={job?.totalBytes || undefined}
        />
      )}
      {job?.status === "downloading" && (
        <p>
          {(job.receivedBytes / 1048576).toFixed(1)} /{" "}
          {(job.totalBytes / 1048576).toFixed(1)} MB ·{" "}
          {Math.floor((job.receivedBytes / Math.max(1, job.totalBytes)) * 100)}%
        </p>
      )}
      {(error || job?.error) && <p role="alert">{error ?? job?.error}</p>}
      {!footer && (
        <>
          <p>
            {de
              ? "Updates werden nur auf deinen Klick geprüft. Der Windows-Installer wird mit Größe und GitHub-SHA256 geprüft; das Setup ist weiterhin unsigniert. Die Installation startet erst nach deinem zweiten Klick und beendet nur diese App, wenn keine Aufnahme, Erkennung oder Auswertung läuft."
              : "Updates are checked only when you click. The Windows installer is verified against its size and GitHub SHA256; setup remains unsigned. Installation requires another click and closes only this app when recording, recognition and analysis are finished."}
          </p>
          <button
            className="secondary"
            disabled={request || running}
            onClick={() => void action(api.checkForUpdate!)}
          >
            <RefreshCw size={15} />
            {de ? "Nach Updates suchen" : "Check for updates"}
          </button>
          {job?.status === "available" && (
            <button
              className="secondary"
              disabled={request}
              onClick={() => void action(api.downloadUpdate!)}
            >
              <Download size={15} />
              {de ? "Windows-Installer laden" : "Download Windows installer"}
            </button>
          )}
          {job?.status === "ready" && (
            <button
              className="primary"
              disabled={request || installBlocked}
              onClick={() => {
                if (
                  window.confirm(
                    de
                      ? "Geprüften Windows-Installer starten und HandyMeeting schließen? Vorhandene Aufnahmen und ältere Programmordner bleiben erhalten."
                      : "Start the verified Windows installer and close HandyMeeting? Recordings and older program folders are retained.",
                  )
                )
                  void action(api.installUpdate!);
              }}
            >
              {de
                ? "Installer starten und App schließen"
                : "Start installer and close app"}
            </button>
          )}
          {installBlocked && job?.status === "ready" && (
            <p>
              {de
                ? "Bitte zuerst Aufnahme, Erkennung, Auswertung, Archivpflege und Indexierung abschließen."
                : "Finish recording, recognition, analysis, archive maintenance and indexing first."}
            </p>
          )}
          <p>
            <a
              href={
                job?.releaseUrl ??
                "https://github.com/Puma7/handy-meeting/releases"
              }
              target="_blank"
              rel="noreferrer"
            >
              {de
                ? "Offizielle Downloads und Hinweise"
                : "Official downloads and release notes"}
            </a>
          </p>
        </>
      )}
      {job?.status === "downloading" && (
        // Cancellation only sets a flag, so it stays available while the download request runs.
        <button
          className="secondary"
          onClick={() =>
            void Promise.resolve(api.cancelUpdate!()).catch((e) =>
              setError(errorMessage(e)),
            )
          }
        >
          {de ? "Download abbrechen" : "Cancel download"}
        </button>
      )}
      {footer && job?.status === "failed" && (
        <button className="secondary" onClick={() => setDismissed(failureKey)}>
          {de ? "Ausblenden" : "Dismiss"}
        </button>
      )}
    </>
  );
  return footer ? (
    <section
      className="notes-progress notes-statusbar"
      role="status"
      aria-label={title}
    >
      {content}
    </section>
  ) : (
    <section className="legacy-panel">
      <h2>{title}</h2>
      {content}
    </section>
  );
}
