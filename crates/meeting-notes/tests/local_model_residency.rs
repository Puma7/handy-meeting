//! Synthetic native-provider protocol tests. No model, audio or external service is used.
use meeting_notes::{
    residency::{LocalModelPool, LocalProvider},
    AnalysisControl, Connection, Kind, Source, Step, TextClient,
};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

struct Server {
    endpoint: String,
    requests: Arc<Mutex<Vec<(String, Value)>>>,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(mut handler: impl FnMut(&str, &Value) -> (u16, Value) + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/v1/", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (seen, stopped) = (Arc::clone(&requests), Arc::clone(&stop));
        let handle = thread::spawn(move || {
            while !stopped.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let (path, body) = read(&mut stream);
                        seen.lock().unwrap().push((path.clone(), body.clone()));
                        let (status, reply) = handler(&path, &body);
                        let bytes = reply.to_string();
                        let response = format!("HTTP/1.1 {status} Synthetic\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{bytes}", bytes.len());
                        let _ = stream.write_all(response.as_bytes());
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("{e}"),
                }
            }
        });
        Self {
            endpoint,
            requests,
            stop,
            handle: Some(handle),
        }
    }
    fn seen(&self) -> Vec<(String, Value)> {
        self.requests.lock().unwrap().clone()
    }
    fn connection(&self) -> Connection {
        Connection {
            endpoint: self.endpoint.clone(),
            model: "synthetic".into(),
            mode: "local".into(),
            cloud_consent: false,
            api_key: String::new(),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.handle.take().unwrap().join().unwrap();
    }
}
fn read(stream: &mut TcpStream) -> (String, Value) {
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buf = [0; 4096];
    let end = loop {
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&buf[..n]);
        assert!(bytes.len() < 256_000);
        if let Some(i) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&bytes[..end]).into_owned();
    let size: usize = head
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .map(|(_, v)| v.trim().parse().unwrap())
        .unwrap_or(0);
    assert!(size < 256_000);
    while bytes.len() < end + size {
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&buf[..n]);
    }
    let path = head
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_owned();
    (
        path,
        if size == 0 {
            Value::Null
        } else {
            serde_json::from_slice(&bytes[end..end + size]).unwrap()
        },
    )
}
fn ollama(path: &str, _: &Value) -> (u16, Value) {
    match path {
        "/api/ps" => (200, json!({"models":[]})),
        "/api/generate" => (200, json!({"done":true,"done_reason":"load"})),
        "/api/chat" => (
            200,
            json!({"done":true,"done_reason":"stop","message":{"content":"{\"items\":[]}"},"eval_count":17,"prompt_eval_count":29}),
        ),
        _ => (500, json!({})),
    }
}
fn lm(path: &str, body: &Value) -> (u16, Value) {
    match path {
        "/api/v1/models" => (
            200,
            json!({"models":[{"type":"llm","key":"synthetic","loaded_instances":[{"id":"foreign"}]}]}),
        ),
        "/api/v1/models/load" => (
            200,
            json!({"type":"llm","status":"loaded","instance_id":"owned"}),
        ),
        "/api/v1/models/unload" => (200, json!({"instance_id":body["instance_id"]})),
        "/v1/chat/completions" => (
            200,
            json!({"choices":[{"finish_reason":"stop","message":{"content":"{\"items\":[]}"}}]}),
        ),
        _ => (500, json!({})),
    }
}
fn generate(client: &TextClient) -> anyhow::Result<String> {
    client.generate(
        &[Source {
            id: "S00001".into(),
            source: "microphone".into(),
            start_ms: 0,
            end_ms: 30_000,
            text: "Vielleicht könnten wir einen Bericht schreiben.".into(),
        }],
        &Step {
            id: "summary".into(),
            name: "Summary".into(),
            kind: Kind::Summary,
            prompt: "Fasse belegte Aussagen zusammen.".into(),
        },
    )
}

