import { afterEach, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import EraseMeetingDialog from "./views/EraseMeetingDialog";
import HistorySearch from "./views/HistorySearch";
import App from "./App";
import PendingErasures from "./views/PendingErasures";
import { testServices } from "./test-services";
import { useNotesJob } from "./useNotesJob";
import type { DesktopApi, ErasurePreview, MeetingSummary } from "./types";

afterEach(cleanup);
afterEach(() => vi.useRealTimers());
const meeting: MeetingSummary = {
  id: "b".repeat(32),
  title: "Synthetic two-second meeting",
  startedAt: "2026-10-30T07:00:00Z",
  durationMs: 2000,
  status: "recorded",
  modelId: "synthetic",
};
const plan: ErasurePreview = {
  meetingId: meeting.id,
  fingerprint: "synthetic-file-inventory",
  files: 8,
  bytes: 128000,
  rebuildSearch: true,
};
function fixture() {
  let active = "preview-job";
  const api = {
    previewMeetingErasure: vi.fn().mockResolvedValue(active),
    getMeetingErasurePreview: vi.fn().mockResolvedValue(plan),
    eraseMeetingNow: vi.fn().mockImplementation(async () => {
      active = "erase-job";
      return active;
    }),
    getMaintenanceProgress: vi.fn().mockImplementation(async () => ({
      jobId: active,
      kind: "meeting-erasure",
      status: "complete",
      confirmedTasks: 8,
      totalTasks: 8,
      elapsedMs: 5,
      skipped: 0,
      error: null,
      cancelRequested: false,
    })),
    cancelMaintenance: vi.fn().mockResolvedValue(true),
  } as unknown as DesktopApi;
  return {
    api,
    onStart: vi.fn(),
    onDeleted: vi.fn(),
    onStopped: vi.fn(),
    onClose: vi.fn(),
  };
}

it("bounds failed deletion status polls, keeps admission and can inspect cancellation later", async () => {
  vi.useFakeTimers();
  const props = fixture();
  let deleting = false,
    recovered = false,
    terminalStatus = "unrecognized";
  props.api.eraseMeetingNow = vi.fn().mockImplementation(async () => {
    deleting = true;
    return "erase-job";
  });
  props.api.getMaintenanceProgress = vi.fn().mockImplementation(async () => {
    if (deleting && !recovered) throw new Error("Synthetic IPC read failure");
    return {
      jobId: deleting ? "erase-job" : "preview-job",
      status: deleting ? terminalStatus : "complete",
      confirmedTasks: 0,
      totalTasks: 8,
      elapsedMs: 0,
      skipped: 0,
      error: null,
      cancelRequested: deleting,
    };
  });
  render(<EraseMeetingDialog {...props} meeting={meeting} locale="en" />);
  await act(() => vi.advanceTimersByTimeAsync(0));
  fireEvent.change(
    screen.getByRole("textbox", { name: "Type DELETE to confirm" }),
    { target: { value: "DELETE" } },
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Permanently delete now" }),
  );
  await act(() => vi.advanceTimersByTimeAsync(10000));
  const calls = vi.mocked(props.api.getMaintenanceProgress!).mock.calls.length;
  expect(calls).toBe(4);
  expect(props.api.cancelMaintenance).toHaveBeenCalledWith("erase-job");
  await act(() => vi.advanceTimersByTimeAsync(10000));
  expect(props.api.getMaintenanceProgress).toHaveBeenCalledTimes(calls);
  expect(props.onStopped).not.toHaveBeenCalled();
  expect(props.onDeleted).not.toHaveBeenCalled();
  expect(props.api.eraseMeetingNow).toHaveBeenCalledOnce();
  expect(
    screen.queryByRole("button", { name: "Permanently delete now" }),
  ).toBeNull();
  recovered = true;
  screen.getByRole("button", { name: "Check job status again" }).focus();
  await act(async () =>
    fireEvent.click(
      screen.getByRole("button", { name: "Check job status again" }),
    ),
  );
  expect(screen.getByRole("alertdialog").contains(document.activeElement)).toBe(
    true,
  );
  await act(() => vi.advanceTimersByTimeAsync(10000));
  expect(props.onStopped).not.toHaveBeenCalled();
  terminalStatus = "cancelled";
  await act(async () =>
    fireEvent.click(
      screen.getByRole("button", { name: "Check job status again" }),
    ),
  );
  expect(props.onStopped).toHaveBeenCalledOnce();
  expect(props.onDeleted).not.toHaveBeenCalled();
});

it("discloses unattributable atomic remnants before confirming erasure", async () => {
  const props = fixture();
  props.api.getMeetingErasurePreview = vi
    .fn()
    .mockResolvedValue({ ...plan, preservedAtomicRemnants: 2 });
  render(<EraseMeetingDialog {...props} meeting={meeting} locale="en" />);
  expect(
    await screen.findByText(/2 interrupted records cannot be assigned/),
  ).toBeTruthy();
  expect(props.api.eraseMeetingNow).not.toHaveBeenCalled();
});

it("offers direct permanent deletion on the exact short meeting in searchable history", async () => {
  const remove = vi.fn();
  render(
    <HistorySearch
      api={
        {
          searchMeetings: vi.fn().mockResolvedValue({
            items: [
              {
                ...meeting,
                pinned: false,
                audioRemoved: false,
                metadataRevision: 0,
                sourceId: null,
                startMs: null,
                endMs: null,
                excerpt: "",
              },
            ],
            offset: 0,
            total: 1,
            limit: 25,
          }),
        } as unknown as DesktopApi
      }
      locale="de"
      onOpen={vi.fn()}
      onErase={remove}
    />,
  );
  await screen.findByText(meeting.title);
  await userEvent.click(
    screen.getByRole("button", {
      name: `Meeting dauerhaft löschen: ${meeting.title}`,
    }),
  );
  expect(remove).toHaveBeenCalledWith(
    expect.objectContaining({ id: meeting.id, durationMs: 2000 }),
  );
});

it("requires the exact confirmation word and binds the call to the inspected meeting", async () => {
  const props = fixture();
  render(<EraseMeetingDialog {...props} meeting={meeting} locale="de" />);
  const input = await screen.findByRole("textbox", {
    name: "Zur Bestätigung LÖSCHEN eingeben",
  });
  const button = screen.getByRole("button", {
    name: "Jetzt endgültig löschen",
  });
  expect((button as HTMLButtonElement).disabled).toBe(true);
  await userEvent.type(input, "ja");
  await userEvent.type(input, "{enter}");
  expect(props.api.eraseMeetingNow).not.toHaveBeenCalled();
  await userEvent.clear(input);
  await userEvent.type(input, "LÖSCHEN");
  await userEvent.click(button);
  await waitFor(() => expect(props.onDeleted).toHaveBeenCalledOnce());
  expect(props.api.eraseMeetingNow).toHaveBeenCalledWith(
    "preview-job",
    meeting.id,
    "LÖSCHEN",
  );
  expect(props.onStart).toHaveBeenCalledOnce();
});

it("does not erase on preview cancellation and cancels a job ID that arrives late", async () => {
  const props = fixture();
  let resolve!: (id: string) => void;
  props.api.previewMeetingErasure = vi.fn().mockImplementation(
    () =>
      new Promise<string>((done) => {
        resolve = done;
      }),
  );
  render(<EraseMeetingDialog {...props} meeting={meeting} locale="de" />);
  await waitFor(() =>
    expect(props.api.previewMeetingErasure).toHaveBeenCalledOnce(),
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Prüfung abbrechen" }),
  );
  await act(async () => resolve("late-preview-job"));
  await waitFor(() =>
    expect(props.api.cancelMaintenance).toHaveBeenCalledWith(
      "late-preview-job",
    ),
  );
  expect(props.api.eraseMeetingNow).not.toHaveBeenCalled();
  expect(props.onDeleted).not.toHaveBeenCalled();
});

it("rejects a preview for another meeting rather than reusing its permission", async () => {
  const props = fixture();
  props.api.getMeetingErasurePreview = vi
    .fn()
    .mockResolvedValue({ ...plan, meetingId: "c".repeat(32) });
  render(<EraseMeetingDialog {...props} meeting={meeting} locale="de" />);
  await screen.findByRole("alert");
  expect(
    screen.queryByRole("button", { name: "Jetzt endgültig löschen" }),
  ).toBeNull();
  expect(props.api.eraseMeetingNow).not.toHaveBeenCalled();
});

it("keeps cancellation and partial failure explicit and never reports failed deletion as complete", async () => {
  const props = fixture();
  props.api.getMaintenanceProgress = vi.fn().mockImplementation(async () => ({
    jobId:
      props.api.eraseMeetingNow &&
      vi.mocked(props.api.eraseMeetingNow).mock.calls.length
        ? "erase-job"
        : "preview-job",
    status:
      props.api.eraseMeetingNow &&
      vi.mocked(props.api.eraseMeetingNow).mock.calls.length
        ? "failed"
        : "complete",
    confirmedTasks: 1,
    totalTasks: 8,
    error: "Synthetic locked file; remaining data retained",
  }));
  render(<EraseMeetingDialog {...props} meeting={meeting} locale="en" />);
  await userEvent.type(
    await screen.findByRole("textbox", { name: "Type DELETE to confirm" }),
    "DELETE",
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Permanently delete now" }),
  );
  await screen.findByText("Synthetic locked file; remaining data retained");
  expect(props.onDeleted).not.toHaveBeenCalled();
  expect(props.onStopped).toHaveBeenCalledOnce();
  expect(screen.getByText(/Files already removed stay deleted/)).toBeTruthy();
});

it("starts focus on the safe control, traps keyboard focus and Escape never confirms deletion", async () => {
  const props = fixture();
  render(<EraseMeetingDialog {...props} meeting={meeting} locale="en" />);
  const input = await screen.findByRole("textbox", {
    name: "Type DELETE to confirm",
  });
  const close = screen.getByRole("button", { name: "Close" });
  expect(document.activeElement).toBe(close);
  await userEvent.type(input, "DELETE");
  const remove = screen.getByRole("button", { name: "Permanently delete now" });
  remove.focus();
  await userEvent.tab();
  expect(document.activeElement).toBe(input);
  await userEvent.keyboard("{Escape}");
  expect(props.onClose).toHaveBeenCalledOnce();
  expect(props.api.eraseMeetingNow).not.toHaveBeenCalled();
});

it("can resume an interrupted erasure even when the original meeting title is already gone", async () => {
  const resume = vi.fn();
  render(
    <PendingErasures
      api={
        {
          listPendingErasures: vi
            .fn()
            .mockResolvedValue([
              { meetingId: meeting.id, confirmedAt: meeting.startedAt },
            ]),
        } as unknown as DesktopApi
      }
      locale="de"
      revision={0}
      onResume={resume}
    />,
  );
  await userEvent.click(
    await screen.findByRole("button", { name: "Prüfen und fortsetzen" }),
  );
  expect(resume).toHaveBeenCalledWith(
    expect.objectContaining({ id: meeting.id, status: "erasure-pending" }),
  );
});

it("clears the deleted meeting and ignores a native state reply that was requested before erasure", async () => {
  localStorage.clear();
  const full = { ...meeting, parts: [], warnings: [] };
  const other = {
    ...meeting,
    id: "c".repeat(32),
    title: "Other meeting retained",
  };
  let erased = false,
    activeJob = "preview-job";
  let deletionCanComplete = false;
  let stale!: (value: unknown) => void;
  let stateCalls = 0;
  const initial = {
    version: "synthetic",
    handy: null,
    recording: false,
    activeMeeting: full,
  };
  const api = {
    ...testServices(),
    getAppState: vi.fn().mockImplementation(() => {
      stateCalls++;
      if (stateCalls === 2)
        return new Promise((done) => {
          stale = done;
        });
      return Promise.resolve(
        erased ? { ...initial, activeMeeting: null } : initial,
      );
    }),
    listDevices: vi.fn().mockResolvedValue([]),
    listMeetings: vi
      .fn()
      .mockImplementation(async () => (erased ? [other] : [meeting, other])),
    getMeeting: vi.fn().mockResolvedValue(full),
    listAnalyses: vi.fn().mockResolvedValue({ analyses: [], skipped: [] }),
    getNotesProgress: vi.fn().mockResolvedValue(null),
    previewMeetingErasure: vi.fn().mockResolvedValue("preview-job"),
    getMeetingErasurePreview: vi.fn().mockResolvedValue(plan),
    eraseMeetingNow: vi.fn().mockImplementation(async () => {
      erased = true;
      activeJob = "erase-job";
      return activeJob;
    }),
    getMaintenanceProgress: vi.fn().mockImplementation(async () => ({
      jobId: activeJob,
      status:
        activeJob === "erase-job" && !deletionCanComplete
          ? "running"
          : "complete",
      confirmedTasks: 8,
      totalTasks: 8,
      error: null,
    })),
    cancelMaintenance: vi.fn().mockResolvedValue(true),
    listPendingErasures: vi.fn().mockResolvedValue([]),
    rebuildSearchIndex: vi.fn().mockResolvedValue("index-job"),
  } as unknown as DesktopApi;
  render(<App api={api} />);
  await waitFor(() => expect(stateCalls).toBeGreaterThanOrEqual(2));
  await userEvent.click(screen.getByRole("button", { name: /Historie/ }));
  const row = (await screen.findByText(meeting.title)).closest(".session-row")!;
  await userEvent.click(
    within(row as HTMLElement).getByRole("button", { name: "Öffnen" }),
  );
  await userEvent.click(
    await screen.findByRole("button", { name: "Meeting löschen" }),
  );
  await userEvent.type(
    await screen.findByRole("textbox", {
      name: "Zur Bestätigung LÖSCHEN eingeben",
    }),
    "LÖSCHEN",
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Jetzt endgültig löschen" }),
  );
  const dialog = screen.getByRole("alertdialog");
  expect(dialog.contains(document.activeElement)).toBe(true);
  expect((document.querySelector("main") as HTMLElement).inert).toBe(true);
  await userEvent.tab();
  expect(dialog.contains(document.activeElement)).toBe(true);
  deletionCanComplete = true;
  await screen.findByText(
    "Meetinginhalte vollständig aus der HandyMeeting-Ablage entfernt.",
  );
  await act(async () => stale(initial));
  expect(screen.queryByText(meeting.title)).toBeNull();
  expect(screen.getByText(other.title)).toBeTruthy();
  expect(api.rebuildSearchIndex).toHaveBeenCalledOnce();
});

it("a confirmed erasure discards delayed analysis results and its saved retry request", async () => {
  let reply!: (value: unknown) => void;
  const resultCallback = vi.fn();
  const api = {
    generateNotes: vi.fn().mockImplementation(
      () =>
        new Promise((done) => {
          reply = done;
        }),
    ),
    getNotesProgress: vi.fn().mockResolvedValue(null),
  } as unknown as DesktopApi;
  const hook = renderHook(() => useNotesJob(api, false, false, resultCallback));
  let pending!: Promise<void>;
  act(() => {
    pending = hook.result.current.run({
      meetingId: meeting.id,
      template: { id: "synthetic", name: "Synthetic", steps: [] },
      endpoint: "http://127.0.0.1:1235/v1/chat/completions",
      model: "synthetic",
      mode: "local",
      cloudConsent: false,
      apiKey: "",
    });
  });
  expect(hook.result.current.lastRequest?.meetingId).toBe(meeting.id);
  act(() => hook.result.current.forget(meeting.id, true));
  await act(async () => {
    reply({ id: "synthetic", meetingId: meeting.id, sections: [] });
    await pending;
  });
  expect(resultCallback).not.toHaveBeenCalled();
  expect(hook.result.current.lastRequest).toBeNull();
  expect(hook.result.current.feedback).toBeNull();
  expect(hook.result.current.progress).toBeNull();
  expect(hook.result.current.busy).toBe(false);
});
