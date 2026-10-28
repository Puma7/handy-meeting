//! Synthetic tests: a scripted local OpenAI-compatible server stands in for the text model.
use super::*;
use crate::Step;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

/// Serves one scripted chat answer per request and records every request body.
struct Server {
    endpoint: String,
    bodies: Arc<Mutex<Vec<serde_json::Value>>>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn start(answers: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/v1/", listener.local_addr().unwrap());
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&bodies);
        let handle = thread::spawn(move || {
            for answer in answers {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0_u8; 8192];
                let header_end = loop {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                    if let Some(i) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&request[..header_end]).to_lowercase();
                let length: usize = headers
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length: "))
                    .map(|v| v.trim().parse().unwrap())
                    .unwrap_or(0);
                while request.len() < header_end + length {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                }
                recorded
                    .lock()
                    .unwrap()
                    .push(serde_json::from_slice(&request[header_end..]).unwrap());
                let body = serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":answer}}]}).to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        Self {
            endpoint,
            bodies,
            handle: Some(handle),
        }
    }
    fn connection(&self, model: &str) -> Connection {
        Connection {
            endpoint: self.endpoint.clone(),
            model: model.into(),
            mode: "local".into(),
            cloud_consent: false,
            api_key: String::new(),
        }
    }
    fn finish(mut self) -> Vec<serde_json::Value> {
        self.handle.take().unwrap().join().unwrap();
        self.bodies.lock().unwrap().clone()
    }
}

fn part(key: &str, track: &str, start_s: u64, text: &str) -> LiveText {
    LiveText {
        key: key.into(),
        track: track.into(),
        start_ms: start_s * 1000,
        end_ms: start_s * 1000 + 30_000,
        text: text.into(),
    }
}

fn fact(text: &str, owner: Option<&str>, source: &str, quote: &str) -> serde_json::Value {
    serde_json::json!({"text":text,"owner":owner,"due":null,"evidence":[{"sourceId":source,"quote":quote}]})
}

fn window_answer(
    points: Vec<serde_json::Value>,
    decisions: Vec<serde_json::Value>,
    tasks: Vec<serde_json::Value>,
    questions: Vec<serde_json::Value>,
) -> String {
    serde_json::json!({"points":points,"decisions":decisions,"tasks":tasks,"questions":questions})
        .to_string()
}

fn overview_answer(items: Vec<serde_json::Value>) -> String {
    serde_json::json!({ "items": items }).to_string()
}

const BUDGET: &str = "Wir besprechen heute das Budget für das dritte Quartal.";
const DECISION: &str = "Wir entscheiden uns für den günstigeren Anbieter.";
const TASK: &str = "Anna schickt das Angebot bis Freitag an alle.";
const QUESTION: &str = "Wer klärt eigentlich die Lizenzfrage mit der Rechtsabteilung?";