#[test]
fn ollama_pins_before_text_and_last_user_releases_after_all_client_clones() {
    let server = Server::new(ollama);
    let pool = LocalModelPool::default();
    let conn = server.connection();
    let live = pool.reserve(&conn, LocalProvider::Ollama).unwrap();
    let post = pool.reserve(&conn, LocalProvider::Ollama).unwrap();
    assert!(
        server.seen().is_empty(),
        "Reservation must not send text or start network"
    );
    live.prepare(&AnalysisControl::default()).unwrap();
    let client = TextClient::new(&conn)
        .unwrap()
        .with_residency(Arc::clone(&live))
        .unwrap();
    assert_eq!(generate(&client).unwrap(), "{\"items\":[]}");
    let seen = server.seen();
    assert_eq!(
        seen.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(),
        vec!["/api/ps", "/api/generate", "/api/chat"]
    );
    assert_eq!(
        seen[1].1,
        json!({"model":"synthetic","stream":false,"keep_alive":-1})
    );
    assert_eq!(seen[2].1["keep_alive"], -1);
    assert_eq!(seen[2].1["options"]["num_predict"], 2048);
    assert!(seen[2].1["format"]["properties"]["items"].is_object());
    assert_eq!(seen[2].1["messages"][0]["role"], "system");
    drop(live);
    drop(post);
    assert_eq!(
        server.seen().len(),
        3,
        "In-flight client still owns the model"
    );
    drop(client);
    let seen = server.seen();
    assert_eq!(seen.len(), 4);
    assert_eq!(seen[3].1["keep_alive"], 0);
}

#[test]
fn lm_studio_uses_its_explicit_instance_and_never_unloads_the_foreign_one() {
    let server = Server::new(lm);
    let conn = server.connection();
    let pool = LocalModelPool::default();
    let lease = pool.reserve(&conn, LocalProvider::LmStudio).unwrap();
    let client = TextClient::new(&conn)
        .unwrap()
        .with_residency(lease)
        .unwrap();
    generate(&client).unwrap();
    let seen = server.seen();
    assert_eq!(
        seen[1],
        ("/api/v1/models/load".into(), json!({"model":"synthetic"}))
    );
    assert_eq!(seen[2].1["model"], "owned");
    assert_eq!(seen[2].1["response_format"]["type"], "json_schema");
    drop(client);
    let seen = server.seen();
    assert_eq!(
        seen[3],
        (
            "/api/v1/models/unload".into(),
            json!({"instance_id":"owned"})
        )
    );
    assert!(!seen
        .iter()
        .any(|(_, b)| b.get("instance_id") == Some(&json!("foreign"))));
}

#[test]
fn lm_studio_reused_foreign_id_is_explicit_failure_without_unload_or_inference() {
    let server = Server::new(|p, b| {
        if p == "/api/v1/models/load" {
            (
                200,
                json!({"type":"llm","status":"loaded","instance_id":"foreign"}),
            )
        } else {
            lm(p, b)
        }
    });
    let pool = LocalModelPool::default();
    let lease = pool
        .reserve(&server.connection(), LocalProvider::LmStudio)
        .unwrap();
    assert!(lease
        .prepare(&AnalysisControl::default())
        .unwrap_err()
        .to_string()
        .contains("bereits vorhandene"));
    drop(lease);
    assert_eq!(server.seen().len(), 2);
}

#[test]
fn borrowed_ollama_indefinite_load_is_preserved_and_finite_idle_grace_is_restored() {
    for (expiry, release) in [
        ("9999-01-01T00:00:00Z", None),
        ("2020-01-01T00:00:00Z", Some(300)),
    ] {
        let server = Server::new(move |p, b| {
            if p == "/api/ps" {
                (
                    200,
                    json!({"models":[{"name":"synthetic:latest","expires_at":expiry}]}),
                )
            } else {
                ollama(p, b)
            }
        });
        let pool = LocalModelPool::default();
        let lease = pool
            .reserve(&server.connection(), LocalProvider::Ollama)
            .unwrap();
        lease.prepare(&AnalysisControl::default()).unwrap();
        drop(lease);
        let seen = server.seen();
        assert_eq!(seen.len(), if release.is_some() { 3 } else { 2 });
        if let Some(seconds) = release {
            assert_eq!(seen[2].1["keep_alive"], seconds);
        }
    }
}

