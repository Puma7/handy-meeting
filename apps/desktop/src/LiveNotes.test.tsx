import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import LiveNotesPanel, {
  liveNotesText,
  type LiveConnection,
} from "./views/LiveNotesPanel";
import type {
  DesktopApi,
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
  expect(screen.getByText("Keine offenen Fragen.")).toBeTruthy();
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
  expect(text).not.toContain("Offene Fragen");
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
  expect(screen.getByText(/„Gemma 4 E4B“ laden/)).toBeTruthy();
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