#[test]
fn windows_and_overview_are_source_checked_stored_and_shown_with_their_tracks() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("live-notes");
    let server = Server::start(vec![
        window_answer(
            vec![
                fact("Budget Q3 wird besprochen", None, "C0", BUDGET),
                fact("Budget Q3 wird besprochen", None, "C0", BUDGET),
                fact("Erfunden", None, "C0", "Das Budget wurde verdoppelt."),
            ],
            vec![fact("Günstigerer Anbieter gewählt", None, "M30", DECISION)],
            vec![],
            vec![],
        ),
        overview_answer(vec![fact("Budget und Anbieterwahl", None, "M30", DECISION)]),
        window_answer(
            vec![],
            vec![],
            vec![fact("Angebot an alle schicken", Some("Anna"), "C60", TASK)],
            vec![
                fact("Lizenzfrage offen", None, "C90", QUESTION),
                // A source from an earlier window is not allowed as evidence for this window.
                fact("Wiederholung", None, "C0", BUDGET),
            ],
        ),
        overview_answer(vec![
            fact("Budget und Anbieterwahl", None, "M30", DECISION),
            fact("Anna schickt das Angebot", None, "C60", TASK),
        ]),
    ]);
    let mut runner =
        LiveRunner::start(&root, "meeting-1", server.connection("synthetic"), "de").unwrap();
    let control = AnalysisControl::default();
    let first = vec![
        part("system-0", "system", 0, BUDGET),
        part("microphone-30", "microphone", 30, DECISION),
    ];
    let pending = runner.pending(&first);
    assert_eq!(pending.len(), 2);
    let outcome = runner
        .update(&pending, Trigger::Interval, &control)
        .unwrap();
    assert_eq!(outcome.window, Some(1));
    assert_eq!(
        outcome.accepted, 2,
        "the duplicate is dropped, the invention rejected"
    );
    assert_eq!(outcome.rejected, 1);
    assert_eq!(outcome.overview, Some(1));
    assert!(runner.pending(&first).is_empty());

    let mut second = first.clone();
    second.push(part("system-60", "system", 60, TASK));
    second.push(part("system-90", "system", 90, QUESTION));
    let pending = runner.pending(&second);
    assert_eq!(pending.len(), 2);
    let outcome = runner.update(&pending, Trigger::Manual, &control).unwrap();
    assert_eq!(
        (outcome.window, outcome.accepted, outcome.rejected),
        (Some(2), 2, 1)
    );
    assert_eq!(outcome.overview, Some(2));

    let view = runner.view();
    assert_eq!(view.windows, 2);
    assert_eq!(view.summarized_sections, 4);
    assert_eq!(view.points.len(), 1);
    assert_eq!(view.decisions[0].tracks, ["microphone"]);
    assert_eq!(view.tasks[0].owner.as_deref(), Some("Anna"));
    assert_eq!(view.tasks[0].evidence[0].start_ms, 60_000);
    assert_eq!(view.questions.len(), 1);
    assert_eq!(view.overview.len(), 2);
    assert_eq!(view.overview_window, 2);
    assert_eq!(view.rejected, 2);

    // The second overview request carried the first overview and only the new statements.
    let bodies = server.finish();
    assert_eq!(bodies.len(), 4);
    let input: serde_json::Value =
        serde_json::from_str(bodies[3]["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(input["previousSummary"].as_array().unwrap().len(), 1);
    assert_eq!(input["newStatements"].as_array().unwrap().len(), 2);
    assert_eq!(input["newStatements"][0]["list"], "tasks");
    // The second window request listed earlier statements as known, and only new sections.
    let window_input: serde_json::Value =
        serde_json::from_str(bodies[2]["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(window_input["knownStatements"].as_array().unwrap().len(), 2);
    let ids: Vec<_> = window_input["newSections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, ["C60", "C90"]);
    for body in &bodies {
        assert_eq!(body["temperature"], 0);
        assert_eq!(body["stream"], false);
    }

    // Everything reads back verified from disk.
    let stored = read_latest(&root).unwrap().unwrap();
    assert_eq!(stored.run_id, view.run_id);
    assert_eq!(stored.decisions.len(), 1);
    assert_eq!(stored.overview.len(), 2);
}

#[test]
fn a_restart_continues_the_matching_run_and_another_model_starts_a_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("live-notes");
    let server = Server::start(vec![window_answer(vec![], vec![], vec![], vec![])]);
    let texts = vec![part("system-0", "system", 0, BUDGET)];
    let first_id = {
        let mut runner =
            LiveRunner::start(&root, "meeting-1", server.connection("synthetic"), "de").unwrap();
        let pending = runner.pending(&texts);
        runner
            .update(&pending, Trigger::Interval, &AnalysisControl::default())
            .unwrap();
        runner.run().id.clone()
    };
    // The same local route after a restart; no request is needed to continue.
    let same_route = server.connection("synthetic");
    let other_model = server.connection("other");
    server.finish();
    let again = LiveRunner::start(&root, "meeting-1", same_route, "de").unwrap();
    assert_eq!(again.run().id, first_id);
    assert!(
        again.pending(&texts).is_empty(),
        "nothing is summarized twice"
    );
    assert!(!again.overview_due(), "an empty window needs no overview");
    let other = LiveRunner::start(&root, "meeting-1", other_model, "de").unwrap();
    assert_ne!(other.run().id, first_id);
    assert_eq!(other.pending(&texts).len(), 1);
    // The earlier run is kept unchanged next to the new one.
    assert!(load_run(&root.join(&first_id)).is_ok());
    assert_eq!(list_runs(&root).unwrap().0.len(), 2);
}

#[test]
fn a_damaged_window_blocks_its_run_and_is_never_shrunk_or_removed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("live-notes");
    let server = Server::start(vec![
        window_answer(
            vec![fact("Budget", None, "C0", BUDGET)],
            vec![],
            vec![],
            vec![],
        ),
        overview_answer(vec![fact("Budget", None, "C0", BUDGET)]),
    ]);
    let id = {
        let mut runner =
            LiveRunner::start(&root, "meeting-1", server.connection("synthetic"), "de").unwrap();
        let pending = runner.pending(&[part("system-0", "system", 0, BUDGET)]);
        runner
            .update(&pending, Trigger::Interval, &AnalysisControl::default())
            .unwrap();
        runner.run().id.clone()
    };
    server.finish();
    let path = root.join(&id).join("window-000001.json");
    let tampered = fs_read(&path).replace("Wir besprechen heute", "Wir verschieben heute");
    std::fs::write(&path, &tampered).unwrap();
    assert!(load_run(&root.join(&id)).is_err());
    let latest = read_latest(&root).unwrap().unwrap();
    assert_eq!(latest.run_id, None);
    assert_eq!(latest.damaged_runs, 1);
    assert_eq!(
        fs_read(&path),
        tampered,
        "the damaged file is left as it was"
    );
    // A gap in the numbering is reported as well.
    std::fs::rename(&path, root.join(&id).join("window-000002.json")).unwrap();
    assert!(load_run(&root.join(&id)).is_err());
}

fn fs_read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn an_empty_or_unsupported_overview_keeps_the_previous_one_and_is_retried() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("live-notes");
    let server = Server::start(vec![
        window_answer(
            vec![fact("Budget", None, "C0", BUDGET)],
            vec![],
            vec![],
            vec![],
        ),
        overview_answer(vec![fact("Budget", None, "C0", BUDGET)]),
        window_answer(
            vec![],
            vec![fact("Anbieter", None, "C30", DECISION)],
            vec![],
            vec![],
        ),
        // Cites a section that does not exist: discarded, so the overview stays as it was.
        overview_answer(vec![fact("Erfunden", None, "C999", DECISION)]),
        // Retried on the next update without new text.
        overview_answer(vec![
            fact("Budget", None, "C0", BUDGET),
            fact("Anbieter", None, "C30", DECISION),
        ]),
    ]);
    let mut runner =
        LiveRunner::start(&root, "meeting-1", server.connection("synthetic"), "en").unwrap();
    let control = AnalysisControl::default();
    let texts = vec![
        part("system-0", "system", 0, BUDGET),
        part("system-30", "system", 30, DECISION),
    ];
    let first = runner.pending(&texts[..1]);
    runner.update(&first, Trigger::Interval, &control).unwrap();
    let second = runner.pending(&texts);
    let outcome = runner.update(&second, Trigger::Interval, &control).unwrap();
    assert_eq!(outcome.window, Some(2));
    assert!(outcome.overview_error.is_some());
    assert_eq!(runner.view().overview[0].text, "Budget");
    assert!(runner.overview_due());
    let retry = runner.update(&[], Trigger::Manual, &control).unwrap();
    assert_eq!((retry.window, retry.overview), (None, Some(2)));
    assert_eq!(runner.view().overview.len(), 2);
    assert!(!runner.overview_due());
    let bodies = server.finish();
    assert!(bodies[0]["messages"][0]["content"]
        .as_str()
        .unwrap()
        .contains("Write every text in English"));
}

#[test]
fn a_structurally_invalid_answer_stores_nothing_and_the_text_stays_pending() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("live-notes");
    let server = Server::start(vec![r#"{"points":[],"decisions":[],"tasks":[]}"#.into()]);
    let mut runner =
        LiveRunner::start(&root, "meeting-1", server.connection("synthetic"), "de").unwrap();
    let texts = vec![part("system-0", "system", 0, BUDGET)];
    let pending = runner.pending(&texts);
    assert!(runner
        .update(&pending, Trigger::Interval, &AnalysisControl::default())
        .is_err());
    assert_eq!(runner.pending(&texts).len(), 1);
    assert!(!root
        .join(&runner.run().id)
        .join("window-000001.json")
        .exists());
    server.finish();
}

#[test]
fn owners_outside_tasks_and_invented_owners_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("live-notes");
    let server = Server::start(vec![window_answer(
        vec![fact("Angebot", Some("Anna"), "C0", TASK)],
        vec![],
        vec![fact("Angebot", Some("Ben"), "C0", TASK)],
        vec![],
    )]);
    let mut runner =
        LiveRunner::start(&root, "meeting-1", server.connection("synthetic"), "de").unwrap();
    let pending = runner.pending(&[part("system-0", "system", 0, TASK)]);
    let outcome = runner
        .update(&pending, Trigger::Interval, &AnalysisControl::default())
        .unwrap();
    assert_eq!(
        (outcome.accepted, outcome.rejected, outcome.overview),
        (0, 2, None)
    );
    server.finish();
}

#[test]
fn cloud_live_notes_need_explicit_text_release() {
    let dir = tempfile::tempdir().unwrap();
    let connection = Connection {
        endpoint: "https://api.example.com/v1/".into(),
        model: "cloud-model".into(),
        mode: "cloud".into(),
        cloud_consent: false,
        api_key: String::new(),
    };
    assert!(LiveRunner::start(dir.path(), "meeting-1", connection, "de").is_err());
    assert!(!dir.path().join("live-notes").exists());
}

/// Regression guard: moving the request code into a shared helper must not change the
/// post-meeting analysis request in any byte that reaches the model.
#[test]
fn the_post_meeting_analysis_request_is_unchanged() {
    let server = Server::start(vec![r#"{"items":[]}"#.into()]);
    let client = TextClient::new(&server.connection("synthetic")).unwrap();
    let step = Step {
        id: "s".into(),
        name: "Summary".into(),
        kind: Kind::Summary,
        prompt: "Fasse zusammen.".into(),
    };
    let sources = vec![crate::Source {
        id: "S00001".into(),
        source: "system".into(),
        start_ms: 0,
        end_ms: 1000,
        text: BUDGET.into(),
    }];
    client.generate(&sources, &step).unwrap();
    let body = server.finish().remove(0);
    let schema = serde_json::json!({"type":"object","additionalProperties":false,"required":["items"],"properties":{"items":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["text","owner","due","evidence"],"properties":{"text":{"type":"string"},"owner":{"type":["string","null"]},"due":{"type":["string","null"]},"evidence":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["sourceId","quote"],"properties":{"sourceId":{"type":"string"},"quote":{"type":"string"}}}}}}}}});
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.starts_with("Du wertest Meetingtext aus."));
    assert!(system.ends_with(
        "\nowner und due bleiben null.\nAnalyseauftrag als JSON-Zeichenfolge: \"Fasse zusammen.\""
    ));
    let expected = serde_json::json!({"model":"synthetic","stream":false,"messages":[{"role":"system","content":system},{"role":"user","content":serde_json::to_string(&sources).unwrap()}],"response_format":{"type":"json_schema","json_schema":{"name":"meeting_notes","strict":true,"schema":schema}},"max_tokens":2048,"temperature":0});
    assert_eq!(body, expected);
}