#[test]
fn failure_keeps_warm_reservation_for_retry_and_rejects_truncated_native_answers() {
    let failed = Arc::new(AtomicBool::new(true));
    let gate = Arc::clone(&failed);
    let server = Server::new(move |p, b| {
        if p == "/api/chat" && gate.load(Ordering::Acquire) {
            (
                200,
                json!({"done":true,"done_reason":"length","message":{"content":"{\"items\":[]}"}}),
            )
        } else {
            ollama(p, b)
        }
    });
    let pool = LocalModelPool::default();
    let conn = server.connection();
    let anchor = pool.reserve(&conn, LocalProvider::Ollama).unwrap();
    let client = TextClient::new(&conn)
        .unwrap()
        .with_residency(Arc::clone(&anchor))
        .unwrap();
    assert!(generate(&client)
        .unwrap_err()
        .to_string()
        .contains("nicht vollständig"));
    drop(client);
    assert_eq!(server.seen().len(), 3);
    failed.store(false, Ordering::Release);
    let client = TextClient::new(&conn)
        .unwrap()
        .with_residency(Arc::clone(&anchor))
        .unwrap();
    generate(&client).unwrap();
    drop(client);
    assert_eq!(
        server
            .seen()
            .iter()
            .filter(|(p, _)| p == "/api/generate")
            .count(),
        1,
        "Retry must not reload or unload"
    );
    drop(anchor);
    assert_eq!(server.seen().last().unwrap().1["keep_alive"], 0);
}

#[test]
fn cancellation_before_prepare_does_not_contact_provider_and_failed_load_sends_no_text() {
    let server = Server::new(|p, b| {
        if p == "/api/generate" {
            (500, json!({"error":"synthetic-private-server-message"}))
        } else {
            ollama(p, b)
        }
    });
    let pool = LocalModelPool::default();
    let lease = pool
        .reserve(&server.connection(), LocalProvider::Ollama)
        .unwrap();
    let control = AnalysisControl::default();
    control.cancel();
    assert!(meeting_notes::is_cancelled(
        &lease.prepare(&control).unwrap_err()
    ));
    assert!(server.seen().is_empty());
    let error = lease
        .prepare(&AnalysisControl::default())
        .unwrap_err()
        .to_string();
    assert!(!error.contains("synthetic-private"));
    drop(lease);
    assert_eq!(server.seen().len(), 2);
}

#[test]
fn failed_cleanup_blocks_unconfirmed_reuse_instead_of_claiming_a_fresh_pin() {
    let server = Server::new(|p, b| {
        if p == "/api/generate" && b["keep_alive"] == 0 {
            (500, json!({}))
        } else {
            ollama(p, b)
        }
    });
    let pool = LocalModelPool::default();
    let conn = server.connection();
    let lease = pool.reserve(&conn, LocalProvider::Ollama).unwrap();
    lease.prepare(&AnalysisControl::default()).unwrap();
    drop(lease);
    assert!(pool.reserve(&conn, LocalProvider::Ollama).is_err());
    assert_eq!(server.seen().len(), 3);
}

#[test]
fn pool_rejects_cloud_remote_route_auth_and_model_mismatches_before_network() {
    let server = Server::new(ollama);
    let pool = LocalModelPool::default();
    let conn = server.connection();
    let lease = pool.reserve(&conn, LocalProvider::Ollama).unwrap();
    let mut changed = server.connection();
    changed.api_key = "synthetic-other-key".into();
    assert!(pool.reserve(&changed, LocalProvider::Ollama).is_err());
    changed = server.connection();
    changed.model = "other".into();
    assert!(TextClient::new(&changed)
        .unwrap()
        .with_residency(Arc::clone(&lease))
        .is_err());
    changed = server.connection();
    changed.mode = "cloud".into();
    changed.cloud_consent = true;
    changed.endpoint = "https://example.invalid/v1/".into();
    assert!(pool.reserve(&changed, LocalProvider::Ollama).is_err());
    changed.mode = "local".into();
    assert!(pool.reserve(&changed, LocalProvider::Ollama).is_err());
    changed = server.connection();
    changed.endpoint = changed.endpoint.replace("/v1/", "/unrelated/");
    assert!(pool.reserve(&changed, LocalProvider::Ollama).is_err());
    drop(lease);
    assert!(server.seen().is_empty());
}

