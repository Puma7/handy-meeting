import { act, render, screen, waitFor, within } from "@testing-library/react";
import { StrictMode } from "react";
import App from "./App";
import { testServices } from "./test-services";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import LiveNotesPanel, {
  endpointAllowed,
  liveNotesText,
  type LiveConnection,
} from "./views/LiveNotesPanel";
import type {
  AppState,
  DesktopApi,
  Meeting,
  LiveItem,
  LiveNotesStatus,
  LiveView,
  LocalModelSetupInfo,
} from "./types";

vi.setConfig({ testTimeout: 15_000 });
// Synthetic IPC only: no text model, network, microphone or application storage is used.

function item(text: string, extra: Partial<LiveItem> = {}): LiveItem {
  return {
    text,
    owner: null,
    due: null,
    window: 1,
    tracks: ["system"],
    evidence: [
      {
        sourceId: "C60",
        quote: `Zitat zu ${text}`,
        track: "system",
        startMs: 60_000,
        endMs: 90_000,
      },
    ],
    ...extra,
  };
}

const view: LiveView = {
  runId: "a".repeat(32),
  model: "gemma4:e4b",
  mode: "local",
  endpoint: "http://127.0.0.1:11434/v1/",
  language: "de",
  promptVersion: "handymeeting-live-notes-1",
  createdAt: "2026-10-28T10:00:00Z",
  updatedAt: "2026-10-28T10:30:00Z",
  windows: 2,
  summarizedSections: 6,
  overview: [item("Budget Q3 besprochen")],
  overviewWindow: 2,
  points: [item("Erster Punkt"), item("Zweiter Punkt", { window: 2 })],
  decisions: [item("Günstigerer Anbieter gewählt")],
  tasks: [
    item("Angebot verschicken", {
      owner: "Anna",
      due: "Freitag",
      tracks: ["microphone"],
    }),
  ],
  questions: [],
  rejected: 2,
  damagedRuns: 0,
};

function status(next: Partial<LiveNotesStatus> = {}): LiveNotesStatus {
  return {
    meetingId: "m".repeat(32),
    status: "off",
    running: false,
    error: null,
    pendingChars: 0,
    intervalMs: 180_000,
    nextUpdateInMs: null,
    lastTrigger: null,
    view: null,
    ...next,
  };
}

const local: LiveConnection = {
  endpoint: "http://127.0.0.1:11434/v1/",
  model: "gemma4:e4b",
  mode: "local",
  apiKey: "",
  useSavedKey: false,
};

function setup(
  api: Partial<DesktopApi>,
  props: Partial<Parameters<typeof LiveNotesPanel>[0]> = {},
) {
  const setPreferences = vi.fn();
  const onUseLocalModel = vi.fn();
  render(
    <LiveNotesPanel
      api={
        { copyText: vi.fn().mockResolvedValue(undefined), ...api } as DesktopApi
      }
      locale="de"
      connected
      meetingId={"m".repeat(32)}
      meetingTitle="Synthetisches Meeting"
      meetingActive
      connection={local}
      preferences={{ auto: false, intervalMinutes: 3 }}
      setPreferences={setPreferences}
      onUseLocalModel={onUseLocalModel}
      onOpenTextModelSettings={vi.fn()}
      focus={false}
      setFocus={vi.fn()}
      {...props}
    />,
  );
  return { setPreferences, onUseLocalModel };
}

it("starts a local live summary with the chosen model, interval and language", async () => {
  const start = vi
    .fn()
    .mockResolvedValue(status({ status: "waiting", running: true }));
  setup({
    getLiveNotes: vi.fn().mockResolvedValue(status()),
    startLiveNotes: start,
  });
  await userEvent.click(
    await screen.findByRole("button", { name: "Live-Zusammenfassung starten" }),
  );
  expect(start).toHaveBeenCalledWith({
    meetingId: "m".repeat(32),
    endpoint: "http://127.0.0.1:11434/v1/",
    model: "gemma4:e4b",
    mode: "local",
    cloudConsent: false,
    apiKey: "",
    useSavedKey: false,
    intervalMs: 180_000,
    language: "de",
  });
});

