import { expect, it, vi } from "vitest";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import RetentionPanel from "./views/RetentionPanel";
import { requestJobStatusRefresh } from "./jobPolling";
import type { DesktopApi, RetentionPolicy, RetentionPlan } from "./types";
const disabled: RetentionPolicy = {
  revision: 0,
  untouchedDays: 30,
  audioArchiveAfterDays: null,
  archiveAfterDays: null,
  convertAfterDays: null,
  maxVisibleMeetings: null,
  deleteArchivesAfterDays: null,
  codec: "flac",
  ffmpegExecutable: null,
};
function pendingChoiceFixture(initialPreview: boolean) {
  // Synthetic IPC: deferred native selection and immutable preview IDs only.
  const originalId = "6".repeat(32),
    nextId = "7".repeat(32);
  let finishChoice!: (path: string | null) => void;
  const choice = new Promise<string | null>((resolve) => {
    finishChoice = resolve;
  });
  const saved = { ...disabled, ffmpegExecutable: "/saved/ffmpeg" };
  let currentJob = originalId;
  const progress = () => ({
    jobId: currentJob,
    kind: "cleanup-preview",
    status: "complete",
    confirmedTasks: 1,
    totalTasks: 1,
    elapsedMs: 500,
    skipped: 0,
    error: null,
    cancelRequested: false,
  });
  const getPlan = vi.fn(async (id: string) => ({
    id,
    policyRevision: 0,
    createdAt: new Date().toISOString(),
    scanned: 1,
    protected: 0,
    unreadable: 0,
    limited: false,
    candidates: [
      {
        meetingId: "8".repeat(32),
        title: "Synthetic pending-choice candidate",
        action: "convert-audio",
        archiveId: null,
        metadataRevision: 0,
        originalManifestSha256: "0".repeat(64),
        ageDays: 90,
        reason: "synthetic",
      },
    ],
  }));
  const apply = vi.fn().mockResolvedValue("synthetic-apply");
  const api = {
    getRetentionPolicy: vi.fn().mockResolvedValue(saved),
    chooseFfmpegExecutable: vi.fn(() => choice),
    previewRetention: vi.fn(async () => {
      currentJob = nextId;
      return nextId;
    }),
    getRetentionPlan: getPlan,
    getMaintenanceProgress: vi
      .fn()
      .mockResolvedValueOnce(initialPreview ? progress() : null)
      .mockImplementation(async () => progress()),
    applyRetention: apply,
  } as unknown as DesktopApi;
  return { api, originalId, nextId, saved, finishChoice, apply, getPlan };
}
it.each([
  { initialPreview: true, approve: true },
  { initialPreview: false, approve: true },
  { initialPreview: false, approve: false },
])(
  "notifies a remounted panel about a pending choice (old preview known=$initialPreview, approve=$approve)",
  async ({ initialPreview, approve }) => {
    const f = pendingChoiceFixture(initialPreview);
    const first = render(<RetentionPanel api={f.api} locale="en" />);
    await screen.findByLabelText(
      "Separately installed FFmpeg · only for optional format conversion",
    );
    if (initialPreview)
      await screen.findByRole("button", { name: "Review action" });
    await userEvent.click(
      screen.getByRole("button", { name: "Choose FFmpeg in file dialog …" }),
    );
    first.unmount();
    render(<RetentionPanel api={f.api} locale="en" />);
    await screen.findByRole("button", { name: "Review action" });
    await userEvent.click(
      screen.getByRole("button", { name: "Review action" }),
    );
    await userEvent.click(
      screen.getByRole("checkbox", {
        name: "Perform this single described action",
      }),
    );
    const staleConfirm = screen.getByRole("button", {
      name: "Run confirmed action",
    });
    await act(async () => {
      f.finishChoice(approve ? f.saved.ffmpegExecutable : null);
      await Promise.resolve();
      // Also exercise a queued click before React has necessarily committed the
      // invalidation render; the apply boundary must check current validity.
      if (approve) fireEvent.click(staleConfirm);
    });
    expect(f.apply).not.toHaveBeenCalled();
    if (!approve) {
      expect(
        (
          screen.getByRole("checkbox", {
            name: "Perform this single described action",
          }) as HTMLInputElement
        ).checked,
      ).toBe(true);
      await userEvent.click(staleConfirm);
      expect(f.apply).toHaveBeenCalledWith(
        f.originalId,
        0,
        `APPLY ${f.originalId} 0`,
      );
      return;
    }
    expect(screen.queryByRole("button", { name: "Review action" })).toBeNull();
    expect(
      screen.queryByRole("region", { name: "Confirm retention action" }),
    ).toBeNull();
    await userEvent.click(
      screen.getByRole("button", { name: "Create retention preview" }),
    );
    await screen.findByRole("button", { name: "Review action" });
    expect(f.getPlan).toHaveBeenLastCalledWith(f.nextId);
    expect(f.apply).not.toHaveBeenCalled();
  },
);
it("requires a new explicit preview when a pending native choice resolves before any preview ID was learned", async () => {
  const f = pendingChoiceFixture(false);
  const first = render(<RetentionPanel api={f.api} locale="en" />);
  await screen.findByLabelText(
    "Separately installed FFmpeg · only for optional format conversion",
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Choose FFmpeg in file dialog …" }),
  );
  first.unmount();
  await act(async () => f.finishChoice(f.saved.ffmpegExecutable));
  render(<RetentionPanel api={f.api} locale="en" />);
  await screen.findByText(/Retention and archive · Preview/);
  expect(f.getPlan).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "Review action" })).toBeNull();
  await userEvent.click(
    screen.getByRole("button", { name: "Create retention preview" }),
  );
  await screen.findByRole("button", { name: "Review action" });
  expect(f.getPlan).toHaveBeenLastCalledWith(f.nextId);
  expect(f.apply).not.toHaveBeenCalled();
});
it.each(["en", "de"] as const)(
  "clears a saved unavailable FFmpeg path before separately saving disabled rules (%s)",
  async (locale) => {
    const de = locale === "de";
    const saved = {
      ...disabled,
      revision: 7,
      convertAfterDays: 60,
      ffmpegExecutable: "/unavailable/ffmpeg",
    };
    const choose = vi.fn().mockResolvedValue(null);
    const save = vi.fn(async (_revision, policy) => {
      if (policy.ffmpegExecutable)
        throw new Error(
          "Synthetic saved program is unavailable; choose it again or clear the selection",
        );
      return { ...policy, revision: 8 };
    });
    const preview = vi.fn();
    const apply = vi.fn();
    const api = {
      getRetentionPolicy: vi.fn().mockResolvedValue(saved),
      chooseFfmpegExecutable: choose,
      saveRetentionPolicy: save,
      previewRetention: preview,
      applyRetention: apply,
    } as unknown as DesktopApi;
    render(<RetentionPanel api={api} locale={locale} />);
    const path = (await screen.findByLabelText(
      de
        ? "Separat installiertes FFmpeg · nur für optionale Formatumwandlung"
        : "Separately installed FFmpeg · only for optional format conversion",
    )) as HTMLInputElement;
    expect(path.value).toBe(saved.ffmpegExecutable);
    await userEvent.click(
      screen.getByRole("checkbox", {
        name: de
          ? "Zusätzliche Audiokopie umwandeln nach Tagen"
          : "Convert an additional audio copy after days",
      }),
    );
    const saveButton = () =>
      screen.getByRole("button", {
        name: de ? "Regeln speichern" : "Save rules",
      });
    await userEvent.click(saveButton());
    await screen.findByText(/Synthetic saved program is unavailable/);
    expect(save).toHaveBeenCalledOnce();
    await userEvent.click(
      screen.getByRole("button", {
        name: de
          ? "FFmpeg im Dateidialog auswählen …"
          : "Choose FFmpeg in file dialog …",
      }),
    );
    expect(choose).toHaveBeenCalledOnce();
    expect(path.value).toBe(saved.ffmpegExecutable);
    expect(save).toHaveBeenCalledOnce();
    await userEvent.click(
      screen.getByRole("button", {
        name: de ? "FFmpeg-Auswahl entfernen" : "Clear FFmpeg selection",
      }),
    );
    expect(path.value).toBe("");
    expect(save).toHaveBeenCalledOnce();
    expect(preview).not.toHaveBeenCalled();
    expect(apply).not.toHaveBeenCalled();
    for (const box of screen.getAllByRole("checkbox"))
      expect((box as HTMLInputElement).checked).toBe(false);
    await userEvent.click(saveButton());
    await waitFor(() =>
      expect(save).toHaveBeenLastCalledWith(7, {
        ...saved,
        convertAfterDays: null,
        ffmpegExecutable: null,
      }),
    );
    expect(preview).not.toHaveBeenCalled();
    expect(apply).not.toHaveBeenCalled();
  },
);
it("selects FFmpeg through the native chooser, retains cancellation and saves only after the user's separate action", async () => {
  const choose = vi
    .fn()
    .mockResolvedValueOnce(null)
    .mockResolvedValueOnce("/native-choice/ffmpeg");
  const save = vi.fn(async (_revision, policy) => ({ ...policy, revision: 1 }));
  const api = {
    getRetentionPolicy: vi.fn().mockResolvedValue(disabled),
    chooseFfmpegExecutable: choose,
    saveRetentionPolicy: save,
  } as unknown as DesktopApi;
  render(<RetentionPanel api={api} locale="en" />);
  const input = (await screen.findByLabelText(
    "Separately installed FFmpeg · only for optional format conversion",
  )) as HTMLInputElement;
  expect(input.readOnly).toBe(true);
  await userEvent.click(
    screen.getByRole("button", { name: "Choose FFmpeg in file dialog …" }),
  );
  expect(input.value).toBe("");
  expect(save).not.toHaveBeenCalled();
  await userEvent.click(
    screen.getByRole("button", { name: "Choose FFmpeg in file dialog …" }),
  );
  await waitFor(() => expect(input.value).toBe("/native-choice/ffmpeg"));
  expect(save).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Save rules" }));
  expect(save).toHaveBeenCalledWith(0, {
    ...disabled,
    ffmpegExecutable: "/native-choice/ffmpeg",
  });
});
it.each(["/native-choice/ffmpeg", "/saved/ffmpeg", null])(
  "keeps an old preview invalid after FFmpeg selection %s is approved or cleared, while cancellation preserves it",
  async (selectedPath) => {
    // Synthetic IPC only: no file, encoder, recording or maintenance action is run.
    const saved: RetentionPolicy = {
      ...disabled,
      revision: 4,
      convertAfterDays: 60,
      ffmpegExecutable: "/saved/ffmpeg",
    };
    const originalId = "b".repeat(32),
      nextId = "d".repeat(32);
    const plan: RetentionPlan = {
      id: originalId,
      policyRevision: saved.revision,
      createdAt: new Date().toISOString(),
      scanned: 1,
      protected: 0,
      unreadable: 0,
      limited: false,
      candidates: [
        {
          meetingId: "a".repeat(32),
          title: "Synthetic conversion candidate",
          action: "convert-audio",
          archiveId: null,
          metadataRevision: 0,
          originalManifestSha256: "0".repeat(64),
          ageDays: 90,
          reason: "synthetic preview",
        },
      ],
    };
    const choose = vi
      .fn()
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(selectedPath);
    let revision = saved.revision;
    let currentPolicy = saved;
    let currentJob = originalId;
    let currentKind = "cleanup-preview";
    const save = vi.fn(async (_revision, value) => {
      revision++;
      currentPolicy = { ...value, revision };
      return currentPolicy;
    });
    const preview = vi.fn(async () => {
      currentJob = nextId;
      return nextId;
    });
    const apply = vi.fn();
    const api = {
      getRetentionPolicy: vi.fn(async () => currentPolicy),
      chooseFfmpegExecutable: choose,
      saveRetentionPolicy: save,
      previewRetention: preview,
      getRetentionPlan: vi.fn(async (id) => ({
        ...plan,
        id,
        policyRevision: revision,
      })),
      getMaintenanceProgress: vi.fn(async () => ({
        jobId: currentJob,
        kind: currentKind,
        status: "complete",
        confirmedTasks: 1,
        totalTasks: 1,
        elapsedMs: 500,
        skipped: 0,
        error: null,
        cancelRequested: false,
      })),
      applyRetention: apply,
    } as unknown as DesktopApi;
    const view = render(<RetentionPanel api={api} locale="en" />);
    await screen.findByRole("button", { name: "Review action" });
    await userEvent.click(
      screen.getByRole("button", { name: "Review action" }),
    );
    const accept = () =>
      screen.getByRole("checkbox", {
        name: "Perform this single described action",
      }) as HTMLInputElement;
    await userEvent.click(accept());
    expect(accept().checked).toBe(true);
    const chooseButton = () =>
      screen.getByRole("button", {
        name: "Choose FFmpeg in file dialog …",
      });
    await userEvent.click(chooseButton());
    expect(choose).toHaveBeenCalledTimes(1);
    expect(accept().checked).toBe(true);
    expect(
      (
        screen.getByRole("button", {
          name: "Run confirmed action",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false);
    const path = screen.getByLabelText(
      "Separately installed FFmpeg · only for optional format conversion",
    ) as HTMLInputElement;
    expect(path.value).toBe(saved.ffmpegExecutable);
    if (selectedPath === null) {
      await userEvent.click(
        screen.getByRole("button", { name: "Clear FFmpeg selection" }),
      );
      expect(choose).toHaveBeenCalledTimes(1);
    } else {
      await userEvent.click(chooseButton());
      await waitFor(() => expect(choose).toHaveBeenCalledTimes(2));
    }
    await waitFor(() =>
      expect(
        screen.queryByRole("region", { name: "Confirm retention action" }),
      ).toBeNull(),
    );
    expect(screen.queryByRole("button", { name: "Review action" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Run confirmed action" }),
    ).toBeNull();
    expect(path.value).toBe(selectedPath ?? "");
    expect(save).not.toHaveBeenCalled();
    expect(preview).not.toHaveBeenCalled();
    expect(apply).not.toHaveBeenCalled();
    // Re-emit the old completed preview after an intervening status notification.
    // This exercises the real status poller without waiting for its idle timer.
    currentJob = "e".repeat(32);
    currentKind = "archive-restore";
    await act(async () => requestJobStatusRefresh());
    await screen.findByText(/Retention and archive · Restore original files/);
    currentJob = originalId;
    currentKind = "cleanup-preview";
    await act(async () => requestJobStatusRefresh());
    await screen.findByText(/Retention and archive · Preview/);
    expect(api.getRetentionPlan).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Review action" })).toBeNull();
    expect(
      screen.queryByRole("region", { name: "Confirm retention action" }),
    ).toBeNull();
    expect(apply).not.toHaveBeenCalled();
    if (selectedPath !== saved.ffmpegExecutable) {
      await userEvent.click(screen.getByRole("button", { name: "Save rules" }));
      await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    }
    // Navigating away and back must not forget a native grant's invalidation.
    view.unmount();
    render(<RetentionPanel api={api} locale="en" />);
    await screen.findByText(/Retention and archive · Preview/);
    expect(api.getRetentionPlan).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Review action" })).toBeNull();
    expect(
      screen.queryByRole("region", { name: "Confirm retention action" }),
    ).toBeNull();
    await userEvent.click(
      screen.getByRole("button", { name: "Create retention preview" }),
    );
    await screen.findByRole("button", { name: "Review action" });
    await userEvent.click(
      screen.getByRole("button", { name: "Review action" }),
    );
    expect(preview).toHaveBeenCalledOnce();
    expect(api.getRetentionPlan).toHaveBeenLastCalledWith(nextId);
    expect(accept().checked).toBe(false);
    expect(
      (
        screen.getByRole("button", {
          name: "Run confirmed action",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(apply).not.toHaveBeenCalled();
  },
);
it("ignores a late rejected invalidated preview after a new native choice and a newer preview", async () => {
  const originalId = "f".repeat(32),
    nextId = "1".repeat(32);
  const saved = { ...disabled, ffmpegExecutable: "/saved/ffmpeg" };
  const plan: RetentionPlan = {
    id: nextId,
    policyRevision: saved.revision,
    createdAt: new Date().toISOString(),
    scanned: 0,
    protected: 0,
    unreadable: 0,
    limited: false,
    candidates: [],
  };
  let rejectOld!: (error: Error) => void;
  const oldPlan = new Promise<RetentionPlan>((_resolve, reject) => {
    rejectOld = reject;
  });
  let currentJob = originalId;
  const getPlan = vi.fn((id: string) =>
    id === originalId ? oldPlan : Promise.resolve(plan),
  );
  const api = {
    getRetentionPolicy: vi.fn().mockResolvedValue(saved),
    chooseFfmpegExecutable: vi.fn().mockResolvedValue(saved.ffmpegExecutable),
    previewRetention: vi.fn(async () => {
      currentJob = nextId;
      return nextId;
    }),
    getRetentionPlan: getPlan,
    getMaintenanceProgress: vi.fn(async () => ({
      jobId: currentJob,
      kind: "cleanup-preview",
      status: "complete",
      confirmedTasks: 0,
      totalTasks: 0,
      elapsedMs: 500,
      skipped: 0,
      error: null,
      cancelRequested: false,
    })),
  } as unknown as DesktopApi;
  render(<RetentionPanel api={api} locale="en" />);
  await waitFor(() => expect(getPlan).toHaveBeenCalledWith(originalId));
  await userEvent.click(
    screen.getByRole("button", { name: "Choose FFmpeg in file dialog …" }),
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Create retention preview" }),
  );
  await screen.findByText("Preview · no files changed");
  expect(getPlan).toHaveBeenLastCalledWith(nextId);
  await act(async () =>
    rejectOld(new Error("Synthetic stale preview read failure")),
  );
  expect(screen.queryByRole("alert")).toBeNull();
  expect(screen.getByText("Preview · no files changed")).toBeTruthy();
});
it("leaves every retention rule disabled until explicitly configured and separates saving from preview", async () => {
  const policy = vi.fn().mockResolvedValue(disabled);
  const save = vi
    .fn()
    .mockImplementation(async (_rev, value) => ({ ...value, revision: 1 }));
  const preview = vi.fn().mockResolvedValue("preview-job");
  const api = {
    getRetentionPolicy: policy,
    saveRetentionPolicy: save,
    previewRetention: preview,
    getMaintenanceProgress: vi.fn().mockResolvedValue(null),
  } as unknown as DesktopApi;
  render(<RetentionPanel api={api} locale="en" />);
  await screen.findByLabelText("Leave meetings untouched initially · days");
  for (const box of screen.getAllByRole("checkbox"))
    expect((box as HTMLInputElement).checked).toBe(false);
  expect(save).not.toHaveBeenCalled();
  expect(preview).not.toHaveBeenCalled();
  await userEvent.click(
    screen.getByLabelText("Archive audio only after days · keep text"),
  );
  expect(
    (
      screen.getByRole("button", {
        name: "Create retention preview",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await userEvent.click(screen.getByRole("button", { name: "Save rules" }));
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(0, {
      ...disabled,
      audioArchiveAfterDays: 90,
    }),
  );
  expect(preview).not.toHaveBeenCalled();
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Create retention preview",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Create retention preview" }),
  );
  expect(preview).toHaveBeenCalledOnce();
});
it("requires reviewing one immutable preview row and a separate exact permanent-delete confirmation", async () => {
  const id = "a".repeat(32),
    planId = "b".repeat(32);
  const apply = vi.fn().mockResolvedValue("apply-job");
  const plan: RetentionPlan = {
    id: planId,
    policyRevision: 0,
    createdAt: "2026-10-27T00:00:00Z",
    scanned: 1,
    protected: 0,
    unreadable: 0,
    limited: false,
    candidates: [
      {
        meetingId: id,
        title: "Archived synthetic meeting",
        action: "delete-archived-meeting",
        archiveId: "c".repeat(32),
        metadataRevision: 0,
        originalManifestSha256: "0".repeat(64),
        ageDays: 400,
        reason: "fixture",
      },
    ],
  };
  const api = {
    getRetentionPolicy: vi.fn().mockResolvedValue(disabled),
    getRetentionPlan: vi.fn().mockResolvedValue(plan),
    getMaintenanceProgress: vi.fn().mockResolvedValue({
      jobId: planId,
      kind: "cleanup-preview",
      status: "complete",
      confirmedTasks: 1,
      totalTasks: 1,
      elapsedMs: 500,
      skipped: 0,
      error: null,
      cancelRequested: false,
    }),
    applyRetention: apply,
  } as unknown as DesktopApi;
  render(<RetentionPanel api={api} locale="en" />);
  await screen.findByRole("button", { name: "Review action" });
  expect(apply).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Review action" }));
  const confirm = screen.getByRole("button", {
    name: "Permanently delete",
  }) as HTMLButtonElement;
  expect(confirm.disabled).toBe(true);
  await userEvent.type(
    screen.getByLabelText("Confirm permanent deletion"),
    "DELETE",
  );
  expect(confirm.disabled).toBe(true);
  expect(apply).not.toHaveBeenCalled();
  await userEvent.clear(screen.getByLabelText("Confirm permanent deletion"));
  await userEvent.type(
    screen.getByLabelText("Confirm permanent deletion"),
    `DELETE ${id}`,
  );
  await userEvent.click(confirm);
  await waitFor(() =>
    expect(apply).toHaveBeenCalledWith(planId, 0, `DELETE ${id}`),
  );
});
it("shows policy-read failures without silently creating replacement settings", async () => {
  const save = vi.fn();
  const api = {
    getRetentionPolicy: vi
      .fn()
      .mockRejectedValue(new Error("Retained settings are unreadable")),
    saveRetentionPolicy: save,
    getMaintenanceProgress: vi.fn().mockResolvedValue(null),
  } as unknown as DesktopApi;
  render(<RetentionPanel api={api} locale="en" />);
  await screen.findByRole("alert");
  expect(screen.queryByRole("button", { name: "Save rules" })).toBeNull();
  expect(save).not.toHaveBeenCalled();
});
