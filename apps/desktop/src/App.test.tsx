import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { testServices } from "./test-services";
import { defaultTemplate, renderTranscript, validateLibrary } from "./domain";
import type { AnalysisResult, AppState, DesktopApi, Meeting } from "./types";

// Synthetic IPC fixtures only. These tests never open an audio device or a model server.
const meeting: Meeting = {
  id: "synthetic-session",
  title: "Synthetic meeting",
  startedAt: "2026-10-25T12:00:00+01:00",
  durationMs: 60000,
  status: "complete",
  modelId: "installed-stream",
  warnings: [],
  parts: [
    {
      file: "system-000000000000.wav",
      source: "system",
      startSample: 0,
      sampleCount: 480000,
      text: "Anna übernimmt den Bericht bis Freitag.",
      status: "complete",
    },
  ],
};
function state(active: Meeting | null = null): AppState {
  return {
    version: "1.0.0-beta.1",
    recording: false,
    activeMeeting: active,
    handy: {
      executable: "synthetic-handy",
      version: "0.9.8",
      models: [
        {
          id: "installed-stream",
          name: "Installed streaming model",
          isDownloaded: true,
          supportsStreaming: true,
        },
      ],
    },
  };
}
function fakeApi(active: Meeting | null = null): DesktopApi {
  return {
    ...testServices(),
    getAppState: vi.fn().mockResolvedValue(state(active)),
    refreshHandy: vi.fn().mockResolvedValue(state().handy),
    listDevices: vi.fn().mockResolvedValue([
      {
        id: "out",
        name: "Synthetic output",
        source: "system",
        isDefault: true,
      },
      { id: "mic", name: "Synthetic microphone", source: "microphone" },
    ]),
    startMeeting: vi.fn().mockRejectedValue("Synthetic capture unavailable"),
    stopMeeting: vi.fn().mockResolvedValue(meeting),
    listMeetings: vi.fn().mockResolvedValue([meeting]),
    getMeeting: vi.fn().mockResolvedValue(active ?? meeting),
    resumeMeeting: vi.fn().mockResolvedValue(active ?? meeting),
    importLegacy: vi.fn().mockResolvedValue({ ...meeting, readOnly: true }),
    generateNotes: vi.fn(),
    listAnalyses: vi.fn().mockResolvedValue({ analyses: [], skipped: [] }),
    listTextModels: vi.fn().mockResolvedValue(["synthetic-text-model"]),
    getNotesProgress: vi.fn().mockResolvedValue(null),
    cancelNotes: vi.fn().mockResolvedValue(false),
    notesMarkdown: vi
      .fn()
      .mockResolvedValue("# Synthetic analysis\n\nEscaped source text.\n"),
    copyText: vi.fn().mockResolvedValue(undefined),
  };
}
async function ready() {
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Aufnahme starten",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
}
beforeEach(() => localStorage.clear());
describe("migration frontend boundaries (synthetic IPC)", () => {
  it("requires actual desktop readiness and passes separate microphone selection", async () => {
    const api = fakeApi();
    render(<App api={api} />);
    await ready();
    await userEvent.click(
      screen.getByLabelText("Mikrofon zusätzlich aufnehmen"),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Aufnahme starten" }),
    );
    await waitFor(() =>
      expect(api.startMeeting).toHaveBeenCalledWith({
        title: expect.stringMatching(
          /^Meeting \d{1,2}\.\d{1,2}\.\d{4} \d{2}:\d{2}$/,
        ),
        modelId: "installed-stream",
        systemDeviceId: "out",
        microphoneDeviceId: "mic",
      }),
    );
    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      expect.stringContaining("Synthetic capture unavailable"),
    );
  });
  it("uses refreshed Handy models and excludes unsupported/uninstalled entries", async () => {
    const api = fakeApi();
    vi.mocked(api.refreshHandy).mockResolvedValue({
      executable: "new-handy",
      version: "1.0.0",
      models: [
        {
          id: "new",
          name: "New upstream model",
          isDownloaded: true,
          supportsStreaming: true,
        },
        {
          id: "batch",
          name: "Batch only",
          isDownloaded: true,
          supportsStreaming: false,
        },
        {
          id: "missing",
          name: "Not installed",
          isDownloaded: false,
          supportsStreaming: true,
        },
      ],
    });
    render(<App api={api} />);
    await ready();
    await userEvent.click(
      screen.getByRole("button", { name: "Aktualisieren" }),
    );
    expect(
      await screen.findByRole("option", { name: "New upstream model" }),
    ).toBeTruthy();
    expect(screen.queryByRole("option", { name: "Batch only" })).toBeNull();
    expect(screen.queryByRole("option", { name: "Not installed" })).toBeNull();
    await waitFor(() =>
      expect(
        (
          screen.getByLabelText(
            "Streaming-Modell aus Handy",
          ) as HTMLSelectElement
        ).value,
      ).toBe(""),
    );
    expect(
      await screen.findByText(
        /zuvor gewählte Modell ist nicht mehr installiert oder kompatibel/,
      ),
    ).toBeTruthy();
    expect(
      (
        screen.getByRole("button", {
          name: "Aufnahme starten",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    await userEvent.selectOptions(
      screen.getByLabelText("Streaming-Modell aus Handy"),
      "new",
    );
    expect(
      (screen.getByLabelText("Streaming-Modell aus Handy") as HTMLSelectElement)
        .value,
    ).toBe("new");
  });
  it("refreshes new Handy models on focus with startup throttling and coalesced in-flight requests", async () => {
    let now = 100000;
    vi.spyOn(Date, "now").mockImplementation(() => now);
    let current = state();
    const api = fakeApi();
    vi.mocked(api.getAppState).mockImplementation(async () => current);
    let finishRefresh!: (value: NonNullable<AppState["handy"]>) => void;
    vi.mocked(api.refreshHandy).mockImplementation(
      () =>
        new Promise((resolve) => {
          finishRefresh = resolve;
        }),
    );
    render(<App api={api} />);
    await ready();
    fireEvent.focus(window);
    expect(api.refreshHandy).not.toHaveBeenCalled();
    now += 31000;
    fireEvent.focus(window);
    await waitFor(() => expect(api.refreshHandy).toHaveBeenCalledTimes(1));
    now += 31000;
    for (let index = 0; index < 5; index++) fireEvent.focus(window);
    expect(api.refreshHandy).toHaveBeenCalledTimes(1);
    current = {
      ...current,
      handy: {
        ...current.handy!,
        models: [
          ...current.handy!.models,
          {
            id: "new-from-handy",
            name: "New model after Handy update",
            isDownloaded: true,
            supportsStreaming: true,
          },
        ],
      },
    };
    await act(async () => {
      finishRefresh(current.handy!);
    });
    expect(
      await screen.findByRole("option", {
        name: "New model after Handy update",
      }),
    ).toBeTruthy();
    expect(
      (screen.getByLabelText("Streaming-Modell aus Handy") as HTMLSelectElement)
        .value,
    ).toBe("installed-stream");
    now += 1000;
    fireEvent.focus(window);
    expect(api.refreshHandy).toHaveBeenCalledTimes(1);
  });
  it("rechecks fresh backend state before a focus refresh and retries after catch-up ends", async () => {
    let now = 100000;
    vi.spyOn(Date, "now").mockImplementation(() => now);
    let current = state();
    const api = fakeApi();
    vi.mocked(api.getAppState).mockImplementation(async () => current);
    render(<App api={api} />);
    await ready();
    current = {
      ...current,
      activeMeeting: { ...meeting, status: "catching-up" },
      recording: false,
    };
    now += 31000;
    const previous = vi.mocked(api.getAppState).mock.calls.length;
    fireEvent.focus(window);
    await waitFor(() =>
      expect(vi.mocked(api.getAppState).mock.calls.length).toBeGreaterThan(
        previous,
      ),
    );
    expect(api.refreshHandy).not.toHaveBeenCalled();
    current = state();
    now += 1;
    fireEvent.focus(window);
    await waitFor(() => expect(api.refreshHandy).toHaveBeenCalledTimes(1));
  });
  it("does not duplicate the backend startup model query before its first catalogue arrives", async () => {
    let now = 100000;
    vi.spyOn(Date, "now").mockImplementation(() => now);
    const api = fakeApi();
    vi.mocked(api.getAppState).mockResolvedValue({ ...state(), handy: null });
    render(<App api={api} />);
    await waitFor(() => expect(api.listDevices).toHaveBeenCalled());
    now += 45000;
    fireEvent.focus(window);
    await act(async () => {
      await Promise.resolve();
    });
    expect(api.refreshHandy).not.toHaveBeenCalled();
  });
  it.each([
    {
      name: "recording",
      recording: true,
      transitioning: false,
      status: "recording",
    },
    {
      name: "ending",
      recording: false,
      transitioning: true,
      status: "recorded",
    },
    {
      name: "ASR catch-up",
      recording: false,
      transitioning: false,
      status: "catching-up",
    },
  ])(
    "does not query Handy on focus while $name is active",
    async ({ recording, transitioning, status }) => {
      let now = 100000;
      vi.spyOn(Date, "now").mockImplementation(() => now);
      const api = fakeApi();
      vi.mocked(api.getAppState).mockResolvedValue({
        ...state({ ...meeting, status }),
        recording,
        transitioning,
      });
      render(<App api={api} />);
      await waitFor(() => expect(api.listDevices).toHaveBeenCalled());
      now += 31000;
      fireEvent.focus(window);
      await act(async () => {
        await Promise.resolve();
      });
      expect(api.refreshHandy).not.toHaveBeenCalled();
    },
  );
  it("does not pretend recording works when the desktop core is missing", async () => {
    const api = fakeApi();
    vi.mocked(api.getAppState).mockRejectedValue("Synthetic disconnected core");
    render(<App api={api} />);
    expect(
      await screen.findByText("Desktop-Verbindung nicht verfügbar"),
    ).toBeTruthy();
    expect(
      (
        screen.getByRole("button", {
          name: "Aufnahme starten",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(api.startMeeting).not.toHaveBeenCalled();
  });
  it("copies all transcript sections beyond the bounded live display", async () => {
    const long: Meeting = {
      ...meeting,
      status: "recording",
      parts: Array.from({ length: 70 }, (_, index) => ({
        ...meeting.parts[0],
        file: "system-" + index + ".wav",
        startSample: index * 480000,
        text: "Synthetic section " + index,
      })),
    };
    const api = fakeApi({
      ...long,
      parts: long.parts.slice(-16),
      livePreviewLimited: true,
    });
    vi.mocked(api.getMeeting).mockResolvedValue(long);
    vi.mocked(api.getAppState).mockResolvedValue({
      ...state({
        ...long,
        parts: long.parts.slice(-16),
        livePreviewLimited: true,
      }),
      recording: true,
    });
    render(<App api={api} />);
    await screen.findByRole("button", { name: "Meeting beenden" });
    expect(screen.queryByText("Synthetic section 0")).toBeNull();
    await userEvent.click(
      screen.getByRole("button", { name: "Transkript kopieren" }),
    );
    await waitFor(() => expect(api.copyText).toHaveBeenCalled());
    expect(api.getMeeting).toHaveBeenCalledWith(long.id);
    const text = vi.mocked(api.copyText).mock.calls[0][0];
    expect(text).toContain("Synthetic section 0");
    expect(text).toContain("Synthetic section 69");
    expect(text).toBe(renderTranscript(long, "de"));
  });
  it("handles pending null text and reports only globally confirmed counters", async () => {
    const pending: Meeting = {
      ...meeting,
      status: "recorded",
      livePreviewLimited: true,
      pendingAudioMs: 210000,
      savedAudioMs: { system: 1000, microphone: 0 },
      parts: [
        {
          ...meeting.parts[0],
          text: null,
          status: "pending",
          confirmedSampleCount: 16000,
        },
      ],
    };
    const api = fakeApi(pending);
    render(<App api={api} />);
    await ready();
    expect(screen.getByText("03:30")).toBeTruthy();
    expect(screen.getByText("00:01")).toBeTruthy();
    expect(
      (
        screen.getByRole("button", {
          name: "Transkript kopieren",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    await userEvent.click(
      screen.getByRole("button", { name: "Offene Abschnitte nachholen" }),
    );
    await waitFor(() =>
      expect(api.resumeMeeting).toHaveBeenCalledWith(pending.id),
    );
  });
  it("keeps a fully opened finished history transcript when bounded status polling continues", async () => {
    const finished: Meeting = {
      ...meeting,
      parts: Array.from({ length: 25 }, (_, index) => ({
        ...meeting.parts[0],
        file: `system-${index}.wav`,
        startSample: index * 480000,
        text: `Finished section ${index}`,
      })),
    };
    const api = fakeApi();
    vi.mocked(api.getMeeting).mockResolvedValue(finished);
    vi.mocked(api.listMeetings).mockResolvedValue([finished]);
    render(<App api={api} />);
    await ready();
    await userEvent.click(screen.getByRole("button", { name: "Historie" }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Öffnen" }),
    );
    expect(await screen.findByText("Finished section 0")).toBeTruthy();
    expect(document.querySelectorAll(".transcript-part")).toHaveLength(25);
    const previousCalls = vi.mocked(api.getAppState).mock.calls.length;
    vi.mocked(api.getAppState).mockResolvedValue(
      state({
        ...finished,
        parts: finished.parts.slice(-16),
        livePreviewLimited: true,
      }),
    );
    await waitFor(
      () =>
        expect(vi.mocked(api.getAppState).mock.calls.length).toBeGreaterThan(
          previousCalls,
        ),
      { timeout: 2500 },
    );
    expect(screen.getByText("Finished section 0")).toBeTruthy();
    expect(screen.getByText("Finished section 24")).toBeTruthy();
    expect(document.querySelectorAll(".transcript-part")).toHaveLength(25);
  });
  it("keeps the explicit ending state while stop waits and status polling reports no active recorder", async () => {
    const active: Meeting = { ...meeting, status: "recording" };
    const api = fakeApi(active);
    vi.mocked(api.getAppState).mockResolvedValue({
      ...state(active),
      recording: true,
    });
    let finishStop!: (value: Meeting) => void;
    vi.mocked(api.stopMeeting).mockImplementation(
      () =>
        new Promise<Meeting>((resolve) => {
          finishStop = resolve;
        }),
    );
    render(<App api={api} />);
    await screen.findByRole("button", { name: "Meeting beenden" });
    await userEvent.click(
      screen.getByRole("button", { name: "Meeting beenden" }),
    );
    expect(
      await screen.findByText(
        /Meeting wird beendet\. Gespeichertes Audio bleibt erhalten/,
      ),
    ).toBeTruthy();
    const before = vi.mocked(api.getAppState).mock.calls.length;
    vi.mocked(api.getAppState).mockResolvedValue({
      ...state({ ...active, status: "catching-up", livePreviewLimited: true }),
      recording: false,
      transitioning: true,
    });
    await waitFor(
      () =>
        expect(vi.mocked(api.getAppState).mock.calls.length).toBeGreaterThan(
          before,
        ),
      { timeout: 2500 },
    );
    expect(screen.queryByText("Bereit zur Aufnahme")).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Aufnahme starten" }),
    ).toBeNull();
    expect(
      (
        screen.getByRole("button", {
          name: "Aufnahme wird beendet …",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    vi.mocked(api.getAppState).mockResolvedValue({
      ...state(meeting),
      recording: false,
      transitioning: false,
    });
    await act(async () => {
      finishStop(meeting);
    });
    await ready();
    expect(
      screen.queryByText(
        /Meeting wird beendet\. Gespeichertes Audio bleibt erhalten/,
      ),
    ).toBeNull();
  });
  it("disables capture readiness during a backend transition discovered at bootstrap", async () => {
    const api = fakeApi();
    vi.mocked(api.getAppState).mockResolvedValue({
      ...state(),
      transitioning: true,
    });
    render(<App api={api} />);
    await waitFor(() => expect(api.listDevices).toHaveBeenCalled());
    expect(screen.queryByText("Bereit zur Aufnahme")).toBeNull();
    const button = screen.getByRole("button", {
      name: "Bitte warten …",
    }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(api.startMeeting).not.toHaveBeenCalled();
  });
  it("does not offer in-place recognition of read-only legacy sessions", async () => {
    const api = fakeApi({ ...meeting, status: "recorded", readOnly: true });
    render(<App api={api} />);
    await ready();
    expect(
      screen.queryByRole("button", { name: "Offene Abschnitte nachholen" }),
    ).toBeNull();
  });
  it("preserves unreadable saved templates instead of silently replacing them", async () => {
    const broken = '{"privateOriginal":"keep me"}';
    localStorage.setItem("handymeeting.rust.beta.templates.v1", broken);
    const api = fakeApi(meeting);
    vi.mocked(api.listTemplates).mockRejectedValue(
      "Synthetic corrupt native library",
    );
    render(<App api={api} />);
    await ready();
    await userEvent.click(
      screen.getByRole("button", { name: "Nachbearbeitung" }),
    );
    expect(
      await screen.findByText(/gespeicherten Vorlagen sind nicht lesbar/),
    ).toBeTruthy();
    expect(localStorage.getItem("handymeeting.rust.beta.templates.v1")).toBe(
      broken,
    );
    expect(
      (
        screen.getByRole("button", {
          name: "Analyseschritt hinzufügen",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
  });
  it("requires fresh explicit text consent for cloud generation and keeps keys out of storage", async () => {
    const api = fakeApi(meeting);
    vi.mocked(api.generateNotes).mockRejectedValue(
      "Synthetic provider failure",
    );
    render(<App api={api} />);
    await ready();
    await userEvent.click(
      screen.getByRole("button", { name: "Nachbearbeitung" }),
    );
    await userEvent.selectOptions(
      screen.getByLabelText("Verarbeitung"),
      "cloud",
    );
    await userEvent.type(
      screen.getByLabelText("Textmodell"),
      "synthetic-text-model",
    );
    fireEvent.change(
      screen.getByLabelText("Schlüssel nur für diese App-Sitzung"),
      { target: { value: "synthetic-secret-do-not-save" } },
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Auswertung erstellen" }),
    );
    expect(await screen.findByRole("dialog")).toBeTruthy();
    expect(api.generateNotes).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Abbrechen" }));
    expect(api.generateNotes).not.toHaveBeenCalled();
    await userEvent.click(
      screen.getByRole("button", { name: "Auswertung erstellen" }),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Text senden und auswerten" }),
    );
    await waitFor(() =>
      expect(api.generateNotes).toHaveBeenCalledWith(
        expect.objectContaining({
          mode: "cloud",
          cloudConsent: true,
          apiKey: "synthetic-secret-do-not-save",
          meetingId: meeting.id,
        }),
      ),
    );
    expect(
      Array.from({ length: localStorage.length }, (_, index) =>
        localStorage.getItem(localStorage.key(index)!),
      ).join("\n"),
    ).not.toContain("synthetic-secret");
    await screen.findByText("Synthetic provider failure");
    await userEvent.click(
      screen.getByRole("button", { name: "Auswertung erstellen" }),
    );
    expect(screen.getByRole("dialog")).toBeTruthy();
    expect(api.generateNotes).toHaveBeenCalledTimes(1);
  });
  it("renders model text literally and opens only matching verbatim evidence", async () => {
    const template = defaultTemplate();
    const result: AnalysisResult = {
      id: "result",
      meetingId: meeting.id,
      createdAt: "2026-10-25T12:01:00Z",
      template,
      sourceHash: "synthetic-hash",
      sections: [
        {
          id: "summary",
          name: "Summary",
          kind: "summary",
          items: [
            {
              text: '<img src=x onerror="alert(1)">',
              owner: null,
              due: null,
              evidence: [{ sourceId: "S00001", quote: "Anna übernimmt" }],
            },
          ],
        },
      ],
      sources: [
        {
          id: "S00001",
          source: "system",
          startMs: 0,
          endMs: 30000,
          text: meeting.parts[0].text!,
        },
      ],
    };
    const api = fakeApi(meeting);
    vi.mocked(api.listAnalyses).mockResolvedValue({
      analyses: [result],
      skipped: [],
    });
    render(<App api={api} />);
    await ready();
    await userEvent.click(
      screen.getByRole("button", { name: "Nachbearbeitung" }),
    );
    expect(
      await screen.findByText('<img src=x onerror="alert(1)">'),
    ).toBeTruthy();
    expect(document.querySelector(".analysis-body img")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: /S00001/ }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("Anna übernimmt").tagName).toBe("MARK");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
  });
  it("adds and edits a custom analysis without modifying saved-result instructions", async () => {
    const api = fakeApi(meeting);
    render(<App api={api} />);
    await ready();
    await userEvent.click(
      screen.getByRole("button", { name: "Nachbearbeitung" }),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Analyseschritt hinzufügen" }),
    );
    await userEvent.type(screen.getByLabelText("Name des Tabs"), "Risiken");
    await userEvent.type(
      screen.getByLabelText("Arbeitsauftrag"),
      "Extrahiere ausdrücklich genannte Risiken mit Originalzitaten.",
    );
    await userEvent.click(screen.getByRole("button", { name: "Speichern" }));
    await waitFor(() =>
      expect(
        screen
          .getByRole("tab", { name: "Risiken" })
          .getAttribute("aria-selected"),
      ).toBe("true"),
    );
    const saved = vi.mocked(api.saveTemplates).mock.calls.at(-1)![0].templates;
    expect(saved[0].steps).toHaveLength(4);
    expect(saved[0].steps[3].prompt).toContain("Originalzitaten");
  });
  it("restores focus and traps keyboard navigation in the instruction editor", async () => {
    render(<App api={fakeApi(meeting)} />);
    await ready();
    await userEvent.click(
      screen.getByRole("button", { name: "Nachbearbeitung" }),
    );
    const opener = screen.getByRole("button", {
      name: "Analyseschritt hinzufügen",
    });
    await userEvent.click(opener);
    expect(document.activeElement).toBe(screen.getByLabelText("Name des Tabs"));
    await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
    expect(document.activeElement).toBe(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Schließen",
      }),
    );
    await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
    expect(document.activeElement).toBe(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Abbrechen",
      }),
    );
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(opener);
  });
});
describe("template structure", () => {
  it("rejects duplicate IDs and incomplete instructions", () => {
    const template = defaultTemplate();
    expect(validateLibrary([template])).toBe(true);
    expect(validateLibrary([template, template])).toBe(false);
    expect(
      validateLibrary([
        { ...template, steps: [{ ...template.steps[0], prompt: "" }] },
      ]),
    ).toBe(false);
  });
});
