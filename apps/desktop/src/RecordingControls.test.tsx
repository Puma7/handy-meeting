import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import App from "./App";
import RecordingControls from "./views/RecordingControls";
import { testServices } from "./test-services";
import type { AppState, DesktopApi, Meeting } from "./types";

vi.setConfig({ testTimeout: 15_000 });
// Synthetic IPC only. No actual microphone, file dialog, executable or app storage is accessed.
const original: Meeting = {
  id: "current-recording",
  title: "Synthetic live",
  startedAt: "2026-10-27T12:00:00Z",
  durationMs: 5000,
  status: "recording",
  modelId: "synthetic",
  warnings: [],
  parts: [
    {
      file: "system-0.wav",
      source: "system",
      startSample: 0,
      sampleCount: 80000,
      status: "streaming",
      text: "Unmodified original.",
    },
  ],
};
/**
 * Provides acknowledged synthetic capture state and mocked controls. State replacement
 * simulates native replies without opening devices, file dialogs or application storage.
 */
function fixture(recording = true) {
  let state: AppState = {
    version: "synthetic",
    handy: null,
    recording,
    activeMeeting: recording ? original : null,
    captureControl: recording
      ? { paused: false, microphoneMuted: false, hasMicrophone: true }
      : null,
  };
  const api = {
    ...testServices(),
    getAppState: vi.fn(async () => structuredClone(state)),
    listDevices: vi.fn().mockResolvedValue([]),
    listMeetings: vi.fn().mockResolvedValue([]),
    getMeeting: vi.fn().mockResolvedValue(original),
    listAnalyses: vi.fn().mockResolvedValue({ analyses: [], skipped: [] }),
    getNotesProgress: vi.fn().mockResolvedValue(null),
    listTextModels: vi.fn().mockResolvedValue([]),
    setCapturePaused: vi.fn(),
    setMicrophoneMuted: vi.fn(),
    refreshHandy: vi.fn().mockResolvedValue({
      executable: "native-choice",
      version: "synthetic",
      models: [],
    }),
    chooseHandyExecutable: vi.fn().mockResolvedValue(null),
    chooseLegacyManifest: vi.fn().mockResolvedValue(null),
    importLegacy: vi.fn().mockResolvedValue({
      ...original,
      id: "legacy-selected",
      status: "complete",
      readOnly: true,
    }),
  } as unknown as DesktopApi;
  return {
    api,
    state: () => state,
    setState: (next: AppState) => {
      state = next;
    },
  };
}
beforeEach(() => localStorage.clear());

it("binds microphone reconnect to the active meeting and waits for native acknowledgement", async () => {
  const f = fixture();
  const state = {
    ...f.state(),
    microphoneReconnectAvailable: true,
    captureControl: {
      paused: true,
      microphoneMuted: true,
      hasMicrophone: false,
    },
  };
  f.setState(state);
  let acknowledge!: (value: AppState) => void;
  f.api.reconnectMicrophone = vi.fn(
    () =>
      new Promise<AppState>((resolve) => {
        acknowledge = resolve;
      }),
  );
  render(<App api={f.api} />);
  const retry = await screen.findByRole("button", {
    name: "Mikrofon erneut verbinden",
  });
  await userEvent.click(retry);
  expect(f.api.reconnectMicrophone).toHaveBeenCalledWith(original.id);
  expect((retry as HTMLButtonElement).disabled).toBe(true);
  expect(screen.getAllByText("Unmodified original.").length).toBeGreaterThan(0);
  const next = {
    ...state,
    microphoneReconnectAvailable: false,
    captureControl: { ...state.captureControl, hasMicrophone: true },
  };
  f.setState(next);
  await act(async () => acknowledge(next));
  await waitFor(() =>
    expect(
      screen.queryByRole("button", { name: "Mikrofon erneut verbinden" }),
    ).toBeNull(),
  );
  expect(
    screen.getByRole("button", { name: "Aufnahme fortsetzen" }),
  ).toBeTruthy();
  expect(
    screen.getByRole("button", { name: "App-Mikrofon einschalten" }),
  ).toBeTruthy();
});