#[test]
fn virtual_idle_expiry_cannot_drop_the_model_between_live_updates() {
    // Advance a provider-side virtual clock past its default idle TTL. This is a protocol
    // regression, not a hardware, performance or real-duration measurement.
    let warm = Arc::new(AtomicBool::new(false));
    let pinned = Arc::new(AtomicBool::new(false));
    let (loaded, forever) = (Arc::clone(&warm), Arc::clone(&pinned));
    let server = Server::new(move |p, b| {
        if p == "/api/generate" {
            loaded.store(b["keep_alive"] != 0, Ordering::Release);
            forever.store(b["keep_alive"] == -1, Ordering::Release);
        }
        if p == "/v1/chat/completions" {
            loaded.store(true, Ordering::Release);
            return (
                200,
                json!({"choices":[{"finish_reason":"stop","message":{"content":"{\"items\":[]}"}}]}),
            );
        }
        ollama(p, b)
    });
    let pool = LocalModelPool::default();
    let conn = server.connection();
    let anchor = pool.reserve(&conn, LocalProvider::Ollama).unwrap();
    let client = TextClient::new(&conn)
        .unwrap()
        .with_residency(Arc::clone(&anchor))
        .unwrap();
    generate(&client).unwrap();
    if !pinned.load(Ordering::Acquire) {
        warm.store(false, Ordering::Release);
    }
    assert!(
        warm.load(Ordering::Acquire),
        "Selected model expired between live updates"
    );
    drop(client);
    assert!(
        warm.load(Ordering::Acquire),
        "Idle coordinator still awaits finalization"
    );
    drop(anchor);
    assert!(!warm.load(Ordering::Acquire));
}

#[test]
fn successful_http_without_load_or_exact_unload_ack_is_not_a_confirmed_lifetime() {
    let server = Server::new(|p, b| {
        if p == "/api/generate" {
            (200, json!({"error":"synthetic"}))
        } else {
            ollama(p, b)
        }
    });
    let pool = LocalModelPool::default();
    let lease = pool
        .reserve(&server.connection(), LocalProvider::Ollama)
        .unwrap();
    assert!(lease.prepare(&AnalysisControl::default()).is_err());
    drop(lease);
    assert_eq!(server.seen().len(), 2);
    let server = Server::new(|p, b| {
        if p == "/api/v1/models/unload" {
            (200, json!({"instance_id":"foreign"}))
        } else {
            lm(p, b)
        }
    });
    let pool = LocalModelPool::default();
    let conn = server.connection();
    let lease = pool.reserve(&conn, LocalProvider::LmStudio).unwrap();
    lease.prepare(&AnalysisControl::default()).unwrap();
    drop(lease);
    assert!(pool.reserve(&conn, LocalProvider::LmStudio).is_err());
}

