import {
  Check,
  ChevronLeft,
  ChevronRight,
  Copy,
  Pencil,
  Plus,
  Sparkles,
  Trash2,
} from "lucide-react";
import { uid, renderAnalysis } from "../domain";
import type {
  AnalysisResult,
  AnalysisStep,
  AnalysisTemplate,
  Evidence,
  Meeting,
  Locale,
} from "../types";
import type { MessageKey } from "../i18n";
type Props = {
  t: Record<MessageKey, string>;
  meeting: Meeting | null;
  libraryInvalid: boolean;
  templates: AnalysisTemplate[];
  notesBusy: boolean;
  mode: "local" | "cloud";
  setMode: (v: "local" | "cloud") => void;
  setEndpoint: (v: string) => void;
  setApiKey: (v: string) => void;
  textModel: string;
  setTextModel: (v: string) => void;
  endpoint: string;
  apiKey: string;
  template: AnalysisTemplate;
  resultId: string;
  analyses: AnalysisResult[];
  setTemplateId: (v: string) => void;
  setStepId: (v: string) => void;
  setResultId: (v: string) => void;
  setTemplateName: (v: { id: string | null; name: string } | null) => void;
  setTemplates: (v: AnalysisTemplate[]) => void;
  connected: boolean;
  recording: boolean;
  busy: boolean;
  beginNotes: () => void;
  result: AnalysisResult | undefined;
  resultSection: AnalysisResult["sections"][number] | undefined;
  selectedStep: AnalysisStep;
  setEditing: (v: AnalysisStep | null) => void;
  setEditingNew: (v: boolean) => void;
  moveStep: (delta: number) => void;
  updateTemplate: (v: AnalysisTemplate) => void;
  copied: boolean;
  copy: (text: string) => Promise<void>;
  showEvidence: (evidence: Evidence) => void;
  locale: Locale;
};
export default function NotesView({
  t,
  meeting,
  libraryInvalid,
  templates,
  notesBusy,
  mode,
  setMode,
  setEndpoint,
  setApiKey,
  textModel,
  setTextModel,
  endpoint,
  apiKey,
  template,
  resultId,
  analyses,
  setTemplateId,
  setStepId,
  setResultId,
  setTemplateName,
  setTemplates,
  connected,
  recording,
  busy,
  beginNotes,
  result,
  resultSection,
  selectedStep,
  setEditing,
  setEditingNew,
  moveStep,
  updateTemplate,
  copied,
  copy,
  showEvidence,
  locale,
}: Props) {
  return (
    <>
      {!meeting && <div className="notice warning">{t.noMeeting}</div>}
      {libraryInvalid && (
        <div role="alert" className="notice error">
          {t.templateError}
        </div>
      )}
      <section className="notes-config">
        <div className="settings-grid">
          <label>
            {t.processing}
            <select
              value={mode}
              disabled={notesBusy}
              onChange={(event) => {
                setMode(event.target.value as "local" | "cloud");
                setEndpoint(
                  event.target.value === "local"
                    ? "http://127.0.0.1:1234/v1/"
                    : "https://openrouter.ai/api/v1/",
                );
                setApiKey("");
              }}
            >
              <option value="local">{t.local}</option>
              <option value="cloud">{t.cloud}</option>
            </select>
          </label>
          <label>
            {t.textModel}
            <input
              value={textModel}
              disabled={notesBusy}
              onChange={(event) => setTextModel(event.target.value)}
              placeholder="google/gemma-4-e4b"
              autoComplete="off"
            />
          </label>
        </div>
        <label>
          {t.endpoint}
          <input
            type="url"
            value={endpoint}
            disabled={notesBusy}
            onChange={(event) => setEndpoint(event.target.value)}
            autoComplete="off"
          />
        </label>
        <details>
          <summary>{t.apiKey}</summary>
          <label>
            {t.apiKey}
            <input
              type="password"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              autoComplete="new-password"
            />
          </label>
          <p>{t.apiKeyHelp}</p>
        </details>
        <div className="template-selector">
          <label>
            {t.template}
            <select
              value={template.id}
              disabled={notesBusy || libraryInvalid}
              onChange={(event) => {
                const next = templates.find(
                  (item) => item.id === event.target.value,
                )!;
                setTemplateId(next.id);
                setStepId(next.steps[0].id);
                setResultId("");
              }}
            >
              {templates.map((item) => (
                <option value={item.id} key={item.id}>
                  {item.name}
                </option>
              ))}
            </select>
          </label>
          <button
            className="icon-button bordered"
            disabled={notesBusy || libraryInvalid}
            title={t.newTemplate}
            aria-label={t.newTemplate}
            onClick={() => setTemplateName({ id: null, name: "" })}
          >
            <Plus size={17} />
          </button>
          <button
            className="icon-button bordered"
            disabled={notesBusy || libraryInvalid}
            title={t.renameTemplate}
            aria-label={t.renameTemplate}
            onClick={() =>
              setTemplateName({ id: template.id, name: template.name })
            }
          >
            <Pencil size={16} />
          </button>
          <button
            className="icon-button bordered"
            disabled={notesBusy || libraryInvalid || templates.length < 2}
            title={t.removeTemplate}
            aria-label={t.removeTemplate}
            onClick={() => {
              const remaining = templates.filter(
                (item) => item.id !== template.id,
              );
              setTemplates(remaining);
              setTemplateId(remaining[0].id);
              setStepId(remaining[0].steps[0].id);
              setResultId("");
            }}
          >
            <Trash2 size={16} />
          </button>
        </div>
        {analyses.length > 0 && (
          <label>
            {t.earlier}
            <select
              value={resultId}
              onChange={(event) => {
                setResultId(event.target.value);
                const next = analyses.find(
                  (item) => item.id === event.target.value,
                );
                if (next) setStepId(next.sections[0]?.id ?? "");
              }}
            >
              <option value="">{t.currentTemplate}</option>
              {analyses.map((item) => (
                <option value={item.id} key={item.id}>
                  {item.template.name} ·{" "}
                  {new Date(item.createdAt).toLocaleString(
                    locale === "de" ? "de-DE" : "en-GB",
                  )}
                </option>
              ))}
            </select>
          </label>
        )}
        <div className="notes-run">
          <span>{meeting?.title ?? t.noMeeting}</span>
          <button
            className="primary"
            disabled={
              !connected ||
              !meeting ||
              meeting.status !== "complete" ||
              recording ||
              notesBusy ||
              busy ||
              !textModel.trim() ||
              !endpoint.trim() ||
              libraryInvalid
            }
            onClick={beginNotes}
          >
            <Sparkles size={17} />
            {notesBusy ? t.generating : t.generate}
          </button>
        </div>
      </section>
      <section className="analysis-panel">
        <div className="tabs-toolbar">
          <div className="analysis-tabs" role="tablist" aria-label={t.notes}>
            {(result ? result.sections : template.steps).map((item) => (
              <button
                role="tab"
                key={item.id}
                aria-selected={
                  (result ? resultSection?.id : selectedStep.id) === item.id
                }
                className={
                  (result ? resultSection?.id : selectedStep.id) === item.id
                    ? "analysis-tab active"
                    : "analysis-tab"
                }
                onClick={() => setStepId(item.id)}
                onContextMenu={(event) => {
                  if (result || libraryInvalid) return;
                  event.preventDefault();
                  setEditing({ ...(item as AnalysisStep) });
                  setEditingNew(false);
                }}
              >
                {item.name}
              </button>
            ))}
          </div>
          {!result && (
            <button
              className="icon-button"
              disabled={notesBusy || libraryInvalid}
              title={t.addTab}
              aria-label={t.addTab}
              onClick={() => {
                setEditing({
                  id: uid(),
                  name: "",
                  kind: "analysis",
                  prompt: "",
                });
                setEditingNew(true);
              }}
            >
              <Plus size={19} />
            </button>
          )}
        </div>
        {!result && (
          <div className="step-actions">
            <button
              className="text-button"
              disabled={libraryInvalid || notesBusy}
              onClick={() => {
                setEditing({ ...selectedStep });
                setEditingNew(false);
              }}
            >
              <Pencil size={14} />
              {t.editTab}
            </button>
            <div>
              <button
                className="icon-button"
                title={t.moveLeft}
                aria-label={t.moveLeft}
                disabled={
                  libraryInvalid ||
                  notesBusy ||
                  template.steps[0].id === selectedStep.id
                }
                onClick={() => moveStep(-1)}
              >
                <ChevronLeft size={16} />
              </button>
              <button
                className="icon-button"
                title={t.moveRight}
                aria-label={t.moveRight}
                disabled={
                  libraryInvalid ||
                  notesBusy ||
                  template.steps.at(-1)?.id === selectedStep.id
                }
                onClick={() => moveStep(1)}
              >
                <ChevronRight size={16} />
              </button>
              <button
                className="icon-button"
                title={t.removeTab}
                aria-label={t.removeTab}
                disabled={
                  libraryInvalid || notesBusy || template.steps.length < 2
                }
                onClick={() => {
                  const steps = template.steps.filter(
                    (item) => item.id !== selectedStep.id,
                  );
                  updateTemplate({ ...template, steps });
                  setStepId(steps[0].id);
                }}
              >
                <Trash2 size={15} />
              </button>
            </div>
          </div>
        )}
        <div className="analysis-body" role="tabpanel">
          {resultSection ? (
            resultSection.items.length ? (
              <ul className="fact-list">
                {resultSection.items.map((fact, index) => (
                  <li key={index}>
                    <p>{fact.text}</p>
                    {resultSection.kind === "tasks" && (
                      <div className="fact-details">
                        {t.owner}: {fact.owner ?? t.unknown} · {t.due}:{" "}
                        {fact.due ?? t.unknown}
                      </div>
                    )}
                    <div className="evidence-list">
                      {fact.evidence.map((evidence, evidenceIndex) => (
                        <button
                          className="source-button"
                          key={evidenceIndex}
                          onClick={() => showEvidence(evidence)}
                          title={t.evidence}
                        >
                          [{evidence.sourceId}] <span>„{evidence.quote}“</span>
                        </button>
                      ))}
                    </div>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="muted">{t.noItems}</p>
            )
          ) : (
            <div className="empty">
              <Sparkles size={29} />
              <p>{t.noResults}</p>
            </div>
          )}
        </div>
        <div className="analysis-footer">
          <span>{result ? t.draft : t.instructionHelp}</span>
          <button
            className="secondary"
            disabled={!result || busy}
            onClick={() => result && void copy(renderAnalysis(result))}
          >
            {copied ? <Check size={16} /> : <Copy size={16} />}
            <span>{copied ? t.copied : t.copyResults}</span>
          </button>
        </div>
      </section>
    </>
  );
}
