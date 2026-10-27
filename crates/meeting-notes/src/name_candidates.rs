//! Name mentions are review candidates, never confirmed speaker identities.
use crate::{Analysis, Fact, Template};
use anyhow::{ensure, Result};
pub const TEMPLATE_ID: &str = "speaker-name-candidates-v1";
pub fn template() -> Result<Template> {
    let value: Template = serde_json::from_str(include_str!(
        "../../../apps/desktop/src/speaker-name-template.json"
    ))?;
    crate::validate_template(&value)?;
    Ok(value)
}
pub fn validate(analysis: &Analysis) -> Result<()> {
    for section in &analysis.sections {
        validate_items(&analysis.template.id, &section.items)?;
    }
    Ok(())
}
/// Checked per answer batch, before it is saved, so one bad batch cannot block a resumed run.
pub fn validate_items(template_id: &str, items: &[Fact]) -> Result<()> {
    if template_id != TEMPLATE_ID {
        return Ok(());
    }
    for item in items {
        ensure!(
            item.owner.is_none() && item.due.is_none(),
            "Namensvorschläge dürfen keine Zuständigen oder Termine zuweisen."
        );
        ensure!(
            !item.text.trim().is_empty()
                && item.text.chars().count() <= 80
                && !item.text.chars().any(char::is_control),
            "Ungültiger Namensvorschlag."
        );
        ensure!(
            !item.evidence.is_empty() && item.evidence.iter().all(|e| e.quote.contains(&item.text)),
            "Jeder Namensvorschlag benötigt seinen wörtlichen Namen im Originalzitat."
        );
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dedicated_prompt_never_promises_speaker_identification() {
        let template = template().unwrap();
        assert_eq!(template.id, TEMPLATE_ID);
        assert!(template.steps[0].prompt.contains("NOT evidence"));
        assert!(template.steps[0]
            .prompt
            .contains("owner and due must both be null"));
    }
}
