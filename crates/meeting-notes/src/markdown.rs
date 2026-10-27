use crate::{Analysis, Kind};
use anyhow::{Context, Result};
use std::collections::HashMap;

/// Only producer-owned fragment links/anchors are emitted. Every model,
/// transcript and template value is rendered as escaped literal content.
pub fn render_markdown(analysis: &Analysis) -> Result<String> {
    crate::service::validate_analysis(analysis)?;
    let anchors: HashMap<_, _> = analysis
        .sources
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), format!("source-{:05}", i + 1)))
        .collect();
    let mut output=format!("# Meeting-Auswertung\n\nKI-Entwurf · {} · Modell: {}\n\nBelege bestätigen Originalzitate, nicht automatisch die inhaltliche Deutung. Zeiten bezeichnen Aufnahmeabschnitte, keine exakten Wortzeiten. Unbekannte Zuständige und Termine bleiben unbekannt.\n",literal(&analysis.mode),literal(&analysis.model));
    for section in &analysis.sections {
        output.push_str(&format!("\n## {}\n\n", literal(&section.name)));
        if section.items.is_empty() {
            output.push_str("Keine ausdrücklich belegten Ergebnisse.\n");
        }
        for fact in &section.items {
            output.push_str(&format!("- {}", literal(&fact.text).replace('\n', "\n  ")));
            if section.kind == Kind::Tasks {
                output.push_str(&format!(
                    " · Zuständig: {} · Termin: {}",
                    fact.owner
                        .as_deref()
                        .map(literal)
                        .unwrap_or_else(|| "nicht genannt".into()),
                    fact.due
                        .as_deref()
                        .map(literal)
                        .unwrap_or_else(|| "nicht genannt".into())
                ));
            }
            for evidence in &fact.evidence {
                let anchor = anchors
                    .get(evidence.source_id.as_str())
                    .context("Der Originalbeleg besitzt keinen vorhandenen Quellenanker.")?;
                output.push_str(&format!(" [{}](#{anchor})", literal(&evidence.source_id)));
            }
            output.push('\n');
            for evidence in &fact.evidence {
                output.push_str(&format!(
                    "\n  > {}\n",
                    literal(&evidence.quote).replace('\n', "\n  > ")
                ));
            }
        }
        if let Some(note) = crate::service::rejected_note(section) {
            output.push_str(&format!("\n{note}\n"));
        }
    }
    output.push_str("\n## Originaltranskript\n\nDieser unveränderte Quellenstand gehört zu dieser Auswertungsfassung.\n");
    for (index, source) in analysis.sources.iter().enumerate() {
        output.push_str(&format!(
            "\n<a id=\"source-{:05}\"></a>\n\n### Quelle {} · {}–{} · {}\n\n{}\n",
            index + 1,
            literal(&source.id),
            clock(source.start_ms),
            clock(source.end_ms),
            literal(&source.source),
            literal(&source.text)
        ));
    }
    Ok(output)
}

fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        (seconds / 60) % 60,
        seconds % 60
    )
}
fn literal(value: &str) -> String {
    let mut output = String::new();
    for c in value.chars() {
        match c {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '\r' => output.push(' '),
            c if c.is_ascii_punctuation() => {
                output.push('\\');
                output.push(c);
            }
            c => output.push(c),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Evidence, Fact, Section, Source, Step, Template};
    fn fixture() -> Analysis {
        let sources=vec![Source{id:"S00001".into(),source:"system".into(),start_ms:0,end_ms:1000,text:"Anna sagt: <script>alert(1)</script> ![Bild](https://evil.invalid/a) und C:\\private\\meeting.wav bleiben Daten.".into()}];
        let step = Step {
            id: "summary".into(),
            name: "<img src=x onerror=bad> [Link](https://evil.invalid)".into(),
            kind: Kind::Summary,
            prompt: "Literal export fixture".into(),
        };
        Analysis {
            id: "fixture".into(),
            meeting_id: "fixture".into(),
            source_hash: crate::source_hash(&sources).unwrap(),
            template: Template {
                id: "fixture".into(),
                name: "Fixture".into(),
                steps: vec![step.clone()],
            },
            sections: vec![Section {
                id: step.id,
                name: step.name,
                kind: step.kind,
                items: vec![Fact {
                    text: "<iframe src=https://evil.invalid> ![a](file:///secret)".into(),
                    owner: None,
                    due: None,
                    evidence: vec![Evidence {
                        source_id: "S00001".into(),
                        quote: sources[0].text.clone(),
                    }],
                }],
                rejected: Default::default(),
            }],
            sources,
            created_at: "fixture".into(),
            model: "model".into(),
            mode: "local".into(),
            endpoint: "http://127.0.0.1/v1/".into(),
            prompt_version: crate::PROMPT_VERSION.into(),
            result_schema: crate::RESULT_SCHEMA,
        }
    }
    #[test]
    fn markdown_keeps_quotes_and_only_static_internal_source_links() {
        let markdown = render_markdown(&fixture()).unwrap();
        assert!(markdown.contains("[S00001](#source-00001)"));
        assert!(markdown.contains("<a id=\"source-00001\"></a>"));
        for unsafe_text in [
            "<script>",
            "<img ",
            "<iframe ",
            "![",
            "https://evil",
            "file://",
            "C:\\private",
        ] {
            assert!(!markdown.contains(unsafe_text), "{unsafe_text}");
        }
        assert!(markdown.contains("&lt;script&gt;"));
        assert!(markdown.contains("keine exakten Wortzeiten"));
    }
    #[test]
    fn markdown_refuses_nonexistent_or_changed_original_evidence() {
        let mut analysis = fixture();
        analysis.sections[0].items[0].evidence[0].quote = "This quote was never spoken.".into();
        assert!(render_markdown(&analysis).is_err());
    }
}
