import NotesView from "./views/NotesView";
import Sidebar, { type Page } from "./views/Sidebar";
import MeetingView from "./views/MeetingView";
import { useCallback, useEffect, useRef, useState } from "react";
import { FolderClock, RefreshCw, X } from "lucide-react";
import { desktopApi, errorMessage } from "./api";
import {
  clock,
  defaultTemplate,
  renderTranscript,
  trackName,
  uid,
  validateLibrary,
} from "./domain";
import { de, en, extraDe, extraEn } from "./i18n";
import type {
  AnalysisResult,
  AnalysisStep,
  AnalysisTemplate,
  AppState,
  AudioDevice,
  DesktopApi,
  Evidence,
  Locale,
  Meeting,
  MeetingSummary,
  NotesRequest,
  Theme,
  TranscriptSource,
} from "./types";

const TEMPLATE_KEY = "handymeeting.rust.beta.templates.v1";
const PREFS_KEY = "handymeeting.rust.beta.preferences.v1";
function loadPreferences(): { locale: Locale; theme: Theme } {
  try {
    const value = JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}");
    return {
      locale: value.locale === "en" ? "en" : "de",
      theme: ["dark", "light", "system"].includes(value.theme)
        ? value.theme
        : "system",
    };
  } catch {
    return { locale: "de", theme: "system" };
  }
}
function loadLibrary(): { templates: AnalysisTemplate[]; invalid: boolean } {
  const initial = { templates: [defaultTemplate()], invalid: false };
  try {
    const raw = localStorage.getItem(TEMPLATE_KEY);
    if (!raw) return initial;
    const templates: unknown = JSON.parse(raw);
    return validateLibrary(templates)
      ? { templates, invalid: false }
      : { ...initial, invalid: true };
  } catch {
    return { ...initial, invalid: true };
  }
}
export default function App({ api = desktopApi }: { api?: DesktopApi }) {
  const [preferences] = useState(loadPreferences);
  const [locale, setLocale] = useState<Locale>(preferences.locale),
    [theme, setTheme] = useState<Theme>(preferences.theme);
  const t = locale === "de" ? de : en,
    x = locale === "de" ? extraDe : extraEn;
  const [page, setPage] = useState<Page>("meeting"),
    [app, setApp] = useState<AppState | null>(null),
    [connected, setConnected] = useState(false),
    [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null),
    [busy, setBusy] = useState(false),
    [isStopping, setIsStopping] = useState(false),
    [refreshingHandy, setRefreshingHandy] = useState(false),
    [copied, setCopied] = useState(false);
  const [devices, setDevices] = useState<AudioDevice[]>([]),
    [history, setHistory] = useState<MeetingSummary[]>([]),
    [meeting, setMeeting] = useState<Meeting | null>(null);
  const [legacyManifest, setLegacyManifest] = useState("");
  const [title, setTitle] = useState(""),
    [modelId, setModelId] = useState(""),
    [systemId, setSystemId] = useState(""),
    [micId, setMicId] = useState(""),
    [micEnabled, setMicEnabled] = useState(false),
    [handyPath, setHandyPath] = useState("");
  const [library] = useState(loadLibrary),
    [templates, setTemplates] = useState(library.templates),
    [templateId, setTemplateId] = useState(library.templates[0].id),
    [stepId, setStepId] = useState(library.templates[0].steps[0].id);
  const [editing, setEditing] = useState<AnalysisStep | null>(null),
    [editingNew, setEditingNew] = useState(false),
    [templateName, setTemplateName] = useState<{
      id: string | null;
      name: string;
    } | null>(null);
  const [endpoint, setEndpoint] = useState("http://127.0.0.1:1234/v1/"),
    [textModel, setTextModel] = useState(""),
    [apiKey, setApiKey] = useState(""),
    [mode, setMode] = useState<"local" | "cloud">("local");
  const [analyses, setAnalyses] = useState<AnalysisResult[]>([]),
    [resultId, setResultId] = useState(""),
    [notesBusy, setNotesBusy] = useState(false),
    [cloudRequest, setCloudRequest] = useState<NotesRequest | null>(null),
    [source, setSource] = useState<{
      source: TranscriptSource;
      quote: string;
    } | null>(null);
  const requestSequence = useRef(0),
    selectedRef = useRef<string | null>(null),
    fullMeetingRef = useRef<string | null>(null),
    initialModelSelected = useRef(false),
    initialCatalogSeen = useRef(false),
    lastModelRefresh = useRef(Date.now()),
    modelRefreshInFlight = useRef<Promise<void> | null>(null),
    mounted = useRef(true),
    copyTimer = useRef<ReturnType<typeof setTimeout>>();
  const template =
    templates.find((item) => item.id === templateId) ?? templates[0];
  const result = analyses.find((item) => item.id === resultId);
  const models = (app?.handy?.models ?? []).filter(
    (item) =>
      item.isDownloaded && item.supportsStreaming && !item.compatibilityError,
  );
  const recording = app?.recording ?? false;
  const catchingUp =
    !recording &&
    ["catching-up", "streaming"].includes(app?.activeMeeting?.status ?? "");
  const latestRuntime = useRef({ app, busy, isStopping, connected });
  latestRuntime.current = { app, busy, isStopping, connected };
  const activeId = app?.activeMeeting?.id;

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.lang = locale;
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify({ locale, theme }));
    } catch {
      /* Preferences are optional. */
    }
  }, [locale, theme]);
  useEffect(() => {
    if (library.invalid) return;
    try {
      localStorage.setItem(TEMPLATE_KEY, JSON.stringify(templates));
    } catch {
      setError(
        locale === "de"
          ? "Vorlagen konnten nicht lokal gespeichert werden."
          : "Templates could not be saved locally.",
      );
    }
  }, [templates, library.invalid, locale]);
  useEffect(
    () => () => {
      if (copyTimer.current) clearTimeout(copyTimer.current);
    },
    [],
  );
  const modalOpen = Boolean(editing || templateName || cloudRequest || source);
  useEffect(() => {
    if (!modalOpen) return;
    const previous = document.activeElement as HTMLElement | null;
    const dialog = document.querySelector<HTMLElement>('[role="dialog"]');
    const surfaces = document.querySelectorAll<HTMLElement>(".sidebar, main");
    surfaces.forEach((element) => {
      element.inert = true;
    });
    const focusable = () =>
      Array.from(
        dialog?.querySelectorAll<HTMLElement>(
          'button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),[tabindex="0"]',
        ) ?? [],
      );
    (
      dialog?.querySelector<HTMLElement>("input,textarea") ?? focusable()[0]
    )?.focus();
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setEditing(null);
        setTemplateName(null);
        setCloudRequest(null);
        setSource(null);
        return;
      }
      if (event.key !== "Tab") return;
      const targets = focusable(),
        first = targets[0],
        last = targets.at(-1);
      if (
        event.shiftKey &&
        (document.activeElement === first ||
          !dialog?.contains(document.activeElement))
      ) {
        event.preventDefault();
        last?.focus();
      } else if (
        !event.shiftKey &&
        (document.activeElement === last ||
          !dialog?.contains(document.activeElement))
      ) {
        event.preventDefault();
        first?.focus();
      }
    };
    document.addEventListener("keydown", keydown);
    return () => {
      document.removeEventListener("keydown", keydown);
      surfaces.forEach((element) => {
        element.inert = false;
      });
      previous?.focus();
    };
  }, [modalOpen]);
  useEffect(() => {
    if (!app?.handy) return;
    if (
      !initialCatalogSeen.current &&
      (app.handy.executable ||
        app.handy.version ||
        app.handy.error ||
        app.handy.models.length)
    ) {
      initialCatalogSeen.current = true;
      lastModelRefresh.current = Date.now();
    }
    if (modelId) {
      initialModelSelected.current = true;
      if (!models.some((item) => item.id === modelId)) {
        setModelId("");
        setError(t.modelUnavailable);
      }
    } else if (!initialModelSelected.current && models.length) {
      initialModelSelected.current = true;
      setModelId(models[0].id);
    }
  }, [app?.handy, modelId, t.modelUnavailable]);
  const applyDevices = useCallback((items: AudioDevice[]) => {
    setDevices(items);
    setSystemId((old) =>
      items.some((d) => d.source === "system" && d.id === old)
        ? old
        : (items.find((d) => d.source === "system" && d.isDefault)?.id ??
          items.find((d) => d.source === "system")?.id ??
          ""),
    );
    setMicId((old) =>
      items.some((d) => d.source === "microphone" && d.id === old)
        ? old
        : (items.find((d) => d.source === "microphone" && d.isDefault)?.id ??
          items.find((d) => d.source === "microphone")?.id ??
          ""),
    );
  }, []);
  const refreshHistory = useCallback(async () => {
    setHistory(await api.listMeetings());
  }, [api]);
  const refreshDevices = useCallback(async () => {
    applyDevices(await api.listDevices());
  }, [api, applyDevices]);
  const poll = useCallback(async () => {
    const seq = ++requestSequence.current;
    try {
      const state = await api.getAppState();
      if (seq !== requestSequence.current) return;
      setApp(state);
      setConnected(true);
      setLoading(false);
      if (
        state.activeMeeting &&
        (!selectedRef.current || selectedRef.current === state.activeMeeting.id)
      ) {
        selectedRef.current = state.activeMeeting.id;
        const active = state.activeMeeting;
        const live =
          state.recording ||
          ["recording", "catching-up", "streaming"].includes(active.status);
        if (live) {
          fullMeetingRef.current = null;
          setMeeting(active);
        } else if (fullMeetingRef.current !== active.id) {
          // A completed history view is loaded once. Its full transcript must
          // never be replaced by the next bounded status-preview response.
          const full = await api.getMeeting(active.id);
          if (
            seq !== requestSequence.current ||
            selectedRef.current !== active.id
          )
            return;
          fullMeetingRef.current = full.id;
          setMeeting(full);
        }
      }
    } catch (failure) {
      if (seq !== requestSequence.current) return;
      setConnected(false);
      setLoading(false);
      setError(errorMessage(failure));
    }
  }, [api]);
  const refreshModels = useCallback(
    async (automatic: boolean) => {
      const current = latestRuntime.current;
      const blocked = (state: AppState | null) =>
        Boolean(
          state?.recording ||
            state?.transitioning ||
            ["recording", "catching-up", "streaming"].includes(
              state?.activeMeeting?.status ?? "",
            ),
        );
      if (
        automatic &&
        (!initialCatalogSeen.current ||
          !current.connected ||
          current.busy ||
          current.isStopping ||
          blocked(current.app) ||
          Date.now() - lastModelRefresh.current < 30000)
      )
        return;
      if (modelRefreshInFlight.current) return modelRefreshInFlight.current;
      let queriedHandy = false;
      const pending = (async () => {
        // Recheck the backend before spawning Handy: the last rendered status can
        // predate a recording or ASR catch-up started by another command.
        const fresh = await api.getAppState();
        if (
          !mounted.current ||
          (automatic && !fresh.handy) ||
          blocked(fresh) ||
          latestRuntime.current.isStopping ||
          (automatic && latestRuntime.current.busy)
        )
          return;
        lastModelRefresh.current = Date.now();
        queriedHandy = true;
        setRefreshingHandy(true);
        const handy = await api.refreshHandy(
          automatic
            ? (fresh.handy?.executable ?? null)
            : handyPath.trim() || null,
        );
        if (!mounted.current) return;
        requestSequence.current++;
        setApp((previous) =>
          previous ? { ...previous, handy } : { ...fresh, handy },
        );
        if (handy.error) setError(handy.error);
      })();
      modelRefreshInFlight.current = pending;
      try {
        await pending;
      } finally {
        modelRefreshInFlight.current = null;
        if (queriedHandy) lastModelRefresh.current = Date.now();
        if (mounted.current) setRefreshingHandy(false);
      }
    },
    [api, handyPath],
  );
  useEffect(() => {
    const onFocus = () => {
      void refreshModels(true).catch((failure) => {
        if (mounted.current) setError(errorMessage(failure));
      });
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refreshModels]);
  useEffect(() => {
    let disposed = false;
    void poll().then(async () => {
      if (disposed) return;
      try {
        await Promise.all([refreshDevices(), refreshHistory()]);
      } catch (failure) {
        if (!disposed) setError(errorMessage(failure));
      }
    });
    const timer = setInterval(() => void poll(), 1500);
    return () => {
      disposed = true;
      requestSequence.current++;
      clearInterval(timer);
    };
  }, [poll, refreshDevices, refreshHistory]);
  async function action(work: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await work();
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setBusy(false);
    }
  }
  async function openMeeting(id: string, nextPage: Page = "meeting") {
    await action(async () => {
      const item = await api.getMeeting(id);
      selectedRef.current = item.id;
      fullMeetingRef.current = item.id;
      setMeeting(item);
      setAnalyses([]);
      setResultId("");
      setPage(nextPage);
      if (nextPage === "notes") await readAnalyses(item.id);
    });
  }
  async function readAnalyses(id: string) {
    const items = await api.listAnalyses(id);
    if (selectedRef.current !== id) return;
    setAnalyses(items);
    setResultId(items[0]?.id ?? "");
  }
  async function navigate(next: Page) {
    setPage(next);
    if (next === "history") void action(refreshHistory);
    if (next === "notes" && meeting)
      void action(() => readAnalyses(meeting.id));
  }
  async function start() {
    await action(async () => {
      const item = await api.startMeeting({
        title: title.trim() || t.emptyTitle,
        modelId,
        systemDeviceId: systemId,
        microphoneDeviceId: micEnabled ? micId : null,
      });
      selectedRef.current = item.id;
      fullMeetingRef.current = null;
      setMeeting(item);
      setAnalyses([]);
      setResultId("");
      await poll();
      await refreshHistory();
    });
  }
  async function stop() {
    await action(async () => {
      setIsStopping(true);
      try {
        const item = await api.stopMeeting();
        selectedRef.current = item.id;
        fullMeetingRef.current = item.id;
        setMeeting(item);
        await poll();
        await refreshHistory();
      } finally {
        setIsStopping(false);
      }
    });
  }
  async function copy(text: string) {
    await action(async () => {
      await api.copyText(text);
      setCopied(true);
      if (copyTimer.current) clearTimeout(copyTimer.current);
      copyTimer.current = setTimeout(() => setCopied(false), 2000);
    });
  }
  async function copyFullTranscript() {
    if (!meeting) return;
    const id = meeting.id;
    await action(async () => {
      const full = await api.getMeeting(id);
      await api.copyText(renderTranscript(full, locale));
      setCopied(true);
      if (copyTimer.current) clearTimeout(copyTimer.current);
      copyTimer.current = setTimeout(() => setCopied(false), 2000);
    });
  }
  function updateTemplate(next: AnalysisTemplate) {
    setTemplates((items) =>
      items.map((item) => (item.id === next.id ? next : item)),
    );
  }
  function saveStep() {
    if (!editing) return;
    if (!editing.name.trim() || !editing.prompt.trim()) {
      setError(t.invalidTemplate);
      return;
    }
    const next = {
      ...editing,
      name: editing.name.trim(),
      prompt: editing.prompt.trim(),
    };
    updateTemplate({
      ...template,
      steps: editingNew
        ? [...template.steps, next]
        : template.steps.map((item) => (item.id === next.id ? next : item)),
    });
    setStepId(next.id);
    setEditing(null);
  }
  function moveStep(delta: number) {
    const index = template.steps.findIndex((item) => item.id === stepId);
    if (
      index < 0 ||
      index + delta < 0 ||
      index + delta >= template.steps.length
    )
      return;
    const steps = [...template.steps];
    [steps[index], steps[index + delta]] = [steps[index + delta], steps[index]];
    updateTemplate({ ...template, steps });
  }
  function beginNotes() {
    if (!meeting) return;
    const request: NotesRequest = {
      meetingId: meeting.id,
      template: structuredClone(template),
      endpoint: endpoint.trim(),
      model: textModel.trim(),
      mode,
      cloudConsent: false,
      apiKey,
    };
    if (mode === "cloud") setCloudRequest(request);
    else void runNotes(request);
  }
  async function runNotes(request: NotesRequest) {
    if (notesBusy) return;
    setNotesBusy(true);
    setError(null);
    setCloudRequest(null);
    try {
      const next = await api.generateNotes(request);
      if (selectedRef.current !== next.meetingId) return;
      setAnalyses((items) => [
        next,
        ...items.filter((item) => item.id !== next.id),
      ]);
      setResultId(next.id);
      setStepId(next.sections[0]?.id ?? template.steps[0].id);
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setNotesBusy(false);
    }
  }
  function showEvidence(evidence: Evidence) {
    const original = result?.sources.find(
      (item) => item.id === evidence.sourceId,
    );
    if (!original || !original.text.includes(evidence.quote)) {
      setError(t.sourceMissing);
      return;
    }
    setSource({ source: original, quote: evidence.quote });
  }
  const statusName = (value: string) =>
    value === "complete"
      ? t.complete
      : value === "recording"
        ? t.recording
        : value === "recorded"
          ? t.locallySaved
          : t.pending;
  const selectedStep =
    template.steps.find((item) => item.id === stepId) ?? template.steps[0];
  const resultSection =
    result?.sections.find((item) => item.id === stepId) ?? result?.sections[0];
  const orderedParts = meeting
    ? [...meeting.parts]
        .sort(
          (a, b) =>
            a.startSample - b.startSample || a.source.localeCompare(b.source),
        )
        .filter((item) => (item.text ?? "").trim())
    : [];
  const recentParts =
    meeting?.status === "complete" && !meeting.livePreviewLimited
      ? orderedParts
      : orderedParts.slice(-40);
  const savedByTrack = (track: "system" | "microphone") =>
    meeting?.savedAudioMs?.[track] ??
    Math.max(
      0,
      ...(meeting?.parts ?? [])
        .filter((item) => item.source === track)
        .map(
          (item) =>
            (item.startSample +
              (item.confirmedSampleCount ?? item.sampleCount)) /
            16,
        ),
    );
  const backlog =
    meeting?.pendingAudioMs ??
    (meeting?.livePreviewLimited
      ? null
      : Math.max(
          0,
          ...["system", "microphone"].map(
            (track) =>
              (meeting?.parts ?? [])
                .filter((item) => item.source === track)
                .reduce(
                  (sum, item) =>
                    sum +
                    (item.status === "complete"
                      ? 0
                      : Math.max(
                          0,
                          item.sampleCount - (item.processedSamples ?? 0),
                        )),
                  0,
                ) / 16,
          ),
        ));

  return (
    <div className="app-shell">
      <Sidebar
        app={app}
        page={page}
        locale={locale}
        theme={theme}
        t={t}
        navigate={navigate}
        setTheme={setTheme}
        setLocale={setLocale}
      />
      <main>
        <header className="page-header">
          <div>
            <h1>{t[page]}</h1>
            <p>
              {page === "meeting"
                ? t.preserve
                : page === "notes"
                  ? t.draft
                  : page === "history"
                    ? t.legacyHelp
                    : t.compatibility}
            </p>
          </div>
          {recording && (
            <button
              className="recording-link"
              onClick={() => {
                if (activeId) void openMeeting(activeId);
              }}
            >
              <span className="recording-dot" />
              {t.recording}
            </button>
          )}
        </header>
        {error && (
          <div role="alert" className="notice error">
            <span>{error}</span>
            <button
              className="icon-button"
              aria-label={t.close}
              onClick={() => setError(null)}
            >
              <X size={17} />
            </button>
          </div>
        )}
        {isStopping && (
          <div role="status" className="notice info">
            {t.stoppingDetail}
          </div>
        )}
        {!!app?.error && app.error !== error && (
          <div className="notice warning" role="status">
            {app.error}
          </div>
        )}
        {!connected && (
          <div className="notice warning">
            <div>
              <strong>{loading ? t.connecting : t.disconnected}</strong>
              <p>{t.connectHelp}</p>
            </div>
            <button className="secondary" onClick={() => void poll()}>
              <RefreshCw size={15} />
              {t.retry}
            </button>
          </div>
        )}
        {page === "meeting" && (
          <MeetingView
            t={t}
            x={x}
            app={app}
            meeting={meeting}
            recording={recording}
            busy={
              busy ||
              refreshingHandy ||
              catchingUp ||
              (app?.transitioning ?? false)
            }
            isStopping={isStopping}
            connected={connected}
            title={title}
            modelId={modelId}
            systemId={systemId}
            micId={micId}
            micEnabled={micEnabled}
            models={models}
            devices={devices}
            copied={copied}
            backlog={backlog}
            recentParts={recentParts}
            locale={locale}
            setTitle={setTitle}
            setModelId={setModelId}
            setSystemId={setSystemId}
            setMicId={setMicId}
            setMicEnabled={setMicEnabled}
            start={start}
            stop={stop}
            savedByTrack={savedByTrack}
            copyFullTranscript={copyFullTranscript}
            refreshHandy={() => void action(() => refreshModels(false))}
            resume={() =>
              void action(async () => {
                if (!meeting) return;
                const current = await api.resumeMeeting(meeting.id);
                selectedRef.current = current.id;
                fullMeetingRef.current = null;
                setMeeting(current);
                await poll();
              })
            }
          />
        )}
        {page === "history" && (
          <section className="history-panel">
            <div className="section-heading">
              <h2>{t.storage}</h2>
              <button
                className="secondary"
                disabled={busy || !connected}
                onClick={() => void action(refreshHistory)}
              >
                <RefreshCw size={15} />
                {t.refresh}
              </button>
            </div>
            {history.length ? (
              <div className="session-list">
                {history.map((item) => (
                  <div className="session-row" key={item.id}>
                    <div>
                      <strong>{item.title}</strong>
                      <p>
                        {new Date(item.startedAt).toLocaleString(
                          locale === "de" ? "de-DE" : "en-GB",
                        )}{" "}
                        · {clock(item.durationMs)} · {statusName(item.status)}
                      </p>
                    </div>
                    <button
                      className="secondary"
                      disabled={busy}
                      onClick={() => void openMeeting(item.id)}
                    >
                      {t.open}
                    </button>
                  </div>
                ))}
              </div>
            ) : (
              <div className="empty">
                <FolderClock size={30} />
                <p>{t.emptyHistory}</p>
              </div>
            )}
          </section>
        )}
        {page === "settings" && (
          <div className="settings-sections">
            <section>
              <h2>{t.installation}</h2>
              <p>{t.executableHelp}</p>
              <label>
                {t.executable}
                <input
                  value={handyPath}
                  onChange={(event) => setHandyPath(event.target.value)}
                  disabled={
                    recording ||
                    busy ||
                    refreshingHandy ||
                    catchingUp ||
                    (app?.transitioning ?? false)
                  }
                  placeholder={
                    locale === "de"
                      ? "Automatisch erkennen"
                      : "Detect automatically"
                  }
                />
              </label>
              <div className="settings-actions">
                <span>
                  {t.version}: {app?.handy?.version ?? t.notFound}
                </span>
                <button
                  className="secondary"
                  disabled={
                    !connected ||
                    busy ||
                    recording ||
                    refreshingHandy ||
                    catchingUp ||
                    (app?.transitioning ?? false)
                  }
                  onClick={() => void action(() => refreshModels(false))}
                >
                  <RefreshCw size={15} />
                  {t.refresh}
                </button>
              </div>
              <p className="muted">{app?.handy?.executable}</p>
            </section>
            <section>
              <h2>{t.theme}</h2>
              <div className="settings-grid">
                <label>
                  {t.language}
                  <select
                    value={locale}
                    onChange={(event) =>
                      setLocale(event.target.value as Locale)
                    }
                  >
                    <option value="de">Deutsch</option>
                    <option value="en">English</option>
                  </select>
                </label>
                <label>
                  {t.theme}
                  <select
                    value={theme}
                    onChange={(event) => setTheme(event.target.value as Theme)}
                  >
                    <option value="system">{t.systemTheme}</option>
                    <option value="light">{t.light}</option>
                    <option value="dark">{t.dark}</option>
                  </select>
                </label>
              </div>
            </section>
            <section>
              <h2>{t.system}</h2>
              <button
                className="secondary"
                disabled={recording || busy || !connected}
                onClick={() => void action(refreshDevices)}
              >
                <RefreshCw size={15} />
                {t.refreshDevices}
              </button>
            </section>
            <section>
              <h2>{t.storage}</h2>
              <p>{t.legacyHelp}</p>
              {app?.dataDirectory && (
                <code className="storage-path">{app.dataDirectory}</code>
              )}
            </section>
          </div>
        )}
        {page === "settings" && (
          <section className="legacy-panel">
            <h2>{t.legacy}</h2>
            <p className="muted">{t.legacyHelp}</p>
            <label>
              {x.manifest}
              <input
                value={legacyManifest}
                onChange={(event) => setLegacyManifest(event.target.value)}
                autoComplete="off"
                placeholder="session.json"
              />
            </label>
            <button
              className="secondary"
              disabled={
                busy || recording || !connected || !legacyManifest.trim()
              }
              onClick={() =>
                void action(async () => {
                  const imported = await api.importLegacy(
                    legacyManifest.trim(),
                  );
                  selectedRef.current = imported.id;
                  fullMeetingRef.current = imported.id;
                  setMeeting(imported);
                  setAnalyses([]);
                  setResultId("");
                  setPage("meeting");
                  await refreshHistory();
                })
              }
            >
              {x.importSession}
            </button>
          </section>
        )}
        {page === "notes" && (
          <NotesView
            t={t}
            meeting={meeting}
            libraryInvalid={library.invalid}
            templates={templates}
            notesBusy={notesBusy}
            mode={mode}
            setMode={setMode}
            setEndpoint={setEndpoint}
            setApiKey={setApiKey}
            textModel={textModel}
            setTextModel={setTextModel}
            endpoint={endpoint}
            apiKey={apiKey}
            template={template}
            resultId={resultId}
            analyses={analyses}
            setTemplateId={setTemplateId}
            setStepId={setStepId}
            setResultId={setResultId}
            setTemplateName={setTemplateName}
            setTemplates={setTemplates}
            connected={connected}
            recording={recording}
            busy={busy}
            beginNotes={beginNotes}
            result={result}
            resultSection={resultSection}
            selectedStep={selectedStep}
            setEditing={setEditing}
            setEditingNew={setEditingNew}
            moveStep={moveStep}
            updateTemplate={updateTemplate}
            copied={copied}
            copy={copy}
            showEvidence={showEvidence}
            locale={locale}
          />
        )}
      </main>
      {editing && (
        <div className="modal-backdrop" onClick={() => setEditing(null)}>
          <section
            role="dialog"
            aria-modal="true"
            aria-labelledby="step-title"
            className="dialog"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="section-heading">
              <h2 id="step-title">{t.editTab}</h2>
              <button
                className="icon-button"
                aria-label={t.close}
                onClick={() => setEditing(null)}
              >
                <X size={19} />
              </button>
            </div>
            <p>{t.instructionHelp}</p>
            <label>
              {t.tabName}
              <input
                maxLength={100}
                value={editing.name}
                onChange={(event) =>
                  setEditing({ ...editing, name: event.target.value })
                }
              />
            </label>
            <label>
              {t.kind}
              <select
                value={editing.kind}
                onChange={(event) =>
                  setEditing({
                    ...editing,
                    kind: event.target.value as AnalysisStep["kind"],
                  })
                }
              >
                {(["summary", "decisions", "tasks", "analysis"] as const).map(
                  (kind) => (
                    <option key={kind} value={kind}>
                      {t[kind]}
                    </option>
                  ),
                )}
              </select>
            </label>
            <label>
              {t.instruction}
              <textarea
                rows={8}
                maxLength={8000}
                value={editing.prompt}
                onChange={(event) =>
                  setEditing({ ...editing, prompt: event.target.value })
                }
              />
            </label>
            <div className="dialog-actions">
              <button className="secondary" onClick={() => setEditing(null)}>
                {t.cancel}
              </button>
              <button
                className="primary"
                onClick={saveStep}
                disabled={!editing.name.trim() || !editing.prompt.trim()}
              >
                {t.save}
              </button>
            </div>
          </section>
        </div>
      )}
      {templateName && (
        <div className="modal-backdrop">
          <form
            className="dialog small"
            role="dialog"
            aria-modal="true"
            aria-labelledby="template-title"
            onSubmit={(event) => {
              event.preventDefault();
              if (!templateName.name.trim()) return;
              if (templateName.id)
                updateTemplate({ ...template, name: templateName.name.trim() });
              else {
                const next = {
                  ...structuredClone(template),
                  id: uid(),
                  name: templateName.name.trim(),
                  steps: template.steps.map((item) => ({ ...item, id: uid() })),
                };
                setTemplates([...templates, next]);
                setTemplateId(next.id);
                setStepId(next.steps[0].id);
                setResultId("");
              }
              setTemplateName(null);
            }}
          >
            <h2 id="template-title">
              {templateName.id ? t.renameTemplate : t.newTemplate}
            </h2>
            <label>
              {t.template}
              <input
                value={templateName.name}
                maxLength={100}
                onChange={(event) =>
                  setTemplateName({ ...templateName, name: event.target.value })
                }
              />
            </label>
            <div className="dialog-actions">
              <button
                type="button"
                className="secondary"
                onClick={() => setTemplateName(null)}
              >
                {t.cancel}
              </button>
              <button className="primary" disabled={!templateName.name.trim()}>
                {t.save}
              </button>
            </div>
          </form>
        </div>
      )}
      {cloudRequest && (
        <div className="modal-backdrop">
          <section
            className="dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="cloud-title"
          >
            <h2 id="cloud-title">{t.cloudTitle}</h2>
            <p>{t.cloudBody}</p>
            <code className="storage-path">{cloudRequest.endpoint}</code>
            <p>
              {cloudRequest.model} · {meeting?.title}
            </p>
            <div className="dialog-actions">
              <button
                className="secondary"
                onClick={() => setCloudRequest(null)}
              >
                {t.cancel}
              </button>
              <button
                className="primary"
                onClick={() =>
                  void runNotes({ ...cloudRequest, cloudConsent: true })
                }
              >
                {t.approve}
              </button>
            </div>
          </section>
        </div>
      )}
      {source && (
        <div className="modal-backdrop" onClick={() => setSource(null)}>
          <section
            className="dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="source-title"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="section-heading">
              <h2 id="source-title">
                {t.evidence} [{source.source.id}]
              </h2>
              <button
                className="icon-button"
                aria-label={t.close}
                onClick={() => setSource(null)}
              >
                <X size={19} />
              </button>
            </div>
            <p className="muted">
              {trackName(source.source.source, locale)} ·{" "}
              {clock(source.source.startMs)}–{clock(source.source.endMs)}
            </p>
            <p className="source-text">
              {source.source.text
                .split(source.quote)
                .map((part, index, array) => (
                  <span key={index}>
                    {part}
                    {index < array.length - 1 && <mark>{source.quote}</mark>}
                  </span>
                ))}
            </p>
            <p className="muted">{t.sourceTimes}</p>
          </section>
        </div>
      )}
    </div>
  );
}
