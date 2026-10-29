import { useEffect, useRef } from "react";
import type { AudioDevice } from "./types";

export type DeviceSource = "system" | "microphone";

/** A selected device that a newer device list no longer contains. */
export interface DeviceLoss {
  source: DeviceSource;
  id: string;
  name: string;
  /** The device belonged to a running recording; its track no longer records. */
  duringRecording: boolean;
  /** Name and ID of the device chosen instead, when not recording. */
  replacement?: string;
  replacementId?: string;
  /** The notice was closed; the record still restores the device when it returns. */
  dismissed?: boolean;
}

/** Interval of the automatic device-list refresh while the window is visible. */
export const DEVICE_REFRESH_MS = 3000;

/**
 * Lists the selected devices of `previous` that `next` no longer contains. A device that
 * was never listed cannot be reported as lost.
 */
export function lostSelections(
  previous: AudioDevice[],
  next: AudioDevice[],
  selected: Record<DeviceSource, string | null>,
): { source: DeviceSource; id: string; name: string }[] {
  return (["system", "microphone"] as const).flatMap((source) => {
    const id = selected[source];
    const known = previous.find((d) => d.source === source && d.id === id);
    if (!id || !known) return [];
    return next.some((d) => d.source === source && d.id === id)
      ? []
      : [{ source, id, name: known.name }];
  });
}

/**
 * Re-reads the device list while the window is visible, on focus and on a reported device
 * change, so connected or removed headsets appear without a restart. Only one query runs
 * at a time; a request during a query runs once right after it, because the running query
 * may predate the change. Automatic failures stay silent; a manual refresh reports them.
 */
export function useDeviceRefresh(
  refresh: () => Promise<void>,
  enabled: boolean,
) {
  const latest = useRef(refresh);
  latest.current = refresh;
  useEffect(() => {
    if (!enabled) return;
    let running = false,
      again = false,
      disposed = false;
    const run = () => {
      if (disposed || document.visibilityState === "hidden") return;
      if (running) {
        again = true;
        return;
      }
      running = true;
      void latest
        .current()
        .catch(() => undefined)
        .finally(() => {
          running = false;
          if (again) {
            again = false;
            run();
          }
        });
    };
    const timer = setInterval(run, DEVICE_REFRESH_MS);
    const media = navigator.mediaDevices;
    window.addEventListener("focus", run);
    document.addEventListener("visibilitychange", run);
    media?.addEventListener?.("devicechange", run);
    return () => {
      disposed = true;
      clearInterval(timer);
      window.removeEventListener("focus", run);
      document.removeEventListener("visibilitychange", run);
      media?.removeEventListener?.("devicechange", run);
    };
  }, [enabled]);
}
