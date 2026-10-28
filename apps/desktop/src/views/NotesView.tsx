import {
  Check,
  ChevronLeft,
  ChevronRight,
  Copy,
  Pencil,
  Plus,
  Sparkles,
  Trash2,
  RefreshCw,
} from "lucide-react";
import { uid, meetingLabel } from "../domain";
import type {
  AnalysisResult,
  AnalysisStep,
  AnalysisTemplate,
  Evidence,
  Meeting,
  MeetingSummary,
  Locale,
} from "../types";
import type { MessageKey } from "../i18n";
import type { TemplateAction } from "./TemplateDialog";
import CredentialPanel from "./CredentialPanel";
import LocalModelSetup from "./LocalModelSetup";
import { useEffect, useState } from "react";
import type { DesktopApi } from "../types";
type Props = {
  t: Record<MessageKey, string>;
  api: DesktopApi;
  useSavedKey: boolean;
  setUseSavedKey: (value: boolean) => void;
  meeting: Meeting | null;
  meetingId: string;
  meetings: MeetingSummary[];
  meetingLoading: boolean;
  selectMeeting: (id: string) => void;
  localModels: string[];
  modelsLoading: boolean;
  modelsError: string | null;
  modelsLoaded: boolean;
  refreshModels: () => void;
  manualModel: boolean;
  setManualModel: (manual: boolean) => void;
  modelReady: boolean;
  libraryInvalid: boolean;
  libraryPending: boolean;
  libraryError: string | null;
  browserInvalid: boolean;
  refreshTemplates: () => void;
  importBrowser: (() => void) | null;
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
  openTemplateAction: (
    kind: TemplateAction["kind"],
    original?: AnalysisTemplate,
  ) => void;
  connected: boolean;
  recording: boolean;
  busy: boolean;
  beginNotes: () => void;
  result: AnalysisResult | undefined;
  resultSection: AnalysisResult["sections"][number] | undefined;
  selectedStep: AnalysisStep;
  setEditing: (v: AnalysisStep | null) => void;
  inspectStep: (v: AnalysisStep) => void;
  setEditingNew: (v: boolean) => void;
  moveStep: (delta: number) => void;
  updateTemplate: (v: AnalysisTemplate) => Promise<boolean>;
  copied: boolean;
  copyResults: () => Promise<void>;
  showEvidence: (evidence: Evidence) => void;
  locale: Locale;
};
export default function NotesView({
  t,
  api,
  useSavedKey,
  setUseSavedKey,
  meeting,
  meetingId,
  meetings,
  meetingLoading,
  selectMeeting,
  localModels,
  modelsLoading,
  modelsError,
  modelsLoaded,
  refreshModels,
  manualModel,
  setManualModel,
  modelReady,
  libraryInvalid,
  libraryPending,
  libraryError,
  browserInvalid,
  refreshTemplates,
  importBrowser,
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
  openTemplateAction,
  connected,
  recording,
  busy,
  beginNotes,
  result,
  resultSection,
  selectedStep,
  setEditing,
  inspectStep,
  setEditingNew,
  moveStep,
  updateTemplate,
  copied,
  copyResults,
  showEvidence,
  locale,
}: Props) {
  const [setupOpen, setSetupOpen] = useState(!result);
  const [localSetupOpen, setLocalSetupOpen] = useState(false);
  useEffect(() => {
    if (notesBusy || result) setSetupOpen(false);
  }, [notesBusy, result?.id]);
  const templateLocked =
    notesBusy || libraryInvalid || libraryPending || !connected;
  const builtIn = template.id === "speaker-name-candidates-v1";
  // The shipped prompt carries the speaker evidence rule; duplicate it to change steps.
  const stepsLocked = templateLocked || builtIn;
  const editableTemplateCount = templates.filter(
    (t) => t.id !== "speaker-name-candidates-v1",
  ).length;
  const visibleSteps = result ? result.sections : template.steps;
  const activeId =
    (result ? resultSection?.id : selectedStep.id) ?? visibleSteps[0].id;
  const tabId = (id: string) => "analysis-tab-" + encodeURIComponent(id);
  return (
    <>
      <section className="notes-meeting-picker">
        <label>
          {t.chooseMeeting}
          <select
            value={meetingId}
            disabled={!connected}
            onChange={(event) => selectMeeting(event.target.value)}
          >
            <option value="">{t.selectMeeting}</option>
            {meetings.map((item) => (
              <option key={item.id} value={item.id}>
                {meetingLabel(item, locale)}
              </option>
            ))}
          </select>
        </label>
        {meetingLoading && (
          <p className="muted">
            <RefreshCw size={14} className="spinning" />
            {t.busy}
          </p>
        )}
      </section>
      {!meeting && <div className="notice warning">{t.noMeeting}</div>}
      {meeting && meeting.status !== "complete" && (
        <div className="notice warning">{t.pendingNotes}</div>
      )}
      {browserInvalid && (
        <div role="alert" className="notice error">
          {t.templateError}
        </div>
      )}
      {libraryError && (
        <div role="alert" className="notice error">
          <div>
            <strong>{t.templateLoadFailed}</strong>
            <p>{libraryError}</p>
          </div>
          <button
            className="secondary"
            disabled={libraryPending}
            onClick={refreshTemplates}
          >
            {t.refreshTemplates}
          </button>
        </div>
      )}
      <section className="notes-config">
        <details
          className="analysis-setup"
          open={setupOpen}
          onToggle={(event) => setSetupOpen(event.currentTarget.open)}
        >
          <summary>
            <strong>{t.analysisSetup}</strong>
            <span>
              {[textModel, template.name].filter(Boolean).join(" · ")}
            </span>
          </summary>
          <div className="analysis-setup-fields">
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
                {mode === "local" && !manualModel ? (
                  <div className="inline-control">
                    <select
                      aria-label={t.textModel}
                      value={textModel}
                      disabled={notesBusy || modelsLoading}
                      onChange={(event) => setTextModel(event.target.value)}
                    >
                      <option value="">
                        {modelsLoading
                          ? t.loadingTextModels
                          : t.selectTextModel}
                      </option>
                      {textModel && !localModels.includes(textModel) && (
                        <option value={textModel} disabled>
                          {textModel} · {t.notFound}
                        </option>
                      )}
                      {localModels.map((model) => (
                        <option value={model} key={model}>
                          {model}
                        </option>
                      ))}
                    </select>
                    <button
                      type="button"
                      className="icon-button bordered"
                      onClick={refreshModels}
                      disabled={notesBusy || modelsLoading || !connected}
                      title={t.refreshTextModels}
                      aria-label={t.refreshTextModels}
                    >
                      <RefreshCw
                        size={17}
                        className={modelsLoading ? "spinning" : undefined}
                      />
                    </button>
                  </div>
                ) : (
                  <input
                    aria-label={t.textModel}
                    value={textModel}
                    disabled={notesBusy}
                    onChange={(event) => setTextModel(event.target.value)}
                    autoComplete="off"
                  />
                )}
              </label>
            </div>
            {mode === "local" && (
              <details className="local-service-help">
                <summary>{t.setupHelp}</summary>
                <p>{t.localServiceHelp}</p>
                <p>{t.firstModelLoad}</p>
              </details>
            )}
            {mode === "local" && (
              <>
                <label className="checkbox-label">
                  <input
                    type="checkbox"
                    checked={manualModel}
                    disabled={notesBusy}
                    onChange={(event) => setManualModel(event.target.checked)}
                  />
                  {t.manualTextModel}
                </label>
                {modelsError && (
                  <div className="notice warning">
                    <div>
                      <strong>{modelsError}</strong>
                      <p>{t.textModelsHelp}</p>
                    </div>
                  </div>
                )}
                {modelsLoaded && !localModels.length && (
                  <p className="muted">
                    {t.noTextModels} {t.textModelsHelp}
                  </p>
                )}
                {textModel &&
                  !manualModel &&
                  modelsLoaded &&
                  !localModels.includes(textModel) && (
                    <p className="inline-error">{t.textModelUnavailable}</p>
                  )}
              </>
            )}
            {mode === "local" && (
              <details
                className="connection-settings"
                onToggle={(event) =>
                  setLocalSetupOpen(event.currentTarget.open)
                }
              >
                <summary>
                  {locale === "de"
                    ? "Lokales Modell einrichten (Ollama oder LM Studio)"
                    : "Set up a local model (Ollama or LM Studio)"}
                </summary>
                {/* Probes loopback and reads memory only while opened. */}
                {localSetupOpen && (
                  <LocalModelSetup
                    api={api}
                    locale={locale}
                    connected={connected}
                    onUse={(nextEndpoint, nextModel) => {
                      // A key entered for another route must never reach the local server.
                      setApiKey("");
                      setUseSavedKey(false);
                      setEndpoint(nextEndpoint);
                      setTextModel(nextModel);
                    }}
                  />
                )}
              </details>
            )}
            <details
              className="connection-settings"
              open={mode === "cloud" ? true : undefined}
            >
              <summary>{t.connectionSettings}</summary>
              <label>
                {mode === "local" ? t.localEndpoint : t.endpoint}
                <input
                  type="url"
                  value={endpoint}
                  disabled={notesBusy}
                  onChange={(event) => setEndpoint(event.target.value)}
                  autoComplete="off"
                />
              </label>
              <CredentialPanel
                api={api}
                endpoint={endpoint}
                mode={mode}
                connected={connected}
                disabled={notesBusy}
                apiKey={apiKey}
                setApiKey={setApiKey}
                useSavedKey={useSavedKey}
                setUseSavedKey={setUseSavedKey}
                t={t}
              />
            </details>
            <div className="template-selector">
              <label>
                {t.template}
                <select
                  value={template.id}
                  disabled={templateLocked}
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
            </div>
            <div className="template-actions" aria-label={t.template}>
              <button
                className="secondary"
                disabled={templateLocked || templates.length >= 100}
                onClick={() => openTemplateAction("new")}
              >
                <Plus size={15} />
                {t.newTemplate}
              </button>
              <button
                className="secondary"
                disabled={templateLocked || templates.length >= 100}
                onClick={() =>
                  openTemplateAction("duplicate", result?.template ?? template)
                }
              >
                <Copy size={15} />
                {t.duplicateTemplate}
              </button>
              <button
                className="text-button"
                disabled={templateLocked || builtIn}
                onClick={() => openTemplateAction("rename")}
              >
                <Pencil size={14} />
                {t.renameTemplate}
              </button>
              <button
                className="text-button"
                disabled={
                  templateLocked || builtIn || editableTemplateCount < 2
                }
                title={
                  editableTemplateCount < 2 ? t.lastTemplate : t.removeTemplate
                }
                onClick={() => openTemplateAction("delete")}
              >
                <Trash2 size={14} />
                {t.removeTemplate}
              </button>
              <button
                className="text-button"
                disabled={notesBusy || libraryPending || !connected}
                onClick={refreshTemplates}
              >
                <RefreshCw
                  size={14}
                  className={libraryPending ? "spinning" : undefined}
                />
                {t.refreshTemplates}
              </button>
            </div>
            <p className="muted">{t.templatesPortable}</p>
            {importBrowser && (
              <details>
                <summary>{t.importBrowserTemplates}</summary>
                <p>{t.browserTemplatesHelp}</p>
                <button
                  className="secondary"
                  disabled={templateLocked}
                  onClick={importBrowser}
                >
                  {t.importBrowserTemplates}
                </button>
              </details>
            )}
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
                      {item.currentPrompt === false &&
                        ` · ${t.earlierPrompt} (${t.readOnly})`}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </div>
        </details>
        <div className="notes-run">
          <span>{meeting ? meetingLabel(meeting, locale) : t.noMeeting}</span>
          <button
            className="primary"
            disabled={
              !connected ||
              !meeting ||
              meetingLoading ||
              meeting.status !== "complete" ||
              recording ||
              notesBusy ||
              busy ||
              !textModel.trim() ||
              !modelReady ||
              !endpoint.trim() ||
              libraryInvalid ||
              libraryPending
            }
            onClick={beginNotes}
          >
            {notesBusy ? (
              <RefreshCw size={17} className="spinning" />
            ) : (
              <Sparkles size={17} />
            )}
            {notesBusy ? t.generating : t.generate}
          </button>
        </div>
      </section>
      <section className="analysis-panel">
        {result?.currentPrompt === false && (
          <p className="notice warning" role="note">
            {t.earlierPrompt}: {result.promptVersion} · {t.earlierPromptResult}
          </p>
        )}
        {result && (
          <div className="saved-template-notice">
            <details>
              <summary>{t.resultTemplate}</summary>
              <p>{t.historicalTemplateHelp}</p>
            </details>
            <button
              className="secondary"
              onClick={() => {
                setResultId("");
                setStepId(template.steps[0].id);
              }}
            >
              {t.editCurrentTemplate}
            </button>
          </div>
        )}
        <div className="tabs-toolbar">
          <div className="analysis-tabs" role="tablist" aria-label={t.notes}>
            {visibleSteps.map((item, position) => (
              <button
                role="tab"
                key={item.id}
                id={tabId(item.id)}
                aria-controls="analysis-step-panel"
                tabIndex={activeId === item.id ? 0 : -1}
                aria-selected={
                  (result ? resultSection?.id : selectedStep.id) === item.id
                }
                className={
                  (result ? resultSection?.id : selectedStep.id) === item.id
                    ? "analysis-tab active"
                    : "analysis-tab"
                }
                onClick={() => setStepId(item.id)}
                onKeyDown={(event) => {
                  if (
                    ["ArrowLeft", "ArrowRight", "Home", "End"].includes(
                      event.key,
                    )
                  ) {
                    event.preventDefault();
                    const next =
                      event.key === "Home"
                        ? 0
                        : event.key === "End"
                          ? visibleSteps.length - 1
                          : (position +
                              (event.key === "ArrowLeft" ? -1 : 1) +
                              visibleSteps.length) %
                            visibleSteps.length;
                    setStepId(visibleSteps[next].id);
                    event.currentTarget
                      .closest('[role="tablist"]')
                      ?.querySelectorAll<HTMLElement>('[role="tab"]')
                      [next]?.focus();
                  } else if (event.key === "F2") {
                    event.preventDefault();
                    if (result) {
                      const saved = result.template.steps.find(
                        (step) => step.id === item.id,
                      );
                      if (saved) inspectStep(saved);
                    } else if (!stepsLocked) {
                      setEditing({ ...(item as AnalysisStep) });
                      setEditingNew(false);
                    }
                  }
                }}
                onContextMenu={(event) => {
                  if (result || stepsLocked) return;
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
              disabled={stepsLocked || template.steps.length >= 100}
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
        <p className="tab-help muted">{t.tabKeyboardHelp}</p>
        {result && (
          <div className="step-actions">
            <button
              className="text-button"
              onClick={() => {
                const saved = result.template.steps.find(
                  (item) => item.id === resultSection?.id,
                );
                if (saved) inspectStep(saved);
              }}
            >
              <Pencil size={14} />
              {t.inspectInstructions}
            </button>
          </div>
        )}
        {!result && (
          <div className="step-actions">
            <button
              className="text-button"
              disabled={stepsLocked}
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
                  stepsLocked || template.steps[0].id === selectedStep.id
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
                  stepsLocked || template.steps.at(-1)?.id === selectedStep.id
                }
                onClick={() => moveStep(1)}
              >
                <ChevronRight size={16} />
              </button>
              <button
                className="icon-button"
                title={t.removeTab}
                aria-label={t.removeTab}
                disabled={stepsLocked || template.steps.length < 2}
                onClick={async () => {
                  const steps = template.steps.filter(
                    (item) => item.id !== selectedStep.id,
                  );
                  if (await updateTemplate({ ...template, steps }))
                    setStepId(steps[0].id);
                }}
              >
                <Trash2 size={15} />
              </button>
            </div>
          </div>
        )}
        <div
          className="analysis-body"
          role="tabpanel"
          id="analysis-step-panel"
          aria-labelledby={tabId(activeId)}
          tabIndex={0}
        >
          {resultSection && rejectedCount(resultSection) > 0 && (
            <p className="muted rejected-note">
              {rejectedCount(resultSection)} {t.rejectedItems}
            </p>
          )}
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
            onClick={() => result && void copyResults()}
            title={t.markdownHelp}
          >
            {copied ? <Check size={16} /> : <Copy size={16} />}
            <span>{copied ? t.copied : t.copyMarkdown}</span>
          </button>
        </div>
      </section>
    </>
  );
}
function rejectedCount(section: AnalysisResult["sections"][number]): number {
  return Object.values(section.rejected ?? {}).reduce(
    (sum, count) => sum + (count ?? 0),
    0,
  );
}
