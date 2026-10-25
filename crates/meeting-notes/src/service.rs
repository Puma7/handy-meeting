use crate::{
    validate_answer, validate_template, Analysis, Connection, Section, Source, Template,
    TextClient, PROMPT_VERSION,
};
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
pub fn source_hash(sources: &[Source]) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(sources)?)
    ))
}
fn create_json(path: &Path, data: &impl Serialize) -> Result<()> {
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().context("Auswertungsordner fehlt.")?)?;
    file.write_all(&serde_json::to_vec_pretty(data)?)?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path).map_err(|_|anyhow::anyhow!("Auswertungsstand konnte nicht atomar gespeichert werden; bestehende Fassungen bleiben erhalten."))?;
    Ok(())
}

pub fn read_analysis(path: &Path) -> Result<Analysis> {
    ensure!(
        fs::metadata(path)?.len() <= 64_000_000,
        "Auswertungsdatei zu groß."
    );
    let result: Analysis = serde_json::from_slice(&fs::read(path)?)?;
    validate_template(&result.template)?;
    ensure!(
        source_hash(&result.sources)? == result.source_hash
            && result.prompt_version == PROMPT_VERSION
            && result.sections.len() == result.template.steps.len(),
        "Auswertungsstand ist beschädigt oder nicht kompatibel."
    );
    for (section, step) in result.sections.iter().zip(&result.template.steps) {
        ensure!(
            section.id == step.id && section.name == step.name && section.kind == step.kind,
            "Auswertung und Vorlagenstand passen nicht zusammen."
        );
        for chunk in section.items.chunks(20) {
            validate_answer(
                &serde_json::to_string(&serde_json::json!({"items":chunk}))?,
                &result.sources,
                &section.kind,
            )?;
        }
    }
    Ok(result)
}

/// Sources and immutable run snapshots are supplied by orchestration, outside transport.
pub fn analyze(
    root: &Path,
    meeting_id: &str,
    sources: Vec<Source>,
    template: Template,
    connection: Connection,
    resume: Option<&str>,
) -> Result<Analysis> {
    validate_template(&template)?;
    ensure!(
        !sources.is_empty()
            && sources.len() <= 100_000
            && sources
                .iter()
                .all(|s| !s.text.is_empty() && s.text.chars().count() <= 1800),
        "Kein gültiger Transkriptstand für die Auswertung vorhanden."
    );
    ensure!(
        sources.iter().map(|s| &s.id).collect::<HashSet<_>>().len() == sources.len(),
        "Doppelte Quellenkennung."
    );
    let client = TextClient::new(&connection)?;
    let id = resume
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
    ensure!(
        id.len() == 32 && id.chars().all(|c| c.is_ascii_hexdigit()),
        "Ungültige Auswertungskennung."
    );
    fs::create_dir_all(root)?;
    let folder = root.join(&id);
    let hash = source_hash(&sources)?;
    let mut analysis = Analysis {
        id,
        meeting_id: meeting_id.to_string(),
        source_hash: hash,
        template,
        sections: Vec::new(),
        sources,
        created_at: chrono::Utc::now().to_rfc3339(),
        model: connection.model,
        mode: connection.mode,
        endpoint: client.base.to_string(),
        prompt_version: PROMPT_VERSION.to_string(),
    };
    if resume.is_some() {
        let previous: Analysis = serde_json::from_slice(&fs::read(folder.join("snapshot.json"))?)?;
        ensure!(
            previous.source_hash == analysis.source_hash
                && previous.meeting_id == analysis.meeting_id
                && previous.model == analysis.model
                && previous.mode == analysis.mode
                && previous.endpoint == analysis.endpoint
                && previous.prompt_version == PROMPT_VERSION
                && serde_json::to_vec(&previous.template)?
                    == serde_json::to_vec(&analysis.template)?,
            "Auswertungsstand passt nicht zu Transkript, Modell, Anbieter oder Vorlage."
        );
        analysis.created_at = previous.created_at;
        if folder.join("result.json").exists() {
            let result = read_analysis(&folder.join("result.json"))?;
            ensure!(
                result.id == analysis.id
                    && result.source_hash == analysis.source_hash
                    && result.model == analysis.model
                    && result.mode == analysis.mode
                    && result.endpoint == analysis.endpoint
                    && serde_json::to_vec(&result.template)?
                        == serde_json::to_vec(&analysis.template)?,
                "Gespeicherte Auswertung passt nicht zum Arbeitsstand."
            );
            return Ok(result);
        }
    } else {
        fs::create_dir(&folder)?;
        create_json(&folder.join("snapshot.json"), &analysis)?;
    }
    let mut batches: Vec<Vec<Source>> = Vec::new();
    let mut batch = Vec::new();
    let mut size = 0;
    for source in &analysis.sources {
        if size + source.text.chars().count() > 6500 && !batch.is_empty() {
            batches.push(batch);
            batch = Vec::new();
            size = 0;
        }
        size += source.text.chars().count();
        batch.push(source.clone());
    }
    if !batch.is_empty() {
        batches.push(batch);
    }
    for (step_index, step) in analysis.template.steps.iter().enumerate() {
        let mut items = Vec::new();
        for (batch_index, sources) in batches.iter().enumerate() {
            let path = folder.join(format!("step-{step_index:04}-{batch_index:06}.json"));
            let parsed = if path.exists() {
                validate_answer(&fs::read_to_string(&path)?, sources, &step.kind)?
            } else {
                let answer = client.generate(sources, step)?;
                let parsed = validate_answer(&answer, sources, &step.kind)?;
                create_json(&path, &serde_json::json!({"items":parsed}))?;
                parsed
            };
            items.extend(parsed);
        }
        analysis.sections.push(Section {
            id: step.id.clone(),
            name: step.name.clone(),
            kind: step.kind.clone(),
            items,
        });
    }
    create_json(&folder.join("result.json"), &analysis)?;
    let mut text = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(folder.join("result.txt"))?;
    text.write_all(render_text(&analysis).as_bytes())?;
    text.sync_all()?;
    Ok(analysis)
}
pub fn render_text(analysis: &Analysis) -> String {
    let mut text = format!("KI-Entwurf · {} · {}\nAbschnittszeiten sind keine Wortzeiten. Belege bestätigen Originalzitate, nicht automatisch die Deutung.\n", analysis.mode, analysis.model);
    for section in &analysis.sections {
        text.push_str(&format!("\n{}\n", section.name));
        if section.items.is_empty() {
            text.push_str("Keine ausdrücklich belegten Ergebnisse.\n");
        }
        for fact in &section.items {
            text.push_str(&format!("- {}", fact.text));
            if let Some(owner) = &fact.owner {
                text.push_str(&format!(" · {owner}"));
            }
            if let Some(due) = &fact.due {
                text.push_str(&format!(" · {due}"));
            }
            text.push('\n');
            for e in &fact.evidence {
                text.push_str(&format!("  [{}] {}\n", e.source_id, e.quote));
            }
        }
    }
    text
}
