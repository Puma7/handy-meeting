import { expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ArchivePanel from "./views/ArchivePanel";
import AudioCopiesPanel from "./views/AudioCopiesPanel";
import type { DesktopApi } from "./types";
it("never restores a whole meeting over an existing session and requires a separate confirmation", async () => {
  const restore = vi.fn().mockResolvedValue("job");
  const entry = {
    archiveId: "a".repeat(32),
    unreadable: null,
    receipt: {
      archiveId: "a".repeat(32),
      meetingId: "b".repeat(32),
      kind: "whole-meeting",
      archiveBytes: 1000,
      originalBytes: 2000,
      createdAt: "2026-01-01T00:00:00Z",
    },
    title: "Synthetic archived meeting",
    sessionPresent: false,
    sourceCopyRetained: true,
  };
  const api = {
    listArchives: vi.fn().mockResolvedValue({
      items: [
        entry,
        {
          ...entry,
          archiveId: "c".repeat(32),
          receipt: { ...entry.receipt, archiveId: "c".repeat(32) },
          title: "Already restored",
          sessionPresent: true,
        },
      ],
      more: false,
    }),
    restoreArchive: restore,
  } as unknown as DesktopApi;
  render(<ArchivePanel api={api} locale="en" job={null} />);
  const buttons = await screen.findAllByRole("button", {
    name: "Review restoration",
  });
  expect((buttons[1] as HTMLButtonElement).disabled).toBe(true);
  expect(restore).not.toHaveBeenCalled();
  await userEvent.click(buttons[0]);
  expect(restore).not.toHaveBeenCalled();
  await userEvent.click(
    screen.getByRole("button", { name: "Restore verified copy" }),
  );
  await waitFor(() =>
    expect(restore).toHaveBeenCalledWith(entry.receipt.archiveId, true),
  );
});
it("does not start compact-audio recognition until a model and new-copy confirmation are chosen", async () => {
  const start = vi.fn().mockResolvedValue("job");
  const runId = "d".repeat(32);
  const api = {
    listAudioCopies: vi.fn().mockResolvedValue([
      {
        runId,
        meetingId: "e".repeat(32),
        title: "Synthetic Opus copy",
        codec: "opus",
        createdAt: "2026-01-01",
        sections: 2,
      },
    ]),
    transcribeAudioCopy: start,
  } as unknown as DesktopApi;
  render(
    <AudioCopiesPanel
      api={api}
      locale="en"
      job={null}
      models={[
        {
          id: "model",
          name: "Installed local model",
          isDownloaded: true,
          supportsStreaming: true,
        },
      ]}
    />,
  );
  await userEvent.click(
    await screen.findByRole("button", { name: "Prepare new transcription" }),
  );
  expect(start).not.toHaveBeenCalled();
  const button = screen.getByRole("button", {
    name: "Start new local transcription",
  }) as HTMLButtonElement;
  expect(button.disabled).toBe(true);
  await userEvent.selectOptions(
    screen.getByLabelText("Model for the new transcription"),
    "model",
  );
  expect(button.disabled).toBe(true);
  await userEvent.click(screen.getByRole("checkbox"));
  await userEvent.click(button);
  await waitFor(() => expect(start).toHaveBeenCalledWith(runId, "model", true));
});

it("requires separate confirmation before removing a redundant raw copy", async () => {
  const compact = vi.fn().mockResolvedValue("job");
  const archiveId = "f".repeat(32);
  const api = {
    listArchives: vi.fn().mockResolvedValue({
      items: [
        {
          archiveId,
          unreadable: null,
          receipt: {
            archiveId,
            meetingId: "a".repeat(32),
            kind: "whole-meeting",
            archiveBytes: 1000,
            originalBytes: 2000,
            createdAt: "2025-01-01",
          },
          title: "Synthetic compact archive",
          sessionPresent: false,
          sourceCopyRetained: true,
        },
      ],
      more: false,
    }),
    compactArchive: compact,
  } as unknown as DesktopApi;
  render(<ArchivePanel api={api} locale="en" job={null} />);
  await userEvent.click(
    await screen.findByRole("button", { name: "Keep archive compactly" }),
  );
  expect(compact).not.toHaveBeenCalled();
  await userEvent.click(
    screen.getByRole("button", {
      name: "Keep verified ZIP, remove raw duplicate",
    }),
  );
  await waitFor(() => expect(compact).toHaveBeenCalledWith(archiveId, true));
});
it("continues a stopped permanent deletion only after the exact typed confirmation", async () => {
  const resume = vi.fn().mockResolvedValue("job");
  const meetingId = "d".repeat(32);
  const api = {
    listArchives: vi.fn().mockResolvedValue({ items: [], more: false }),
    listUnfinishedDeletions: vi.fn().mockResolvedValue([
      {
        logId: "delete-" + "e".repeat(32),
        meetingId,
        deletedFiles: 2,
        totalFiles: 5,
        confirmedAt: "2026-10-27T00:00:00Z",
      },
    ]),
    resumePermanentDeletion: resume,
  } as unknown as DesktopApi;
  render(<ArchivePanel api={api} locale="en" job={null} />);
  await userEvent.click(
    await screen.findByRole("button", { name: "Review continuation" }),
  );
  const go = screen.getByRole("button", { name: "Continue deletion" });
  expect((go as HTMLButtonElement).disabled).toBe(true);
  const input = screen.getByLabelText(`Type "DELETE ${meetingId}" to confirm`);
  await userEvent.type(input, "DELETE");
  expect((go as HTMLButtonElement).disabled).toBe(true);
  await userEvent.type(input, ` ${meetingId}`);
  await userEvent.click(go);
  await waitFor(() =>
    expect(resume).toHaveBeenCalledWith(
      "delete-" + "e".repeat(32),
      `DELETE ${meetingId}`,
    ),
  );
});
it("shows unreadable archive entries as preserved, without restore or compaction actions", async () => {
  const restore = vi.fn().mockResolvedValue("job");
  const compact = vi.fn().mockResolvedValue("job");
  const readable = {
    archiveId: "a".repeat(32),
    unreadable: null,
    receipt: {
      archiveId: "a".repeat(32),
      meetingId: "b".repeat(32),
      kind: "whole-meeting",
      archiveBytes: 1000,
      originalBytes: 2000,
      createdAt: "2026-01-01T00:00:00Z",
    },
    title: "Readable archive",
    sessionPresent: false,
    sourceCopyRetained: true,
  };
  const api = {
    listArchives: vi.fn().mockResolvedValue({
      items: [
        {
          archiveId: "0".repeat(32),
          receipt: null,
          title: "0".repeat(32),
          sessionPresent: false,
          sourceCopyRetained: true,
          unreadable: "receipt",
        },
        {
          ...readable,
          archiveId: "c".repeat(32),
          receipt: { ...readable.receipt, archiveId: "c".repeat(32) },
          title: "Archive with unreadable labels",
          unreadable: "labels",
        },
        readable,
      ],
      more: false,
    }),
    restoreArchive: restore,
    compactArchive: compact,
  } as unknown as DesktopApi;
  render(<ArchivePanel api={api} locale="en" job={null} />);
  expect(await screen.findByText("Unreadable archive")).toBeTruthy();
  expect(screen.getByText("0".repeat(32))).toBeTruthy();
  expect(screen.getByText("Archive with unreadable labels")).toBeTruthy();
  expect(screen.getByText("Readable archive")).toBeTruthy();
  expect(
    screen.getByText(/archive receipt cannot be read/).textContent,
  ).toContain("All files are kept unchanged");
  expect(
    screen.getByText(/saved meeting labels cannot be read/).textContent,
  ).toContain("kept unchanged");
  // Only the readable entry offers actions.
  expect(
    screen.getAllByRole("button", { name: "Review restoration" }),
  ).toHaveLength(1);
  expect(
    screen.getAllByRole("button", { name: "Keep archive compactly" }),
  ).toHaveLength(1);
  expect(restore).not.toHaveBeenCalled();
  expect(compact).not.toHaveBeenCalled();
});
it("keeps the archive list when the deletion log cannot be read", async () => {
  const api = {
    listArchives: vi.fn().mockResolvedValue({
      items: [
        {
          archiveId: "a".repeat(32),
          unreadable: null,
          receipt: {
            archiveId: "a".repeat(32),
            meetingId: "b".repeat(32),
            kind: "whole-meeting",
            archiveBytes: 1000,
            originalBytes: 2000,
            createdAt: "2026-01-01T00:00:00Z",
          },
          title: "Still browsable",
          sessionPresent: false,
          sourceCopyRetained: false,
        },
      ],
      more: false,
    }),
    listUnfinishedDeletions: vi.fn().mockRejectedValue("log unreadable"),
  } as unknown as DesktopApi;
  render(<ArchivePanel api={api} locale="en" job={null} />);
  expect(await screen.findByText("Still browsable")).toBeTruthy();
  expect(await screen.findByRole("alert")).toBeTruthy();
});
