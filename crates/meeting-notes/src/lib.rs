//! Source-linked text analysis, separated into data, validation, transport and orchestration.
mod control;
mod markdown;
mod models;
mod service;
mod templates;
mod transport;
mod validation;
pub use control::{is_cancelled, new_job_id, AnalysisControl, AnalysisProgress};
pub use markdown::render_markdown;
pub use models::{Analysis, Connection, Evidence, Fact, Kind, Section, Source, Step, Template};
pub use service::{analyze, analyze_controlled, read_analysis, render_text, source_hash};
pub use templates::{validate_library, TemplateLibrary, TemplateStore};
pub use transport::{credential_account, local_model_ids, TextClient};
pub use validation::{validate_answer, validate_template};
pub const PROMPT_VERSION: &str = "handymeeting-rust-notes-2";

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    fn config(endpoint: &str) -> Connection {
        Connection {
            endpoint: endpoint.into(),
            model: "local-test".into(),
            mode: "local".into(),
            cloud_consent: false,
            api_key: String::new(),
        }
    }
    fn sources() -> Vec<Source> {
        vec![Source {
            id: "S00001".into(),
            source: "system".into(),
            start_ms: 0,
            end_ms: 1000,
            text: "Anna schreibt den Bericht bis Freitag.".into(),
        }]
    }
    #[test]
    fn local_rejects_remote_credentials_and_redirectable_url_parts() {
        for url in [
            "https://example.com/v1/",
            "http://127.0.0.1@evil.test/",
            "http://127.0.0.1/v1/?key=secret",
            "file:///tmp/model",
        ] {
            assert!(TextClient::new(&config(url)).is_err());
        }
    }
    #[test]
    fn cloud_needs_per_run_consent_and_https() {
        let mut c = config("https://api.example.com/v1/");
        c.mode = "cloud".into();
        assert!(TextClient::new(&c).is_err());
        c.cloud_consent = true;
        assert!(TextClient::new(&c).is_ok());
        c.endpoint = "http://api.example.com/v1/".into();
        assert!(TextClient::new(&c).is_err());
    }
    #[test]
    fn exact_quotes_and_existing_source_required() {
        for evidence in [
            r#"{"sourceId":"S00002","quote":"Anna schreibt"}"#,
            r#"{"sourceId":"S00001","quote":"Anna prüft den Bericht"}"#,
        ] {
            let json = format!(
                r#"{{"items":[{{"text":"Bericht","owner":null,"due":null,"evidence":[{evidence}]}}]}}"#
            );
            assert!(validate_answer(&json, &sources(), &Kind::Summary).is_err());
        }
    }
    #[test]
    fn invented_owner_or_duplicate_field_rejected() {
        let json = r#"{"items":[{"text":"Bericht","owner":"Ben","due":null,"evidence":[{"sourceId":"S00001","quote":"Anna schreibt den Bericht bis Freitag."}]}]}"#;
        assert!(validate_answer(json, &sources(), &Kind::Tasks).is_err());
        assert!(validate_answer(r#"{"items":[],"items":[]}"#, &sources(), &Kind::Summary).is_err());
    }
    #[test]
    fn missing_nullable_fields_are_not_silently_accepted() {
        let json = r#"{"items":[{"text":"Bericht","evidence":[{"sourceId":"S00001","quote":"Anna schreibt den Bericht bis Freitag."}]}]}"#;
        assert!(validate_answer(json, &sources(), &Kind::Tasks).is_err());
    }
    #[test]
    fn valid_task_and_plain_text_export() {
        let json = r#"{"items":[{"text":"<script>Bericht</script>","owner":"Anna","due":"Freitag","evidence":[{"sourceId":"S00001","quote":"Anna schreibt den Bericht bis Freitag."}]}]}"#;
        assert_eq!(
            validate_answer(json, &sources(), &Kind::Tasks)
                .unwrap()
                .len(),
            1
        );
        assert!(validate_answer(json, &sources(), &Kind::Decisions).is_err());
    }
    #[test]
    fn redirects_are_not_followed_and_no_proxy_is_used() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0; 8192];
            let _ = stream.read(&mut buffer);
            stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: https://example.com/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        let client = TextClient::new(&config(&format!("http://{address}/v1/"))).unwrap();
        assert!(client.models().is_err());
        thread.join().unwrap();
    }
}
