//! Real local reference test for live notes (AGENTS.md): synthetic conversation text, an actual
//! local text model. Usage:
//!   cargo run -p meeting-notes --example live_reference -- <new-output-dir> [endpoint] [model]
//! Defaults: Ollama on http://127.0.0.1:11434/v1/ with gemma4:e4b.
use anyhow::{ensure, Context, Result};
use meeting_notes::live::{read_latest, LiveRunner, LiveText, LiveView, Trigger};
use meeting_notes::{AnalysisControl, Connection};
use std::{fs, path::Path, path::PathBuf, time::Instant};

fn part(index: u64, track: &str, text: &str) -> LiveText {
    LiveText {
        key: format!("{track}-{index:012}.wav"),
        track: track.into(),
        start_ms: index * 30_000,
        end_ms: index * 30_000 + 30_000,
        text: text.into(),
    }
}

/// Feeds the conversation in two halves, like a running meeting, and summarizes every window.
fn run(root: &Path, connection: Connection, parts: &[LiveText]) -> Result<(LiveView, usize)> {
    let mut runner = LiveRunner::start(root, "synthetic-live-reference", connection, "de")?;
    let control = AnalysisControl::default();
    let mut updates = 0;
    for visible in [parts.len() / 2, parts.len()] {
        loop {
            let pending = runner.pending(&parts[..visible]);
            if pending.is_empty() {
                break;
            }
            let outcome = runner.update(&pending, Trigger::Manual, &control)?;
            updates += 1;
            ensure!(
                outcome.overview_error.is_none(),
                "Overview failed: {:?}",
                outcome.overview_error
            );
        }
    }
    let stored = read_latest(root)?.context("The run must read back from disk.")?;
    Ok((stored, updates))
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let folder = PathBuf::from(args.next().context("Output directory required")?);
    let endpoint = args
        .next()
        .unwrap_or_else(|| "http://127.0.0.1:11434/v1/".into());
    let model = args.next().unwrap_or_else(|| "gemma4:e4b".into());
    ensure!(
        !folder.exists(),
        "Preserve prior reference runs; choose a new output directory."
    );
    fs::create_dir_all(&folder)?;
    let connection = || Connection {
        endpoint: endpoint.clone(),
        model: model.clone(),
        mode: "local".into(),
        cloud_consent: false,
        api_key: String::new(),
    };
    let positive = [
        part(0, "system", "Guten Morgen zusammen. Heute geht es um das Budget für das dritte Quartal und um den Hosting-Anbieter."),
        part(1, "microphone", "Ich habe die beiden Angebote verglichen. Anbieter B ist bei gleicher Leistung deutlich günstiger."),
        part(2, "system", "Gut, dann entscheiden wir uns ausdrücklich für Anbieter B. Das ist hiermit beschlossen."),
        part(3, "microphone", "Anna übernimmt ausdrücklich die Aufgabe, den Vertrag bis Freitag vorzubereiten."),
        part(4, "system", "Offen ist noch: Wer klärt die Lizenzfrage mit der Rechtsabteilung? Das konnten wir heute nicht beantworten."),
        part(5, "microphone", "Die Testumgebung läuft seit Montag stabil, das ist eine wichtige Erkenntnis für die Migration."),
    ];
    let nonbinding = [
        part(0, "system", "Vielleicht könnten wir irgendwann Anbieter B prüfen, aber niemand hat etwas beschlossen."),
        part(1, "microphone", "Anna könnte eventuell einen Vertrag ansehen, sie hat aber keine Aufgabe übernommen."),
        part(2, "system", "Maybe we look at it next month. There is no commitment and no deadline."),
        part(3, "microphone", "Das war nur ein lockerer Austausch ohne Ergebnis."),
    ];
    let start = Instant::now();
    let (good, good_updates) = run(&folder.join("positive"), connection(), &positive)?;
    let positive_seconds = start.elapsed().as_secs_f64();
    ensure!(
        good.decisions.len() == 1,
        "Expected exactly one explicit decision, got {}.",
        good.decisions.len()
    );
    ensure!(
        good.tasks.len() == 1
            && good.tasks[0].owner.as_deref() == Some("Anna")
            && good.tasks[0].due.as_deref() == Some("Freitag"),
        "Expected exactly one task for Anna due Freitag."
    );
    ensure!(!good.overview.is_empty(), "Expected a verified overview.");
    let negative_start = Instant::now();
    let (none, none_updates) = run(&folder.join("nonbinding"), connection(), &nonbinding)?;
    ensure!(
        none.decisions.is_empty() && none.tasks.is_empty(),
        "Nonbinding statements must not become decisions or tasks."
    );
    let report = serde_json::json!({
        "success": true,
        "syntheticText": true,
        "actualLocalModel": true,
        "endpoint": endpoint,
        "model": model,
        "promptVersion": meeting_notes::live::LIVE_PROMPT_VERSION,
        "positive": {
            "updates": good_updates, "windows": good.windows, "seconds": positive_seconds,
            "overview": good.overview.len(), "points": good.points.len(),
            "decisions": good.decisions.len(), "tasks": good.tasks.len(),
            "questions": good.questions.len(), "rejected": good.rejected,
        },
        "nonbinding": {
            "updates": none_updates, "windows": none.windows,
            "seconds": negative_start.elapsed().as_secs_f64(),
            "decisions": none.decisions.len(), "tasks": none.tasks.len(),
            "rejected": none.rejected,
        },
        "cloudTested": false,
        "concurrentCaptureMeasured": false,
    });
    fs::write(
        folder.join("reference.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
