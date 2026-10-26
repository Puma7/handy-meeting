import { useState } from "react";
import type { MessageKey } from "../i18n";
import type { DesktopApi, NotesRequest, PendingAnalysis } from "../types";
import CredentialPanel from "./CredentialPanel";

export default function ResumeAnalysisDialog({
  run,
  api,
  t,
  blocked,
  onClose,
  onResume,
}: {
  run: PendingAnalysis;
  api: DesktopApi;
  t: Record<MessageKey, string>;
  blocked: boolean;
  onClose: () => void;
  onResume: (request: NotesRequest) => void;
}) {
  const [apiKey, setApiKey] = useState("");
  const [useSavedKey, setUseSavedKey] = useState(false);
  return (
    <div className="modal-backdrop">
      <section
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="resume-analysis-title"
      >
        <h2 id="resume-analysis-title">{t.resumeAnalysis}</h2>
        <p>{t.resumeAnalysisHelp}</p>
        <p>
          <strong>{run.template.name}</strong> · {run.confirmedTasks}/
          {run.totalTasks} {t.confirmedTasks}
        </p>
        <p>
          {run.model} · {run.mode === "local" ? t.local : t.cloud}
        </p>
        <code className="storage-path">{run.endpoint}</code>
        {run.mode === "cloud" && <p>{t.cloudBody}</p>}
        <CredentialPanel
          api={api}
          endpoint={run.endpoint}
          mode={run.mode}
          connected={true}
          disabled={blocked}
          apiKey={apiKey}
          setApiKey={setApiKey}
          useSavedKey={useSavedKey}
          setUseSavedKey={setUseSavedKey}
          t={t}
        />
        <div className="dialog-actions">
          <button className="secondary" onClick={onClose}>
            {t.cancel}
          </button>
          <button
            className="primary"
            disabled={blocked}
            onClick={() =>
              onResume({
                meetingId: run.meetingId,
                resumeRunId: run.id,
                template: structuredClone(run.template),
                endpoint: run.endpoint,
                model: run.model,
                mode: run.mode,
                cloudConsent: run.mode === "cloud",
                apiKey,
                useSavedKey,
              })
            }
          >
            {run.mode === "cloud" ? t.approve : t.resumeAnalysis}
          </button>
        </div>
      </section>
    </div>
  );
}
