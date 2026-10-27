import MeetingLabels from "./views/MeetingLabels";
import SpeakerLabels from "./views/SpeakerLabels";
import SpeakerExperiments from "./views/SpeakerExperiments";
import { SPEAKER_NAME_TEMPLATE } from "./speaker-proposals";
import HistorySearch from "./views/HistorySearch";
import LibraryStatus from "./views/LibraryStatus";
import UpdatePanel from "./views/UpdatePanel";
import RetentionPanel from "./views/RetentionPanel";
import MaintenanceStatus from "./views/MaintenanceStatus";
import NotesView from "./views/NotesView";
import ResumeAnalysisDialog from "./views/ResumeAnalysisDialog";
import Sidebar, { type Page } from "./views/Sidebar";
import MeetingView from "./views/MeetingView";
import MeetingDetail from "./views/MeetingDetail";
import NotesProgressPanel from "./views/NotesProgressPanel";
import TemplateDialog, { type TemplateAction } from "./views/TemplateDialog";
import SourceDialog, { type SourceSelection } from "./views/SourceDialog";
import AgentPanel from "./views/AgentPanel";
import { useTemplateLibrary } from "./useTemplateLibrary";
import { useLocalTextModels } from "./useLocalTextModels";
import { useNotesJob } from "./useNotesJob";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FolderClock, RefreshCw, X } from "lucide-react";
import { desktopApi, errorMessage } from "./api";
import {
  clock,
  defaultMeetingTitle,
  meetingLabel,
  defaultTemplate,
  renderTranscript,
  uid,
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
  PendingAnalysis,
  Theme,
} from "./types";

