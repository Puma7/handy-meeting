import { ArrowLeft, Check, Copy, Sparkles } from "lucide-react";
import { meetingLabel } from "../domain";
import type { Meeting, Locale } from "../types";
import type { MessageKey } from "../i18n";
import TranscriptBody from "./TranscriptBody";
export default function MeetingDetail({
  meeting,
  locale,
  t,
  busy,
  copied,
  onBack,
  onCopy,
  onAnalyse,
}: {
  meeting: Meeting;
  locale: Locale;
  t: Record<MessageKey, string>;
  busy: boolean;
  copied: boolean;
  onBack: () => void;
  onCopy: () => void;
  onAnalyse: () => void;
}) {
  const parts = [...meeting.parts]
    .sort(
      (a, b) =>
        a.startSample - b.startSample || a.source.localeCompare(b.source),
    )
    .filter((part) => (part.text ?? "").trim());
  return (
    <section className="meeting-detail">
      <button className="text-button" onClick={onBack}>
        <ArrowLeft size={15} />
        {t.backHistory}
      </button>
      <div className="detail-heading">
        <div>
          <h2>{meeting.title}</h2>
          <p>{meetingLabel(meeting, locale)}</p>
          <p>{t.sourceTimes}</p>
        </div>
        <div className="detail-actions">
          <button
            className="secondary"
            disabled={busy || !parts.length}
            title={t.copyTooltip}
            onClick={onCopy}
          >
            {copied ? <Check size={16} /> : <Copy size={16} />}
            <span>{copied ? t.copied : t.copy}</span>
          </button>
          <button
            className="primary"
            disabled={busy || meeting.status !== "complete" || !parts.length}
            onClick={onAnalyse}
          >
            <Sparkles size={16} />
            {t.analyzeMeeting}
          </button>
        </div>
      </div>
      {meeting.status !== "complete" && (
        <div className="notice warning">{t.pendingNotes}</div>
      )}
      {meeting.warnings.map((warning, index) => (
        <div className="notice warning" key={index}>
          {warning}
        </div>
      ))}
      <TranscriptBody parts={parts} locale={locale} t={t} />
    </section>
  );
}
