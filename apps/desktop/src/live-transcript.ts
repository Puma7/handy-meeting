import type { AudioPart, Meeting } from "./types";

/**
 * Merges full and bounded publications without discarding newer confirmed sections.
 * Audio-journal sequence orders metadata; each transcript sequence orders its own text.
 */
export function mergeLiveTranscript(
  previous: Meeting | null,
  next: Meeting,
): Meeting {
  if (!previous || previous.id !== next.id || previous.modelId !== next.modelId)
    return next;
  const key = (part: AudioPart) =>
    part.file ?? `${part.source}:${part.startSample}`;
  const parts = new Map(previous.parts.map((part) => [key(part), part]));
  for (const part of next.parts) {
    const saved = parts.get(key(part));
    if (
      !saved ||
      (part.transcriptSequence ?? 0) > (saved.transcriptSequence ?? 0) ||
      ((part.transcriptSequence ?? 0) === (saved.transcriptSequence ?? 0) &&
        (part.confirmedSampleCount ?? part.sampleCount) >=
          (saved.confirmedSampleCount ?? saved.sampleCount))
    )
      parts.set(key(part), part);
  }
  return {
    ...(previous.snapshotSequence !== undefined &&
    next.snapshotSequence !== undefined &&
    next.snapshotSequence < previous.snapshotSequence
      ? previous
      : next),
    parts: [...parts.values()],
    snapshotSequence:
      previous.snapshotSequence === undefined
        ? next.snapshotSequence
        : Math.max(
            previous.snapshotSequence,
            next.snapshotSequence ?? previous.snapshotSequence,
          ),
    livePreviewLimited: Boolean(
      previous.livePreviewLimited && next.livePreviewLimited,
    ),
  };
}

/** A full refresh also picks up catch-up recognition in sections outside the poll preview. */
export function hydrateLiveTranscript(
  current: Meeting | null,
  full: Meeting,
): Meeting | null {
  if (!current || current.id !== full.id || current.modelId !== full.modelId)
    return current;
  const parts = new Map(
    full.parts.map((part) => [
      part.file ?? `${part.source}:${part.startSample}`,
      part,
    ]),
  );
  for (const part of current.parts) {
    const key = part.file ?? `${part.source}:${part.startSample}`;
    const saved = parts.get(key);
    if (
      !saved ||
      (part.transcriptSequence ?? 0) > (saved.transcriptSequence ?? 0) ||
      ((part.transcriptSequence ?? 0) === (saved.transcriptSequence ?? 0) &&
        (part.confirmedSampleCount ?? part.sampleCount) >
          (saved.confirmedSampleCount ?? saved.sampleCount))
    )
      parts.set(key, part);
  }
  return {
    ...current,
    parts: [...parts.values()],
    snapshotSequence:
      current.snapshotSequence === undefined
        ? full.snapshotSequence
        : Math.max(
            current.snapshotSequence,
            full.snapshotSequence ?? current.snapshotSequence,
          ),
    livePreviewLimited: false,
  };
}