it("keeps Stop available during a pending reconnect and treats the cancelled reconnect as no device error", async () => {
  const f = fixture();
  const state = {
    ...f.state(),
    microphoneReconnectAvailable: true,
    captureControl: {
      paused: false,
      microphoneMuted: false,
      hasMicrophone: false,
    },
  };
  f.setState(state);
  let fail!: (error: Error) => void;
  f.api.reconnectMicrophone = vi.fn(
    () =>
      new Promise<AppState>((_, reject) => {
        fail = reject;
      }),
  );
  const stopped = { ...original, status: "complete" as const };
  f.api.stopMeeting = vi.fn(async () => {
    // The native stop cancels the reconnect before it closes the recording.
    fail(
      new Error(
        "Mikrofon konnte nicht verbunden werden. (The recording is stopping)",
      ),
    );
    f.setState({
      ...state,
      recording: false,
      activeMeeting: null,
      captureControl: null,
    });
    return stopped;
  });
  render(<App api={f.api} />);
  await userEvent.click(
    await screen.findByRole("button", { name: "Mikrofon erneut verbinden" }),
  );
  // The native side reports its transition while the device starts.
  f.setState({ ...state, transitioning: true });
  const stop = screen.getByRole("button", { name: "Meeting beenden" });
  expect((stop as HTMLButtonElement).disabled).toBe(false);
  await userEvent.click(stop);
  expect(f.api.stopMeeting).toHaveBeenCalledOnce();
  await waitFor(() =>
    expect(
      screen.queryByRole("button", { name: "Meeting beenden" }),
    ).toBeNull(),
  );
  expect(
    screen.queryByText(/Mikrofon konnte nicht verbunden werden/),
  ).toBeNull();
});

it("sends Stop during a reconnect even while an unrelated history refresh is pending", async () => {
  const f = fixture();
  const state = {
    ...f.state(),
    microphoneReconnectAvailable: true,
    captureControl: {
      paused: false,
      microphoneMuted: false,
      hasMicrophone: false,
    },
  };
  f.setState(state);
  f.api.reconnectMicrophone = vi.fn(
    () => new Promise<AppState>(() => undefined),
  );
  render(<App api={f.api} />);
  await userEvent.click(
    await screen.findByRole("button", { name: "Mikrofon erneut verbinden" }),
  );
  // Opening the history starts a foreground refresh that never finishes here.
  vi.mocked(f.api.listMeetings).mockImplementation(
    () => new Promise(() => undefined),
  );
  await userEvent.click(screen.getByRole("button", { name: "Historie" }));
  await userEvent.click(screen.getByRole("button", { name: "Meeting" }));
  f.api.stopMeeting = vi
    .fn()
    .mockResolvedValue({ ...original, status: "complete" });
  const stop = await screen.findByRole("button", { name: "Meeting beenden" });
  expect((stop as HTMLButtonElement).disabled).toBe(false);
  await userEvent.click(stop);
  expect(f.api.stopMeeting).toHaveBeenCalledOnce();
});

it("ignores a reconnect answer that arrives after Stop closed the recording", async () => {
  const f = fixture();
  const state = {
    ...f.state(),
    microphoneReconnectAvailable: true,
    captureControl: {
      paused: false,
      microphoneMuted: false,
      hasMicrophone: false,
    },
  };
  f.setState(state);
  let acknowledge!: (value: AppState) => void;
  f.api.reconnectMicrophone = vi.fn(
    () =>
      new Promise<AppState>((resolve) => {
        acknowledge = resolve;
      }),
  );
  const ended: AppState = {
    ...state,
    recording: false,
    activeMeeting: null,
    captureControl: null,
  };
  f.api.stopMeeting = vi.fn(async () => {
    f.setState(ended);
    return { ...original, status: "complete" as const };
  });
  render(<App api={f.api} />);
  await userEvent.click(
    await screen.findByRole("button", { name: "Mikrofon erneut verbinden" }),
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Meeting beenden" }),
  );
  await waitFor(() =>
    expect(
      screen.queryByRole("button", { name: "Meeting beenden" }),
    ).toBeNull(),
  );
  // The reconnect snapshot was taken while the recording still ran.
  await act(async () =>
    acknowledge({
      ...state,
      captureControl: { ...state.captureControl, hasMicrophone: true },
    }),
  );
  expect(screen.queryByRole("button", { name: "Meeting beenden" })).toBeNull();
});

it("waits for native pause acknowledgement, binds it to the current session and retains transcript text", async () => {
  const f = fixture();
  let acknowledge!: (value: AppState) => void;
  vi.mocked(f.api.setCapturePaused!).mockImplementation(
    () =>
      new Promise((resolve) => {
        acknowledge = resolve;
      }),
  );
  render(<App api={f.api} />);
  const pause = await screen.findByRole("button", {
    name: "Aufnahme pausieren",
  });
  await userEvent.click(pause);
  expect(f.api.setCapturePaused).toHaveBeenCalledWith(original.id, true);
  expect(pause.getAttribute("aria-pressed")).toBe("false");
  expect((pause as HTMLButtonElement).disabled).toBe(true);
  expect(
    screen.queryByRole("button", { name: "Aufnahme fortsetzen" }),
  ).toBeNull();
  const next = {
    ...f.state(),
    captureControl: {
      paused: true,
      microphoneMuted: false,
      hasMicrophone: true,
    },
  };
  f.setState(next);
  await act(async () => acknowledge(next));
  await screen.findByRole("button", { name: "Aufnahme fortsetzen" });
  expect(screen.getByText(/Gesprächston wird nicht gespeichert/)).toBeTruthy();
  expect(
    screen
      .getByRole("button", { name: "Aufnahme fortsetzen" })
      .getAttribute("title"),
  ).toMatch(/Je nach Plattform können Geräte geöffnet bleiben/);
  expect(screen.getByText("Unmodified original.")).toBeTruthy();
});

