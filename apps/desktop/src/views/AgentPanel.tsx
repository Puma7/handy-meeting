import { useState } from "react";
import { ShieldCheck } from "lucide-react";
import { clock } from "../domain";
import type { AgentState } from "../types";
import type { MessageKey } from "../i18n";
export default function AgentPanel({
  agent,
  blocked,
  devicesReady,
  microphoneReady,
  busy,
  t,
  onDecide,
  onRevoke,
}: {
  agent: AgentState | null | undefined;
  blocked: boolean;
  devicesReady: boolean;
  microphoneReady: boolean;
  busy: boolean;
  t: Record<MessageKey, string>;
  onDecide: (approve: boolean) => void;
  onRevoke: () => void;
}) {
  const [microphoneRequestId, setMicrophoneRequestId] = useState<string | null>(
    null,
  );
  if (!agent) return null;
  const pending = agent.pending;
  const allowMicrophone = Boolean(
    pending && microphoneRequestId === pending.requestId,
  );
  const needsMicrophone = pending?.includeMicrophone ?? false;
  const denied =
    blocked ||
    !devicesReady ||
    (needsMicrophone && (!allowMicrophone || !microphoneReady));
  return (
    <section className="agent-panel" aria-label={t.agentControl}>
      <div className="section-heading">
        <h2>
          <ShieldCheck size={19} />
          {t.agentControl}
        </h2>
        <span>
          {agent.connected && !agent.revoked
            ? t.agentConnected
            : t.agentDisconnected}
        </span>
      </div>
      <p>
        <strong>{agent.label}</strong> · <code>{agent.ownerId}</code>
      </p>
      <p>{t.agentScope}</p>
      {agent.ownedSessionId && (
        <p>
          {t.agentOwnedSession}: <code>{agent.ownedSessionId}</code>
          {agent.expiresAtUnixMs
            ? " · " +
              t.agentExpires +
              " " +
              new Date(agent.expiresAtUnixMs).toLocaleTimeString()
            : ""}
        </p>
      )}
      {pending && (
        <div className="agent-request">
          <strong>{pending.title}</strong>
          <p>
            {t.agentDuration}: {clock(pending.maxDurationSeconds * 1000)} ·{" "}
            {needsMicrophone ? t.agentWithMicrophone : t.agentSystemOnly}
          </p>
          {needsMicrophone && (
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={allowMicrophone}
                disabled={busy || pending.status !== "pending"}
                onChange={(event) =>
                  setMicrophoneRequestId(
                    event.target.checked ? pending.requestId : null,
                  )
                }
              />
              {t.agentMicrophoneConsent}
            </label>
          )}
          {!devicesReady && (
            <p className="inline-error">{t.agentChooseDevices}</p>
          )}
          {needsMicrophone && !microphoneReady && (
            <p className="inline-error">{t.agentChooseMicrophone}</p>
          )}
          {blocked && <p className="muted">{t.agentBusy}</p>}
          <div className="template-actions">
            <button
              className="secondary"
              disabled={busy || pending.status !== "pending"}
              onClick={() => onDecide(false)}
            >
              {t.agentDeny}
            </button>
            <button
              className="primary"
              disabled={
                busy ||
                denied ||
                pending.status !== "pending" ||
                !agent.connected ||
                agent.revoked
              }
              onClick={() => onDecide(true)}
            >
              {pending.status === "starting" ? t.busy : t.agentApprove}
            </button>
          </div>
        </div>
      )}
      {!agent.revoked && (
        <button className="secondary" disabled={busy} onClick={onRevoke}>
          {t.agentRevoke}
        </button>
      )}
      {agent.revoked && <p className="muted">{t.agentRevoked}</p>}
    </section>
  );
}
