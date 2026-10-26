import { Download, FolderOpen } from "lucide-react";
import type { MessageKey } from "../i18n";

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
  onExport: () => void;
  savedFile: string | null;
}) {
  return (
    <>
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
        onClick={onExport}
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
