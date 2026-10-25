import type {
  AnalysisResult,
  AnalysisTemplate,
  Locale,
  Meeting,
  MeetingSummary,
  Track,
} from "./types";
export function meetingLabel(meeting: MeetingSummary, locale: Locale): string {
  return (
    meeting.title +
    " · " +
    new Date(meeting.startedAt).toLocaleString(
      locale === "de" ? "de-DE" : "en-GB",
    ) +
    " · " +
    clock(meeting.durationMs)
  );
}
export function defaultMeetingTitle(locale: Locale, date = new Date()): string {
  return (
    "Meeting " +
    date.toLocaleDateString(locale === "de" ? "de-DE" : "en-GB") +
    " " +
    date.toLocaleTimeString(locale === "de" ? "de-DE" : "en-GB", {
      hour: "2-digit",
      minute: "2-digit",
    })
  );
}
export function uid(): string {
  return crypto.randomUUID().replaceAll("-", "");
}
export const trackName = (track: Track, locale: Locale) =>
  track === "system"
    ? locale === "de"
      ? "Computerton"
      : "System audio"
    : locale === "de"
      ? "Mikrofon"
      : "Microphone";
export function clock(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  return (
    (s >= 3600 ? Math.floor(s / 3600) + ":" : "") +
    String(Math.floor(s / 60) % 60).padStart(2, "0") +
    ":" +
    String(s % 60).padStart(2, "0")
  );
}
export function defaultTemplate(): AnalysisTemplate {
  return {
    id: uid(),
    name: "Standard",
    steps: [
      {
        id: uid(),
        name: "Zusammenfassung",
        kind: "summary",
        prompt:
          "Fasse das Gespräch auf Deutsch knapp und verständlich zusammen. Hebe Themen und Ergebnisse hervor.",
      },
      {
        id: uid(),
        name: "Entscheidungen",
        kind: "decisions",
        prompt:
          "Extrahiere ausschließlich ausdrücklich getroffene Entscheidungen. Wünsche, Vorschläge, Aufgabenübernahmen und Zustandsbeschreibungen sind keine Beschlüsse. Ohne ausdrücklichen Beschluss bleibt items leer.",
      },
      {
        id: uid(),
        name: "To-dos",
        kind: "tasks",
        prompt:
          "Extrahiere ausschließlich ausdrücklich zugesagte oder zugewiesene Aufgaben. Zuständige und Termine bleiben unbekannt, soweit sie nicht ausdrücklich genannt und wörtlich belegt sind.",
      },
    ],
  };
}
export function validateLibrary(value: unknown): value is AnalysisTemplate[] {
  if (!Array.isArray(value) || value.length === 0) return false;
  const templateIds = new Set<string>();
  return value.every((t) => {
    if (
      !t ||
      typeof t.id !== "string" ||
      !t.id ||
      templateIds.has(t.id) ||
      typeof t.name !== "string" ||
      !t.name.trim() ||
      t.name.length > 100 ||
      !Array.isArray(t.steps) ||
      t.steps.length === 0
    )
      return false;
    templateIds.add(t.id);
    const stepIds = new Set<string>();
    return t.steps.every((s: unknown) => {
      const a = s as Record<string, unknown>;
      if (
        !a ||
        typeof a.id !== "string" ||
        !a.id ||
        stepIds.has(a.id) ||
        typeof a.name !== "string" ||
        !a.name.trim() ||
        a.name.length > 100 ||
        typeof a.prompt !== "string" ||
        !a.prompt.trim() ||
        a.prompt.length > 8000 ||
        !["summary", "decisions", "tasks", "analysis"].includes(String(a.kind))
      )
        return false;
      stepIds.add(a.id);
      return true;
    });
  });
}
export function renderTranscript(meeting: Meeting, locale: Locale): string {
  return [
    meeting.title,
    new Date(meeting.startedAt).toLocaleString(
      locale === "de" ? "de-DE" : "en-GB",
    ) +
      " · " +
      clock(meeting.durationMs),
    locale === "de"
      ? "Abschnittszeiten, keine exakten Wortzeiten."
      : "Section times, not exact word timestamps.",
    ...(meeting.status === "complete"
      ? []
      : [
          locale === "de"
            ? "UNVOLLSTÄNDIG: Es stehen Abschnitte aus."
            : "INCOMPLETE: Some sections remain pending.",
        ]),
    ...meeting.warnings,
    ...[...meeting.parts]
      .sort(
        (a, b) =>
          a.startSample - b.startSample || a.source.localeCompare(b.source),
      )
      .filter((p) => (p.text ?? "").trim())
      .map(
        (p) =>
          "\n[" +
          clock(p.startSample / 16) +
          "–" +
          clock((p.startSample + p.sampleCount) / 16) +
          "] " +
          trackName(p.source, locale) +
          "\n\n" +
          (p.text ?? ""),
      ),
  ].join("\n");
}
export function renderAnalysis(result: AnalysisResult): string {
  return [
    result.template.name,
    "KI-Entwurf / AI draft · " + result.createdAt,
    ...result.sections.flatMap((s) => [
      "\n" + s.name,
      ...(s.items.length
        ? s.items.map(
            (i) =>
              i.text +
              (i.owner ? " · " + i.owner : "") +
              (i.due ? " · " + i.due : "") +
              "\n" +
              i.evidence
                .map((e) => "[" + e.sourceId + "] „" + e.quote + "“")
                .join("\n"),
          )
        : ["—"]),
    ]),
  ].join("\n");
}
