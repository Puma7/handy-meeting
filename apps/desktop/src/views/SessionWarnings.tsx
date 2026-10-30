import { describeWarning } from "../domain";
import { useLayoutEffect, useRef } from "react";
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
  const detail = useRef<HTMLDetailsElement>(null);
  const previous = useRef(new Set<string>());
  // New raw warnings reopen once; fresh polling arrays with identical warnings
  // preserve the user's closed disclosure and never steal focus.
  useLayoutEffect(() => {
    const current = new Set(warnings);
    if (
      [...current].some((warning) => !previous.current.has(warning)) &&
      detail.current
    )
      detail.current.open = true;
    previous.current = current;
  }, [warnings]);
  const originals = [...new Set(warnings)];
  if (!originals.length) return null;
  const descriptions = [
    ...new Set(originals.map((value) => describeWarning(value, locale))),
  ];
  const technical = originals.filter(
    (value) => describeWarning(value, locale) !== value,
  );
  // Capture caveats start visible; the original technical text stays in its disclosure.
  return (
    <details
      ref={detail}
      open
      className="session-warnings"
      aria-label={t.warnings}
    >
      <summary>
        <strong>{technical.length ? t.audioQuality : t.warnings}</strong>
        {descriptions.length > 1 && ` (${descriptions.length})`}
      </summary>
      {descriptions.map((value) => (
        <p key={value}>{value}</p>
      ))}
      {!!technical.length && (
        <p className="muted">
          {locale === "de"
            ? "Das Treibersignal belegt keine hörbare Lücke; lückenlose Aufnahme ist damit aber auch nicht bestätigt. Höre die betroffene Stelle ab. Fehlender Ton wird nicht wiederhergestellt."
            : "The driver signal does not prove an audible gap or confirm uninterrupted capture. Listen to the affected range. Missing audio is not restored."}
        </p>
      )}
      {!!technical.length && (
        <details>
          <summary>{t.technicalDetails}</summary>
          <p className="muted">{t.audioQualityHelp}</p>
          {technical.map((value) => (
            <p key={value}>{value}</p>
          ))}
        </details>
      )}
    </details>
  );
}
