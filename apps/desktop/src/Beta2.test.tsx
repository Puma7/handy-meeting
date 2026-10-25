import { beforeEach, describe, it, expect, vi } from "vitest";
import {
  render,
  screen,
  waitFor,
  within,
  act,
  fireEvent,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { clock, meetingLabel } from "./domain";
import type {
  AnalysisResult,
  AppState,
  DesktopApi,
  Meeting,
  NotesProgress,
  NotesRequest,
} from "./types";

// Synthetic IPC only: no audio device, installed model, provider or clipboard is used.
const first: Meeting = {
  id: "synthetic-a",
  title: "Jour fixe",
  startedAt: "2026-10-25T09:00:00+01:00",
  durationMs: 60000,
  status: "complete",
  modelId: "synthetic-asr",
  warnings: [],
  parts: [
    {
      file: "system-a.wav",
      source: "system",
      startSample: 0,
      sampleCount: 480000,
      text: "Anna übernimmt ausdrücklich den Bericht bis Freitag.",
      status: "complete",
    },
  ],
};
const second: Meeting = {
  ...first,
  id: "synthetic-b",
  startedAt: "2026-10-25T14:00:00+01:00",
  durationMs: 120000,
  parts: [
    {
      ...first.parts[0],
      file: "system-b.wav",
      text: "Ben prüft ausdrücklich die Rechnung bis Montag.",
    },
  ],
};
function appState(): AppState {
  return {
    version: "1.0.0-beta.2",
    recording: false,
    activeMeeting: first,
    handy: {
      executable: "synthetic-handy",
      version: "0.9.8",
      models: [
        {
          id: "synthetic-asr",
          name: "Synthetic streaming",
          isDownloaded: true,
          supportsStreaming: true,
        },
      ],
    },
  };
}
function apiFixture() {
  const meetings = new Map([
      [first.id, first],
      [second.id, second],
    ]),
    analyses = new Map<string, AnalysisResult[]>();
  let progress: NotesProgress | null = null;
  const api: DesktopApi = {
    getAppState: vi.fn().mockResolvedValue(appState()),
    refreshHandy: vi.fn().mockResolvedValue(appState().handy),
    listDevices: vi.fn().mockResolvedValue([
      {
        id: "output",
        name: "Synthetic output",
        source: "system",
        isDefault: true,
      },
    ]),
    startMeeting: vi.fn(),
    stopMeeting: vi.fn(),
    listMeetings: vi.fn().mockResolvedValue([second, first]),
    getMeeting: vi.fn(async (id) => {
      const value = meetings.get(id);
      if (!value) throw new Error("Synthetic missing session");
      return structuredClone(value);
    }),
    resumeMeeting: vi.fn(),
    importLegacy: vi.fn(),
    generateNotes: vi.fn(),
    listAnalyses: vi.fn(async (id) => analyses.get(id) ?? []),
    listTextModels: vi
      .fn()
      .mockResolvedValue(["synthetic-local", "synthetic-alternative"]),
    getNotesProgress: vi.fn(async () => progress),
    cancelNotes: vi.fn().mockResolvedValue(false),
    notesMarkdown: vi.fn(),
    copyText: vi.fn().mockResolvedValue(undefined),
  };
  return {
    api,
    analyses,
    setProgress: (next: NotesProgress | null) => {
      progress = next;
    },
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (error: unknown) => void;
  const promise = new Promise<T>((a, b) => {
    resolve = a;
    reject = b;
  });
  return { promise, resolve, reject };
}
async function openNotes() {
  await screen.findByRole("button", { name: "Dieses Meeting auswerten" });
  await userEvent.click(
    screen.getByRole("button", { name: "Nachbearbeitung" }),
  );
  await screen.findByRole("option", { name: "synthetic-local" });
  await waitFor(() =>
    expect(
      (screen.getByLabelText("Meeting für die Auswertung") as HTMLSelectElement)
        .value,
    ).toBe(first.id),
  );
}
async function prepareLocal() {
  await openNotes();
  await userEvent.selectOptions(
    screen.getByLabelText("Textmodell"),
    "synthetic-local",
  );
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Auswertung erstellen",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
}
function resultFor(
  request: NotesRequest,
  meeting: Meeting,
  text: string,
): AnalysisResult {
  return {
    id: "synthetic-result-" + meeting.id,
    meetingId: meeting.id,
    sourceHash: "synthetic-source-hash",
    template: request.template,
    createdAt: "2026-10-25T15:01:00Z",
    sources: [
      {
        id: "S00001",
        source: "system",
        startMs: 0,
        endMs: 30000,
        text: meeting.parts[0].text!,
      },
    ],
    sections: request.template.steps.map((step, index) => ({
      id: step.id,
      name: step.name,
      kind: step.kind,
      items: index
        ? []
        : [
            {
              text,
              owner: null,
              due: null,
              evidence: [{ sourceId: "S00001", quote: meeting.parts[0].text! }],
            },
          ],
    })),
  };
}
function running(request: NotesRequest): NotesProgress {
  return {
    jobId: request.jobId!,
    runId: "0123456789abcdef0123456789abcdef",
    meetingId: request.meetingId,
    status: "running",
    stage: "requesting",
    stepName: "Entscheidungen",
    stepIndex: 2,
    totalSteps: 3,
    batchIndex: 1,
    totalBatches: 1,
    confirmedTasks: 1,
    totalTasks: 3,
    elapsedMs: 1500,
    error: null,
    cancelRequested: false,
  };
}
beforeEach(() => localStorage.clear());

describe("beta.2 guided meeting analysis", () => {
  it("loads real-service model IDs on entering notes, keeps choices explicit and never discovers cloud models", async () => {
    const { api } = apiFixture();
    render(<App api={api} />);
    await openNotes();
    expect(api.listTextModels).toHaveBeenCalledWith(
      "http://127.0.0.1:1234/v1/",
    );
    expect(
      (screen.getByLabelText("Textmodell") as HTMLSelectElement).value,
    ).toBe("");
    await userEvent.selectOptions(
      screen.getByLabelText("Textmodell"),
      "synthetic-local",
    );
    vi.mocked(api.listTextModels).mockResolvedValue(["new-service-model"]);
    await userEvent.click(
      screen.getByRole("button", { name: "Textmodelle aktualisieren" }),
    );
    await screen.findByRole("option", { name: "new-service-model" });
    expect(
      (screen.getByLabelText("Textmodell") as HTMLSelectElement).value,
    ).toBe("synthetic-local");
    expect(
      screen.getByText(
        /bisherige Modellwahl wird von diesem Dienst nicht angeboten/,
      ),
    ).toBeTruthy();
    expect(
      (
        screen.getByRole("button", {
          name: "Auswertung erstellen",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    const before = vi.mocked(api.listTextModels).mock.calls.length;
    await userEvent.selectOptions(
      screen.getByLabelText("Verarbeitung"),
      "cloud",
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(api.listTextModels).toHaveBeenCalledTimes(before);
  });
  it("does not apply an old endpoint's late model response or query each address keystroke", async () => {
    const { api } = apiFixture(),
      slow = deferred<string[]>();
    vi.mocked(api.listTextModels)
      .mockImplementationOnce(() => slow.promise)
      .mockResolvedValue(["second-endpoint-model"]);
    render(<App api={api} />);
    await screen.findByRole("button", { name: "Dieses Meeting auswerten" });
    await userEvent.click(
      screen.getByRole("button", { name: "Nachbearbeitung" }),
    );
    await waitFor(() => expect(api.listTextModels).toHaveBeenCalledTimes(1));
    fireEvent.change(
      screen.getByLabelText("Adresse des lokalen Textmodelldienstes"),
      { target: { value: "http://127.0.0.1:4321/v1/" } },
    );
    expect(api.listTextModels).toHaveBeenCalledTimes(1);
    await userEvent.click(
      screen.getByRole("button", { name: "Textmodelle aktualisieren" }),
    );
    await screen.findByRole("option", { name: "second-endpoint-model" });
    await act(async () => {
      slow.resolve(["old-endpoint-model"]);
    });
    expect(
      screen.queryByRole("option", { name: "old-endpoint-model" }),
    ).toBeNull();
    expect(api.listTextModels).toHaveBeenLastCalledWith(
      "http://127.0.0.1:4321/v1/",
    );
  });
  it("opens history as session details and analyses that exact dated session", async () => {
    const { api } = apiFixture();
    render(<App api={api} />);
    await screen.findByRole("button", { name: "Dieses Meeting auswerten" });
    await userEvent.click(screen.getByRole("button", { name: "Historie" }));
    const date =
      new Date(second.startedAt).toLocaleString("de-DE") +
      " · " +
      clock(second.durationMs) +
      " · Abgeschlossen";
    const row = screen.getByText(date).closest(".session-row") as HTMLElement;
    await userEvent.click(within(row).getByRole("button", { name: "Öffnen" }));
    expect(
      await screen.findByRole("heading", { name: "Sitzungsdetails" }),
    ).toBeTruthy();
    expect(screen.queryByLabelText("Meetingtitel")).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Aufnahme starten" }),
    ).toBeNull();
    expect(screen.getByText(second.parts[0].text!)).toBeTruthy();
    expect(screen.queryByText(first.parts[0].text!)).toBeNull();
    await userEvent.click(
      screen.getByRole("button", { name: "Dieses Meeting auswerten" }),
    );
    await waitFor(() =>
      expect(
        (
          screen.getByLabelText(
            "Meeting für die Auswertung",
          ) as HTMLSelectElement
        ).value,
      ).toBe(second.id),
    );
    expect(
      screen.getByRole("option", { name: meetingLabel(second, "de") }),
    ).toBeTruthy();
  });
  it("uses the current capture meeting when returning from another history detail", async () => {
    const { api } = apiFixture();
    render(<App api={api} />);
    await screen.findByRole("button", { name: "Dieses Meeting auswerten" });
    await userEvent.click(screen.getByRole("button", { name: "Historie" }));
    const row = screen
      .getByText(
        new Date(second.startedAt).toLocaleString("de-DE") +
          " · " +
          clock(second.durationMs) +
          " · Abgeschlossen",
      )
      .closest(".session-row") as HTMLElement;
    await userEvent.click(within(row).getByRole("button", { name: "Öffnen" }));
    await screen.findByRole("heading", { name: "Sitzungsdetails" });
    await userEvent.click(
      screen.getByRole("button", { name: "Meeting" }),
    );
    expect(screen.getByText(first.parts[0].text!)).toBeTruthy();
    expect(screen.queryByText(second.parts[0].text!)).toBeNull();
    await userEvent.click(
      screen.getByRole("button", { name: "Dieses Meeting auswerten" }),
    );
    await waitFor(() =>
      expect(
        (
          screen.getByLabelText(
            "Meeting für die Auswertung",
          ) as HTMLSelectElement
        ).value,
      ).toBe(first.id),
    );
  });
  it("does not let analysis A overwrite the selected meeting B when completion arrives", async () => {
    const fixture = apiFixture(),
      work = deferred<AnalysisResult>();
    let request!: NotesRequest;
    vi.mocked(fixture.api.generateNotes).mockImplementation((value) => {
      request = value;
      fixture.setProgress(running(value));
      return work.promise;
    });
    render(<App api={fixture.api} />);
    await prepareLocal();
    await userEvent.click(
      screen.getByRole("button", { name: "Auswertung erstellen" }),
    );
    await waitFor(() => expect(request.meetingId).toBe(first.id));
    await userEvent.selectOptions(
      screen.getByLabelText("Meeting für die Auswertung"),
      second.id,
    );
    await waitFor(() =>
      expect(fixture.api.listAnalyses).toHaveBeenCalledWith(second.id),
    );
    const panel = screen.getByRole("status", { name: "Auswertung" });
    expect(within(panel).getByText(meetingLabel(first, "de"))).toBeTruthy();
    const result = resultFor(request, first, "Saved result for meeting A");
    fixture.analyses.set(first.id, [result]);
    fixture.setProgress({
      ...running(request),
      status: "complete",
      stage: "finished",
      confirmedTasks: 3,
      elapsedMs: 2200,
    });
    await act(async () => {
      work.resolve(result);
    });
    expect(await screen.findByText("Auswertung gespeichert.")).toBeTruthy();
    expect(
      (screen.getByLabelText("Meeting für die Auswertung") as HTMLSelectElement)
        .value,
    ).toBe(second.id);
    expect(screen.queryByText("Saved result for meeting A")).toBeNull();
    await userEvent.click(
      screen.getByRole("button", { name: "Zugehöriges Meeting öffnen" }),
    );
    expect(await screen.findByText("Saved result for meeting A")).toBeTruthy();
    expect(
      (screen.getByLabelText("Meeting für die Auswertung") as HTMLSelectElement)
        .value,
    ).toBe(first.id);
  });
  it("cancels the running job A by nonce while leaving the selected meeting B unchanged", async () => {
    const fixture = apiFixture(),
      work = deferred<AnalysisResult>();
    let request!: NotesRequest;
    vi.mocked(fixture.api.generateNotes).mockImplementation((value) => {
      request = value;
      fixture.setProgress(running(value));
      return work.promise;
    });
    vi.mocked(fixture.api.cancelNotes).mockImplementation(async (jobId) => {
      expect(jobId).toBe(request.jobId);
      fixture.setProgress({
        ...running(request),
        status: "cancelled",
        stage: "finished",
        cancelRequested: true,
      });
      work.reject(new Error("Synthetic cancellation"));
      return true;
    });
    render(<App api={fixture.api} />);
    await prepareLocal();
    await userEvent.click(
      screen.getByRole("button", { name: "Auswertung erstellen" }),
    );
    await waitFor(() => expect(fixture.api.generateNotes).toHaveBeenCalled());
    await userEvent.selectOptions(
      screen.getByLabelText("Meeting für die Auswertung"),
      second.id,
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Auswertung abbrechen" }),
    );
    expect(
      await screen.findByText(
        "Auswertung abgebrochen. Gesicherte Schritte bleiben erhalten.",
      ),
    ).toBeTruthy();
    expect(fixture.api.cancelNotes).toHaveBeenCalledWith(request.jobId);
    expect(
      (screen.getByLabelText("Meeting für die Auswertung") as HTMLSelectElement)
        .value,
    ).toBe(second.id);
    expect(screen.queryByText("Auswertung gespeichert.")).toBeNull();
  });
  it("shows only confirmed task progress with elapsed time while the model remains pending", async () => {
    const fixture = apiFixture(),
      work = deferred<AnalysisResult>();
    let request!: NotesRequest;
    vi.mocked(fixture.api.generateNotes).mockImplementation((value) => {
      request = value;
      fixture.setProgress(running(value));
      return work.promise;
    });
    render(<App api={fixture.api} />);
    await prepareLocal();
    await userEvent.click(
      screen.getByRole("button", { name: "Auswertung erstellen" }),
    );
    const bar = (await screen.findByRole("progressbar", {
      name: "geprüfte und gespeicherte Teilaufgaben",
    })) as HTMLProgressElement;
    expect(bar.value).toBe(1);
    expect(bar.max).toBe(3);
    expect(
      screen.getByText(/1\/3 geprüfte und gespeicherte Teilaufgaben · 33%/),
    ).toBeTruthy();
    expect(
      screen.getByText(/Rechenzeit des Modells ist nicht vorhersehbar/),
    ).toBeTruthy();
    const panel = screen.getByRole("status", { name: "Auswertung" });
    await waitFor(() => expect(within(panel).getByText("00:02")).toBeTruthy(), {
      timeout: 3500,
    });
    const result = resultFor(request, first, "Finished result");
    fixture.setProgress({
      ...running(request),
      status: "complete",
      stage: "finished",
      confirmedTasks: 3,
    });
    await act(async () => {
      work.resolve(result);
    });
  });
  it("keeps completion when an earlier cancellation request returns late", async () => {
    const fixture = apiFixture(),
      work = deferred<AnalysisResult>(),
      cancellation = deferred<boolean>();
    let request!: NotesRequest;
    vi.mocked(fixture.api.generateNotes).mockImplementation((value) => {
      request = value;
      fixture.setProgress(running(value));
      return work.promise;
    });
    vi.mocked(fixture.api.cancelNotes).mockReturnValue(cancellation.promise);
    render(<App api={fixture.api} />);
    await prepareLocal();
    await userEvent.click(
      screen.getByRole("button", { name: "Auswertung erstellen" }),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Auswertung abbrechen" }),
    );
    await waitFor(() =>
      expect(fixture.api.cancelNotes).toHaveBeenCalledWith(request.jobId),
    );
    fixture.setProgress({
      ...running(request),
      status: "complete",
      stage: "finished",
      confirmedTasks: 3,
    });
    await act(async () => {
      work.resolve(resultFor(request, first, "Completed before cancellation"));
    });
    await screen.findByText("Auswertung gespeichert.");
    await act(async () => cancellation.resolve(true));
    expect(screen.getByText("Completed before cancellation")).toBeTruthy();
    expect(screen.getByText("Auswertung gespeichert.")).toBeTruthy();
    expect(screen.queryByText("Abbruch angefordert …")).toBeNull();
    expect(
      screen.queryByText(
        "Auswertung abgebrochen. Gesicherte Schritte bleiben erhalten.",
      ),
    ).toBeNull();
  });
  it("resumes only the saved request snapshot with a fresh job ID", async () => {
    const fixture = apiFixture(),
      original = deferred<AnalysisResult>(),
      resumed = deferred<AnalysisResult>();
    const requests: NotesRequest[] = [];
    vi.mocked(fixture.api.generateNotes).mockImplementation((request) => {
      requests.push(request);
      fixture.setProgress(running(request));
      return requests.length === 1 ? original.promise : resumed.promise;
    });
    render(<App api={fixture.api} />);
    await prepareLocal();
    await userEvent.click(
      screen.getByRole("button", { name: "Auswertung erstellen" }),
    );
    await waitFor(() => expect(requests).toHaveLength(1));
    const initial = requests[0];
    fixture.setProgress({
      ...running(initial),
      status: "failed",
      stage: "finished",
      error: "Synthetic provider failure",
    });
    await act(async () =>
      original.reject(new Error("Synthetic provider failure")),
    );
    await screen.findByRole("button", {
      name: "Gesicherte Auswertung fortsetzen",
    });
    await userEvent.selectOptions(
      screen.getByLabelText("Textmodell"),
      "synthetic-alternative",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Gesicherte Auswertung fortsetzen" }),
    );
    await waitFor(() => expect(requests).toHaveLength(2));
    const next = requests[1];
    expect(next.jobId).not.toBe(initial.jobId);
    expect(next.resumeRunId).toBe(running(initial).runId);
    expect(next.model).toBe(initial.model);
    expect(next.meetingId).toBe(initial.meetingId);
    expect(next.template).toEqual(initial.template);
    fixture.setProgress({
      ...running(next),
      status: "complete",
      stage: "finished",
      confirmedTasks: 3,
    });
    await act(async () =>
      resumed.resolve(resultFor(next, first, "Resumed result")),
    );
    expect(await screen.findByText("Resumed result")).toBeTruthy();
  });
  it("copies the complete backend-validated Markdown rather than rebuilding unescaped model output", async () => {
    const fixture = apiFixture();
    const request: NotesRequest = {
      meetingId: first.id,
      template: {
        id: "template",
        name: "Saved",
        steps: [
          {
            id: "summary",
            name: "Summary",
            kind: "summary",
            prompt: "Summarise with evidence",
          },
        ],
      },
      endpoint: "http://127.0.0.1:1234/v1/",
      model: "synthetic-local",
      mode: "local",
      cloudConsent: false,
      apiKey: "",
    };
    fixture.analyses.set(first.id, [
      resultFor(request, first, "<script>untrusted model content</script>"),
    ]);
    const markdown =
      "# Saved analysis\n\n\\<script\\>untrusted model content\\</script\\>\n\n[Source S00001](#source-00001)\n\n## Original transcript\n\n" +
      first.parts[0].text;
    vi.mocked(fixture.api.notesMarkdown).mockResolvedValue(markdown);
    render(<App api={fixture.api} />);
    await openNotes();
    await screen.findByText("<script>untrusted model content</script>");
    await userEvent.click(
      screen.getByRole("button", {
        name: "Alle Ergebnisse als Markdown kopieren",
      }),
    );
    await waitFor(() =>
      expect(fixture.api.copyText).toHaveBeenCalledWith(markdown),
    );
    expect(fixture.api.notesMarkdown).toHaveBeenCalledWith(
      first.id,
      "synthetic-result-" + first.id,
    );
    expect(document.querySelector(".analysis-body script")).toBeNull();
  });
  it("preserves future preference fields and damaged preference contents", async () => {
    const key = "handymeeting.rust.beta.preferences.v1";
    localStorage.setItem(
      key,
      JSON.stringify({
        locale: "de",
        theme: "light",
        futureSetting: { keep: true },
      }),
    );
    const rendered = render(<App api={apiFixture().api} />);
    await screen.findByRole("button", { name: "Dieses Meeting auswerten" });
    await userEvent.selectOptions(screen.getByLabelText("Sprache"), "en");
    expect(JSON.parse(localStorage.getItem(key)!).futureSetting).toEqual({
      keep: true,
    });
    rendered.unmount();
    localStorage.setItem(key, "{damaged-original");
    render(<App api={apiFixture().api} />);
    await screen.findByRole("button", { name: "Dieses Meeting auswerten" });
    expect(localStorage.getItem(key)).toBe("{damaged-original");
  });
  it("describes a detected Handy without a version as a metadata limit", async () => {
    const fixture = apiFixture();
    vi.mocked(fixture.api.getAppState).mockResolvedValue({
      ...appState(),
      handy: { ...appState().handy!, version: null },
    });
    render(<App api={fixture.api} />);
    await screen.findByRole("button", { name: "Dieses Meeting auswerten" });
    await userEvent.click(
      screen.getByRole("button", { name: "Einstellungen" }),
    );
    expect(
      screen.getByText(
        /Handy erkannt; die Installation liefert keine Versionsangabe/,
      ),
    ).toBeTruthy();
  });
});
