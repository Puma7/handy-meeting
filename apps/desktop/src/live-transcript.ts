import type { AudioPart, Meeting } from "./types";

/** Recent native snapshots are a transport optimization, not a replacement transcript. */
export function mergeLiveTranscript(
  previous: Meeting | null,
  next: Meeting,
): Meeting {
  if (
    !previous ||
    previous.id !== next.id ||
    previous.modelId !== next.modelId ||
    !next.livePreviewLimited
  )
    return next;
  const key = (part: AudioPart) =>
    part.file ?? `${part.source}:${part.startSample}`;
  const parts = new Map(previous.parts.map((part) => [key(part), part]));
  for (const part of next.parts) parts.set(key(part), part);
  return {
    ...next,
    parts: [...parts.values()],
    livePreviewLimited: Boolean(previous.livePreviewLimited),
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
  return { ...current, parts: [...parts.values()], livePreviewLimited: false };
}
