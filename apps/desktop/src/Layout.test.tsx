import { afterAll, beforeAll, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import RetentionPanel from "./views/RetentionPanel";
import type { DesktopApi, RetentionPolicy } from "./types";

// Vitest's normal CSS stub also empties ?raw imports. Read the real styles
// through its test-only module loader without adding Node types to the app.
const { readFileSync } = await vi.importActual<{
  readFileSync(path: string, encoding: "utf8"): string;
}>("node:fs");
const styles = readFileSync("src/styles.css", "utf8");
const theme = readFileSync("src/handy-theme.css", "utf8");

const stylesheet = document.createElement("style");
beforeAll(() => {
  // Include the global field-label rule as well as panel rules so its inherited
  // column direction cannot silently break checkbox labels again.
  stylesheet.textContent = styles.replace(/^@import[^;]+;/m, "");
  document.head.append(stylesheet);
});
afterAll(() => stylesheet.remove());

it("defines every referenced theme token so panel borders cannot silently lose their color", () => {
  expect(styles).toContain(".retention-rule");
  expect(theme).toContain(":root");
  const css = `${theme}\n${styles}`.replace(/\/\*[\s\S]*?\*\//g, "");
  const declared = new Set(
    [...css.matchAll(/(--[\w-]+)\s*:/g)].map((match) => match[1]),
  );
  const referenced = new Set(
    [...css.matchAll(/var\(\s*(--[\w-]+)/g)].map((match) => match[1]),
  );
  expect([...referenced].filter((token) => !declared.has(token))).toEqual([]);
});

it("keeps opt-in checkboxes beside their wrapping rule text and their labels operable", async () => {
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
  const save = vi.fn();
  const api = {
    getRetentionPolicy: vi.fn().mockResolvedValue(policy),
    saveRetentionPolicy: save,
    getMaintenanceProgress: vi.fn().mockResolvedValue(null),
  } as unknown as DesktopApi;
  render(<RetentionPanel api={api} locale="en" />);
  const checkbox = (await screen.findByRole("checkbox", {
    name: "Archive audio only after days · keep text",
  })) as HTMLInputElement;
  const label = checkbox.closest("label")!;
  for (const box of screen.getAllByRole("checkbox")) {
    const computed = getComputedStyle(box.closest("label")!);
    expect(computed.display).toBe("flex");
    expect(computed.flexDirection).toBe("row");
    expect(computed.alignItems).toBe("center");
    expect(getComputedStyle(box).minWidth).toBe("16px");
  }
  expect(checkbox.checked).toBe(false);
  await userEvent.click(label);
  expect(checkbox.checked).toBe(true);
  expect(
    (
      screen.getByRole("spinbutton", {
        name: "Archive audio only after days · keep text",
      }) as HTMLInputElement
    ).value,
  ).toBe("90");
  // Layout and selecting a rule must not persist it or start maintenance.
  expect(save).not.toHaveBeenCalled();
});
