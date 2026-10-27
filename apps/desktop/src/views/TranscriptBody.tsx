import { AudioLines } from "lucide-react";
import { useEffect, useRef } from "react";
import { clock, trackName } from "../domain";
import type { AudioPart, Locale } from "../types";
import type { MessageKey } from "../i18n";
export default function TranscriptBody({
  parts,
  locale,
  t,
  selectedFile,
}: {
  parts: AudioPart[];
  locale: Locale;
  t: Record<MessageKey, string>;
  selectedFile?: string | null;
}) {
  const scrolledFile = useRef<string | null>(null);
  useEffect(() => {
    if (!selectedFile) {
      scrolledFile.current = null;
      return;
    }
    if (scrolledFile.current === selectedFile) return;
    const target = document.getElementById(
      "transcript-part-" + encodeURIComponent(selectedFile),
    );
    if (target) {
      target.scrollIntoView?.({ block: "center" });
      scrolledFile.current = selectedFile;
    }
  }, [selectedFile, parts]);
  return (
    <div
      className="transcript-content"
      role="region"
      aria-label={t.live}
      tabIndex={0}
      aria-live="polite"
    >
      {parts.length ? (
        parts.map((part, index) => (
          <article
            className="transcript-part"
            id={
              part.file
                ? "transcript-part-" + encodeURIComponent(part.file)
                : undefined
            }
            data-highlighted={part.file === selectedFile || undefined}
            key={part.file ?? part.source + part.startSample + index}
          >
            <div className="part-meta">
              <span>
                {clock(part.startSample / 16)}–
                {clock((part.startSample + part.sampleCount) / 16)}
              </span>
              <span>{trackName(part.source, locale)}</span>
              {part.status !== "complete" && <span>{t.pending}</span>}
            </div>
            <p>{part.text}</p>
          </article>
        ))
      ) : (
        <div className="empty">
          <AudioLines size={30} />
          <p>{t.emptyTranscript}</p>
        </div>
      )}
    </div>
  );
}
