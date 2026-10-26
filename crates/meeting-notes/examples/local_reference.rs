use anyhow::{ensure, Context, Result};
use meeting_notes::{analyze, Connection, Kind, Source, Step, Template};
use std::{fs, path::PathBuf};

fn main() -> Result<()> {
    let folder = PathBuf::from(
        std::env::args()
            .nth(1)
            .context("Output directory required")?,
    );
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
        endpoint: "http://127.0.0.1:1234/v1/".into(),
        model: "google/gemma-4-e4b".into(),
        mode: "local".into(),
        cloud_consent: false,
        api_key: String::new(),
    };
    let start = std::time::Instant::now();
    let positive=analyze(&folder.join("positive"),"synthetic-positive",source("Wir entscheiden uns ausdrücklich für Option B. Anna übernimmt ausdrücklich die Aufgabe, den Bericht bis Freitag zu schreiben. Die Audiodateien bleiben lokal."),template.clone(),connection(),None)?;
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
    let negative=analyze(&folder.join("negative"),"synthetic-negative",source("Vielleicht könnten wir nächste Woche Option B prüfen. Niemand hat das beschlossen. Anna könnte eventuell einen Bericht schreiben, hat aber keine Aufgabe übernommen. There is no commitment and no deadline."),template,connection(),None)?;
    ensure!(
        negative.sections[1].items.is_empty() && negative.sections[2].items.is_empty(),
        "Nonbinding proposals must not become decisions or tasks."
    );
    let report = serde_json::json!({"success":true,"syntheticText":true,"actualLocalModel":true,"model":"google/gemma-4-e4b","mode":"local","promptVersion":meeting_notes::PROMPT_VERSION,"elapsedSeconds":start.elapsed().as_secs_f64(),"positiveDecisions":positive.sections[1].items.len(),"positiveTasks":positive.sections[2].items.len(),"owner":"Anna","due":"Freitag","negativeDecisions":negative.sections[1].items.len(),"negativeTasks":negative.sections[2].items.len(),"cloudTested":false});
    fs::write(
        folder.join("reference.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
