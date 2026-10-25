//! Independent integration regressions with a synthetic loopback HTTP service.
//! No real model, cloud credentials, TLS provider or recording is used here.
use meeting_notes::{analyze, read_analysis, Connection, Kind, Source, Step, Template, TextClient};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const ORIGINAL: &str = "Anna schreibt den Bericht bis Freitag.";
const SYNTHETIC_KEY: &str = "synthetic-test-key-not-a-real-credential";

#[derive(Clone)]
struct Reply {
    status: u16,
    body: String,
    location: Option<String>,
}

impl Reply {
    fn successful(task: bool) -> Self {
        let answer = json!({"items":[{"text":"Bericht schreiben", "owner":if task {Some("Anna")} else {None}, "due":if task {Some("Freitag")} else {None}, "evidence":[{"sourceId":"S00001","quote":ORIGINAL}]}]});
        Self {
            status: 200,
            body: json!({"choices":[{"finish_reason":"stop","message":{"content":answer.to_string()}}]}).to_string(),
            location: None,
        }
    }

    fn failed() -> Self {
        Self {
            status: 500,
            body: json!({"error":{"message":"Synthetic unavailable provider"}}).to_string(),
            location: None,
        }
    }
}

struct SyntheticHttp {
    endpoint: String,
    replies: Arc<Mutex<VecDeque<Reply>>>,
    count: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<String>>>,
    stopped: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<String> {
    // Accepted sockets can inherit nonblocking mode from the listener on Windows.
    // The fixture polls accept, but complete request reads must wait for TCP data.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let header_end;
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() > 256_000 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            header_end = end + 4;
            break;
        }
    }
    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let length = headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if length > 256_000 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    while bytes.len() < header_end + length {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    Ok(String::from_utf8_lossy(&bytes[..header_end + length]).into_owned())
}

impl SyntheticHttp {
    fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/v1/", listener.local_addr().unwrap());
        let replies = Arc::new(Mutex::new(VecDeque::from(replies)));
        let count = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let (queue, seen, incoming, finish) = (
            Arc::clone(&replies),
            Arc::clone(&count),
            Arc::clone(&requests),
            Arc::clone(&stopped),
        );
        let thread = thread::spawn(move || {
            while !finish.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        if let Ok(request) = read_request(&mut stream) {
                            incoming.lock().unwrap().push(request);
                            seen.fetch_add(1, Ordering::SeqCst);
                            let reply = queue
                                .lock()
                                .unwrap()
                                .pop_front()
                                .unwrap_or_else(Reply::failed);
                            let location = reply
                                .location
                                .map(|url| format!("Location: {url}\r\n"))
                                .unwrap_or_default();
                            let response = format!("HTTP/1.1 {} Synthetic\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}", reply.status, reply.body.len(), location, reply.body);
                            let _ = stream.write_all(response.as_bytes());
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("Synthetic listener failed: {error}"),
                }
            }
        });
        Self {
            endpoint,
            replies,
            count,
            requests,
            stopped,
            thread: Some(thread),
        }
    }

    fn requests(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
    fn queue(&self, reply: Reply) {
        self.replies.lock().unwrap().push_back(reply);
    }
}

impl Drop for SyntheticHttp {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn connection(endpoint: &str) -> Connection {
    Connection {
        endpoint: endpoint.into(),
        model: "synthetic-model".into(),
        mode: "local".into(),
        cloud_consent: false,
        api_key: SYNTHETIC_KEY.into(),
    }
}
fn sources() -> Vec<Source> {
    vec![Source {
        id: "S00001".into(),
        source: "system".into(),
        start_ms: 0,
        end_ms: 30_000,
        text: ORIGINAL.into(),
    }]
}
fn template() -> Template {
    Template {
        id: "synthetic-template".into(),
        name: "Synthetic steps".into(),
        steps: vec![
            Step {
                id: "summary".into(),
                name: "Summary".into(),
                kind: Kind::Summary,
                prompt: "Fasse den belegten Text zusammen.".into(),
            },
            Step {
                id: "tasks".into(),
                name: "Tasks".into(),
                kind: Kind::Tasks,
                prompt: "Extrahiere ausdrücklich zugewiesene Aufgaben mit Zitaten.".into(),
            },
        ],
    }
}
fn run_folder(root: &Path) -> PathBuf {
    let entries: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "Exactly one immutable analysis run should exist"
    );
    entries.into_iter().next().unwrap()
}
fn only_run_id(folder: &Path) -> &str {
    folder.file_name().unwrap().to_str().unwrap()
}

