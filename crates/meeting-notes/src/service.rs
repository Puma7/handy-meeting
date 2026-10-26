use crate::{
    validate_answer, validate_template, Analysis, AnalysisControl, AnalysisProgress, Connection,
    Section, Source, Template, TextClient, PROMPT_VERSION,
};
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, io::Write, path::Path};
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
    validate_analysis(&result)?;
    Ok(result)
}
pub(crate) fn validate_analysis(result: &Analysis) -> Result<()> {
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
    ensure!(
        result
            .sources
            .iter()
            .map(|s| &s.id)
            .collect::<HashSet<_>>()
            .len()
            == result.sources.len(),
        "Doppelte Quellenkennung im gespeicherten Ergebnis."
    );
    Ok(())
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
    analyze_controlled(
        root,
        meeting_id,
        sources,
        template,
        connection,
        resume,
        &AnalysisControl::default(),
        |_| {},
    )
}

#[allow(clippy::too_many_arguments)]
pub fn analyze_controlled(
    root: &Path,
    meeting_id: &str,
    sources: Vec<Source>,
    template: Template,
    connection: Connection,
    resume: Option<&str>,
    control: &AnalysisControl,
    mut on_progress: impl FnMut(AnalysisProgress),
) -> Result<Analysis> {
    control.check()?;
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
        model: connection.model.clone(),
        mode: connection.mode.clone(),
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
                    && result.meeting_id == analysis.meeting_id
                    && result.source_hash == analysis.source_hash
                    && result.model == analysis.model
                    && result.mode == analysis.mode
                    && result.endpoint == analysis.endpoint
                    && serde_json::to_vec(&result.template)?
                        == serde_json::to_vec(&analysis.template)?,
                "Gespeicherte Auswertung passt nicht zum Arbeitsstand."
            );
            let batches = source_batches(&result.sources);
            let tasks = result.template.steps.len() * batches.len();
            on_progress(AnalysisProgress {
                run_id: result.id.clone(),
                stage: "finished".into(),
                step_name: None,
                step_index: result.template.steps.len(),
                total_steps: result.template.steps.len(),
                batch_index: batches.len(),
                total_batches: batches.len(),
                confirmed_tasks: tasks,
                total_tasks: tasks,
            });
            return Ok(result);
        }
    } else {
        fs::create_dir(&folder)?;
        create_json(&folder.join("snapshot.json"), &analysis)?;
    }
    let batches = source_batches(&analysis.sources);
    let mut progress = AnalysisProgress {
        run_id: analysis.id.clone(),
        stage: "preparing".into(),
        step_name: None,
        step_index: 0,
        total_steps: analysis.template.steps.len(),
        batch_index: 0,
        total_batches: batches.len(),
        confirmed_tasks: 0,
        total_tasks: analysis.template.steps.len() * batches.len(),
    };
    on_progress(progress.clone());
    for (step_index, step) in analysis.template.steps.iter().enumerate() {
        let mut items = Vec::new();
        for (batch_index, sources) in batches.iter().enumerate() {
            control.check()?;
            progress.step_name = Some(step.name.clone());
            progress.step_index = step_index + 1;
            progress.batch_index = batch_index + 1;
            let path = folder.join(format!("step-{step_index:04}-{batch_index:06}.json"));
            let parsed = if path.exists() {
                progress.stage = "validating".into();
                on_progress(progress.clone());
                validate_answer(&fs::read_to_string(&path)?, sources, &step.kind)?
            } else {
                progress.stage = "requesting".into();
                on_progress(progress.clone());
                let answer = client.generate_controlled(sources, step, control)?;
                control.check()?;
                progress.stage = "validating".into();
                on_progress(progress.clone());
                let parsed = validate_answer(&answer, sources, &step.kind)?;
                control.check()?;
                progress.stage = "saving".into();
                on_progress(progress.clone());
                create_json(&path, &serde_json::json!({"items":parsed}))?;
                parsed
            };
            progress.confirmed_tasks += 1;
            progress.stage = "saving".into();
            on_progress(progress.clone());
            items.extend(parsed);
        }
        analysis.sections.push(Section {
            id: step.id.clone(),
            name: step.name.clone(),
            kind: step.kind.clone(),
            items,
        });
    }
    control.check()?;
    create_export(
        &folder.join("result.txt"),
        render_text(&analysis).as_bytes(),
    )?;
    control.check()?;
    create_export(
        &folder.join("result.md"),
        crate::render_markdown(&analysis)?.as_bytes(),
    )?;
    control.check()?;
    // This durable result is the commit point. Completion wins a later cancel.
    create_json(&folder.join("result.json"), &analysis)?;
    progress.stage = "finished".into();
    on_progress(progress);
    Ok(analysis)
}
fn source_batches(sources: &[Source]) -> Vec<Vec<Source>> {
    let mut batches = Vec::new();
    let mut batch = Vec::new();
    let mut size = 0;
    for source in sources {
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
    batches
}
fn create_export(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        ensure!(fs::read(path)?==bytes,"Ein gespeicherter Export weicht vom bestätigten Auswertungsstand ab; vorhandene Daten bleiben erhalten.");
        return Ok(());
    }
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().context("Auswertungsordner fehlt.")?)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path).map_err(|_| {
        anyhow::anyhow!(
            "Export konnte nicht atomar gesichert werden; vorhandene Fassungen bleiben erhalten."
        )
    })?;
    Ok(())
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
