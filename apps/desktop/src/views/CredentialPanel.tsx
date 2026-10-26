import { useEffect, useRef, useState } from "react";
import { errorMessage } from "../api";
import type { CredentialStatus, DesktopApi } from "../types";
import type { MessageKey } from "../i18n";
export default function CredentialPanel({
  api,
  endpoint,
  mode,
  connected,
  disabled,
  apiKey,
  setApiKey,
  useSavedKey,
  setUseSavedKey,
  t,
}: {
  api: DesktopApi;
  endpoint: string;
  mode: "local" | "cloud";
  connected: boolean;
  disabled: boolean;
  apiKey: string;
  setApiKey: (value: string) => void;
  useSavedKey: boolean;
  setUseSavedKey: (value: boolean) => void;
  t: Record<MessageKey, string>;
}) {
  const [status, setStatus] = useState<CredentialStatus | null>(null),
    [pending, setPending] = useState(false),
    [error, setError] = useState<string | null>(null),
    [replace, setReplace] = useState(false),
    [remove, setRemove] = useState(false);
  const sequence = useRef(0),
    mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      sequence.current++;
    };
  }, []);
  useEffect(() => {
    const id = ++sequence.current;
    setUseSavedKey(false);
    setStatus(null);
    setReplace(false);
    setRemove(false);
    setError(null);
    setPending(false);
    if (!connected || !endpoint.trim()) return;
    setPending(true);
    void api
      .credentialStatus(endpoint.trim(), mode)
      .then((value) => {
        if (mounted.current && id === sequence.current) setStatus(value);
      })
      .catch((failure) => {
        if (mounted.current && id === sequence.current)
          setError(errorMessage(failure));
      })
      .finally(() => {
        if (mounted.current && id === sequence.current) setPending(false);
      });
  }, [api, endpoint, mode, connected, setUseSavedKey]);
  async function persist(deleting: boolean) {
    if (pending || disabled || !status || status.state === "unavailable")
      return;
    const id = sequence.current;
    setPending(true);
    setError(null);
    try {
      const next = deleting
        ? await api.removeCredential(endpoint.trim(), mode, true)
        : await api.saveCredential({
            endpoint: endpoint.trim(),
            mode,
            apiKey,
            replaceExisting: replace,
          });
      if (!mounted.current || id !== sequence.current) return;
      setStatus(next);
      setReplace(false);
      setRemove(false);
      if (deleting) setUseSavedKey(false);
    } catch (failure) {
      if (mounted.current && id === sequence.current)
        setError(errorMessage(failure));
    } finally {
      if (mounted.current && id === sequence.current) setPending(false);
    }
  }
  return (
    <details className="credential-panel">
      <summary>{t.apiKey}</summary>
      <p>{t.apiKeyHelp}</p>
      <label>
        {t.sessionKey}
        <input
          type="password"
          value={apiKey}
          disabled={disabled || useSavedKey || pending}
          onChange={(event) => setApiKey(event.target.value)}
          autoComplete="new-password"
        />
      </label>
      <p className="muted">
        {pending
          ? t.busy
          : status?.state === "stored"
            ? t.credentialStored
            : status?.state === "missing"
              ? t.credentialMissing
              : t.credentialUnavailable}
      </p>
      {status?.message && <p className="muted">{status.message}</p>}
      {error && (
        <p role="alert" className="inline-error">
          {error}
        </p>
      )}
      {status?.state === "stored" && (
        <>
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={useSavedKey}
              disabled={disabled || pending}
              onChange={(event) => {
                setUseSavedKey(event.target.checked);
                if (event.target.checked) setApiKey("");
              }}
            />
            {t.useSavedKey}
          </label>
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={replace}
              disabled={disabled || pending || useSavedKey}
              onChange={(event) => setReplace(event.target.checked)}
            />
            {t.replaceCredential}
          </label>
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={remove}
              disabled={disabled || pending}
              onChange={(event) => setRemove(event.target.checked)}
            />
            {t.confirmRemoveCredential}
          </label>
          <button
            className="secondary"
            disabled={disabled || pending || !remove}
            onClick={() => void persist(true)}
          >
            {t.removeCredential}
          </button>
        </>
      )}
      <button
        className="secondary"
        disabled={
          disabled ||
          pending ||
          !apiKey.trim() ||
          useSavedKey ||
          !status ||
          status.state === "unavailable" ||
          (status.state === "stored" && !replace)
        }
        onClick={() => void persist(false)}
      >
        {t.saveCredential}
      </button>
      <p className="muted">{t.credentialHelp}</p>
    </details>
  );
}