#[test]
fn failed_second_step_preserves_first_checkpoint_and_resume_only_requests_missing_work() {
    let root = tempfile::tempdir().unwrap();
    let server = SyntheticHttp::new(vec![Reply::successful(false), Reply::failed()]);
    assert!(analyze(
        root.path(),
        "synthetic-meeting",
        sources(),
        template(),
        connection(&server.endpoint),
        None
    )
    .is_err());
    assert_eq!(server.requests(), 2);
    let folder = run_folder(root.path());
    let snapshot = fs::read(folder.join("snapshot.json")).unwrap();
    let confirmed = fs::read(folder.join("step-0000-000000.json")).unwrap();
    assert!(!folder.join("step-0001-000000.json").exists());
    assert!(!folder.join("result.json").exists());
    server.queue(Reply::successful(true));
    let result = analyze(
        root.path(),
        "synthetic-meeting",
        sources(),
        template(),
        connection(&server.endpoint),
        Some(only_run_id(&folder)),
    )
    .unwrap();
    assert_eq!(
        server.requests(),
        3,
        "The confirmed first step must not be regenerated"
    );
    assert_eq!(result.sections.len(), 2);
    assert_eq!(result.sections[1].items[0].owner.as_deref(), Some("Anna"));
    assert_eq!(result.sections[1].items[0].due.as_deref(), Some("Freitag"));
    assert_eq!(fs::read(folder.join("snapshot.json")).unwrap(), snapshot);
    assert_eq!(
        fs::read(folder.join("step-0000-000000.json")).unwrap(),
        confirmed
    );
    let finished = fs::read(folder.join("result.json")).unwrap();
    let again = analyze(
        root.path(),
        "synthetic-meeting",
        sources(),
        template(),
        connection(&server.endpoint),
        Some(&result.id),
    )
    .unwrap();
    assert_eq!(again.id, result.id);
    assert_eq!(
        server.requests(),
        3,
        "A completed run must be read without a model request"
    );
    assert_eq!(fs::read(folder.join("result.json")).unwrap(), finished);
    for entry in fs::read_dir(&folder).unwrap() {
        let data = fs::read_to_string(entry.unwrap().path()).unwrap();
        assert!(
            !data.contains(SYNTHETIC_KEY),
            "API key must not enter any snapshot, step or export"
        );
    }
    let requests = server.requests.lock().unwrap();
    for request in requests
        .iter()
        .filter(|request| request.starts_with("POST"))
    {
        let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        let text_sources: Vec<Source> =
            serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(
            text_sources,
            sources(),
            "Transport must receive only the supplied transcript sources"
        );
        assert!(body.get("session").is_none());
        assert!(body.get("audio").is_none());
    }
}

#[test]
fn changing_model_provider_prompt_or_original_sources_refuses_resume_before_network() {
    let root = tempfile::tempdir().unwrap();
    let server = SyntheticHttp::new(vec![Reply::successful(false), Reply::failed()]);
    assert!(analyze(
        root.path(),
        "synthetic-meeting",
        sources(),
        template(),
        connection(&server.endpoint),
        None
    )
    .is_err());
    let folder = run_folder(root.path());
    let id = only_run_id(&folder);
    let snapshot = fs::read(folder.join("snapshot.json")).unwrap();
    let checkpoint = fs::read(folder.join("step-0000-000000.json")).unwrap();
    let mut changed = connection(&server.endpoint);
    changed.model = "different-model".into();
    assert!(analyze(
        root.path(),
        "synthetic-meeting",
        sources(),
        template(),
        changed,
        Some(id)
    )
    .is_err());
    let other_provider = SyntheticHttp::new(vec![Reply::successful(false)]);
    assert!(analyze(
        root.path(),
        "synthetic-meeting",
        sources(),
        template(),
        connection(&other_provider.endpoint),
        Some(id)
    )
    .is_err());
    let mut changed_template = template();
    changed_template.steps[0]
        .prompt
        .push_str(" Neue Anweisung.");
    assert!(analyze(
        root.path(),
        "synthetic-meeting",
        sources(),
        changed_template,
        connection(&server.endpoint),
        Some(id)
    )
    .is_err());
    let mut changed_sources = sources();
    changed_sources[0].text = "Anna schreibt einen anderen Bericht bis Freitag.".into();
    assert!(analyze(
        root.path(),
        "synthetic-meeting",
        changed_sources,
        template(),
        connection(&server.endpoint),
        Some(id)
    )
    .is_err());
    assert_eq!(server.requests(), 2);
    assert_eq!(other_provider.requests(), 0);
    assert_eq!(fs::read(folder.join("snapshot.json")).unwrap(), snapshot);
    assert_eq!(
        fs::read(folder.join("step-0000-000000.json")).unwrap(),
        checkpoint
    );
}

