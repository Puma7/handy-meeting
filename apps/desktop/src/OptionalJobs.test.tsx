import { expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import UpdatePanel from "./views/UpdatePanel";
import SpeakerExperiments from "./views/SpeakerExperiments";
import { nameCandidates, SPEAKER_NAME_TEMPLATE } from "./speaker-proposals";
import type { DesktopApi, Meeting, AnalysisResult } from "./types";
const meeting: Meeting = {
  id: "0123456789abcdef0123456789abcdef",
  title: "Synthetic fixture",
  startedAt: "2026-10-27T00:00:00Z",
  durationMs: 30000,
  status: "complete",
  modelId: "asr",
  warnings: [],
  parts: [
    {
      file: "system-000000000000.wav",
      source: "system",
      startSample: 0,
      sampleCount: 480000,
      status: "complete",
      text: "Anna, please read the report.",
    },
  ],
};
it("never checks or installs updates merely by opening settings and blocks installation during active work", async () => {
  const check = vi.fn(),
    download = vi.fn(),
    install = vi.fn();
  const api = {
    checkForUpdate: check,
    downloadUpdate: download,
    installUpdate: install,
    getUpdateProgress: vi.fn().mockResolvedValue({
      status: "ready",
      version: "1.0.3",
      releaseUrl: "https://github.com/Puma7/handy-meeting/releases/tag/v1.0.3",
      receivedBytes: 10,
      totalBytes: 10,
      elapsedMs: 20,
      error: null,
    }),
  } as unknown as DesktopApi;
  render(<UpdatePanel api={api} locale="en" blocked />);
  const button = await screen.findByRole("button", {
    name: "Start installer and close app",
  });
  expect((button as HTMLButtonElement).disabled).toBe(true);
  expect(check).not.toHaveBeenCalled();
  expect(download).not.toHaveBeenCalled();
  expect(install).not.toHaveBeenCalled();
});
it("shows download progress only from confirmed bytes", async () => {
  const api = {
    checkForUpdate: vi.fn(),
    getUpdateProgress: vi.fn().mockResolvedValue({
      status: "downloading",
      version: "1.0.3",
      releaseUrl: null,
      receivedBytes: 25,
      totalBytes: 100,
      elapsedMs: 7000,
      error: null,
    }),
    cancelUpdate: vi.fn().mockResolvedValue(undefined),
  } as unknown as DesktopApi;
  render(<UpdatePanel api={api} locale="en" footer />);
  const bar = await screen.findByRole("progressbar");
  expect(bar.getAttribute("value")).toBe("25");
  expect(bar.getAttribute("max")).toBe("100");
  expect(screen.getByText(/25%/)).toBeTruthy();
  await userEvent.click(
    screen.getByRole("button", { name: "Cancel download" }),
  );
  expect(api.cancelUpdate).toHaveBeenCalledOnce();
});
it("keeps live and post-meeting speaker actions separate and never starts them automatically", async () => {
  const start = vi.fn().mockResolvedValue("speaker-job");
  const api = {
    startSpeakerExperiment: start,
    getSpeakerProgress: vi.fn().mockResolvedValue(null),
    getSpeakerSections: vi.fn().mockResolvedValue(null),
  } as unknown as DesktopApi;
  render(
    <SpeakerExperiments
      api={api}
      locale="en"
      meeting={meeting}
      models={[
        {
          id: "optional-model",
          name: "Installed optional model",
          isDownloaded: true,
          supportsStreaming: false,
        },
      ]}
      recording
    />,
  );
  await userEvent.selectOptions(
    screen.getByLabelText("Additional local speaker model"),
    "optional-model",
  );
  expect(start).not.toHaveBeenCalled();
  expect(
    (
      screen.getByRole("button", {
        name: "Run local speaker separation",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await userEvent.click(
    screen.getByRole("button", { name: "Check live voice sections" }),
  );
  await waitFor(() =>
    expect(start).toHaveBeenCalledWith(meeting.id, "optional-model", "live"),
  );
});
it("targets cancellation only at the owned speaker job and displays unsupported-model failure", async () => {
  const cancel = vi.fn().mockResolvedValue(true);
  const get = vi.fn().mockResolvedValue({
    jobId: "owned-speaker-job",
    meetingId: meeting.id,
    mode: "live",
    status: "running",
    stage: "waiting",
    modelId: "optional",
    confirmedTasks: 2,
    totalTasks: 0,
    elapsedMs: 4000,
    error: null,
    cancelRequested: false,
    lastSection: null,
  });
  const api = {
    startSpeakerExperiment: vi.fn(),
    getSpeakerProgress: get,
    cancelSpeakerExperiment: cancel,
    stopMeeting: vi.fn(),
  } as unknown as DesktopApi;
  render(<SpeakerExperiments api={api} locale="en" footer />);
  const button = await screen.findByRole("button", {
    name: "Cancel only the speaker experiment",
  });
  expect(
    (screen.getByRole("progressbar") as HTMLProgressElement).hasAttribute(
      "value",
    ),
  ).toBe(false);
  await userEvent.click(button);
  expect(cancel).toHaveBeenCalledWith("owned-speaker-job");
  expect(api.stopMeeting).not.toHaveBeenCalled();
});
it("filters forged, stale and wrong-meeting name candidates before they can prefill a manual label", () => {
  const analysis = {
    id: "result",
    meetingId: meeting.id,
    sourceHash: "synthetic",
    template: SPEAKER_NAME_TEMPLATE,
    createdAt: "synthetic",
    sources: [
      {
        id: "original",
        source: "system",
        startMs: 0,
        endMs: 30000,
        text: meeting.parts[0].text!,
      },
    ],
    sections: [
      {
        id: "names",
        name: "Names",
        kind: "summary",
        items: [
          {
            text: "Anna",
            owner: null,
            due: null,
            evidence: [{ sourceId: "original", quote: "Anna, please read" }],
          },
        ],
      },
    ],
  } as AnalysisResult;
  expect(nameCandidates([analysis], meeting).map((c) => c.name)).toEqual([
    "Anna",
  ]);
  expect(
    nameCandidates([{ ...analysis, meetingId: "other" }], meeting),
  ).toEqual([]);
  const forged = structuredClone(analysis);
  forged.sections[0].items[0].text = "Peter";
  expect(nameCandidates([forged], meeting)).toEqual([]);
  const stale = {
    ...meeting,
    parts: [{ ...meeting.parts[0], text: "Changed original text" }],
  };
  expect(nameCandidates([analysis], stale)).toEqual([]);
  const owner = structuredClone(analysis);
  owner.sections[0].items[0].owner = "Anna";
  expect(nameCandidates([owner], meeting)).toEqual([]);
});
