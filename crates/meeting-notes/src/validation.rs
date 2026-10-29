use crate::models::Answer;
use crate::{CheckedAnswer, Fact, Kind, RejectionReason, Rejections, Source, Template};
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
/// Maximum number of statements kept from one model answer.
pub const MAX_ITEMS_PER_ANSWER: usize = 20;

/// Screens one model answer. Structurally invalid JSON or a wrong schema fails hard; individual
/// statements that fail the literal source check are discarded and counted instead of failing the
/// whole analysis (a deterministic retry would reproduce the same rejection). Kept statements are
/// unchanged model output that passed every check; nothing is repaired or invented.
pub fn validate_answer(
    json: &str,
    sources: &[Source],
    kind: &Kind,
    template_id: &str,
) -> Result<CheckedAnswer> {
    let answer: Answer =
        serde_json::from_str(json).context("Ungültige oder doppelte Felder in der KI-Antwort.")?;
    Ok(screen_facts(
        answer.items,
        sources,
        kind,
        template_id,
        MAX_ITEMS_PER_ANSWER,
    ))
}

/// Applies the literal source check to already parsed statements and keeps at most `limit`
/// valid ones in their original order. Discarded statements are only counted.
pub(crate) fn screen_facts(
    facts: Vec<Fact>,
    sources: &[Source],
    kind: &Kind,
    template_id: &str,
    limit: usize,
) -> CheckedAnswer {
    let allowed = allowed_sources(sources);
    let mut checked = CheckedAnswer::default();
    for fact in facts {
        let reason = check_fact(&fact, &allowed, kind)
            .or_else(|| generation_check(&fact, kind))
            .or_else(|| {
                crate::name_candidates::check_item(template_id, &fact)
                    .err()
                    .map(|_| RejectionReason::InvalidNameCandidate)
            });
        match reason {
            Some(reason) => *checked.rejected.entry(reason).or_default() += 1,
            None if checked.items.len() >= limit => {
                *checked
                    .rejected
                    .entry(RejectionReason::ItemLimit)
                    .or_default() += 1
            }
            None => checked.items.push(fact),
        }
    }
    checked
}

/// Additional conservative rules for newly generated answers only. Historical checkpoints
/// still use their original literal-source contract; reading never rewrites older work.
fn generation_check(fact: &Fact, kind: &Kind) -> Option<RejectionReason> {
    let quoted: HashSet<String> = fact
        .evidence
        .iter()
        .flat_map(|e| number_tokens(&e.quote))
        .collect();
    if number_tokens(&fact.text)
        .iter()
        .any(|n| !quoted.contains(n))
    {
        return Some(RejectionReason::NumberNotQuoted);
    }
    if *kind == Kind::Tasks && fact.evidence.iter().all(|e| nonbinding(&e.quote)) {
        return Some(RejectionReason::NonbindingTask);
    }
    None
}