#[test]
fn tampered_final_quotes_or_source_hash_are_rejected_without_requesting_a_replacement() {
    let root = tempfile::tempdir().unwrap();
    let server = SyntheticHttp::new(vec![Reply::successful(false), Reply::successful(true)]);
    let result = analyze(
        root.path(),
        "synthetic-meeting",
        sources(),
        template(),
        connection(&server.endpoint),
        None,
    )
    .unwrap();
    let path = root.path().join(&result.id).join("result.json");
    let original = fs::read(&path).unwrap();
    let mut hash_tamper: Value = serde_json::from_slice(&original).unwrap();
    hash_tamper["sourceHash"] = "changed-hash".into();
    fs::write(&path, serde_json::to_vec(&hash_tamper).unwrap()).unwrap();
    assert!(read_analysis(&path).is_err());
    assert!(analyze(
        root.path(),
        "synthetic-meeting",
        sources(),
        template(),
        connection(&server.endpoint),
        Some(&result.id)
    )
    .is_err());
    let mut quote_tamper: Value = serde_json::from_slice(&original).unwrap();
    quote_tamper["sections"][0]["items"][0]["evidence"][0]["quote"] =
        "Ben verfasst die Abrechnung bis Dienstag.".into();
    fs::write(&path, serde_json::to_vec(&quote_tamper).unwrap()).unwrap();
    assert!(read_analysis(&path).is_err());
    assert!(analyze(
        root.path(),
        "synthetic-meeting",
        sources(),
        template(),
        connection(&server.endpoint),
        Some(&result.id)
    )
    .is_err());
    assert_eq!(
        server.requests(),
        2,
        "Corruption must not silently trigger paid or changed replacement analysis"
    );
}

#[test]
fn corrupt_confirmed_step_is_retained_and_blocks_resume_before_the_missing_request() {
    let root = tempfile::tempdir().unwrap();
    let server = SyntheticHttp::new(vec![Reply::successful(false), Reply::failed()]);
    assert!(analyze(
        root.path(),
        "synthetic-meeting",
        sources(),
        template(),
        connection(&server.endpoint),
        None
    )
    .is_err());
    let folder = run_folder(root.path());
    let step = folder.join("step-0000-000000.json");
    let corrupt=br#"{"items":[{"text":"Invented","owner":null,"due":null,"evidence":[{"sourceId":"S99999","quote":"Anna schreibt den Bericht bis Freitag."}]}]}"#;
    fs::write(&step, corrupt).unwrap();
    assert!(analyze(
        root.path(),
        "synthetic-meeting",
        sources(),
        template(),
        connection(&server.endpoint),
        Some(only_run_id(&folder))
    )
    .is_err());
    assert_eq!(server.requests(), 2);
    assert_eq!(fs::read(step).unwrap(), corrupt);
}

#[test]
fn local_redirect_never_reaches_the_second_listener_and_remote_hosts_are_rejected() {
    let target = SyntheticHttp::new(vec![Reply::successful(false)]);
    let initial = SyntheticHttp::new(vec![Reply {
        status: 302,
        body: String::new(),
        location: Some(format!("{}models", target.endpoint)),
    }]);
    let client = TextClient::new(&connection(&initial.endpoint)).unwrap();
    assert!(client.models().is_err());
    assert_eq!(initial.requests(), 1);
    assert_eq!(target.requests(), 0);
    for endpoint in [
        "https://example.invalid/v1/",
        "http://127.0.0.1@example.invalid/v1/",
        "http://127.0.0.1/v1/?credential=synthetic",
    ] {
        assert!(TextClient::new(&connection(endpoint)).is_err());
    }
    assert_eq!(target.requests(), 0);
}

#[test]
fn simulated_cloud_configuration_requires_each_run_consent_and_https_without_real_provider_calls() {
    // Configuration simulation only: no real cloud key, TLS server or provider is exercised.
    let untouched = SyntheticHttp::new(Vec::new());
    let mut config = connection(&untouched.endpoint.replace("http:", "https:"));
    config.mode = "cloud".into();
    assert!(TextClient::new(&config).is_err());
    config.cloud_consent = true;
    assert!(TextClient::new(&config).is_ok());
    config.cloud_consent = false;
    assert!(TextClient::new(&config).is_err());
    config.cloud_consent = true;
    config.endpoint = untouched.endpoint.clone();
    assert!(TextClient::new(&config).is_err());
    assert_eq!(untouched.requests(), 0);
}
