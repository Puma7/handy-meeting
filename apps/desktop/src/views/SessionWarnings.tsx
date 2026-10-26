import { describeWarning } from "../domain";
import type { Locale } from "../types";
import type { MessageKey } from "../i18n";

export default function SessionWarnings({
  warnings,
  locale,
  t,
}: {
  warnings: string[];
  locale: Locale;
  t: Record<MessageKey, string>;
}) {
  const originals = [...new Set(warnings)];
  if (!originals.length) return null;
  const descriptions = [
    ...new Set(originals.map((value) => describeWarning(value, locale))),
  ];
  const technical = originals.filter(
    (value) => describeWarning(value, locale) !== value,
  );
  return (
    <aside className="session-warnings" aria-label={t.warnings}>
      <strong>{technical.length ? t.audioQuality : t.warnings}</strong>
      {descriptions.map((value) => (
        <p key={value}>{value}</p>
      ))}
      {!!technical.length && (
        <details>
          <summary>{t.technicalDetails}</summary>
          <p className="muted">{t.audioQualityHelp}</p>
          {technical.map((value) => (
            <p key={value}>{value}</p>
          ))}
        </details>
      )}
    </aside>
  );
}
