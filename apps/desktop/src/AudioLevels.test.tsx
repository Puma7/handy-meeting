import {
  act,
  fireEvent,
  render,
  renderHook,
  screen,
} from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { de, extraDe } from "./i18n";
import type { AudioLevel, AudioLevels, Meeting } from "./types";
import { useAudioLevels } from "./useAudioLevels";
import AudioLevelMeter from "./views/AudioLevelMeter";
import AudioSources from "./views/AudioSources";
import MeetingView from "./views/MeetingView";

// Synthetic signal/IPC fixtures only: no devices, audio files or real recordings are opened.
function active(peakDbfs = -24): AudioLevel {
  return {
    status: "active",
    peakDbfs,
    rmsDbfs: peakDbfs - 3,
    clipped: false,
    ageMs: 0,
  };
}
function levels(meetingId = "capture-a", peak = -24): AudioLevels {
  return { meetingId, system: active(peak), microphone: active(-36) };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
const meeting: Meeting = {
  id: "capture-a",
  title: "Synthetic capture",
  startedAt: "2026-10-28T08:00:00Z",
  durationMs: 1000,
  status: "recording",
  modelId: "fixture",
  parts: [],
  warnings: [],
};
function sourceProps(): ComponentProps<typeof AudioSources> {
  return {
    t: de,
    locale: "de",
    devices: [
      { id: "output", name: "Synthetic output", source: "system" },
      { id: "input", name: "Synthetic microphone", source: "microphone" },
    ],
    systemId: "output",
    micId: "input",
    micEnabled: true,
    recording: true,
    busy: false,
    connected: true,
    isStopping: false,
    activeMeetingId: meeting.id,
    displayedMeetingId: meeting.id,
    captureControl: {
      paused: false,
      microphoneMuted: false,
      hasMicrophone: true,
    },
    getAudioLevels: vi.fn().mockResolvedValue(levels()),
    setSystemId: vi.fn(),
    setMicId: vi.fn(),
    setMicEnabled: vi.fn(),
  };
}
async function advance(milliseconds: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(milliseconds);
  });
}
beforeEach(() => {
  vi.useFakeTimers();
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

it("lights green, yellow and red from real peak values and adds an explicit clipping label", () => {
  const props = {
    label: "Computerton",
    locale: "de" as const,
    level: active(-36),
  };
  const view = render(<AudioLevelMeter {...props} />);
  const lit = (zone: string) =>
    view.container.querySelectorAll(`.${zone}[data-lit]`).length;
  expect(screen.getByRole("meter").getAttribute("aria-valuenow")).toBe("-36");
  expect(lit("green")).toBeGreaterThan(0);
  expect(lit("yellow")).toBe(0);
  expect(lit("red")).toBe(0);
  view.rerender(<AudioLevelMeter {...props} level={active(-12)} />);
  expect(lit("yellow")).toBeGreaterThan(0);
  expect(lit("red")).toBe(0);
  view.rerender(<AudioLevelMeter {...props} level={active(-2)} />);
  expect(lit("red")).toBeGreaterThan(0);
  expect(screen.queryByText("Übersteuert")).toBeNull();
  view.rerender(
    <AudioLevelMeter {...props} level={{ ...active(0), clipped: true }} />,
  );
  expect(screen.getByText("Übersteuert")).toBeTruthy();
  expect(screen.getByRole("meter").getAttribute("aria-valuetext")).toContain(
    "Übersteuert",
  );
  view.rerender(<AudioLevelMeter {...props} state="muted" />);
  expect(screen.queryByRole("meter")).toBeNull();
  expect(view.container.querySelectorAll("[data-lit]").length).toBe(0);
  expect(screen.queryByText("Übersteuert")).toBeNull();
  expect(screen.getByText("In dieser App stumm")).toBeTruthy();
});

it("lets the bar fall gradually after a short pause instead of dropping to zero", () => {
  vi.useFakeTimers();
  try {
    const props = { label: "Microphone", locale: "en" as const };
    const view = render(<AudioLevelMeter {...props} level={active(-12)} />);
    const lit = () => view.container.querySelectorAll("[data-lit]").length;
    const loud = lit();
    // A breath 100 ms later: the measured value is shown, the bar only falls about 2.4 dB.
    vi.advanceTimersByTime(100);
    view.rerender(
      <AudioLevelMeter {...props} level={{ ...active(-96), rmsDbfs: -96 }} />,
    );
    expect(screen.getByRole("meter").getAttribute("aria-valuenow")).toBe("-96");
    expect(lit()).toBeGreaterThanOrEqual(loud - 1);
    expect(screen.queryByText("Silence")).toBeNull();
    // Lasting silence is shown as silence after the bar has fallen.
    vi.advanceTimersByTime(5000);
    view.rerender(
      <AudioLevelMeter {...props} level={{ ...active(-96), rmsDbfs: -96 }} />,
    );
    expect(lit()).toBe(0);
    expect(screen.getByText("Silence")).toBeTruthy();
  } finally {
    vi.useRealTimers();
  }
});

it("distinguishes measured silence from missing samples and still indicates a very quiet signal", () => {
  const props = { label: "Microphone", locale: "en" as const };
  const view = render(
    <AudioLevelMeter {...props} level={{ ...active(-96), rmsDbfs: -96 }} />,
  );
  expect(screen.getByText("Silence")).toBeTruthy();
  expect(screen.getByRole("meter").getAttribute("aria-valuenow")).toBe("-96");
  expect(view.container.querySelectorAll("[data-lit]").length).toBe(0);
  view.rerender(<AudioLevelMeter {...props} level={active(-70)} />);
  expect(screen.getByText("Very quiet signal")).toBeTruthy();
  expect(view.container.querySelectorAll("[data-lit]").length).toBe(1);
  view.rerender(<AudioLevelMeter {...props} level={null} />);
  expect(screen.getByText("Waiting for audio samples")).toBeTruthy();
  expect(screen.queryByRole("meter")).toBeNull();
  expect(view.container.querySelectorAll("[data-lit]").length).toBe(0);
});

it("does not poll or claim a signal before capture starts, for a different displayed meeting, or after stop", async () => {
  const props = sourceProps();
  const view = render(
    <AudioSources
      {...props}
      recording={false}
      activeMeetingId={null}
      displayedMeetingId={null}
    />,
  );
  await advance(1200);
  expect(props.getAudioLevels).not.toHaveBeenCalled();
  expect(screen.getAllByText("Bei Aufnahmebeginn verfügbar").length).toBe(2);
  view.rerender(<AudioSources {...props} displayedMeetingId="old-meeting" />);
  await advance(1200);
  expect(props.getAudioLevels).not.toHaveBeenCalled();
  expect(screen.queryByRole("meter")).toBeNull();
  view.rerender(<AudioSources {...props} />);
  await advance(0);
  expect(screen.getAllByRole("meter").length).toBe(2);
  view.rerender(<AudioSources {...props} recording={false} />);
  const calls = vi.mocked(props.getAudioLevels!).mock.calls.length;
  await advance(1200);
  expect(props.getAudioLevels).toHaveBeenCalledTimes(calls);
  expect(screen.queryByRole("meter")).toBeNull();
  expect(screen.getAllByText("Aufnahme beendet").length).toBe(2);
});

it("polls only the lightweight command and clears a previously valid signal when a reply fails", async () => {
  const props = sourceProps();
  const pending = deferred<AudioLevels | null>();
  vi.mocked(props.getAudioLevels!)
    .mockResolvedValueOnce(levels())
    .mockImplementation(() => pending.promise);
  const view = render(<AudioSources {...props} />);
  await advance(0);
  expect(screen.getAllByRole("meter").length).toBe(2);
  await advance(150);
  await act(async () => pending.reject(new Error("Synthetic IPC failure")));
  expect(screen.queryByRole("meter")).toBeNull();
  expect(screen.getAllByText("Kein aktueller Pegel").length).toBe(2);
  expect(view.container.querySelectorAll("[data-lit]").length).toBe(0);
  expect(props.setMicEnabled).not.toHaveBeenCalled();
});

it("expires a hung reply locally, avoids overlapping polls and rejects an old delayed signal", async () => {
  const pending = deferred<AudioLevels | null>();
  const getLevels = vi.fn().mockReturnValue(pending.promise);
  const { result } = renderHook(() =>
    useAudioLevels(getLevels, meeting.id, true),
  );
  await advance(1200);
  expect(getLevels).toHaveBeenCalledTimes(1);
  expect(result.current.system?.status).toBe("stale");
  await act(async () => pending.resolve(levels()));
  expect(result.current.system?.status).toBe("stale");
  expect(result.current.system?.peakDbfs).toBeNull();
});

it("expires samples by native sample age even when IPC replies keep arriving", async () => {
  const value = levels();
  value.system.ageMs = 950;
  const pending = deferred<AudioLevels | null>();
  const getLevels = vi
    .fn()
    .mockResolvedValueOnce(value)
    .mockReturnValue(pending.promise);
  const { result } = renderHook(() =>
    useAudioLevels(getLevels, meeting.id, true),
  );
  await advance(0);
  expect(result.current.system?.status).toBe("active");
  await advance(150);
  expect(result.current.system?.status).toBe("stale");
  expect(result.current.microphone?.status).toBe("active");
});

it("keeps a healthy source live while separately reporting missing, stale or unavailable samples on the other source", async () => {
  const props = sourceProps();
  const snapshot = levels();
  snapshot.microphone = {
    status: "waiting",
    peakDbfs: null,
    rmsDbfs: null,
    clipped: false,
    ageMs: null,
  };
  vi.mocked(props.getAudioLevels!).mockImplementation(async () =>
    structuredClone(snapshot),
  );
  const view = render(<AudioSources {...props} />);
  await advance(0);
  expect(screen.getAllByRole("meter").length).toBe(1);
  expect(screen.getByText("Warte auf Audiosignal")).toBeTruthy();
  snapshot.microphone = {
    ...snapshot.microphone,
    status: "stale",
    ageMs: 1200,
  };
  await advance(150);
  expect(screen.getAllByRole("meter").length).toBe(1);
  expect(screen.getByText("Kein aktueller Pegel")).toBeTruthy();
  snapshot.microphone = { ...snapshot.microphone, status: "unavailable" };
  await advance(150);
  expect(screen.getAllByRole("meter").length).toBe(1);
  expect(screen.getByText("Nicht verfügbar")).toBeTruthy();
  expect(
    view.container
      .querySelectorAll(".audio-source")[1]
      .querySelectorAll("[data-lit]").length,
  ).toBe(0);
});

it("ignores late old-session replies, keeps one request in flight and binds the next reply to the new session", async () => {
  const pending = deferred<AudioLevels | null>();
  const getLevels = vi
    .fn()
    .mockReturnValueOnce(pending.promise)
    .mockResolvedValue(levels("capture-b", -6));
  const { result, rerender } = renderHook(
    ({ id }) => useAudioLevels(getLevels, id, true),
    {
      initialProps: { id: meeting.id },
    },
  );
  rerender({ id: "capture-b" });
  await advance(450);
  expect(getLevels).toHaveBeenCalledTimes(1);
  expect(result.current.system).toBeNull();
  await act(async () => pending.resolve(levels()));
  expect(result.current.system).toBeNull();
  await advance(150);
  expect(getLevels).toHaveBeenCalledTimes(2);
  expect(result.current.system?.peakDbfs).toBe(-6);
});

it("rejects a mismatched session, non-finite signal, and missing active samples instead of displaying evidence of audio", async () => {
  const malformed = levels();
  malformed.system.peakDbfs = Number.NaN;
  const missing = levels();
  missing.system.rmsDbfs = null;
  const getLevels = vi
    .fn()
    .mockResolvedValueOnce(levels("other-session"))
    .mockResolvedValueOnce(malformed)
    .mockResolvedValueOnce(missing);
  const { result } = renderHook(() =>
    useAudioLevels(getLevels, meeting.id, true),
  );
  for (const wait of [0, 150, 150]) {
    await advance(wait);
    expect(result.current.system?.status).toBe("stale");
    expect(result.current.system?.peakDbfs).toBeNull();
  }
});

it("clears the meters immediately for acknowledged pause/mute and stopping, without changing controls", async () => {
  const props = sourceProps();
  const view = render(<AudioSources {...props} />);
  await advance(0);
  view.rerender(
    <AudioSources
      {...props}
      captureControl={{
        paused: false,
        microphoneMuted: true,
        hasMicrophone: true,
      }}
    />,
  );
  expect(screen.getAllByRole("meter").length).toBe(1);
  expect(screen.getByText("In dieser App stumm")).toBeTruthy();
  view.rerender(
    <AudioSources
      {...props}
      captureControl={{
        paused: true,
        microphoneMuted: false,
        hasMicrophone: true,
      }}
    />,
  );
  expect(screen.queryByRole("meter")).toBeNull();
  expect(screen.getAllByText("Pausiert").length).toBe(2);
  view.rerender(<AudioSources {...props} isStopping />);
  expect(screen.queryByRole("meter")).toBeNull();
  expect(props.setMicEnabled).not.toHaveBeenCalled();
});

it("does not relight cached pre-pause samples after a rapid pause/resume before the next poll", async () => {
  const props = sourceProps();
  const view = render(<AudioSources {...props} />);
  await advance(0);
  expect(screen.getAllByRole("meter").length).toBe(2);
  view.rerender(
    <AudioSources
      {...props}
      captureControl={{
        paused: true,
        microphoneMuted: false,
        hasMicrophone: true,
      }}
    />,
  );
  view.rerender(<AudioSources {...props} />);
  expect(props.getAudioLevels).toHaveBeenCalledTimes(1);
  expect(screen.queryAllByRole("meter").length).toBe(0);
  await advance(150);
  expect(screen.getAllByRole("meter").length).toBe(2);
});

it("invalidates cached microphone samples on rapid mute/unmute while keeping the healthy system meter", async () => {
  const props = sourceProps();
  const view = render(<AudioSources {...props} />);
  await advance(0);
  const system = screen.getByRole("meter", { name: /Computerton/ });
  view.rerender(
    <AudioSources
      {...props}
      captureControl={{
        paused: false,
        microphoneMuted: true,
        hasMicrophone: true,
      }}
    />,
  );
  view.rerender(<AudioSources {...props} />);
  expect(props.getAudioLevels).toHaveBeenCalledTimes(1);
  expect(screen.getAllByRole("meter").length).toBe(1);
  expect(screen.getByRole("meter", { name: /Computerton/ })).toBe(system);
  expect(screen.queryByRole("meter", { name: /Mikrofon/ })).toBeNull();
  await advance(150);
  expect(screen.getAllByRole("meter").length).toBe(2);
});

it.each(["pause", "mute"] as const)(
  "ignores pre-control %s replies settling after reopening and retains single-flight polling",
  async (gate) => {
    const props = sourceProps();
    const pending = deferred<AudioLevels | null>();
    const fresh = levels();
    fresh.system = active(-6);
    fresh.microphone = active(-12);
    vi.mocked(props.getAudioLevels!)
      .mockReturnValueOnce(pending.promise)
      .mockResolvedValue(fresh);
    const view = render(<AudioSources {...props} />);
    view.rerender(
      <AudioSources
        {...props}
        captureControl={{
          paused: gate === "pause",
          microphoneMuted: gate === "mute",
          hasMicrophone: true,
        }}
      />,
    );
    view.rerender(<AudioSources {...props} />);
    await advance(300);
    expect(props.getAudioLevels).toHaveBeenCalledTimes(1);
    await act(async () => pending.resolve(levels()));
    expect(screen.queryAllByRole("meter").length).toBe(
      gate === "pause" ? 0 : 1,
    );
    expect(screen.queryByRole("meter", { name: /Mikrofon/ })).toBeNull();
    await advance(150);
    expect(props.getAudioLevels).toHaveBeenCalledTimes(2);
    expect(
      screen
        .getByRole("meter", { name: /Computerton/ })
        .getAttribute("aria-valuenow"),
    ).toBe("-6");
    expect(
      screen
        .getByRole("meter", { name: /Mikrofon/ })
        .getAttribute("aria-valuenow"),
    ).toBe("-12");
  },
);

it("stops polling while the document is hidden and after unmount, and ignores a pending reply", async () => {
  const pending = deferred<AudioLevels | null>();
  const getLevels = vi
    .fn()
    .mockResolvedValueOnce(levels())
    .mockReturnValue(pending.promise);
  const { result, unmount } = renderHook(() =>
    useAudioLevels(getLevels, meeting.id, true),
  );
  await advance(150);
  expect(getLevels).toHaveBeenCalledTimes(2);
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
  act(() => {
    fireEvent(document, new Event("visibilitychange"));
  });
  expect(result.current.system).toBeNull();
  await advance(1200);
  expect(getLevels).toHaveBeenCalledTimes(2);
  unmount();
  await act(async () => pending.resolve(levels()));
  await advance(1200);
  expect(getLevels).toHaveBeenCalledTimes(2);
});

it("keeps exactly one meter beneath each source selector when recording setup is collapsed", async () => {
  const sources = sourceProps();
  const props: ComponentProps<typeof MeetingView> = {
    ...sources,
    t: de,
    x: extraDe,
    app: {
      version: "synthetic",
      handy: null,
      recording: true,
      activeMeeting: meeting,
      captureControl: sources.captureControl,
      agent: {
        connected: true,
        label: "Fixture",
        ownerId: "synthetic",
        revoked: false,
        pending: null,
        ownedSessionId: meeting.id,
        expiresAtUnixMs: null,
        activeCapture: true,
      },
    },
    meeting,
    title: meeting.title,
    defaultTitle: "Synthetic",
    onAnalyse: vi.fn(),
    onOpenFolder: vi.fn(),
    onExport: vi.fn(),
    onPause: vi.fn(),
    onMute: vi.fn(),
    controlsAvailable: true,
    savedFile: null,
    modelId: "fixture",
    models: [],
    copied: false,
    backlog: null,
    recentParts: [],
    setTitle: vi.fn(),
    setModelId: vi.fn(),
    refreshHandy: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
    savedByTrack: () => 0,
    resume: vi.fn(),
    copyFullTranscript: vi.fn(),
  };
  const view = render(<MeetingView {...props} />);
  await advance(0);
  expect(
    (
      view.container.querySelector(
        "details.setup-details",
      ) as HTMLDetailsElement
    ).open,
  ).toBe(false);
  const meters = screen.getAllByRole("meter");
  expect(meters.length).toBe(2);
  for (const meter of meters) {
    expect(meter.closest("details")).toBeNull();
    const source = meter.closest(".audio-source")!;
    expect(source.querySelector("select")).toBeNull();
    expect(source.querySelector("label")?.textContent).toBeTruthy();
    expect(source.querySelectorAll("[role=meter]").length).toBe(1);
  }
  // Viewing an agent-owned signal grants no ability to use the existing controls.
  expect(
    (
      screen.getByRole("button", {
        name: "Aufnahme pausieren",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(props.onPause).not.toHaveBeenCalled();
  expect(props.onMute).not.toHaveBeenCalled();
});

it.each([true, false])(
  "shows acknowledged native microphone inclusion (%s) during agent capture while preserving the idle preference",
  async (hasMicrophone) => {
    const props = sourceProps();
    const idlePreference = !hasMicrophone;
    const control = { paused: false, microphoneMuted: false, hasMicrophone };
    const view = render(
      <AudioSources
        {...props}
        micEnabled={idlePreference}
        captureControl={control}
      />,
    );
    await advance(0);
    const checkbox = screen.getByRole("checkbox", {
      name: /Mikrofon/,
    }) as HTMLInputElement;
    expect(checkbox.checked).toBe(hasMicrophone);
    expect(checkbox.disabled).toBe(true);
    expect(screen.queryByRole("meter", { name: /Mikrofon/ }) !== null).toBe(
      hasMicrophone,
    );
    expect(props.setMicEnabled).not.toHaveBeenCalled();
    view.rerender(
      <AudioSources
        {...props}
        micEnabled={idlePreference}
        captureControl={null}
      />,
    );
    expect(checkbox.checked).toBe(idlePreference);
    expect(checkbox.disabled).toBe(true);
    view.rerender(
      <AudioSources
        {...props}
        micEnabled={idlePreference}
        recording={false}
        captureControl={null}
      />,
    );
    expect(checkbox.checked).toBe(idlePreference);
    expect(checkbox.disabled).toBe(false);
  },
);
