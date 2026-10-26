use crate::models::Answer;
use crate::{Fact, Kind, Source, Template};
use anyhow::{ensure, Context, Result};
use std::collections::{HashMap, HashSet};
pub fn validate_template(template: &Template) -> Result<()> {
    ensure!(
        !template.name.trim().is_empty()
            && template.name.chars().count() <= 100
            && !template.steps.is_empty()
            && template.steps.len() <= 100,
        "Vorlage benötigt einen Namen und 1–100 Schritte."
    );
    let mut ids = HashSet::new();
    for step in &template.steps {
        ensure!(
            !step.id.is_empty()
                && ids.insert(&step.id)
                && !step.name.trim().is_empty()
                && step.name.chars().count() <= 100
                && !step.prompt.trim().is_empty()
                && step.prompt.chars().count() <= 8000,
            "Ungültiger oder doppelter Analyseschritt."
        );
    }
    Ok(())
}
pub fn validate_answer(json: &str, sources: &[Source], kind: &Kind) -> Result<Vec<Fact>> {
    let result: Answer =
        serde_json::from_str(json).context("Ungültige oder doppelte Felder in der KI-Antwort.")?;
    ensure!(result.items.len() <= 20, "Zu viele KI-Aussagen.");
    let allowed: HashMap<_, _> = sources
        .iter()
        .map(|s| (s.id.as_str(), s.text.as_str()))
        .collect();
    for fact in &result.items {
        ensure!(
            !fact.text.trim().is_empty()
                && fact.text.chars().count() <= 1600
                && (1..=6).contains(&fact.evidence.len()),
            "Eine KI-Aussage benötigt gültige Belege."
        );
        for e in &fact.evidence {
            ensure!(
                (8..=600).contains(&e.quote.chars().count())
                    && allowed
                        .get(e.source_id.as_str())
                        .is_some_and(|text| text.contains(&e.quote)),
                "Ein Quellenbeleg stimmt nicht mit dem Original überein."
            );
        }
        if *kind != Kind::Tasks {
            ensure!(
                fact.owner.is_none() && fact.due.is_none(),
                "Zuständige und Termine gehören ausschließlich zu Aufgaben."
            );
        }
        for value in [&fact.owner, &fact.due].into_iter().flatten() {
            ensure!(
                !value.trim().is_empty()
                    && value.chars().count() <= 200
                    && fact
                        .evidence
                        .iter()
                        .any(|e| e.quote.to_lowercase().contains(&value.to_lowercase())),
                "Zuständigkeit oder Termin ist nicht wörtlich belegt."
            );
        }
    }
    Ok(result.items)
}
