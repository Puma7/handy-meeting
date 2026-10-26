import { useState } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import CredentialPanel from "./views/CredentialPanel";
import AgentPanel from "./views/AgentPanel";
import { de } from "./i18n";
import type { AgentState, CredentialStatus, DesktopApi } from "./types";
import { testServices } from "./test-services";

// Synthetic owner UI only: no system key store, provider, agent process or capture opens.
function Credentials({
  api,
  endpoint = "https://example.test/v1/",
  mode = "cloud",
}: {
  api: DesktopApi;
  endpoint?: string;
  mode?: "local" | "cloud";
}) {
  const [key, setKey] = useState(""),
    [saved, setSaved] = useState(false);
  return (
    <CredentialPanel
      api={api}
      endpoint={endpoint}
      mode={mode}
      connected
      disabled={false}
      apiKey={key}
      setApiKey={setKey}
      useSavedKey={saved}
      setUseSavedKey={setSaved}
      t={de}
    />
  );
}
function apiFixture() {
  return testServices() as unknown as DesktopApi;
}
const stored: CredentialStatus = {
  state: "stored",
  provider: "synthetic",
  message: null,
};
beforeEach(() => localStorage.clear());

it("keeps keys session-only until explicit save and requires separate replacement/removal confirmation", async () => {
  const api = apiFixture();
  vi.mocked(api.credentialStatus).mockResolvedValue(stored);
  render(<Credentials api={api} />);
  await userEvent.click(
    screen.getByText("API-Schlüssel (optional)", { selector: "summary" }),
  );
  await screen.findByText(de.credentialStored);
  await userEvent.type(
    screen.getByLabelText(de.sessionKey),
    "synthetic-secret",
  );
  expect(api.saveCredential).not.toHaveBeenCalled();
  expect(api.removeCredential).not.toHaveBeenCalled();
  expect(
    (
      screen.getByRole("button", {
        name: de.saveCredential,
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await userEvent.click(screen.getByLabelText(de.replaceCredential));
  await userEvent.click(
    screen.getByRole("button", { name: de.saveCredential }),
  );
  await waitFor(() =>
    expect(api.saveCredential).toHaveBeenCalledWith({
      endpoint: "https://example.test/v1/",
      mode: "cloud",
      apiKey: "synthetic-secret",
      replaceExisting: true,
    }),
  );
  expect(
    (
      screen.getByRole("button", {
        name: de.removeCredential,
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await userEvent.click(screen.getByLabelText(de.confirmRemoveCredential));
  await userEvent.click(
    screen.getByRole("button", { name: de.removeCredential }),
  );
  await waitFor(() =>
    expect(api.removeCredential).toHaveBeenCalledWith(
      "https://example.test/v1/",
      "cloud",
      true,
    ),
  );
  expect(
    Array.from({ length: localStorage.length }, (_, index) =>
      localStorage.getItem(localStorage.key(index)!),
    ).join("\n"),
  ).not.toContain("synthetic-secret");
});

it("resets stored-key selection on exact provider scope changes and ignores stale key status replies", async () => {
  const api = apiFixture();
  vi.mocked(api.credentialStatus).mockResolvedValue(stored);
  const view = render(<Credentials api={api} />);
  await userEvent.click(
    screen.getByText("API-Schlüssel (optional)", { selector: "summary" }),
  );
  await screen.findByText(de.credentialStored);
  await userEvent.click(screen.getByLabelText(de.useSavedKey));
  expect(
    (screen.getByLabelText(de.useSavedKey) as HTMLInputElement).checked,
  ).toBe(true);
  view.rerender(
    <Credentials api={api} endpoint="http://127.0.0.1:1234/v1/" mode="local" />,
  );
  await waitFor(() =>
    expect(api.credentialStatus).toHaveBeenLastCalledWith(
      "http://127.0.0.1:1234/v1/",
      "local",
    ),
  );
  expect(
    (screen.getByLabelText(de.useSavedKey) as HTMLInputElement).checked,
  ).toBe(false);
  let resolve!: (value: CredentialStatus) => void;
  vi.mocked(api.credentialStatus)
    .mockImplementationOnce(
      () =>
        new Promise((reply) => {
          resolve = reply;
        }),
    )
    .mockResolvedValue({ state: "missing", provider: "new", message: null });
  view.rerender(
    <Credentials api={api} endpoint="http://127.0.0.1:4321/v1/" mode="local" />,
  );
  view.rerender(
    <Credentials api={api} endpoint="http://127.0.0.1:5678/v1/" mode="local" />,
  );
  await screen.findByText(de.credentialMissing);
  await act(async () => resolve(stored));
  expect(screen.queryByLabelText(de.useSavedKey)).toBeNull();
  expect(api.saveCredential).not.toHaveBeenCalled();
});

it("preserves unreadable stored keys by blocking replacement and removal", async () => {
  const api = apiFixture();
  vi.mocked(api.credentialStatus).mockResolvedValue({
    state: "unavailable",
    provider: "synthetic",
    message: "Synthetic locked system store",
  });
  render(<Credentials api={api} />);
  await userEvent.click(
    screen.getByText("API-Schlüssel (optional)", { selector: "summary" }),
  );
  await screen.findByText("Synthetic locked system store");
  await userEvent.type(screen.getByLabelText(de.sessionKey), "new-session-key");
  expect(
    (
      screen.getByRole("button", {
        name: de.saveCredential,
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(
    screen.queryByRole("button", { name: de.removeCredential }),
  ).toBeNull();
  expect(api.saveCredential).not.toHaveBeenCalled();
  expect(api.removeCredential).not.toHaveBeenCalled();
});

const agent: AgentState = {
  connected: true,
  label: "Synthetic local agent",
  ownerId: "synthetic-owner",
  revoked: false,
  pending: {
    requestId: "request-a",
    title: "Requested meeting",
    includeMicrophone: true,
    maxDurationSeconds: 600,
    status: "pending",
  },
  ownedSessionId: null,
  expiresAtUnixMs: null,
};
it("requires microphone consent for each individual agent request and keeps the recording scope visible", async () => {
  const decide = vi.fn(),
    revoke = vi.fn();
  const view = render(
    <AgentPanel
      agent={agent}
      blocked={false}
      devicesReady
      microphoneReady
      busy={false}
      t={de}
      onDecide={decide}
      onRevoke={revoke}
    />,
  );
  expect(screen.getByText(de.agentScope)).toBeTruthy();
  const approve = () =>
    screen.getByRole("button", { name: de.agentApprove }) as HTMLButtonElement;
  expect(approve().disabled).toBe(true);
  await userEvent.click(screen.getByLabelText(de.agentMicrophoneConsent));
  await userEvent.click(approve());
  expect(decide).toHaveBeenCalledWith(true);
  view.rerender(
    <AgentPanel
      agent={{
        ...agent,
        pending: { ...agent.pending!, requestId: "request-b" },
      }}
      blocked={false}
      devicesReady
      microphoneReady
      busy={false}
      t={de}
      onDecide={decide}
      onRevoke={revoke}
    />,
  );
  expect(
    (screen.getByLabelText(de.agentMicrophoneConsent) as HTMLInputElement)
      .checked,
  ).toBe(false);
  expect(approve().disabled).toBe(true);
  await userEvent.click(screen.getByRole("button", { name: de.agentDeny }));
  expect(decide).toHaveBeenLastCalledWith(false);
  await userEvent.click(screen.getByRole("button", { name: de.agentRevoke }));
  expect(revoke).toHaveBeenCalledTimes(1);
});

it("blocks agent approval during a manual recording while preserving visible revocation", async () => {
  const decide = vi.fn(),
    revoke = vi.fn();
  render(
    <AgentPanel
      agent={agent}
      blocked
      devicesReady
      microphoneReady
      busy={false}
      t={de}
      onDecide={decide}
      onRevoke={revoke}
    />,
  );
  await userEvent.click(screen.getByLabelText(de.agentMicrophoneConsent));
  expect(
    (screen.getByRole("button", { name: de.agentApprove }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  await userEvent.click(screen.getByRole("button", { name: de.agentRevoke }));
  expect(revoke).toHaveBeenCalledTimes(1);
  expect(decide).not.toHaveBeenCalled();
});
