use anyhow::{ensure, Context, Result};
use meeting_notes::{
    analyze_controlled, is_cancelled, read_pending_analysis, read_snapshot, Analysis,
    AnalysisControl, Connection, Kind, Source, Step, Template,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn reference_run(
    root: &Path,
    id: &str,
    sources: Vec<Source>,
    template: Template,
    connection: Connection,
) -> Result<Analysis> {
    let control = AnalysisControl::default();
    let first = analyze_controlled(
        root,
        id,
        sources,
        template,
        connection,
        None,
        &control,
        |p| {
            if p.confirmed_tasks == 1 {
                control.cancel();
            }
        },
    );
    ensure!(
        first.as_ref().err().is_some_and(is_cancelled),
        "Expected interruption after the first durable step."
    );
    let entries: Vec<_> = fs::read_dir(root)?.collect::<std::io::Result<_>>()?;
    ensure!(entries.len() == 1, "Expected one retained analysis run.");
    let folder = entries[0].path();
    let original_snapshot = fs::read(folder.join("snapshot.json"))?;
    let original_step = fs::read(folder.join("step-0000-000000.json"))?;
    let pending = read_pending_analysis(&folder)?.context("Saved run must remain discoverable.")?;
    ensure!(
        pending.confirmed_tasks == 1 && pending.total_tasks == 3,
        "Expected one reusable confirmed step."
    );
    // Reconstruct only from durable text metadata, using a fresh controller/client.
    let snapshot = read_snapshot(&folder.join("snapshot.json"))?;
    let resumed_connection = Connection {
        endpoint: snapshot.endpoint,
        model: snapshot.model,
        mode: snapshot.mode,
        cloud_consent: false,
        api_key: String::new(),
    };
    let mut requests = 0;
    let result = analyze_controlled(
        root,
        &snapshot.meeting_id,
        snapshot.sources,
        snapshot.template,
        resumed_connection,
        Some(&snapshot.id),
        &AnalysisControl::default(),
        |p| {
            if p.stage == "requesting" {
                requests += 1;
                assert!(
                    p.step_index > 1,
                    "The saved first step must never be requested again"
                );
            }
        },
    )?;
    ensure!(
        requests == 2
            && fs::read(folder.join("snapshot.json"))? == original_snapshot
            && fs::read(folder.join("step-0000-000000.json"))? == original_step,
        "Original snapshot/checkpoint changed or confirmed work was repeated."
    );
    ensure!(
        read_pending_analysis(&folder)?.is_none(),
        "Completed run must leave the unfinished list."
    );
    Ok(result)
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let folder = PathBuf::from(args.next().context("Output directory required")?);
    let endpoint = args
        .next()
        .unwrap_or_else(|| "http://127.0.0.1:1234/v1/".into());
    let model = args.next().unwrap_or_else(|| "google/gemma-4-e4b".into());
    ensure!(
        !folder.exists(),
        "Preserve prior reference runs; choose a new output directory."
    );
    fs::create_dir_all(&folder)?;
    let template=Template{id:"reference-template".into(),name:"Local reference".into(),steps:vec![
        Step{id:"summary".into(),name:"Zusammenfassung".into(),kind:Kind::Summary,prompt:"Fasse das Gespräch auf Deutsch knapp und verständlich zusammen. Hebe Themen und Ergebnisse hervor.".into()},
        Step{id:"decisions".into(),name:"Entscheidungen".into(),kind:Kind::Decisions,prompt:"Extrahiere ausschließlich ausdrücklich getroffene Entscheidungen. Wünsche, Vorschläge, Aufgabenübernahmen und bestehende Zustände sind keine Beschlüsse. Ohne ausdrücklichen Beschluss bleibt items leer.".into()},
        Step{id:"tasks".into(),name:"To-dos".into(),kind:Kind::Tasks,prompt:"Extrahiere ausschließlich ausdrücklich zugesagte oder zugewiesene Aufgaben mit Zuständigen und Terminen, soweit genannt. Unverbindliche Vorschläge sind keine Aufgaben.".into()},
    ]};
    let source = |text: &str| {
        vec![Source {
            id: "S00001".into(),
            source: "system".into(),
            start_ms: 0,
            end_ms: 30000,
            text: text.into(),
        }]
    };
    let connection = || Connection {
        endpoint: endpoint.clone(),
        model: model.clone(),
        mode: "local".into(),
        cloud_consent: false,
        api_key: String::new(),
    };
    let start = std::time::Instant::now();
    let positive=reference_run(&folder.join("positive"),"synthetic-positive",source("Wir entscheiden uns ausdrücklich für Option B. Anna übernimmt ausdrücklich die Aufgabe, den Bericht bis Freitag zu schreiben. Die Audiodateien bleiben lokal."),template.clone(),connection())?;
    ensure!(
        positive.sections[1].items.len() == 1,
        "Expected exactly one explicit decision."
    );
    ensure!(
        positive.sections[2].items.len() == 1,
        "Expected exactly one explicit task."
    );
    ensure!(
        positive.sections[2].items[0].owner.as_deref() == Some("Anna")
            && positive.sections[2].items[0].due.as_deref() == Some("Freitag"),
        "Expected literal Anna/Freitag attribution."
    );
    let negative=reference_run(&folder.join("negative"),"synthetic-negative",source("Vielleicht könnten wir nächste Woche Option B prüfen. Niemand hat das beschlossen. Anna könnte eventuell einen Bericht schreiben, hat aber keine Aufgabe übernommen. There is no commitment and no deadline."),template,connection())?;
    ensure!(
        negative.sections[1].items.is_empty() && negative.sections[2].items.is_empty(),
        "Nonbinding proposals must not become decisions or tasks."
    );
    // The run checks the answers, not which model the loopback server actually served.
    let report = serde_json::json!({"success":true,"syntheticText":true,"requestedModel":model,"servedModelVerified":false,"mode":"local","promptVersion":meeting_notes::PROMPT_VERSION,"elapsedSeconds":start.elapsed().as_secs_f64(),"positiveDecisions":positive.sections[1].items.len(),"positiveTasks":positive.sections[2].items.len(),"owner":"Anna","due":"Freitag","negativeDecisions":negative.sections[1].items.len(),"negativeTasks":negative.sections[2].items.len(),"resumedFromDurableCheckpoint":true,"confirmedStepsReused":2,"processCrashTested":false,"cloudTested":false});
    fs::write(
        folder.join("reference.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
