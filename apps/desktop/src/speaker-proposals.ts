import template from "./speaker-name-template.json";
import type { AnalysisTemplate, AnalysisResult, Meeting } from "./types";
export const SPEAKER_NAME_TEMPLATE = template as AnalysisTemplate;
export function nameCandidates(analyses: AnalysisResult[], meeting: Meeting) {
  const candidates: {
    name: string;
    quote: string;
    file: string;
    analysisId: string;
  }[] = [];
  for (const analysis of analyses) {
    // Results of an earlier prompt version stay read-only and never feed new speaker labels.
    if (
      analysis.meetingId !== meeting.id ||
      analysis.template.id !== SPEAKER_NAME_TEMPLATE.id ||
      analysis.currentPrompt === false
    )
      continue;
    for (const section of analysis.sections)
      for (const item of section.items) {
        if (
          item.owner !== null ||
          item.due !== null ||
          !/^[\p{L}\p{M}][\p{L}\p{M} .'-]{0,79}$/u.test(item.text)
        )
          continue;
        for (const evidence of item.evidence) {
          if (!evidence.quote.includes(item.text)) continue;
          const source = analysis.sources.find(
            (s) =>
              s.id === evidence.sourceId && s.text.includes(evidence.quote),
          );
          const part =
            source &&
            meeting.parts.find(
              (p) =>
                p.file &&
                p.source === source.source &&
                p.startSample / 16 === source.startMs &&
                p.text?.includes(evidence.quote),
            );
          if (part?.file)
            candidates.push({
              name: item.text,
              quote: evidence.quote,
              file: part.file,
              analysisId: analysis.id,
            });
        }
      }
  }
  return candidates;
}