fn number_tokens(text: &str) -> Vec<String> {
    let chars: Vec<_> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut token = String::new();
    for (index, &c) in chars.iter().enumerate() {
        if c.is_numeric()
            || (!token.is_empty()
                && matches!(c, '.' | ',' | ':' | '/')
                && chars.get(index + 1).is_some_and(|c| c.is_numeric()))
        {
            token.push(c);
        } else if !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    tokens
}

fn nonbinding(quote: &str) -> bool {
    let text = quote.to_lowercase();
    [
        "vielleicht",
        "könnten ",
        "könnte ",
        "wäre es",
        "unverbindlich",
        "no commitment",
        "we could",
        "maybe",
        "perhaps",
        "might ",
    ]
    .iter()
    .any(|marker| text.contains(marker))
        || ((text.contains("werde ") || text.contains("werden ")) && text.contains(" nicht "))
        || text.contains("will not ")
        || text.contains("won't ")
}

/// Reads a saved per-batch checkpoint strictly: saved items are never filtered or repaired,
/// so a damaged checkpoint blocks instead of silently shrinking confirmed work.
/// Name-candidate rules are checked separately by the caller (legacy batches are set aside).
pub(crate) fn read_checkpoint(
    json: &str,
    sources: &[Source],
    kind: &Kind,
) -> Result<CheckedAnswer> {
    let checkpoint: CheckedAnswer = serde_json::from_str(json)
        .context("Ungültige oder doppelte Felder im gespeicherten Auswertungsabschnitt.")?;
    ensure!(
        checkpoint.items.len() <= MAX_ITEMS_PER_ANSWER,
        "Zu viele KI-Aussagen im gespeicherten Auswertungsabschnitt."
    );
    verify_facts(&checkpoint.items, sources, kind)?;
    validate_rejections(&checkpoint.rejected)?;
    Ok(checkpoint)
}

/// Strict check for stored statements: every item must pass the same literal source check.
pub(crate) fn verify_facts(items: &[Fact], sources: &[Source], kind: &Kind) -> Result<()> {
    let allowed = allowed_sources(sources);
    for fact in items {
        if let Some(reason) = check_fact(fact, &allowed, kind) {
            anyhow::bail!(
                "Eine gespeicherte KI-Aussage besteht die Quellenprüfung nicht ({reason:?})."
            );
        }
    }
    Ok(())
}

/// Recorded counts are bounded, positive and never exceed what one stored run can contain.
pub(crate) fn validate_rejections(rejected: &Rejections) -> Result<()> {
    ensure!(
        rejected
            .values()
            .all(|count| (1..=1_000_000).contains(count)),
        "Ungültige Anzahl verworfener KI-Aussagen."
    );
    Ok(())
}

fn allowed_sources(sources: &[Source]) -> HashMap<&str, &str> {
    sources
        .iter()
        .map(|s| (s.id.as_str(), s.text.as_str()))
        .collect()
}

fn check_fact(fact: &Fact, allowed: &HashMap<&str, &str>, kind: &Kind) -> Option<RejectionReason> {
    if fact.text.trim().is_empty() || fact.text.chars().count() > 1600 {
        return Some(RejectionReason::InvalidText);
    }
    if !(1..=6).contains(&fact.evidence.len()) {
        return Some(RejectionReason::EvidenceCount);
    }
    let literal = fact.evidence.iter().all(|e| {
        (8..=600).contains(&e.quote.chars().count())
            && allowed
                .get(e.source_id.as_str())
                .is_some_and(|text| text.contains(&e.quote))
    });
    if !literal {
        return Some(RejectionReason::QuoteNotLiteral);
    }
    if *kind != Kind::Tasks && (fact.owner.is_some() || fact.due.is_some()) {
        return Some(RejectionReason::OwnerOrDueOutsideTasks);
    }
    let verified = [&fact.owner, &fact.due].into_iter().flatten().all(|value| {
        !value.trim().is_empty()
            && value.chars().count() <= 200
            && fact
                .evidence
                .iter()
                .any(|e| e.quote.to_lowercase().contains(&value.to_lowercase()))
    });
    if !verified {
        return Some(RejectionReason::UnverifiedOwnerOrDue);
    }
    None
}

#[cfg(test)]
mod generation_tests {
    use super::*;
    fn check(quote: &str, text: &str, kind: Kind) -> CheckedAnswer {
        let source = Source {
            id: "synthetic".into(),
            source: "system".into(),
            start_ms: 0,
            end_ms: 1000,
            text: quote.into(),
        };
        let answer = serde_json::json!({"items":[{"text":text,"owner":null,"due":null,"evidence":[{"sourceId":"synthetic","quote":quote}]}]}).to_string();
        validate_answer(&answer, &[source], &kind, "synthetic").unwrap()
    }
    #[test]
    fn introduced_numbers_are_not_rescued_by_a_literal_quote() {
        for (quote, text) in [
            (
                "Es liegen sechsundvierzig Entwürfe vor.",
                "Es liegen 640 Entwürfe vor.",
            ),
            ("Es liegen 850 Entwürfe vor.", "Es liegen 8 Entwürfe vor."),
            (
                "Der Anteil beträgt 2,5 Prozent.",
                "Der Anteil beträgt 25 Prozent.",
            ),
        ] {
            assert!(check(quote, text, Kind::Summary).items.is_empty());
        }
        assert_eq!(
            check(
                "Es liegen 46 Entwürfe vor.",
                "46 Entwürfe liegen vor.",
                Kind::Summary
            )
            .items
            .len(),
            1
        );
        assert_eq!(
            check(
                "Es liegen sechsundvierzig Entwürfe vor.",
                "Sechsundvierzig Entwürfe liegen vor.",
                Kind::Summary
            )
            .items
            .len(),
            1
        );
    }
    #[test]
    fn an_explicitly_nonbinding_quote_is_not_a_task() {
        for quote in [
            "Vielleicht könnten wir den Entwurf prüfen.",
            "We could review the draft, but no commitment yet.",
            "Ich werde den Entwurf nicht prüfen.",
        ] {
            assert!(check(quote, "Entwurf prüfen", Kind::Tasks).items.is_empty());
            assert_eq!(
                check(quote, "Unverbindlicher Vorschlag", Kind::Summary)
                    .items
                    .len(),
                1
            );
        }
        assert_eq!(
            check(
                "Ich übernehme die Prüfung des Entwurfs.",
                "Entwurf prüfen",
                Kind::Tasks
            )
            .items
            .len(),
            1
        );
    }
    #[test]
    fn stricter_generation_rules_do_not_rewrite_historical_checkpoints() {
        let source = Source {
            id: "old".into(),
            source: "system".into(),
            start_ms: 0,
            end_ms: 1000,
            text: "Es liegen sechsundvierzig Entwürfe vor.".into(),
        };
        let checkpoint = serde_json::json!({"items":[{"text":"640 Entwürfe","owner":null,"due":null,"evidence":[{"sourceId":"old","quote":source.text}]}]}).to_string();
        let old = read_checkpoint(&checkpoint, &[source], &Kind::Summary).unwrap();
        assert_eq!(
            old.items[0].text, "640 Entwürfe",
            "Old evidence is preserved, not silently repaired or hidden"
        );
    }
}