it("never sends cloud text without the per-start release and never auto-starts cloud", async () => {
  // The provider refuses, so the panel stays on the start screen afterwards.
  const start = vi.fn().mockRejectedValue("HTTP 401");
  setup(
    {
      getLiveNotes: vi.fn().mockResolvedValue(status()),
      startLiveNotes: start,
    },
    {
      connection: {
        ...local,
        mode: "cloud",
        endpoint: "https://openrouter.ai/api/v1/",
        model: "cloud-model",
      },
      preferences: { auto: true, intervalMinutes: 3 },
    },
  );
  const button = await screen.findByRole("button", {
    name: "Live-Zusammenfassung starten",
  });
  await waitFor(() =>
    expect((button as HTMLButtonElement).disabled).toBe(true),
  );
  expect(start).not.toHaveBeenCalled();
  await userEvent.click(
    screen.getByRole("checkbox", { name: /an https:\/\/openrouter.ai/ }),
  );
  await userEvent.click(button);
  expect(start).toHaveBeenCalledWith(
    expect.objectContaining({ mode: "cloud", cloudConsent: true }),
  );
  expect(await screen.findByRole("alert")).toBeTruthy();
  // The release covered that start only.
  expect(
    (
      screen.getByRole("checkbox", {
        name: /an https:\/\/openrouter.ai/,
      }) as HTMLInputElement
    ).checked,
  ).toBe(false);
});

it("auto-starts once for a local model while the meeting is recording", async () => {
  const start = vi
    .fn()
    .mockResolvedValue(status({ status: "waiting", running: true }));
  setup(
    {
      getLiveNotes: vi.fn().mockResolvedValue(status()),
      startLiveNotes: start,
    },
    { preferences: { auto: true, intervalMinutes: 5 } },
  );
  await waitFor(() => expect(start).toHaveBeenCalledOnce());
  expect(start.mock.calls[0][0].intervalMs).toBe(300_000);
});

it("retries an automatic start while the previous meeting's summary is still finishing", async () => {
  // Codex review 5686078293: a refused first attempt used up the automatic start for good.
  vi.useFakeTimers({ shouldAdvanceTime: true });
  try {
    const start = vi
      .fn()
      .mockRejectedValueOnce(
        "Eine Live-Zusammenfassung läuft bereits. Bitte sie zuerst beenden.",
      )
      .mockResolvedValue(status({ status: "waiting", running: true }));
    setup(
      {
        getLiveNotes: vi.fn().mockResolvedValue(status()),
        startLiveNotes: start,
      },
      { preferences: { auto: true, intervalMinutes: 3 } },
    );
    await waitFor(() => expect(start).toHaveBeenCalledTimes(1));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    await waitFor(() => expect(start).toHaveBeenCalledTimes(2));
  } finally {
    vi.useRealTimers();
  }
});

it("does not auto-start after an explicit stop or without a model", async () => {
  const start = vi.fn();
  setup(
    {
      getLiveNotes: vi.fn().mockResolvedValue(status({ status: "stopped" })),
      startLiveNotes: start,
    },
    { preferences: { auto: true, intervalMinutes: 3 } },
  );
  await screen.findByText("Angehalten");
  expect(start).not.toHaveBeenCalled();
});

