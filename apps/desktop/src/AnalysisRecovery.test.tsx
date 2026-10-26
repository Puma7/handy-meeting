import { beforeEach, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { de } from "./i18n";
import { testServices } from "./test-services";
import type {
  AnalysisResult,
  DesktopApi,
  Meeting,
  NotesRequest,
  PendingAnalysis,
} from "./types";

// Synthetic IPC only: neither this cloud-consent test nor local recovery contacts a provider.
const meeting: Meeting = {
  id: "0123456789abcdef0123456789abcdef",
  title: "Saved meeting",
  startedAt: "2026-10-26T08:00:00Z",
  durationMs: 30000,
  status: "complete",
  modelId: "synthetic-asr",
  warnings: [],
  parts: [
    {
      file: "system-000000000000.wav",
      source: "system",
      startSample: 0,
      sampleCount: 480000,
      status: "complete",
      text: "A newer transcript version.",
    },
  ],
};
function fixture(mode: "local" | "cloud") {
  const pending: PendingAnalysis = {
    id: "1123456789abcdef0123456789abcdef",
    meetingId: meeting.id,
    sourceHash: "b".repeat(64),
    createdAt: "2026-10-26T08:01:00Z",
    model: "original-model",
    mode,
    endpoint:
      mode === "local"
        ? "http://127.0.0.1:1234/v1/"
        : "https://example.test/v1/",
    template: {
      id: "original-template",
      name: "Original instructions",
      steps: [
        {
          id: "original-step",
          name: "Original summary",
          kind: "summary",
          prompt: "Original immutable instructions with evidence.",
        },
      ],
    },
    confirmedTasks: 1,
    totalTasks: 2,
  };
  let finished = false;
  const api = {
    ...testServices(),
    getAppState: vi
      .fn()
      .mockResolvedValue({
        version: "synthetic",
        handy: null,
        recording: false,
        activeMeeting: meeting,
      }),
    listDevices: vi.fn().mockResolvedValue([]),
    listMeetings: vi.fn().mockResolvedValue([meeting]),
    getMeeting: vi.fn().mockResolvedValue(meeting),
    listAnalyses: vi.fn().mockResolvedValue([]),
    getNotesProgress: vi.fn().mockResolvedValue(null),
    listTextModels: vi.fn().mockResolvedValue(["newer-model"]),
    listUnfinishedAnalyses: vi.fn(async () =>
      finished ? [] : [structuredClone(pending)],
    ),
    generateNotes: vi.fn(async (_request: NotesRequest) => {
      finished = true;
      return {
        ...pending,
        sections: [{ ...pending.template.steps[0], items: [] }],
        sources: [],
      } as AnalysisResult;
    }),
  } as unknown as DesktopApi;
  return { api, pending };
}
beforeEach(() => localStorage.clear());

for (const mode of ["local", "cloud"] as const) {
  it(`rediscovers a ${mode} run after remount and requires a fresh explicit resume with original settings`, async () => {
    const { api, pending } = fixture(mode);
    const first = render(<App api={api} />);
    await screen.findByRole("button", { name: de.analyzeMeeting });
    first.unmount();
    render(<App api={api} />);
    await userEvent.click(
      await screen.findByRole("button", { name: de.analyzeMeeting }),
    );
    const region = await screen.findByRole("region", {
      name: de.unfinishedAnalyses,
    });
    expect(within(region).getByText("Original instructions")).toBeTruthy();
    expect(api.generateNotes).not.toHaveBeenCalled();
    await userEvent.click(
      within(region).getByRole("button", { name: de.resumeAnalysis }),
    );
    let dialog = screen.getByRole("dialog", { name: de.resumeAnalysis });
    expect(within(dialog).getByText(pending.endpoint)).toBeTruthy();
    expect(within(dialog).getByText(/original-model/)).toBeTruthy();
    await userEvent.click(within(dialog).getByText(de.apiKey));
    await userEvent.type(
      within(dialog).getByLabelText(de.sessionKey),
      "synthetic-discarded-key",
    );
    await userEvent.click(
      within(dialog).getByRole("button", { name: de.cancel }),
    );
    expect(api.generateNotes).not.toHaveBeenCalled();
    await userEvent.click(
      within(region).getByRole("button", { name: de.resumeAnalysis }),
    );
    dialog = screen.getByRole("dialog", { name: de.resumeAnalysis });
    await userEvent.click(within(dialog).getByText(de.apiKey));
    expect(
      (within(dialog).getByLabelText(de.sessionKey) as HTMLInputElement).value,
    ).toBe("");
    if (mode === "cloud")
      expect(within(dialog).getByText(de.cloudBody)).toBeTruthy();
    expect(api.generateNotes).not.toHaveBeenCalled();
    await userEvent.click(
      within(dialog).getByRole("button", {
        name: mode === "cloud" ? de.approve : de.resumeAnalysis,
      }),
    );
    await waitFor(() => expect(api.generateNotes).toHaveBeenCalledTimes(1));
    const request = vi.mocked(api.generateNotes).mock.calls[0][0];
    expect(request).toMatchObject({
      meetingId: pending.meetingId,
      resumeRunId: pending.id,
      template: pending.template,
      endpoint: pending.endpoint,
      model: pending.model,
      mode,
      cloudConsent: mode === "cloud",
      apiKey: "",
      useSavedKey: false,
    });
    expect(JSON.stringify(request)).not.toContain("newer-model");
    expect(JSON.stringify(request)).not.toContain("synthetic-discarded-key");
    await waitFor(() =>
      expect(
        screen.queryByRole("region", { name: de.unfinishedAnalyses }),
      ).toBeNull(),
    );
  });
}

it("rejects an unfinished run belonging to another meeting without offering resume", async () => {
  const { api, pending } = fixture("local");
  vi.mocked(api.listUnfinishedAnalyses).mockResolvedValue([
    { ...pending, meetingId: "another-meeting" },
  ]);
  render(<App api={api} />);
  await userEvent.click(
    await screen.findByRole("button", { name: de.analyzeMeeting }),
  );
  await screen.findByText(de.wrongMeeting);
  expect(
    screen.queryByRole("region", { name: de.unfinishedAnalyses }),
  ).toBeNull();
  expect(api.generateNotes).not.toHaveBeenCalled();
});