it("does not claim a microphone mute when the native control fails", async () => {
  const f = fixture();
  vi.mocked(f.api.setMicrophoneMuted!).mockRejectedValue(
    "Synthetic control write failed; audio retained",
  );
  render(<App api={f.api} />);
  await userEvent.click(
    await screen.findByRole("button", { name: "App-Mikrofon stummschalten" }),
  );
  await screen.findByRole("alert");
  expect(f.api.setMicrophoneMuted).toHaveBeenCalledWith(original.id, true);
  expect(
    screen
      .getByRole("button", { name: "App-Mikrofon stummschalten" })
      .getAttribute("aria-pressed"),
  ).toBe("false");
  expect(screen.queryByText(/App-Mikrofon stumm\./)).toBeNull();
  expect(original.parts[0].text).toBe("Unmodified original.");
});

it("requires revoking an agent-owned capture before offering manual controls", async () => {
  const f = fixture();
  f.setState({
    ...f.state(),
    agent: {
      connected: true,
      label: "Synthetic agent",
      ownerId: "agent-owner",
      revoked: false,
      pending: null,
      ownedSessionId: original.id,
      activeCapture: true,
      expiresAtUnixMs: Date.now() + 60000,
    },
  });
  render(<App api={f.api} />);
  const pause = await screen.findByRole("button", {
    name: "Aufnahme pausieren",
  });
  expect((pause as HTMLButtonElement).disabled).toBe(true);
  expect(
    (
      screen.getByRole("button", {
        name: "App-Mikrofon stummschalten",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(screen.getByText(/Entziehe zuerst die Agentenfreigabe/)).toBeTruthy();
  await userEvent.click(pause);
  expect(f.api.setCapturePaused).not.toHaveBeenCalled();
  expect(f.api.setMicrophoneMuted).not.toHaveBeenCalled();
});

it("does not offer app mute without a microphone track and explains that meeting apps are unaffected", () => {
  render(
    <RecordingControls
      control={{ paused: true, microphoneMuted: false, hasMicrophone: false }}
      locale="en"
      disabled={false}
      onPause={vi.fn()}
      onMute={vi.fn()}
    />,
  );
  expect(screen.queryByRole("button", { name: /microphone/i })).toBeNull();
  const resume = screen.getByRole("button", { name: "Resume recording" });
  // The explanation is a hover tooltip and a collapsed disclosure, not a permanent paragraph.
  expect(resume.getAttribute("title")).toMatch(
    /This does not mute Teams, Zoom/,
  );
  const help = screen
    .getByText("What do pause and mute do?")
    .closest("details");
  expect(help?.open).toBe(false);
  expect(help?.textContent).toMatch(/This does not mute Teams, Zoom/);
  expect(resume.getAttribute("aria-pressed")).toBe("true");
});

it("uses native file choice, preserves cancellation and never turns an edited displayed path into permission", async () => {
  const f = fixture(false);
  render(<App api={f.api} />);
  await screen.findByRole("button", { name: "Aufnahme starten" });
  await userEvent.click(screen.getByRole("button", { name: "Einstellungen" }));
  const path = screen.getByLabelText(
    "Handy-Programmdatei (optional)",
  ) as HTMLInputElement;
  expect(path.readOnly).toBe(true);
  fireEvent.change(path, { target: { value: "/unapproved/Handy.exe" } });
  await userEvent.click(
    screen.getByRole("button", { name: "Handy-Programm auswählen …" }),
  );
  expect(f.api.chooseHandyExecutable).toHaveBeenCalledOnce();
  expect(f.api.refreshHandy).not.toHaveBeenCalled();
  vi.mocked(f.api.chooseHandyExecutable!).mockResolvedValue(
    "/native-selected/Handy.exe",
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Handy-Programm auswählen …" }),
  );
  await waitFor(() =>
    expect(f.api.refreshHandy).toHaveBeenCalledWith(
      "/native-selected/Handy.exe",
    ),
  );
  expect(f.api.refreshHandy).not.toHaveBeenCalledWith("/unapproved/Handy.exe");
  await userEvent.click(
    screen.getByRole("button", {
      name: "Sitzungsdatei auswählen und nur lesbar öffnen …",
    }),
  );
  expect(f.api.importLegacy).not.toHaveBeenCalled();
  vi.mocked(f.api.chooseLegacyManifest!).mockResolvedValue(
    "/native-selected/session.json",
  );
  await userEvent.click(
    screen.getByRole("button", {
      name: "Sitzungsdatei auswählen und nur lesbar öffnen …",
    }),
  );
  await waitFor(() =>
    expect(f.api.importLegacy).toHaveBeenCalledWith(
      "/native-selected/session.json",
    ),
  );
});