it("shows summary, decisions, tasks with owner and track, evidence and the discarded count", async () => {
  setup({
    getLiveNotes: vi.fn().mockResolvedValue(
      status({
        status: "waiting",
        running: true,
        view,
        pendingChars: 400,
        nextUpdateInMs: 90_000,
      }),
    ),
    startLiveNotes: vi.fn(),
    refreshLiveNotes: vi.fn().mockResolvedValue(true),
  });
  expect(await screen.findByText("Budget Q3 besprochen")).toBeTruthy();
  expect(screen.getByText("Günstigerer Anbieter gewählt")).toBeTruthy();
  expect(screen.getByText("Anna · Freitag")).toBeTruthy();
  expect(screen.getByText(/01:00 · Mikrofon/)).toBeTruthy();
  // Codex review 5683390412: the on-screen evidence names its source like the copy text.
  expect(
    screen.getAllByText(/\[C60\] 01:00 · Computerton/).length,
  ).toBeGreaterThan(0);
  // Codex review 5686078293: a later answer is not reconciled, so the list must not claim
  // that its questions are still open.
  expect(screen.getByText("Keine Fragen aufgeworfen.")).toBeTruthy();
  expect(screen.queryByText(/Offene Fragen/)).toBeNull();
  expect(screen.getByText(/nächste Aktualisierung in ca. 2 Min./)).toBeTruthy();
  expect(
    screen.getByText(/2 KI-Aussage\(n\) ohne gültigen Beleg wurden verworfen/),
  ).toBeTruthy();
  expect(screen.getAllByText("Zitat zu Budget Q3 besprochen")[0]).toBeTruthy();
  expect(
    (
      screen.getByRole("button", {
        name: "Jetzt aktualisieren",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(false);
});

it("copies a plain text summary for reading out", async () => {
  const copy = vi.fn().mockResolvedValue(undefined);
  setup({
    getLiveNotes: vi.fn().mockResolvedValue(status({ view })),
    startLiveNotes: vi.fn(),
    copyText: copy,
  });
  await userEvent.click(
    await screen.findByRole("button", { name: "Zusammenfassung kopieren" }),
  );
  const text = copy.mock.calls[0][0] as string;
  expect(text).toContain("Bisheriger Verlauf\n- Budget Q3 besprochen");
  expect(text).toContain("Aufgaben\n- Angebot verschicken (Anna, Freitag)");
  expect(text).not.toContain("Aufgeworfene Fragen");
  expect(text).toContain("KI-Entwurf");
  expect(liveNotesText(view, "T", "en")).toContain("Summary so far");
});

it("recommends a model from memory, applies a running Ollama model and downloads a missing one", async () => {
  const setupInfo: LocalModelSetupInfo = {
    totalMemoryBytes: 16 * 1024 ** 3,
    recommendation: {
      tier: "standard",
      model: "gemma4:e4b",
      lmStudioSearch: "Gemma 4 E4B",
    },
    providers: [
      {
        id: "ollama",
        name: "Ollama",
        endpoint: "http://127.0.0.1:11434/v1/",
        reachable: true,
        models: ["llama3.2:latest"],
        recommendedInstalled: false,
      },
      {
        id: "lmstudio",
        name: "LM Studio",
        endpoint: "http://127.0.0.1:1234/v1/",
        reachable: false,
        models: [],
        recommendedInstalled: false,
      },
    ],
  };
  const pull = vi.fn().mockResolvedValue({
    model: "gemma4:e4b",
    status: "running",
    detail: "pulling manifest",
    completed: 0,
    total: 0,
    error: null,
  });
  const { onUseLocalModel } = setup(
    {
      getLiveNotes: vi.fn().mockResolvedValue(status()),
      startLiveNotes: vi.fn(),
      localModelSetup: vi.fn().mockResolvedValue(setupInfo),
      pullOllamaModel: pull,
      getModelPullProgress: vi.fn().mockResolvedValue(null),
    },
    { connection: { ...local, model: "" } },
  );
  expect(
    (
      (await screen.findByRole("button", {
        name: "Live-Zusammenfassung starten",
      })) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await userEvent.click(
    screen.getByRole("button", { name: "Lokales Modell einrichten" }),
  );
  expect(
    await screen.findByText(/Arbeitsspeicher: 16 GB · Empfehlung: gemma4:e4b/),
  ).toBeTruthy();
  expect(
    screen.getByText(
      /„Gemma 4 E4B“ herunterladen und unter „Developer“ den lokalen Server starten/,
    ),
  ).toBeTruthy();
  await userEvent.click(
    screen.getByRole("button", { name: "Verwenden: llama3.2:latest" }),
  );
  expect(onUseLocalModel).toHaveBeenCalledWith(
    "http://127.0.0.1:11434/v1/",
    "llama3.2:latest",
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Lokales Modell einrichten" }),
  );
  await userEvent.click(
    await screen.findByRole("button", { name: "gemma4:e4b in Ollama laden" }),
  );
  expect(pull).toHaveBeenCalledWith("gemma4:e4b");
  expect(await screen.findByText(/Lädt gemma4:e4b/)).toBeTruthy();
});

it("warns instead of suggesting a local model on too little memory", async () => {
  setup(
    {
      getLiveNotes: vi.fn().mockResolvedValue(status()),
      startLiveNotes: vi.fn(),
      localModelSetup: vi.fn().mockResolvedValue({
        totalMemoryBytes: 4 * 1024 ** 3,
        recommendation: { tier: "none", model: null, lmStudioSearch: null },
        providers: [],
      }),
    },
    { connection: { ...local, model: "" } },
  );
  await userEvent.click(
    await screen.findByRole("button", { name: "Lokales Modell einrichten" }),
  );
  expect(
    await screen.findByText(/Für eine lokale Live-Zusammenfassung zu wenig/),
  ).toBeTruthy();
});

const ollamaReady: LocalModelSetupInfo = {
  totalMemoryBytes: 16 * 1024 ** 3,
  recommendation: {
    tier: "standard",
    model: "gemma4:e4b",
    lmStudioSearch: "Gemma 4 E4B",
  },
  providers: [
    {
      id: "ollama",
      name: "Ollama",
      endpoint: "http://127.0.0.1:11434/v1/",
      reachable: true,
      models: ["gemma4:e4b"],
      recommendedInstalled: true,
    },
  ],
};

it("keeps polling, starting and model discovery working under StrictMode", async () => {
  const start = vi
    .fn()
    .mockResolvedValue(status({ status: "waiting", running: true }));
  render(
    <StrictMode>
      <LiveNotesPanel
        api={
          {
            copyText: vi.fn(),
            getLiveNotes: vi.fn().mockResolvedValue(status()),
            startLiveNotes: start,
            localModelSetup: vi.fn().mockResolvedValue(ollamaReady),
          } as unknown as DesktopApi
        }
        locale="de"
        connected
        meetingId={"m".repeat(32)}
        meetingTitle="Synthetisches Meeting"
        meetingActive
        connection={local}
        preferences={{ auto: true, intervalMinutes: 3 }}
        setPreferences={vi.fn()}
        onUseLocalModel={vi.fn()}
        onOpenTextModelSettings={vi.fn()}
        focus={false}
        setFocus={vi.fn()}
      />
    </StrictMode>,
  );
  // The automatic start needs the polled "off" state, which only arrives while mounted.
  await waitFor(() => expect(start).toHaveBeenCalled());
  expect(await screen.findByText(/Läuft/)).toBeTruthy();
  await userEvent.click(
    screen.getByRole("button", { name: "Lokales Modell einrichten" }),
  );
  expect(
    await screen.findByRole("button", { name: "Verwenden: gemma4:e4b" }),
  ).toBeTruthy();
});

it("copies every statement with its source identifier, time, track and verbatim quote", () => {
  const text = liveNotesText(view, "T", "de");
  expect(text).toContain(
    "- Budget Q3 besprochen\n  [C60 · 01:00 · Computerton] „Zitat zu Budget Q3 besprochen“",
  );
  expect(text).toContain(
    "- Angebot verschicken (Anna, Freitag)\n  [C60 · 01:00 · Computerton] „Zitat zu Angebot verschicken“",
  );
});

it("shows the running route, not settings that were changed for the next start", async () => {
  setup(
    {
      getLiveNotes: vi.fn().mockResolvedValue(
        status({
          status: "waiting",
          running: true,
          view: {
            ...view,
            mode: "cloud",
            model: "cloud-model",
            endpoint: "https://openrouter.ai/api/v1/",
          },
        }),
      ),
      startLiveNotes: vi.fn(),
    },
    // The settings now point at a local model, but the cloud run is still active.
    { connection: local },
  );
  expect(await screen.findByText("Cloud")).toBeTruthy();
  expect(screen.getByText(/cloud-model/)).toBeTruthy();
  expect(
    screen.getByText(/Geänderte Einstellungen gelten ab dem nächsten Start/),
  ).toBeTruthy();
  expect(screen.queryByText("Lokal")).toBeNull();
});

it("never sends a key typed for another route to a local model chosen from the presets", async () => {
  const meeting: Meeting = {
    id: "c".repeat(32),
    title: "Synthetic live",
    startedAt: "2026-10-28T10:00:00Z",
    durationMs: 5000,
    status: "recording",
    modelId: "synthetic",
    warnings: [],
    parts: [],
  };
  const state: AppState = {
    version: "synthetic",
    handy: null,
    recording: true,
    activeMeeting: meeting,
    captureControl: {
      paused: false,
      microphoneMuted: false,
      hasMicrophone: true,
    },
  };
  const start = vi
    .fn()
    .mockResolvedValue(status({ status: "waiting", running: true }));
  const api = {
    ...testServices(),
    getAppState: vi.fn(async () => structuredClone(state)),
    listDevices: vi.fn().mockResolvedValue([]),
    listMeetings: vi.fn().mockResolvedValue([]),
    getMeeting: vi.fn().mockResolvedValue(meeting),
    listAnalyses: vi.fn().mockResolvedValue({ analyses: [], skipped: [] }),
    getNotesProgress: vi.fn().mockResolvedValue(null),
    listTextModels: vi.fn().mockResolvedValue([]),
    refreshHandy: vi.fn().mockResolvedValue({
      executable: "native-choice",
      version: "synthetic",
      models: [],
    }),
    getLiveNotes: vi.fn().mockResolvedValue(status({ meetingId: meeting.id })),
    startLiveNotes: start,
    localModelSetup: vi.fn().mockResolvedValue(ollamaReady),
  } as unknown as DesktopApi;
  render(<App api={api} />);
  await screen.findByRole("button", { name: "Live-Zusammenfassung starten" });
  // A cloud key is typed on the analysis page.
  await userEvent.click(
    screen.getByRole("button", { name: "Nachbearbeitung" }),
  );
  await userEvent.selectOptions(
    await screen.findByLabelText("Verarbeitung"),
    "cloud",
  );
  await userEvent.type(
    screen.getByLabelText("Schlüssel nur für diese App-Sitzung"),
    "synthetic-cloud-key",
  );
  // Back on the recording page, a local model is chosen from the presets.
  await userEvent.click(screen.getByRole("button", { name: "Meeting" }));
  await userEvent.click(
    await screen.findByRole("button", { name: "Lokales Modell einrichten" }),
  );
  await userEvent.click(
    await screen.findByRole("button", { name: "Verwenden: gemma4:e4b" }),
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Live-Zusammenfassung starten" }),
  );
  await waitFor(() => expect(start).toHaveBeenCalled());
  expect(start.mock.calls[0][0]).toMatchObject({
    mode: "local",
    endpoint: "http://127.0.0.1:11434/v1/",
    model: "gemma4:e4b",
    apiKey: "",
    useSavedKey: false,
  });
});

it("shows the detected local servers on the analysis page when the configured endpoint has no model", async () => {
  // Earlier tests remember a local endpoint; this case starts from the default port.
  localStorage.clear();
  // 1.0.5 feedback: LM Studio/Ollama were installed but the analysis page only showed an
  // error at the default port; the detection was hidden behind a collapsed section.
  const meeting: Meeting = {
    id: "d".repeat(32),
    title: "Synthetic analysis",
    startedAt: "2026-10-28T10:00:00Z",
    durationMs: 5000,
    status: "complete",
    modelId: "synthetic",
    warnings: [],
    parts: [],
  };
  const api = {
    ...testServices(),
    getAppState: vi.fn(async () => ({
      version: "synthetic",
      handy: null,
      recording: false,
      activeMeeting: null,
    })),
    listDevices: vi.fn().mockResolvedValue([]),
    listMeetings: vi.fn().mockResolvedValue([]),
    getMeeting: vi.fn().mockResolvedValue(meeting),
    listAnalyses: vi.fn().mockResolvedValue({ analyses: [], skipped: [] }),
    getNotesProgress: vi.fn().mockResolvedValue(null),
    listTextModels: vi.fn(async (endpoint: string) => {
      if (endpoint.includes(":11434")) return ["gemma4:e4b"];
      throw "Synthetic: no local server at 127.0.0.1:1234";
    }),
    refreshHandy: vi.fn().mockResolvedValue({
      executable: "native-choice",
      version: "synthetic",
      models: [],
    }),
    localModelSetup: vi.fn().mockResolvedValue(ollamaReady),
  } as unknown as DesktopApi;
  render(<App api={api} />);
  await userEvent.click(
    await screen.findByRole("button", { name: "Nachbearbeitung" }),
  );
  await userEvent.click(
    await screen.findByRole("button", { name: "Verwenden: gemma4:e4b" }),
  );
  expect(api.localModelSetup).toHaveBeenCalled();
  // CodeRabbit review 5683206688: the chosen server's models load without a manual refresh.
  await waitFor(() =>
    expect(api.listTextModels).toHaveBeenCalledWith(
      "http://127.0.0.1:11434/v1/",
    ),
  );
  const select = screen.getByRole("combobox", { name: "Textmodell" });
  await waitFor(() =>
    expect((select as HTMLSelectElement).value).toBe("gemma4:e4b"),
  );
  expect(
    within(select).queryByRole("option", { name: /Noch nicht erkannt/ }),
  ).toBeNull();
});

it("offers the start only for endpoints the backend transport accepts", () => {
  // CodeRabbit review 5682803580: a remote address in local mode passed the start gate and
  // failed only in the backend.
  expect(endpointAllowed("local", "http://127.0.0.1:11434/v1/")).toBe(true);
  expect(endpointAllowed("local", "http://localhost:1234/v1/")).toBe(true);
  expect(endpointAllowed("local", "http://[::1]:1234/v1/")).toBe(true);
  expect(endpointAllowed("local", "https://ai.example.com/v1/")).toBe(false);
  expect(endpointAllowed("local", "http://192.168.1.5:11434/v1/")).toBe(false);
  expect(endpointAllowed("local", "http://user:pw@127.0.0.1:1234/v1/")).toBe(
    false,
  );
  expect(endpointAllowed("local", "ftp://127.0.0.1/")).toBe(false);
  expect(endpointAllowed("local", "not a url")).toBe(false);
  expect(endpointAllowed("cloud", "https://openrouter.ai/api/v1/")).toBe(true);
  expect(endpointAllowed("cloud", "http://openrouter.ai/api/v1/")).toBe(false);
  expect(endpointAllowed("cloud", "https://openrouter.ai/api/v1/?key=x")).toBe(
    false,
  );
});

it("reconnects to a model download that kept running while the setup was closed", async () => {
  // Codex review 5683390412: reopening offered a second download instead of the progress.
  const running = {
    model: "gemma4:e4b",
    status: "running",
    detail: "pulling",
    completed: 5,
    total: 10,
    error: null,
  };
  const pull = vi.fn();
  setup(
    {
      getLiveNotes: vi.fn().mockResolvedValue(status()),
      startLiveNotes: vi.fn(),
      localModelSetup: vi.fn().mockResolvedValue({
        ...ollamaReady,
        providers: [
          {
            ...ollamaReady.providers[0],
            models: [],
            recommendedInstalled: false,
          },
        ],
      }),
      pullOllamaModel: pull,
      getModelPullProgress: vi.fn().mockResolvedValue(running),
      cancelModelPull: vi.fn(),
    },
    { connection: { ...local, model: "" } },
  );
  await userEvent.click(
    await screen.findByRole("button", { name: "Lokales Modell einrichten" }),
  );
  expect(await screen.findByText(/Lädt gemma4:e4b · 50 %/)).toBeTruthy();
  expect(screen.getByRole("button", { name: "Abbrechen" })).toBeTruthy();
  expect(
    screen.queryByRole("button", { name: "gemma4:e4b in Ollama laden" }),
  ).toBeNull();
  expect(pull).not.toHaveBeenCalled();
});

it("reloads the model list after Use even when the address stays the same", async () => {
  // Codex review 5683390412: a model loaded later at the configured server stayed unavailable.
  localStorage.clear();
  const meeting: Meeting = {
    id: "e".repeat(32),
    title: "Synthetic analysis",
    startedAt: "2026-10-28T10:00:00Z",
    durationMs: 5000,
    status: "complete",
    modelId: "synthetic",
    warnings: [],
    parts: [],
  };
  let loaded: string[] = [];
  const api = {
    ...testServices(),
    getAppState: vi.fn(async () => ({
      version: "synthetic",
      handy: null,
      recording: false,
      activeMeeting: null,
    })),
    listDevices: vi.fn().mockResolvedValue([]),
    listMeetings: vi.fn().mockResolvedValue([]),
    getMeeting: vi.fn().mockResolvedValue(meeting),
    listAnalyses: vi.fn().mockResolvedValue({ analyses: [], skipped: [] }),
    getNotesProgress: vi.fn().mockResolvedValue(null),
    listTextModels: vi.fn(async () => loaded),
    refreshHandy: vi.fn().mockResolvedValue({
      executable: "native-choice",
      version: "synthetic",
      models: [],
    }),
    localModelSetup: vi.fn(async () => ({
      ...ollamaReady,
      providers: [
        {
          id: "lmstudio",
          name: "LM Studio",
          endpoint: "http://127.0.0.1:1234/v1/",
          reachable: true,
          models: loaded,
          recommendedInstalled: false,
        },
      ],
    })),
  } as unknown as DesktopApi;
  render(<App api={api} />);
  await userEvent.click(
    await screen.findByRole("button", { name: "Nachbearbeitung" }),
  );
  // No model yet: the detection opens; the user then loads one in LM Studio.
  await screen.findByText("LM Studio");
  loaded = ["gemma-4-e4b"];
  await userEvent.click(screen.getByRole("button", { name: "Erneut prüfen" }));
  await userEvent.click(
    await screen.findByRole("button", { name: "Verwenden: gemma-4-e4b" }),
  );
  const select = screen.getByRole("combobox", { name: "Textmodell" });
  await waitFor(() =>
    expect(
      within(select).getByRole("option", { name: "gemma-4-e4b" }),
    ).toBeTruthy(),
  );
  expect((select as HTMLSelectElement).value).toBe("gemma-4-e4b");
});

it("reports a failed download-state lookup instead of leaving an unhandled rejection", async () => {
  // CodeRabbit review on cc67da1: the reconnect lookup had no error path.
  setup(
    {
      getLiveNotes: vi.fn().mockResolvedValue(status()),
      startLiveNotes: vi.fn(),
      localModelSetup: vi.fn().mockResolvedValue(ollamaReady),
      getModelPullProgress: vi
        .fn()
        .mockRejectedValue("Synthetic: download state unavailable"),
    },
    { connection: { ...local, model: "" } },
  );
  await userEvent.click(
    await screen.findByRole("button", { name: "Lokales Modell einrichten" }),
  );
  expect(
    await screen.findByText("Synthetic: download state unavailable"),
  ).toBeTruthy();
});

it("ignores start answers and errors that arrive after the meeting was switched", async () => {
  // Review 6474935681 (P2): only the mount was checked, so a late answer for meeting A
  // replaced meeting B's state, error and busy flag, and Copy used B's title for A's notes.
  const a = "a".repeat(32),
    b = "b".repeat(32);
  const answers: {
    resolve: (next: LiveNotesStatus) => void;
    reject: (error: unknown) => void;
  }[] = [];
  const start = vi.fn(
    () =>
      new Promise<LiveNotesStatus>((resolve, reject) =>
        answers.push({ resolve, reject }),
      ),
  );
  const copyText = vi.fn().mockResolvedValue(undefined);
  const api = {
    copyText,
    getLiveNotes: vi.fn((id: string) =>
      Promise.resolve(status({ meetingId: id })),
    ),
    startLiveNotes: start,
  } as unknown as DesktopApi;
  const panel = (meetingId: string, title: string) => (
    <LiveNotesPanel
      api={api}
      locale="de"
      connected
      meetingId={meetingId}
      meetingTitle={title}
      meetingActive
      connection={local}
      preferences={{ auto: false, intervalMinutes: 3 }}
      setPreferences={vi.fn()}
      onUseLocalModel={vi.fn()}
      onOpenTextModelSettings={vi.fn()}
      focus={false}
      setFocus={vi.fn()}
    />
  );
  const { rerender } = render(panel(a, "Meeting A"));
  const startButton = () =>
    screen.getByRole("button", {
      name: "Live-Zusammenfassung starten",
    }) as HTMLButtonElement;
  await userEvent.click(
    await screen.findByRole("button", { name: "Live-Zusammenfassung starten" }),
  );
  expect(start).toHaveBeenCalledWith(expect.objectContaining({ meetingId: a }));
  rerender(panel(b, "Meeting B"));
  await waitFor(() => expect(startButton().disabled).toBe(false));
  await act(async () =>
    answers[0].resolve(
      status({ meetingId: a, status: "waiting", running: true, view }),
    ),
  );
  expect(screen.queryByText("Erster Punkt")).toBeNull();
  expect(startButton().disabled).toBe(false);
  expect(
    (
      screen.getByRole("button", {
        name: "Zusammenfassung kopieren",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);

  // The same for a refusal: meeting B's start fails only after A is shown again.
  await userEvent.click(startButton());
  expect(start).toHaveBeenLastCalledWith(
    expect.objectContaining({ meetingId: b }),
  );
  rerender(panel(a, "Meeting A"));
  await act(async () => answers[1].reject("Synthetischer Fehler für B"));
  expect(screen.queryByText(/Synthetischer Fehler für B/)).toBeNull();
  expect(startButton().disabled).toBe(false);
  expect(copyText).not.toHaveBeenCalled();
});
