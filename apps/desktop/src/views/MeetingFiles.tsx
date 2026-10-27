import { Download, FolderOpen } from "lucide-react";
import type { MessageKey } from "../i18n";
import { useState } from "react";
import type { TranscriptFormat } from "../types";

export default function MeetingFiles({
  t,
  disabled,
  onOpenFolder,
  onExport,
  savedFile,
}: {
  t: Record<MessageKey, string>;
  disabled: boolean;
  onOpenFolder: () => void;
  onExport: (format: TranscriptFormat) => void;
  savedFile: string | null;
}) {
  const [format, setFormat] = useState<TranscriptFormat>("txt");
  return (
    <>
      <label className="transcript-format">
        {t.exportFormat}
        <select
          value={format}
          disabled={disabled}
          onChange={(event) =>
            setFormat(event.target.value as TranscriptFormat)
          }
        >
          <option value="txt">Text (.txt)</option>
          <option value="md">Markdown (.md)</option>
          <option value="json">JSON (.json)</option>
        </select>
      </label>
      <button
        className="secondary"
        disabled={disabled}
        title={t.openFolderHelp}
        onClick={onOpenFolder}
      >
        <FolderOpen size={16} />
        <span>{t.openFolder}</span>
      </button>
      <button
        className="secondary"
        disabled={disabled}
        title={t.exportTranscriptHelp}
        onClick={() => onExport(format)}
      >
        <Download size={16} />
        <span>{t.exportTranscript}</span>
      </button>
      {savedFile && (
        <p className="export-feedback" role="status">
          {t.exportSaved} {savedFile}
        </p>
      )}
      <p className="muted file-save-help">{t.savedFilesHelp}</p>
    </>
  );
}
