import type { AnalysisTemplate } from "../types";
import type { MessageKey } from "../i18n";

export type TemplateAction = {
  kind: "new" | "duplicate" | "rename" | "delete";
  name: string;
  original: AnalysisTemplate;
};
export default function TemplateDialog({
  action,
  pending,
  error,
  t,
  onChange,
  onSave,
  onClose,
  onRefresh,
}: {
  action: TemplateAction;
  pending: boolean;
  error: string | null;
  t: Record<MessageKey, string>;
  onChange: (name: string) => void;
  onSave: () => void;
  onClose: () => void;
  onRefresh: () => void;
}) {
  const title =
    action.kind === "new"
      ? t.newTemplate
      : action.kind === "duplicate"
        ? t.duplicateTemplate
        : action.kind === "rename"
          ? t.renameTemplate
          : t.removeTemplate;
  return (
    <div className="modal-backdrop">
      <form
        className="dialog small"
        role="dialog"
        aria-modal="true"
        aria-labelledby="template-title"
        onSubmit={(event) => {
          event.preventDefault();
          if (!pending) onSave();
        }}
      >
        <h2 id="template-title">{title}</h2>
        <p>
          {action.kind === "new"
            ? t.newTemplateHelp
            : action.kind === "duplicate"
              ? t.duplicateTemplateHelp
              : action.kind === "delete"
                ? t.deleteTemplateHelp
                : t.renameTemplateHelp}
        </p>
        {action.kind === "delete" ? (
          <p>
            <strong>{action.original.name}</strong>
          </p>
        ) : (
          <label>
            {t.templateName}
            <input
              maxLength={100}
              value={action.name}
              disabled={pending}
              onChange={(event) => onChange(event.target.value)}
            />
          </label>
        )}
        {error && (
          <p role="alert" className="inline-error">
            {error}
          </p>
        )}
        {error && (
          <button
            type="button"
            className="secondary"
            disabled={pending}
            onClick={onRefresh}
          >
            {t.refreshTemplates}
          </button>
        )}
        <div className="dialog-actions">
          <button
            type="button"
            className="secondary"
            disabled={pending}
            onClick={onClose}
          >
            {t.cancel}
          </button>
          <button
            className="primary"
            disabled={
              pending || (action.kind !== "delete" && !action.name.trim())
            }
          >
            {pending
              ? t.busy
              : action.kind === "delete"
                ? t.removeTemplate
                : t.save}
          </button>
        </div>
      </form>
    </div>
  );
}
