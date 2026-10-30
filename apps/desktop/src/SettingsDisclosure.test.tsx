import { expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { de } from "./i18n";
import SettingsView from "./views/SettingsView";
import type { DesktopApi, RetentionPolicy } from "./types";

// Synthetic IPC only: this does not open, archive, remove or restore any file.
it("keeps a running maintenance job and cancellation outside collapsed settings", async () => {
  const policy: RetentionPolicy = {
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
  const cancel = vi.fn().mockResolvedValue(true);
  const api = {
    getRetentionPolicy: vi.fn().mockResolvedValue(policy),
    getMaintenanceProgress: vi.fn().mockResolvedValue({
      jobId: "synthetic-running-archive",
      kind: "archive-restore",
      status: "running",
      confirmedTasks: 1,
      totalTasks: 3,
      elapsedMs: 1200,
      skipped: 0,
      error: null,
      cancelRequested: false,
    }),
    cancelMaintenance: cancel,
  } as unknown as DesktopApi;
  const noop = () => undefined;
  render(
    <SettingsView
      api={api}
      app={null}
      locale="de"
      theme="system"
      t={de}
      busy={false}
      blocked={false}
      connected
      refreshingHandy={false}
      preferencesPersistable
      handyPath=""
      onChooseHandy={noop}
      onRefreshHandy={noop}
      onRefreshDevices={noop}
      onImportLegacy={noop}
      onRebuildIndex={noop}
      onLocale={noop}
      onTheme={noop}
    />,
  );

  const disclosure = screen.getByText("Daten und Wartung").closest("details")!;
  expect(disclosure.open).toBe(false);
  const job = await screen.findByRole("status", {
    name: "Aufbewahrung und Archiv",
  });
  // jsdom does not fully render native details hiding; the DOM ancestry must also
  // prove that neither status nor cancel depends on opening a collapsed ancestor.
  expect(job.closest("details")).toBeNull();
  expect(api.getMaintenanceProgress).toHaveBeenCalledTimes(1);
  await userEvent.click(
    within(job).getByRole("button", {
      name: "Aufbewahrungsauftrag abbrechen",
    }),
  );
  expect(cancel).toHaveBeenCalledExactlyOnceWith("synthetic-running-archive");
  expect(disclosure.open).toBe(false);
});

it("keeps the exact legacy import control visible when settings collapse during an import", async () => {
  const importLegacy = vi.fn();
  const noop = () => undefined;
  const props = {
    api: { chooseLegacyManifest: vi.fn() } as unknown as DesktopApi,
    app: null,
    locale: "de" as const,
    theme: "system" as const,
    t: de,
    busy: false,
    blocked: false,
    connected: true,
    refreshingHandy: false,
    preferencesPersistable: true,
    handyPath: "",
    onChooseHandy: noop,
    onRefreshHandy: noop,
    onRefreshDevices: noop,
    onImportLegacy: importLegacy,
    onRebuildIndex: noop,
    onLocale: noop,
    onTheme: noop,
  };
  const view = render(<SettingsView {...props} />);
  const disclosure = screen.getByText("Daten und Wartung").closest("details")!;
  await userEvent.click(disclosure.querySelector("summary")!);
  const button = screen.getByRole("button", {
    name: "Sitzungsdatei auswählen und nur lesbar öffnen …",
  });
  await userEvent.click(button);
  expect(importLegacy).toHaveBeenCalledOnce();
  view.rerender(<SettingsView {...props} busy />);
  await userEvent.click(disclosure.querySelector("summary")!);
  expect(disclosure.open).toBe(false);
  expect((button as HTMLButtonElement).disabled).toBe(true);
  // Unlike jsdom's partial details rendering, actual DOM ancestry is decisive:
  // closing any disclosure must not hide the only in-progress import control.
  expect(button.closest("details")).toBeNull();
});
