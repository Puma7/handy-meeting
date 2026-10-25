import { RefreshCw, Square, X, ArrowUpRight } from "lucide-react";
import { clock } from "../domain";
import type { NotesProgress } from "../types";
import type { MessageKey } from "../i18n";
import type { JobFeedback } from "../useNotesJob";

type Props = {
  progress: NotesProgress | null;
  elapsedMs: number;
  cancelPending: boolean;
  feedback: JobFeedback | null;
  pollError: string | null;
  meetingTitle: string;
  t: Record<MessageKey, string>;
  onCancel: () => void;
  onOpen: () => void;
  onDismiss: () => void;
  onResume: (() => void) | null;
};
export default function NotesProgressPanel({
  progress,
  elapsedMs,
  cancelPending,
  feedback,
  pollError,
  meetingTitle,
  t,
  onCancel,
  onOpen,
  onDismiss,
  onResume,
}: Props) {
  if (!progress && !feedback) return null;
  const running = progress?.status === "running";
  const total = progress?.totalTasks ?? 0;
  const confirmed = Math.min(
    Math.max(0, progress?.confirmedTasks ?? 0),
    Math.max(0, total),
  );
  const status =
    feedback?.kind === "cancelled"
      ? t.notesCancelled
      : feedback?.kind === "failed"
        ? t.notesFailed
        : feedback?.kind === "cancel-unavailable"
          ? t.cancelUnavailable
          : feedback?.kind === "complete"
            ? t.notesComplete
            : t.notesTask;
  const stage =
    progress?.stage === "requesting"
      ? t.stageRequesting
      : progress?.stage === "validating"
        ? t.stageValidating
        : progress?.stage === "saving"
          ? t.stageSaving
          : t.stagePreparing;
  return (
    <section
      className="notes-progress"
      data-status={progress?.status ?? feedback?.kind}
      aria-label={t.notesTask}
      role="status"
    >
      <div className="notes-progress-heading">
        <div>
          <strong>
            {running && <RefreshCw size={16} className="spinning" />}
            {running
              ? cancelPending || progress?.cancelRequested
                ? t.cancelRequested
                : stage
              : status}
          </strong>
          <p>{meetingTitle}</p>
        </div>
        {running ? (
          <button
            className="secondary"
            disabled={cancelPending || progress?.cancelRequested}
            onClick={onCancel}
            title={t.cancelHelp}
          >
            {cancelPending || progress?.cancelRequested ? (
              <RefreshCw size={15} className="spinning" />
            ) : (
              <Square size={15} />
            )}
            <span>
              {cancelPending || progress?.cancelRequested
                ? t.cancelRequested
                : t.cancelAnalysis}
            </span>
          </button>
        ) : (
          <button
            className="icon-button"
            onClick={onDismiss}
            aria-label={t.dismiss}
          >
            <X size={17} />
          </button>
        )}
      </div>
      {progress && (
        <>
          <div className="notes-progress-meta">
            <span>
              {t.elapsed}: <strong>{clock(elapsedMs)}</strong>
            </span>
            {progress.stepName && (
              <span>
                {progress.stepName}
                {progress.totalSteps > 0
                  ? " · " + progress.stepIndex + "/" + progress.totalSteps
                  : ""}
              </span>
            )}
          </div>
          {total > 0 ? (
            <>
              <progress
                value={confirmed}
                max={total}
                aria-label={t.confirmedTasks}
              />
              <p className="muted">
                {confirmed}/{total} {t.confirmedTasks} ·{" "}
                {Math.floor((confirmed * 100) / total)}%
              </p>
            </>
          ) : running ? (
            <progress aria-label={t.stagePreparing} />
          ) : null}
        </>
      )}
      {running && (
        <p className="muted">
          {cancelPending || progress?.cancelRequested
            ? t.cancelHelp
            : t.inferenceUnknown}
        </p>
      )}
      {running && feedback?.kind === "cancel-unavailable" && (
        <p className="inline-error">{t.cancelUnavailable}</p>
      )}
      {pollError && (
        <p className="inline-error">
          {t.progressUnavailable} {pollError}
        </p>
      )}
      {feedback?.message && <p className="inline-error">{feedback.message}</p>}
      {!running && (
        <div className="progress-result-actions">
          <button className="text-button" onClick={onOpen}>
            <ArrowUpRight size={15} />
            {t.openAnalysisMeeting}
          </button>
          {onResume && (
            <button
              className="secondary"
              onClick={onResume}
              title={t.resumeHelp}
            >
              <RefreshCw size={15} />
              {t.resumeAnalysis}
            </button>
          )}
        </div>
      )}
    </section>
  );
}
