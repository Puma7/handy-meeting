// Status bars poll quickly only while their job runs; idle checks stay cheap.
export const ACTIVE_POLL_MS = 500;
export const IDLE_POLL_MS = 3000;
const REFRESH_EVENT = "handymeeting-job-status-refresh";

/** Asks every mounted status poller to check now, e.g. right after a job was started. */
export function requestJobStatusRefresh() {
  if (typeof window !== "undefined")
    window.dispatchEvent(new Event(REFRESH_EVENT));
}

/**
 * Runs `poll` now and again after ACTIVE_POLL_MS while it reports active work,
 * otherwise after IDLE_POLL_MS. Only one request is in flight at a time.
 * Returns a function that stops polling.
 */
export function startAdaptivePoll(poll: () => Promise<boolean>): () => void {
  let stopped = false,
    inFlight = false,
    again = false,
    active = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  async function tick() {
    if (stopped) return;
    if (inFlight) {
      again = true;
      return;
    }
    clearTimeout(timer);
    timer = undefined;
    inFlight = true;
    try {
      active = await poll();
    } catch {
      // Keep the previous pace; the caller shows its own polling error.
    } finally {
      inFlight = false;
    }
    if (stopped) return;
    if (again) {
      again = false;
      void tick();
      return;
    }
    timer = setTimeout(
      () => void tick(),
      active ? ACTIVE_POLL_MS : IDLE_POLL_MS,
    );
  }
  const refresh = () => void tick();
  window.addEventListener(REFRESH_EVENT, refresh);
  void tick();
  return () => {
    stopped = true;
    clearTimeout(timer);
    window.removeEventListener(REFRESH_EVENT, refresh);
  };
}
