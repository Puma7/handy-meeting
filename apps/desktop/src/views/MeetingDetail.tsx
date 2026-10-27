import { ArrowLeft, Check, Copy, Sparkles } from "lucide-react";
import { meetingLabel } from "../domain";
import type { Meeting, Locale } from "../types";
import type { MessageKey } from "../i18n";
import TranscriptBody from "./TranscriptBody";
import SessionWarnings from "./SessionWarnings";
import MeetingFiles from "./MeetingFiles";
export default function MeetingDetail({
  meeting,
  locale,
  t,
  busy,
  copied,
  onBack,
  onCopy,
  onAnalyse,
  onOpenFolder,
  onExport,
  savedFile,
  selectedFile,
  speakers,
}: {
  meeting: Meeting;
  locale: Locale;
  t: Record<MessageKey, string>;
  busy: boolean;
  copied: boolean;
  onBack: () => void;
  onCopy: () => void;
  onAnalyse: () => void;
  onOpenFolder: () => void;
  onExport: () => void;
  savedFile: string | null;
  selectedFile?: string | null;
  speakers?: Record<string, string>;
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
      <div className="meeting-files">
        <MeetingFiles
          t={t}
          disabled={busy}
          onOpenFolder={onOpenFolder}
          onExport={onExport}
          savedFile={savedFile}
        />
      </div>
      <SessionWarnings warnings={meeting.warnings} locale={locale} t={t} />
      <TranscriptBody
        parts={parts}
        locale={locale}
        t={t}
        selectedFile={selectedFile}
        speakers={speakers}
      />
    </section>
  );
}
