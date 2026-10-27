import { vi } from "vitest";
import type { TemplateLibrary } from "./types";

// Synthetic IPC only. This in-memory fixture never accesses real app storage or keys.
export function testServices() {
  let library: TemplateLibrary = { revision: 0, templates: [] };
  return {
    listUnfinishedAnalyses: vi
      .fn()
      .mockResolvedValue({ items: [], skipped: [] }),
    openMeetingFolder: vi.fn().mockResolvedValue(undefined),
    exportTranscript: vi
      .fn()
      .mockResolvedValue("exports/transcript-synthetic.txt"),
    listTemplates: vi.fn(async () => structuredClone(library)),
    saveTemplates: vi.fn(async (next: TemplateLibrary) => {
      if (next.revision !== library.revision)
        throw new Error("Synthetic revision conflict");
      library = {
        revision: next.revision + 1,
        templates: structuredClone(next.templates),
      };
      return structuredClone(library);
    }),
    importTemplates: vi.fn(async (templates: TemplateLibrary["templates"]) => {
      library = {
        revision: library.revision + 1,
        templates: [...library.templates, ...structuredClone(templates)],
      };
      return structuredClone(library);
    }),
    credentialStatus: vi.fn().mockResolvedValue({
      state: "missing",
      provider: "synthetic",
      message: null,
    }),
    saveCredential: vi.fn().mockResolvedValue({
      state: "stored",
      provider: "synthetic",
      message: null,
    }),
    removeCredential: vi.fn().mockResolvedValue({
      state: "missing",
      provider: "synthetic",
      message: null,
    }),
    agentDecideRecording: vi.fn().mockResolvedValue(undefined),
    agentRevoke: vi.fn().mockResolvedValue(undefined),
  };
}
