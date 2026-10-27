import { expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import RetentionPanel from "./views/RetentionPanel";
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