#[test]
fn cancellation_during_native_load_still_tracks_the_created_instance_until_release() {
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let (loading, allow_reply) = (Arc::clone(&entered), Arc::clone(&release));
    let server = Server::new(move |p, b| {
        if p == "/api/v1/models/load" {
            loading.store(true, Ordering::Release);
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while !allow_reply.load(Ordering::Acquire) {
                assert!(std::time::Instant::now() < deadline);
                thread::sleep(Duration::from_millis(2));
            }
        }
        lm(p, b)
    });
    let pool = LocalModelPool::default();
    let lease = pool
        .reserve(&server.connection(), LocalProvider::LmStudio)
        .unwrap();
    let control = AnalysisControl::default();
    let cancel = control.clone();
    let worker = thread::spawn(move || {
        let result = lease.prepare(&control);
        drop(lease);
        result
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !entered.load(Ordering::Acquire) {
        assert!(std::time::Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    cancel.cancel();
    release.store(true, Ordering::Release);
    assert!(meeting_notes::is_cancelled(
        &worker.join().unwrap().unwrap_err()
    ));
    assert_eq!(
        server.seen().last().unwrap(),
        &(
            "/api/v1/models/unload".into(),
            json!({"instance_id":"owned"})
        ),
        "Cancelled load must not leave an untracked owned model"
    );
}

#[test]
fn failed_live_overview_stays_warm_and_resume_preserves_the_window_before_final_release() {
    use meeting_notes::live::{read_latest, LiveRunner, LiveText, Trigger};
    let fail = Arc::new(AtomicBool::new(true));
    let failure = Arc::clone(&fail);
    let quote = "Wir besprechen heute die Dokumentation.";
    let server = Server::new(move |p, b| {
        if p == "/api/chat" {
            let answer = if b["format"]["properties"].get("points").is_some() {
                json!({"points":[{"text":"Dokumentation besprechen","owner":null,"due":null,"evidence":[{"sourceId":"C0","quote":quote}]}],"decisions":[],"tasks":[],"questions":[]})
            } else {
                if failure.load(Ordering::Acquire) {
                    return (500, json!({}));
                }
                json!({"items":[{"text":"Dokumentation besprechen","owner":null,"due":null,"evidence":[{"sourceId":"C0","quote":quote}]}]})
            };
            return (
                200,
                json!({"done":true,"done_reason":"stop","message":{"content":answer.to_string()}}),
            );
        }
        ollama(p, b)
    });
    let folder = tempfile::tempdir().unwrap();
    let conn = server.connection();
    let pool = LocalModelPool::default();
    let anchor = pool.reserve(&conn, LocalProvider::Ollama).unwrap();
    let mut runner = LiveRunner::start(
        folder.path(),
        "synthetic-live-residency",
        server.connection(),
        "de",
    )
    .unwrap()
    .with_residency(Arc::clone(&anchor))
    .unwrap();
    let text = [LiveText {
        key: "synthetic-part".into(),
        track: "system".into(),
        start_ms: 0,
        end_ms: 30_000,
        text: quote.into(),
    }];
    let outcome = runner
        .update(
            &runner.pending(&text),
            Trigger::Final,
            &AnalysisControl::default(),
        )
        .unwrap();
    assert!(outcome.overview_error.is_some());
    assert_eq!(outcome.accepted, 1);
    assert!(runner.overview_due());
    let manifest = folder
        .path()
        .join(&runner.run().id)
        .join("window-000001.json");
    // Use the stored run's actual window path, without assuming a private session layout.
    let window = std::fs::read_dir(manifest.parent().unwrap())
        .unwrap()
        .map(|p| p.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("window-")
        })
        .unwrap();
    let before = std::fs::read(&window).unwrap();
    drop(runner);
    assert!(!server.seen().iter().any(|(_, b)| b["keep_alive"] == 0));
    fail.store(false, Ordering::Release);
    let mut resumed = LiveRunner::start(folder.path(), "synthetic-live-residency", conn, "de")
        .unwrap()
        .with_residency(Arc::clone(&anchor))
        .unwrap();
    assert!(resumed.pending(&text).is_empty());
    resumed
        .update(&[], Trigger::Final, &AnalysisControl::default())
        .unwrap();
    let stored = read_latest(folder.path(), "synthetic-live-residency")
        .unwrap()
        .unwrap();
    assert!(!stored.overview_pending);
    assert_eq!(stored.overview[0].evidence[0].quote, quote);
    assert_eq!(std::fs::read(window).unwrap(), before);
    drop(resumed);
    assert!(!server.seen().iter().any(|(_, b)| b["keep_alive"] == 0));
    drop(anchor);
    assert_eq!(server.seen().last().unwrap().1["keep_alive"], 0);
}

#[test]
fn loopback_aliases_defer_cleanup_without_merging_routes_or_model_instances() {
    let server = Server::new(ollama);
    let pool = LocalModelPool::default();
    let first = pool
        .reserve(&server.connection(), LocalProvider::Ollama)
        .unwrap();
    let mut alias = server.connection();
    alias.endpoint = alias.endpoint.replace("127.0.0.1", "localhost");
    alias.model = "synthetic:latest".into();
    let second = pool.reserve(&alias, LocalProvider::Ollama).unwrap();
    first.prepare(&AnalysisControl::default()).unwrap();
    second.prepare(&AnalysisControl::default()).unwrap();
    assert!(
        !first
            .matches_connection(&alias, LocalProvider::Ollama)
            .unwrap(),
        "The selected actual target must remain distinct"
    );
    drop(first);
    assert!(
        !server.seen().iter().any(|(_, b)| b["keep_alive"] == 0),
        "Other loopback spelling is still in use"
    );
    drop(second);
    assert_eq!(
        server
            .seen()
            .iter()
            .filter(|(_, b)| b["keep_alive"] == 0)
            .count(),
        2
    );
}
