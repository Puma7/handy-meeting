import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage } from "./api";
import { defaultTemplate, validateLibrary } from "./domain";
import type {
  AnalysisTemplate,
  DesktopApi,
  Locale,
  TemplateLibrary,
} from "./types";

export const TEMPLATE_KEY = "handymeeting.rust.beta.templates.v1";
function readBrowser(locale: Locale) {
  const fallback = [defaultTemplate(locale)];
  try {
    const raw = localStorage.getItem(TEMPLATE_KEY);
    if (!raw) return { templates: fallback, exists: false, invalid: false };
    const value: unknown = JSON.parse(raw);
    return validateLibrary(value)
      ? { templates: value, exists: true, invalid: false }
      : { templates: fallback, exists: true, invalid: true };
  } catch {
    return { templates: fallback, exists: true, invalid: true };
  }
}
function validNative(value: TemplateLibrary): boolean {
  return (
    Number.isSafeInteger(value.revision) &&
    value.revision >= 0 &&
    ((value.revision === 0 && value.templates.length === 0) ||
      validateLibrary(value.templates))
  );
}
export function useTemplateLibrary(
  api: DesktopApi,
  connected: boolean,
  locale: Locale,
) {
  const [browser] = useState(() => readBrowser(locale));
  const [library, setLibrary] = useState<TemplateLibrary>({
    revision: 0,
    templates: browser.templates,
  });
  const [ready, setReady] = useState(false),
    [pending, setPending] = useState(false),
    [error, setError] = useState<string | null>(null),
    [migrationError, setMigrationError] = useState<string | null>(null);
  const current = useRef(library),
    working = useRef(false),
    generation = useRef(0),
    mounted = useRef(true);
  current.current = library;
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      generation.current++;
    };
  }, []);
  const refresh = useCallback(async () => {
    if (!connected || working.current) return;
    const sequence = ++generation.current;
    working.current = true;
    setPending(true);
    setError(null);
    setMigrationError(null);
    try {
      let next = await api.listTemplates();
      if (!validNative(next))
        throw new Error("Invalid native template library");
      if (!next.templates.length) {
        next = await api.saveTemplates({
          revision: next.revision,
          templates: structuredClone(browser.templates),
        });
      }
      if (!validNative(next) || !next.templates.length)
        throw new Error("Invalid saved template library");
      if (!mounted.current || sequence !== generation.current) return;
      current.current = next;
      setLibrary(next);
      setReady(true);
    } catch (failure) {
      if (mounted.current && sequence === generation.current) {
        setError(errorMessage(failure));
        setReady(false);
      }
    } finally {
      working.current = false;
      if (mounted.current && sequence === generation.current) setPending(false);
    }
  }, [api, connected, browser]);
  useEffect(() => {
    if (connected) void refresh();
  }, [connected, refresh]);
  const commit = useCallback(
    async (templates: AnalysisTemplate[]): Promise<boolean> => {
      if (!ready || working.current) return false;
      if (!validateLibrary(templates)) {
        setError(
          locale === "de"
            ? "Bitte die Vorlagen- und Zeichenlimits prüfen."
            : "Please check template and character limits.",
        );
        return false;
      }
      working.current = true;
      setPending(true);
      setError(null);
      try {
        const next = await api.saveTemplates({
          revision: current.current.revision,
          templates: structuredClone(templates),
        });
        if (!validNative(next) || !next.templates.length)
          throw new Error("Invalid saved template library");
        if (!mounted.current) return false;
        current.current = next;
        setLibrary(next);
        return true;
      } catch (failure) {
        if (mounted.current) setError(errorMessage(failure));
        return false;
      } finally {
        working.current = false;
        if (mounted.current) setPending(false);
      }
    },
    [api, ready, locale],
  );
  const importBrowser = useCallback(async () => {
    if (!ready || working.current || browser.invalid || !browser.exists) return;
    working.current = true;
    setPending(true);
    setMigrationError(null);
    try {
      const next = await api.importTemplates(
        structuredClone(browser.templates),
      );
      if (!validNative(next) || !next.templates.length)
        throw new Error("Invalid imported template library");
      if (!mounted.current) return;
      current.current = next;
      setLibrary(next);
    } catch (failure) {
      if (mounted.current) setMigrationError(errorMessage(failure));
    } finally {
      working.current = false;
      if (mounted.current) setPending(false);
    }
  }, [api, ready, browser]);
  return {
    templates: library.templates,
    ready,
    pending,
    error,
    browserInvalid: browser.invalid,
    hasBrowserTemplates: browser.exists && !browser.invalid,
    migrationError,
    refresh,
    commit,
    importBrowser,
  };
}
