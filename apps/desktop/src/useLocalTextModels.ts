import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage } from "./api";
import type { DesktopApi } from "./types";

export function useLocalTextModels(
  api: DesktopApi,
  endpoint: string,
  mode: "local" | "cloud",
  connected: boolean,
  visible: boolean,
) {
  const [models, setModels] = useState<string[]>([]),
    [loading, setLoading] = useState(false),
    [error, setError] = useState<string | null>(null),
    [loaded, setLoaded] = useState(false);
  const latest = useRef({ endpoint, mode, connected });
  latest.current = { endpoint, mode, connected };
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
    sequence.current++;
    setModels([]);
    setError(null);
    setLoaded(false);
    setLoading(false);
  }, [endpoint, mode]);
  const refresh = useCallback(async () => {
    const current = latest.current;
    if (!current.connected || current.mode !== "local") return;
    const id = ++sequence.current;
    setLoading(true);
    setError(null);
    try {
      const response = await api.listTextModels(current.endpoint.trim());
      if (
        !mounted.current ||
        id !== sequence.current ||
        latest.current.endpoint !== current.endpoint ||
        latest.current.mode !== "local"
      )
        return;
      setModels([
        ...new Set(
          response.filter((value) => typeof value === "string" && value.trim()),
        ),
      ]);
      setLoaded(true);
    } catch (failure) {
      if (mounted.current && id === sequence.current) {
        setModels([]);
        setLoaded(false);
        setError(errorMessage(failure));
      }
    } finally {
      if (mounted.current && id === sequence.current) setLoading(false);
    }
  }, [api]);
  // Opening the view or returning to local mode refreshes metadata. Typing an
  // address never issues a request on every keystroke; the refresh button applies it.
  useEffect(() => {
    if (visible && mode === "local" && connected) void refresh();
  }, [visible, mode, connected, refresh]);
  return { models, loading, error, loaded, refresh };
}
