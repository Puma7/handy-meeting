import { useEffect, useRef, useState } from "react";
import type {
  AudioLevel,
  AudioLevels,
  CaptureControl,
  DesktopApi,
  Track,
} from "./types";

const POLL_MS = 150;
const STALE_MS = 1000;
const statuses = new Set<AudioLevel["status"]>([
  "active",
  "waiting",
  "stale",
  "paused",
  "muted",
  "unavailable",
]);
type Snapshot = {
  meetingId: string;
  levels: AudioLevels | null;
  observedAt: number;
  failed: boolean;
  generations: Record<Track, number>;
};

/** Reject malformed native values instead of lighting a meter with an invented signal. */
function validLevel(level: AudioLevel | undefined): boolean {
  if (
    !level ||
    !statuses.has(level.status) ||
    typeof level.clipped !== "boolean"
  )
    return false;
  const dbfs = (value: number | null) =>
    value === null || (Number.isFinite(value) && value >= -96 && value <= 0);
  return (
    dbfs(level.peakDbfs) &&
    dbfs(level.rmsDbfs) &&
    (level.ageMs === null ||
      (Number.isFinite(level.ageMs) && level.ageMs >= 0)) &&
    (level.status !== "active" ||
      (level.peakDbfs !== null &&
        level.rmsDbfs !== null &&
        level.ageMs !== null))
  );
}

/**
 * Only the small source panel subscribes to fast meter updates. Requests never overlap,
 * even across session changes; late replies cannot attach to a different meeting.
 * Every acknowledged source gate transition invalidates that source's cached samples
 * and in-flight reply generation, including rapid close/reopen between two polls.
 * A local watchdog clears the signal when an IPC reply hangs or samples become stale.
 */
export function useAudioLevels(
  getLevels: DesktopApi["getAudioLevels"],
  meetingId: string | null,
  enabled: boolean,
  control?: CaptureControl | null,
) {
  const [visible, setVisible] = useState(document.visibilityState !== "hidden");
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [now, setNow] = useState(Date.now);
  const systemOpen = !control?.paused;
  const microphoneOpen =
    systemOpen && !control?.microphoneMuted && control?.hasMicrophone !== false;
  const [gates, setGates] = useState(() => ({
    systemOpen,
    microphoneOpen,
    generations: { system: 0, microphone: 0 },
    changedAt: { system: Date.now(), microphone: Date.now() },
  }));
  // Adjust state during render so React discards the old source generation before
  // committing children. An effect-only reset could expose a cached peak on reopening.
  if (
    gates.systemOpen !== systemOpen ||
    gates.microphoneOpen !== microphoneOpen
  ) {
    const changedAt = Date.now();
    setGates({
      systemOpen,
      microphoneOpen,
      generations: {
        system:
          gates.generations.system + Number(gates.systemOpen !== systemOpen),
        microphone:
          gates.generations.microphone +
          Number(gates.microphoneOpen !== microphoneOpen),
      },
      changedAt: {
        system:
          gates.systemOpen !== systemOpen ? changedAt : gates.changedAt.system,
        microphone:
          gates.microphoneOpen !== microphoneOpen
            ? changedAt
            : gates.changedAt.microphone,
      },
    });
  }
  const inFlight = useRef<Promise<void> | null>(null);
  const latest = useRef({
    meetingId,
    enabled,
    visible,
    getLevels,
    generations: gates.generations,
  });
  latest.current = {
    meetingId,
    enabled,
    visible,
    getLevels,
    generations: gates.generations,
  };

  useEffect(() => {
    const change = () => {
      setSnapshot(null);
      setVisible(document.visibilityState !== "hidden");
    };
    document.addEventListener("visibilitychange", change);
    return () => document.removeEventListener("visibilitychange", change);
  }, []);

  useEffect(() => {
    if (!enabled || !visible || !meetingId || !getLevels) return;
    let disposed = false;
    const startedAt = Date.now();
    setSnapshot({
      meetingId,
      levels: null,
      observedAt: startedAt,
      failed: false,
      generations: latest.current.generations,
    });
    setNow(startedAt);
    const current = () =>
      !disposed &&
      latest.current.meetingId === meetingId &&
      latest.current.enabled &&
      latest.current.visible &&
      latest.current.getLevels === getLevels;
    const poll = () => {
      if (!current()) return;
      setNow(Date.now());
      if (inFlight.current) return;
      const requestedAt = Date.now();
      const generations = latest.current.generations;
      const pending = (async () => {
        try {
          const levels = await getLevels();
          if (!current()) return;
          const observedAt = Date.now();
          const valid =
            levels === null ||
            (levels.meetingId === meetingId &&
              validLevel(levels.system) &&
              validLevel(levels.microphone));
          const delay = Math.max(0, observedAt - requestedAt);
          const adjusted =
            valid && levels
              ? {
                  ...levels,
                  system: {
                    ...levels.system,
                    ageMs:
                      levels.system.ageMs === null
                        ? null
                        : levels.system.ageMs + delay,
                  },
                  microphone: {
                    ...levels.microphone,
                    ageMs:
                      levels.microphone.ageMs === null
                        ? null
                        : levels.microphone.ageMs + delay,
                  },
                }
              : null;
          setSnapshot({
            meetingId,
            levels: adjusted,
            observedAt,
            failed: !valid || delay >= STALE_MS,
            generations,
          });
          setNow(observedAt);
        } catch {
          if (current()) {
            setSnapshot({
              meetingId,
              levels: null,
              observedAt: Date.now(),
              failed: true,
              generations,
            });
            setNow(Date.now());
          }
        }
      })();
      inFlight.current = pending;
      void pending.finally(() => {
        if (inFlight.current === pending) inFlight.current = null;
      });
    };
    poll();
    const timer = window.setInterval(poll, POLL_MS);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [getLevels, meetingId, enabled, visible]);

  const bound =
    enabled && visible && snapshot?.meetingId === meetingId ? snapshot : null;
  const elapsed = bound ? Math.max(0, now - bound.observedAt) : 0;
  const level = (track: "system" | "microphone"): AudioLevel | null => {
    if (!bound) return null;
    const generationMatches =
      bound.generations[track] === gates.generations[track];
    if (!generationMatches && now - gates.changedAt[track] < STALE_MS)
      return null;
    const original = bound.levels?.[track];
    if (
      !generationMatches ||
      bound.failed ||
      elapsed >= STALE_MS ||
      (original?.status === "active" &&
        (original.ageMs ?? STALE_MS) + elapsed >= STALE_MS)
    ) {
      return {
        status: "stale",
        peakDbfs: null,
        rmsDbfs: null,
        clipped: false,
        ageMs: null,
      };
    }
    return original ?? null;
  };
  return { system: level("system"), microphone: level("microphone"), visible };
}