const PREFS_KEY = "handymeeting.rust.beta.preferences.v1";
function loadPreferences(): {
  locale: Locale;
  theme: Theme;
  persistable: boolean;
} {
  try {
    const value = JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}");
    if (!value || typeof value !== "object" || Array.isArray(value))
      throw new Error("Invalid preferences");
    return {
      locale: value.locale === "en" ? "en" : "de",
      theme: ["dark", "light", "system"].includes(value.theme)
        ? value.theme
        : "system",
      persistable: true,
    };
  } catch {
    return { locale: "de", theme: "system", persistable: false };
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
    [copied, setCopied] = useState<"transcript" | "notes" | null>(null);
  const [fileExport, setFileExport] = useState<{
    id: string;
    file: string;
  } | null>(null);
  const [devices, setDevices] = useState<AudioDevice[]>([]),
    [history, setHistory] = useState<MeetingSummary[]>([]),
    [meeting, setMeeting] = useState<Meeting | null>(null),
    [liveMeeting, setLiveMeeting] = useState<Meeting | null>(null),
    [notesMeetingId, setNotesMeetingId] = useState(""),
    [notesMeetingLoading, setNotesMeetingLoading] = useState(false);
  const [speakerLabels, setSpeakerLabels] = useState<{
    meetingId: string;
    labels: Record<string, string>;
  } | null>(null);
  const onSpeakerLabels = useCallback(
    (meetingId: string, labels: Record<string, string>) =>
      setSpeakerLabels({ meetingId, labels }),
    [],
  );
  const [searchSource, setSearchSource] = useState<{
    meetingId: string;
    file: string | null;
  } | null>(null);
  const [legacyManifest, setLegacyManifest] = useState("");
  const [title, setTitle] = useState(""),
    [modelId, setModelId] = useState(""),
    [systemId, setSystemId] = useState(""),
    [micId, setMicId] = useState(""),
    [micEnabled, setMicEnabled] = useState(false),
    [handyPath, setHandyPath] = useState("");
  const templateStore = useTemplateLibrary(api, connected, locale);
  const templates = useMemo(
    () =>
      templateStore.templates.some((t) => t.id === SPEAKER_NAME_TEMPLATE.id)
        ? templateStore.templates
        : [...templateStore.templates, SPEAKER_NAME_TEMPLATE],
    [templateStore.templates],
  );
  const [templateId, setTemplateId] = useState(""),
    [stepId, setStepId] = useState("");
  const [editing, setEditing] = useState<AnalysisStep | null>(null),
    [editingNew, setEditingNew] = useState(false),
    [inspectingStep, setInspectingStep] = useState(false),
    [templateAction, setTemplateAction] = useState<TemplateAction | null>(null),
    [templateActionError, setTemplateActionError] = useState<string | null>(
      null,
    );
  const [endpoint, setEndpoint] = useState("http://127.0.0.1:1234/v1/"),
    [textModel, setTextModel] = useState(""),
    [manualTextModel, setManualTextModel] = useState(false),
    [apiKey, setApiKey] = useState(""),
    [savedKeyScope, setSavedKeyScope] = useState<string | null>(null),
    [mode, setMode] = useState<"local" | "cloud">("local");
  const credentialScope = JSON.stringify([mode, endpoint.trim()]);
  const useSavedKey = savedKeyScope === credentialScope;
  const setUseSavedKey = useCallback(
    (value: boolean) => setSavedKeyScope(value ? credentialScope : null),
    [credentialScope],
  );
  const [analyses, setAnalyses] = useState<AnalysisResult[]>([]),
    [resultId, setResultId] = useState(""),
    [cloudRequest, setCloudRequest] = useState<NotesRequest | null>(null),
    [source, setSource] = useState<SourceSelection | null>(null);
  const [pendingAnalyses, setPendingAnalyses] = useState<PendingAnalysis[]>([]);
  const [resumeAnalysis, setResumeAnalysis] = useState<PendingAnalysis | null>(
    null,
  );
  const requestSequence = useRef(0),
    selectedRef = useRef<string | null>(null),
    fullMeetingRef = useRef<string | null>(null),
    liveFullRef = useRef<string | null>(null),
    notesSelectionRef = useRef(""),
    notesLoadSequence = useRef(0),
    initialModelSelected = useRef(false),
    initialCatalogSeen = useRef(false),
    lastModelRefresh = useRef(Date.now()),
    modelRefreshInFlight = useRef<Promise<void> | null>(null),
    mounted = useRef(true),
    copyTimer = useRef<ReturnType<typeof setTimeout>>();
  const template =
    templates.find((item) => item.id === templateId) ?? templates[0];
  const templatePending = useRef(templateStore.pending);
  templatePending.current = templateStore.pending;
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
  const onNotesResult = useCallback((next: AnalysisResult) => {
    if (notesSelectionRef.current !== next.meetingId) return;
    setAnalyses((items) => [
      next,
      ...items.filter((item) => item.id !== next.id),
    ]);
    setResultId(next.id);
    setStepId(next.sections[0]?.id ?? "");
    setPendingAnalyses((items) => items.filter((item) => item.id !== next.id));
  }, []);
  const notesJob = useNotesJob(
    api,
    connected,
    app?.notesRunning ?? false,
    onNotesResult,
  );
  const notesBusy = notesJob.busy;
  const localTextModels = useLocalTextModels(
    api,
    endpoint,
    mode,
    connected,
    page === "notes",
  );
  const notesMeeting = meeting?.id === notesMeetingId ? meeting : null;
  const notesMeetingOptions =
    history.some((item) => item.id === meeting?.id) || !meeting
      ? history
      : [meeting, ...history];
  const notesModelReady =
    mode === "cloud" ||
    manualTextModel ||
    (localTextModels.loaded && localTextModels.models.includes(textModel));

  const canStartNotes =
    connected &&
    Boolean(notesMeeting) &&
    notesMeeting?.status === "complete" &&
    !recording &&
    !notesBusy &&
    !busy &&
    !notesMeetingLoading &&
    Boolean(textModel.trim()) &&
    notesModelReady &&
    Boolean(endpoint.trim()) &&
    templateStore.ready &&
    !templateStore.pending;

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.lang = locale;
    if (!preferences.persistable) return;
    try {
      const previous = JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}");
      if (!previous || typeof previous !== "object" || Array.isArray(previous))
        throw new Error("Invalid preferences");
      localStorage.setItem(
        PREFS_KEY,
        JSON.stringify({ ...previous, locale, theme }),
      );
    } catch {
      setError(t.preferencesError);
    }
  }, [locale, theme]);
  useEffect(
    () => () => {
      if (copyTimer.current) clearTimeout(copyTimer.current);
    },
    [],
  );
  const modalOpen = Boolean(
    editing || templateAction || cloudRequest || source || resumeAnalysis,
  );
  useEffect(() => {
    if (!modalOpen) return;
    const previous = document.activeElement as HTMLElement | null;
    const dialog = document.querySelector<HTMLElement>('[role="dialog"]');
    const surfaces = document.querySelectorAll<HTMLElement>(
      ".sidebar, main, .notes-statusbar, .job-status-stack",
    );
    surfaces.forEach((element) => {
      element.inert = true;
    });
    const focusable = () =>
      Array.from(
        dialog?.querySelectorAll<HTMLElement>(
          'button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),summary,[tabindex="0"]',
        ) ?? [],
      ).filter((element) => {
        if (element.closest("[hidden]")) return false;
        let parent = element.parentElement;
        while (parent && parent !== dialog) {
          if (
            parent instanceof HTMLDetailsElement &&
            !parent.open &&
            parent.querySelector(":scope > summary") !== element
          )
            return false;
          parent = parent.parentElement;
        }
        return true;
      });
    const initialTargets = focusable();
    (
      initialTargets.find((element) => element.matches("input,textarea")) ??
      initialTargets[0]
    )?.focus();
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        if (templatePending.current) {
          event.preventDefault();
          return;
        }
        setEditing(null);
        setTemplateAction(null);
        setCloudRequest(null);
        setSource(null);
        setResumeAnalysis(null);
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
  useEffect(() => {
    const updated = () => {
      void refreshHistory().catch((e) => setError(errorMessage(e)));
    };
    window.addEventListener("library-index-updated", updated);
    return () => window.removeEventListener("library-index-updated", updated);
  }, [refreshHistory]);
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
      if (state.activeMeeting) {
        const active = state.activeMeeting;
        const live =
          state.recording ||
          ["recording", "catching-up", "streaming"].includes(active.status);
        if (live) {
          liveFullRef.current = null;
          setLiveMeeting(active);
          if (!selectedRef.current || selectedRef.current === active.id) {
            selectedRef.current = active.id;
            fullMeetingRef.current = null;
            setMeeting(active);
          }
        } else if (
          liveFullRef.current !== active.id ||
          ((!selectedRef.current || selectedRef.current === active.id) &&
            fullMeetingRef.current !== active.id)
        ) {
          const full = await api.getMeeting(active.id);
          if (seq !== requestSequence.current) return;
          liveFullRef.current = full.id;
          setLiveMeeting(full);
          if (!selectedRef.current || selectedRef.current === active.id) {
            selectedRef.current = full.id;
            fullMeetingRef.current = full.id;
            setMeeting(full);
          }
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
  async function openMeeting(
    id: string,
    nextPage: Page = "detail",
    source: string | null = null,
  ) {
    if (nextPage === "notes") {
      await openNotesFor(id);
      return;
    }
    await action(async () => {
      const item = await api.getMeeting(id);
      // A search hit applies only to the meeting it opened; other opens clear it.
      setSearchSource(source ? { meetingId: item.id, file: source } : null);
      selectedRef.current = item.id;
      fullMeetingRef.current = item.id;
      setMeeting(item);
      setAnalyses([]);
      setResultId("");
      setPage(nextPage);
    });
  }
  async function selectNotesMeeting(id: string) {
    const sequence = ++notesLoadSequence.current;
    const isCurrentSelection = () =>
      mounted.current &&
      sequence === notesLoadSequence.current &&
      notesSelectionRef.current === id;
    notesSelectionRef.current = id;
    setNotesMeetingId(id);
    setAnalyses([]);
    setResultId("");
    setPendingAnalyses([]);
    setResumeAnalysis(null);
    if (!id) {
      setNotesMeetingLoading(false);
      return;
    }
    setNotesMeetingLoading(true);
    setError(null);
    try {
      const current = await api.getMeeting(id);
      if (
        !mounted.current ||
        sequence !== notesLoadSequence.current ||
        notesSelectionRef.current !== id
      )
        return;
      if (current.id !== id) throw new Error(t.wrongMeeting);
      selectedRef.current = current.id;
      fullMeetingRef.current = current.id;
      setMeeting(current);
      const failures: string[] = [];
      const reportDiscoveryFailure = (failure: unknown) => {
        if (!isCurrentSelection()) return;
        failures.push(errorMessage(failure));
        setError([...new Set(failures)].join("\n"));
      };
      await Promise.all([
        api
          .listAnalyses(id)
          .then((items) => {
            if (!isCurrentSelection()) return;
            if (items.some((item) => item.meetingId !== id))
              throw new Error(t.wrongMeeting);
            setAnalyses(items);
            setResultId(items[0]?.id ?? "");
          })
          .catch(reportDiscoveryFailure),
        api
          .listUnfinishedAnalyses(id)
          .then((pending) => {
            if (!isCurrentSelection()) return;
            if (pending.some((item) => item.meetingId !== id))
              throw new Error(t.wrongMeeting);
            setPendingAnalyses(pending);
          })
          .catch(reportDiscoveryFailure),
      ]);
    } catch (failure) {
      if (mounted.current && sequence === notesLoadSequence.current)
        setError(errorMessage(failure));
    } finally {
      if (mounted.current && sequence === notesLoadSequence.current)
        setNotesMeetingLoading(false);
    }
  }
  async function openNotesFor(id: string) {
    setPage("notes");
    void refreshHistory().catch((failure) => setError(errorMessage(failure)));
    await selectNotesMeeting(id);
  }
  async function navigate(next: Page) {
    setPage(next);
    if (next === "history") void action(refreshHistory);
    if (next === "notes") {
      const id =
        (page === "detail"
          ? meeting?.id
          : page === "meeting"
            ? liveMeeting?.id
            : notesSelectionRef.current) ||
        meeting?.id ||
        "";
      void refreshHistory().catch((failure) => setError(errorMessage(failure)));
      void selectNotesMeeting(id);
    }
  }
  async function start() {
    await action(async () => {
      const item = await api.startMeeting({
        title: title.trim() || defaultMeetingTitle(locale),
        modelId,
        systemDeviceId: systemId,
        microphoneDeviceId: micEnabled ? micId : null,
      });
      selectedRef.current = item.id;
      fullMeetingRef.current = null;
      liveFullRef.current = null;
      setLiveMeeting(item);
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
        liveFullRef.current = item.id;
        setLiveMeeting(item);
        setMeeting(item);
        await poll();
        await refreshHistory();
      } finally {
        setIsStopping(false);
      }
    });
  }
  async function copyFullTranscript(target: Meeting | null = meeting) {
    if (!target) return;
    const id = target.id;
    await action(async () => {
      const full = await api.getMeeting(id);
      await api.copyText(renderTranscript(full, locale));
      setCopied("transcript");
      if (copyTimer.current) clearTimeout(copyTimer.current);
      copyTimer.current = setTimeout(() => setCopied(null), 2000);
    });
  }
  async function updateTemplate(next: AnalysisTemplate) {
    if (next.id === SPEAKER_NAME_TEMPLATE.id) {
      setError(
        locale === "de"
          ? "Diese integrierte Vorlage ist geschützt. Bitte zuerst duplizieren."
          : "This built-in template is protected. Duplicate it first.",
      );
      return false;
    }
    return templateStore.commit(
      templateStore.templates.map((item) =>
        item.id === next.id ? next : item,
      ),
    );
  }
  async function saveStep() {
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
    if (
      !(await updateTemplate({
        ...template,
        steps: editingNew
          ? [...template.steps, next]
          : template.steps.map((item) => (item.id === next.id ? next : item)),
      }))
    )
      return;
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
    void updateTemplate({ ...template, steps });
  }
  function openTemplateAction(
    kind: TemplateAction["kind"],
    original = template,
  ) {
    setTemplateActionError(null);
    setTemplateAction({
      kind,
      original: structuredClone(original),
      name:
        kind === "new"
          ? ""
          : kind === "duplicate"
            ? original.name + " · " + t.duplicateName
            : original.name,
    });
  }
  async function saveTemplateAction() {
    if (!templateAction) return;
    const { kind, original } = templateAction;
    const name = templateAction.name.trim();
    if (
      kind !== "delete" &&
      templates.some(
        (item) =>
          item.name.trim().toLocaleLowerCase() === name.toLocaleLowerCase() &&
          !(kind === "rename" && item.id === original.id),
      )
    ) {
      setTemplateActionError(t.duplicateTemplateName);
      return;
    }
    let next: AnalysisTemplate | null = null;
    let updated: AnalysisTemplate[];
    if (kind === "delete") {
      if (
        templateStore.templates.length < 2 ||
        original.id === SPEAKER_NAME_TEMPLATE.id
      )
        return;
      updated = templateStore.templates.filter(
        (item) => item.id !== original.id,
      );
    } else if (kind === "rename") {
      const current = templateStore.templates.find(
        (item) => item.id === original.id,
      );
      if (!current) {
        setTemplateActionError(t.templateSaveFailed);
        return;
      }
      next = { ...current, name };
      updated = templateStore.templates.map((item) =>
        item.id === original.id ? next! : item,
      );
    } else {
      const base = kind === "new" ? defaultTemplate(locale) : original;
      next = {
        ...structuredClone(base),
        id: uid(),
        name,
        steps: base.steps.map((item) => ({ ...item, id: uid() })),
      };
      updated = [...templateStore.templates, next];
    }
    if (!(await templateStore.commit(updated))) {
      setTemplateActionError(t.templateSaveFailed);
      return;
    }
    const selected = next ?? updated[0];
    setTemplateId(selected.id);
    setStepId(selected.steps[0].id);
    setResultId("");
    setTemplateAction(null);
  }
  function beginNotes() {
    if (!notesMeeting || !canStartNotes) return;
    const request: NotesRequest = {
      meetingId: notesMeeting.id,
      template: structuredClone(template),
      endpoint: endpoint.trim(),
      model: textModel.trim(),
      mode,
      cloudConsent: false,
      apiKey,
      useSavedKey,
    };
    if (mode === "cloud") setCloudRequest(request);
    else void runNotes(request);
  }
  async function runNotes(request: NotesRequest) {
    setError(null);
    setCloudRequest(null);
    await notesJob.run(request);
    const sequence = notesLoadSequence.current;
    try {
      const pending = await api.listUnfinishedAnalyses(request.meetingId);
      if (
        mounted.current &&
        sequence === notesLoadSequence.current &&
        notesSelectionRef.current === request.meetingId
      ) {
        if (pending.some((item) => item.meetingId !== request.meetingId))
          throw new Error(t.wrongMeeting);
        setPendingAnalyses(pending);
      }
    } catch (failure) {
      if (mounted.current && notesSelectionRef.current === request.meetingId)
        setError(errorMessage(failure));
    }
  }
  function resumeNotes() {
    const previous = notesJob.lastRequest,
      runId = notesJob.progress?.runId;
    if (!previous || !runId || previous.meetingId !== notesMeetingId) return;
    const request = { ...previous, resumeRunId: runId, cloudConsent: false };
    if (request.mode === "cloud") setCloudRequest(request);
    else void runNotes(request);
  }
  async function copyNotesMarkdown() {
    if (!result || !notesMeeting || result.meetingId !== notesMeeting.id) {
      setError(t.wrongMeeting);
      return;
    }
    await action(async () => {
      const markdown = await api.notesMarkdown(result.meetingId, result.id);
      await api.copyText(markdown);
      setCopied("notes");
      if (copyTimer.current) clearTimeout(copyTimer.current);
      copyTimer.current = setTimeout(() => setCopied(null), 2000);
    });
  }
  function showEvidence(evidence: Evidence) {
    const original = result?.sources.find(
      (item) => item.id === evidence.sourceId,
    );
    if (!original || !original.text.includes(evidence.quote)) {
      setError(t.sourceMissing);
      return;
    }
    setSource({
      sources: result!.sources,
      index: result!.sources.indexOf(original),
      quote: evidence.quote,
    });
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
  const shownMeeting = page === "meeting" ? liveMeeting : meeting;
  const orderedParts = shownMeeting
    ? [...shownMeeting.parts]
        .sort(
          (a, b) =>
            a.startSample - b.startSample || a.source.localeCompare(b.source),
        )
        .filter((item) => (item.text ?? "").trim())
    : [];
  const recentParts =
    shownMeeting?.status === "complete" && !shownMeeting.livePreviewLimited
      ? orderedParts
      : orderedParts.slice(-40);
  const savedByTrack = (track: "system" | "microphone") =>
    shownMeeting?.savedAudioMs?.[track] ??
    Math.max(
      0,
      ...(shownMeeting?.parts ?? [])
        .filter((item) => item.source === track)
        .map(
          (item) =>
            (item.startSample +
              (item.confirmedSampleCount ?? item.sampleCount)) /
            16,
        ),
    );
  const backlog =
    shownMeeting?.pendingAudioMs ??
    (shownMeeting?.livePreviewLimited
      ? null
      : Math.max(
          0,
          ...["system", "microphone"].map(
            (track) =>
              (shownMeeting?.parts ?? [])
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
            <h1>{page === "detail" ? t.meetingDetail : t[page]}</h1>
            <p>
              {page === "meeting"
                ? t.preserve
                : page === "notes"
                  ? t.draft
                  : page === "history" || page === "detail"
                    ? t.legacyHelp
                    : t.compatibility}
            </p>
          </div>
          {recording && (
            <button
              className="recording-link"
              onClick={() => {
                setPage("meeting");
              }}
            >
              <span className="recording-dot" />
              {t.recording}
            </button>
          )}
        </header>
        {(page === "meeting"
          ? liveMeeting
          : page === "detail"
            ? meeting
            : null) && (
          <MeetingLabels
            api={api}
            meeting={(page === "meeting" ? liveMeeting : meeting)!}
            locale={locale}
            onChanged={(meta) => {
              const target = (page === "meeting" ? liveMeeting : meeting)!;
              setMeeting((old) =>
                old?.id === target.id
                  ? {
                      ...old,
                      title: meta.title ?? old.title,
                      userMetadata: meta,
                    }
                  : old,
              );
              setLiveMeeting((old) =>
                old?.id === target.id
                  ? {
                      ...old,
                      title: meta.title ?? old.title,
                      userMetadata: meta,
                    }
                  : old,
              );
              void refreshHistory().catch((e) => setError(errorMessage(e)));
            }}
          />
        )}
        {page === "detail" && meeting && (
          <SpeakerLabels
            key={meeting.id}
            api={api}
            meeting={meeting}
            locale={locale}
            proposeDisabled={notesBusy}
            onLabels={onSpeakerLabels}
            onPropose={() => {
              if (notesBusy) return;
              void action(async () => {
                // Name suggestions run locally only; local settings stay as they are.
                if (mode === "cloud") {
                  setMode("local");
                  setApiKey("");
                  setSavedKeyScope(null);
                  setEndpoint("http://127.0.0.1:1234/v1/");
                  setTextModel("");
                }
                await openNotesFor(meeting.id);
                if (notesSelectionRef.current === meeting.id) {
                  setTemplateId(SPEAKER_NAME_TEMPLATE.id);
                  setResultId("");
                }
              });
            }}
          />
        )}
        {(page === "detail"
          ? meeting
          : page === "meeting"
            ? liveMeeting
            : null) && (
          <SpeakerExperiments
            key={(page === "detail" ? meeting : liveMeeting)!.id}
            api={api}
            locale={locale}
            meeting={page === "detail" ? meeting : liveMeeting}
            models={app?.handy?.models ?? []}
            recording={Boolean(
              app?.recording &&
                (page === "detail" ? meeting : liveMeeting)?.id ===
                  liveMeeting?.id,
            )}
          />
        )}
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
        <AgentPanel
          agent={app?.agent}
          blocked={
            recording || isStopping || catchingUp || Boolean(app?.transitioning)
          }
          devicesReady={Boolean(
            modelId &&
              models.some((item) => item.id === modelId) &&
              devices.some(
                (item) => item.id === systemId && item.source === "system",
              ),
          )}
          microphoneReady={devices.some(
            (item) => item.id === micId && item.source === "microphone",
          )}
          busy={busy}
          t={t}
          onDecide={(approve) => {
            const pending = app?.agent?.pending;
            if (!pending) return;
            void action(async () => {
              await api.agentDecideRecording(
                pending.requestId,
                approve,
                approve
                  ? {
                      title: pending.title,
                      modelId,
                      systemDeviceId: systemId,
                      microphoneDeviceId: pending.includeMicrophone
                        ? micId
                        : null,
                    }
                  : undefined,
              );
              await poll();
              await refreshHistory();
            });
          }}
          onRevoke={() =>
            void action(async () => {
              await api.agentRevoke();
              await poll();
            })
          }
        />
        {page === "meeting" && (
          <MeetingView
            t={t}
            x={x}
            app={app}
            meeting={liveMeeting}
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
            defaultTitle={defaultMeetingTitle(locale)}
            onAnalyse={() => {
              if (liveMeeting) void openNotesFor(liveMeeting.id);
            }}
            onOpenFolder={() =>
              void action(async () => {
                if (liveMeeting) await api.openMeetingFolder(liveMeeting.id);
              })
            }
            onExport={() =>
              void action(async () => {
                if (liveMeeting) {
                  const id = liveMeeting.id;
                  const file = await api.exportTranscript(id, locale);
                  setFileExport({ id, file });
                }
              })
            }
            savedFile={
              fileExport?.id === liveMeeting?.id
                ? (fileExport?.file ?? null)
                : null
            }
            modelId={modelId}
            systemId={systemId}
            micId={micId}
            micEnabled={micEnabled}
            models={models}
            devices={devices}
            copied={copied === "transcript"}
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
            copyFullTranscript={() => copyFullTranscript(liveMeeting)}
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
        {page === "detail" && meeting && (
          <MeetingDetail
            meeting={meeting}
            selectedFile={
              searchSource?.meetingId === meeting.id ? searchSource.file : null
            }
            locale={locale}
            t={t}
            busy={busy}
            copied={copied === "transcript"}
            onBack={() => void navigate("history")}
            onCopy={() => void copyFullTranscript(meeting)}
            onAnalyse={() => void openNotesFor(meeting.id)}
            onOpenFolder={() =>
              void action(() => api.openMeetingFolder(meeting.id))
            }
            onExport={() =>
              void action(async () => {
                const id = meeting.id;
                const file = await api.exportTranscript(id, locale);
                setFileExport({ id, file });
              })
            }
            savedFile={fileExport?.id === meeting.id ? fileExport.file : null}
            speakers={
              speakerLabels?.meetingId === meeting.id
                ? speakerLabels.labels
                : undefined
            }
          />
        )}
        {page === "history" && api.searchMeetings && (
          <HistorySearch
            api={api}
            locale={locale}
            imported={history.filter((item) => item.id.startsWith("legacy-"))}
            onOpen={(id, file) => {
              void openMeeting(id, "detail", file);
            }}
          />
        )}
        {page === "history" && !api.searchMeetings && (
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
            {!preferences.persistable && (
              <div className="notice warning">{t.preferencesError}</div>
            )}
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
                  {t.version}:{" "}
                  {app?.handy?.version ??
                    (app?.handy?.executable
                      ? t.handyVersionMissing
                      : t.notFound)}
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
          <UpdatePanel
            api={api}
            locale={locale}
            blocked={Boolean(
              app?.recording ||
                app?.transitioning ||
                app?.notesRunning ||
                liveMeeting?.status === "catching-up",
            )}
          />
        )}
        {page === "settings" && api.rebuildSearchIndex && (
          <section className="legacy-panel">
            <h2>
              {locale === "de" ? "Lokaler Suchindex" : "Local search index"}
            </h2>
            <p className="muted">
              {locale === "de"
                ? "Der Suchindex beschleunigt große Historien. Er wird lokal aus deinen Sitzungen aufgebaut; Originale werden nicht geändert."
                : "The local index speeds up large histories. Rebuilding never changes original sessions."}
            </p>
            <button
              className="secondary"
              onClick={() =>
                void api
                  .rebuildSearchIndex?.()
                  .catch((e) => setError(errorMessage(e)))
              }
            >
              <RefreshCw size={15} />
              {locale === "de"
                ? "Suchindex aktualisieren"
                : "Update search index"}
            </button>
          </section>
        )}
        {page === "settings" && (
          <RetentionPanel
            api={api}
            locale={locale}
            models={app?.handy?.models ?? []}
          />
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
                  setPage("detail");
                  await refreshHistory();
                })
              }
            >
              {x.importSession}
            </button>
          </section>
        )}
        {page === "notes" && pendingAnalyses.length > 0 && (
          <section
            className="pending-analyses"
            aria-label={t.unfinishedAnalyses}
          >
            <h2>{t.unfinishedAnalyses}</h2>
            <p>{t.resumeHelp}</p>
            <div className="pending-analyses-list">
              {pendingAnalyses.map((run) => (
                <div className="pending-analysis-row" key={run.id}>
                  <div>
                    <strong>{run.template.name}</strong> ·{" "}
                    {new Date(run.createdAt).toLocaleString(
                      locale === "de" ? "de-DE" : "en-GB",
                    )}
                    <p>
                      {run.model} · {run.confirmedTasks}/{run.totalTasks}{" "}
                      {t.confirmedTasks}
                    </p>
                  </div>
                  <button
                    className="secondary"
                    disabled={
                      !connected ||
                      notesBusy ||
                      recording ||
                      busy ||
                      notesMeetingLoading
                    }
                    onClick={() => setResumeAnalysis(run)}
                  >
                    {t.resumeAnalysis}
                  </button>
                </div>
              ))}
            </div>
          </section>
        )}
        {page === "notes" && (
          <NotesView
            t={t}
            api={api}
            useSavedKey={useSavedKey}
            setUseSavedKey={setUseSavedKey}
            meeting={notesMeeting}
            meetingId={notesMeetingId}
            meetings={notesMeetingOptions}
            meetingLoading={notesMeetingLoading}
            selectMeeting={(id) => void selectNotesMeeting(id)}
            localModels={localTextModels.models}
            modelsLoading={localTextModels.loading}
            modelsError={localTextModels.error}
            modelsLoaded={localTextModels.loaded}
            refreshModels={() => void localTextModels.refresh()}
            manualModel={manualTextModel}
            setManualModel={setManualTextModel}
            modelReady={notesModelReady}
            libraryInvalid={!templateStore.ready}
            libraryPending={templateStore.pending}
            libraryError={templateStore.error ?? templateStore.migrationError}
            browserInvalid={templateStore.browserInvalid}
            refreshTemplates={() => void templateStore.refresh()}
            importBrowser={
              templateStore.hasBrowserTemplates
                ? () => void templateStore.importBrowser()
                : null
            }
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
            openTemplateAction={openTemplateAction}
            connected={connected}
            recording={recording}
            busy={busy || notesMeetingLoading}
            beginNotes={beginNotes}
            result={result}
            resultSection={resultSection}
            selectedStep={selectedStep}
            setEditing={(value) => {
              setInspectingStep(false);
              setEditing(value);
            }}
            inspectStep={(value) => {
              setInspectingStep(true);
              setEditing(value);
            }}
            setEditingNew={setEditingNew}
            moveStep={moveStep}
            updateTemplate={updateTemplate}
            copied={copied === "notes"}
            copyResults={copyNotesMarkdown}
            showEvidence={showEvidence}
            locale={locale}
          />
        )}
      </main>
      <div className="job-status-stack">
        <MaintenanceStatus api={api} locale={locale} />
        <SpeakerExperiments api={api} locale={locale} footer />
        <UpdatePanel api={api} locale={locale} footer />
        <LibraryStatus api={api} connected={connected} locale={locale} />
        <NotesProgressPanel
          progress={notesJob.progress}
          elapsedMs={notesJob.elapsedMs}
          cancelPending={notesJob.cancelPending}
          feedback={notesJob.feedback}
          pollError={notesJob.pollError}
          awaitingProgress={Boolean(app?.notesRunning) && !notesJob.progress}
          meetingTitle={(() => {
            const id =
              notesJob.progress?.meetingId ?? notesJob.feedback?.meetingId;
            const value = [meeting, liveMeeting, ...history].find(
              (item) => item?.id === id,
            );
            return value
              ? meetingLabel(value, locale)
              : [t.notesTask, id].filter(Boolean).join(" · ");
          })()}
          setup={
            page === "notes"
              ? {
                  loading: notesMeetingLoading,
                  message: notesMeetingLoading
                    ? t.busy
                    : !notesMeeting
                      ? t.noMeeting
                      : notesMeeting.status !== "complete" || recording
                        ? t.pendingNotes
                        : !notesModelReady || !textModel.trim()
                          ? t.analysisChooseModel
                          : !connected
                            ? t.disconnected
                            : !templateStore.ready || templateStore.pending
                              ? t.templateLoadFailed
                              : t.analysisReady,
                  startDisabled: !canStartNotes,
                  onStart: beginNotes,
                }
              : undefined
          }
          t={t}
          onCancel={() => void notesJob.cancel()}
          onOpen={() => {
            const id =
              notesJob.progress?.meetingId ?? notesJob.feedback?.meetingId;
            if (id) void openNotesFor(id);
          }}
          onDismiss={notesJob.dismiss}
          onResume={
            notesJob.progress?.runId &&
            ["failed", "cancelled"].includes(notesJob.progress.status) &&
            notesJob.lastRequest?.meetingId === notesMeetingId
              ? resumeNotes
              : null
          }
        />
      </div>
      {resumeAnalysis && (
        <ResumeAnalysisDialog
          key={resumeAnalysis.id}
          run={resumeAnalysis}
          api={api}
          t={t}
          blocked={
            !connected || notesBusy || recording || busy || notesMeetingLoading
          }
          onClose={() => setResumeAnalysis(null)}
          onResume={(request) => {
            setResumeAnalysis(null);
            void runNotes(request);
          }}
        />
      )}
      {editing && (
        <div
          className="modal-backdrop"
          onClick={() => {
            if (!templateStore.pending) setEditing(null);
          }}
        >
          <section
            role="dialog"
            aria-modal="true"
            aria-labelledby="step-title"
            className="dialog"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="section-heading">
              <h2 id="step-title">
                {inspectingStep ? t.snapshotInstructions : t.editTab}
              </h2>
              <button
                className="icon-button"
                disabled={templateStore.pending}
                aria-label={t.close}
                onClick={() => setEditing(null)}
              >
                <X size={19} />
              </button>
            </div>
            <p>{t.instructionHelp}</p>
            <p className="muted">{t.templateLimit}</p>
            <label>
              {t.tabName}
              <input
                maxLength={100}
                value={editing.name}
                readOnly={inspectingStep || templateStore.pending}
                onChange={(event) =>
                  setEditing({ ...editing, name: event.target.value })
                }
              />
            </label>
            <label>
              {t.kind}
              <select
                value={editing.kind}
                disabled={inspectingStep || templateStore.pending}
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
            <p className="muted">{t.stepKindHelp}</p>
            <label>
              {t.instruction}
              <textarea
                rows={8}
                maxLength={8000}
                value={editing.prompt}
                readOnly={inspectingStep || templateStore.pending}
                onChange={(event) =>
                  setEditing({ ...editing, prompt: event.target.value })
                }
              />
            </label>
            <div className="dialog-actions">
              {templateStore.error && (
                <div>
                  <p className="inline-error" role="alert">
                    {t.templateSaveFailed}
                  </p>
                  <button
                    className="secondary"
                    disabled={templateStore.pending}
                    onClick={() => void templateStore.refresh()}
                  >
                    {t.refreshTemplates}
                  </button>
                </div>
              )}
              <button
                className="secondary"
                disabled={templateStore.pending}
                onClick={() => setEditing(null)}
              >
                {inspectingStep ? t.close : t.cancel}
              </button>
              {!inspectingStep && (
                <button
                  className="primary"
                  onClick={() => void saveStep()}
                  disabled={
                    templateStore.pending ||
                    !editing.name.trim() ||
                    !editing.prompt.trim()
                  }
                >
                  {templateStore.pending ? t.busy : t.save}
                </button>
              )}
            </div>
          </section>
        </div>
      )}
      {templateAction && (
        <TemplateDialog
          action={templateAction}
          pending={templateStore.pending}
          error={templateActionError}
          t={t}
          onChange={(name) => setTemplateAction({ ...templateAction, name })}
          onSave={() => void saveTemplateAction()}
          onClose={() => setTemplateAction(null)}
          onRefresh={() => void templateStore.refresh()}
        />
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
        <SourceDialog
          selection={source}
          locale={locale}
          t={t}
          onClose={() => setSource(null)}
        />
      )}
    </div>
  );
}
