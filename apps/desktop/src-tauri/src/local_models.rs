//! One-click local text model setup: known local servers, a model suggestion from installed
//! memory and an optional Ollama model download. Only fixed loopback addresses are contacted;
//! no transcript, session or key is involved.
use crate::AppResult;
use serde::Serialize;
use std::{
    io::{BufRead, BufReader},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

pub(crate) const OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434/v1/";
pub(crate) const LM_STUDIO_ENDPOINT: &str = "http://127.0.0.1:1234/v1/";
const OLLAMA_PULL: &str = "http://127.0.0.1:11434/api/pull";
pub(crate) const SMALL_MODEL: &str = "gemma4:e2b";
pub(crate) const STANDARD_MODEL: &str = "gemma4:e4b";
const GIB: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Recommendation {
    /// none | small | standard | large | unknown
    tier: &'static str,
    model: Option<&'static str>,
    /// Name to search for in LM Studio's model catalog.
    lm_studio_search: Option<&'static str>,
}

/// Suggests a model size from physical memory. The thresholds leave room for the meeting
/// app, the speech model and the operating system next to the text model.
pub(crate) fn recommend(total_memory: Option<u64>) -> Recommendation {
    let (tier, model) = match total_memory {
        None => ("unknown", Some(STANDARD_MODEL)),
        Some(bytes) if bytes < 7 * GIB => ("none", None),
        Some(bytes) if bytes < 15 * GIB => ("small", Some(SMALL_MODEL)),
        Some(bytes) if bytes < 31 * GIB => ("standard", Some(STANDARD_MODEL)),
        Some(_) => ("large", Some(STANDARD_MODEL)),
    };
    Recommendation {
        tier,
        model,
        lm_studio_search: model.map(|m| {
            if m == SMALL_MODEL {
                "Gemma 4 E2B"
            } else {
                "Gemma 4 E4B"
            }
        }),
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Provider {
    id: &'static str,
    name: &'static str,
    endpoint: &'static str,
    reachable: bool,
    models: Vec<String>,
    recommended_installed: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalModelSetup {
    total_memory_bytes: Option<u64>,
    recommendation: Recommendation,
    providers: Vec<Provider>,
}

/// Installed physical memory, read without extra libraries. `None` when it cannot be read.
pub(crate) fn total_memory() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let info = std::fs::read_to_string("/proc/meminfo").ok()?;
        parse_meminfo(&info)
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/usr/sbin/sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        String::from_utf8(output.stdout).ok()?.trim().parse().ok()
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let root = std::env::var_os("SystemRoot")?;
        let shell = std::path::Path::new(&root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        let output = std::process::Command::new(shell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[string](Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        String::from_utf8(output.stdout).ok()?.trim().parse().ok()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        None
    }
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_meminfo(info: &str) -> Option<u64> {
    let line = info.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    kib.checked_mul(1024)
}

fn installed(models: &[String], wanted: Option<&str>) -> bool {
    wanted.is_some_and(|wanted| {
        models
            .iter()
            .any(|m| m == wanted || m.strip_suffix(":latest") == Some(wanted))
    })
}

/// Checks both known local servers on loopback and suggests a model.
pub(crate) fn setup() -> LocalModelSetup {
    let total_memory_bytes = total_memory();
    let recommendation = recommend(total_memory_bytes);
    let providers = [
        ("ollama", "Ollama", OLLAMA_ENDPOINT),
        ("lmstudio", "LM Studio", LM_STUDIO_ENDPOINT),
    ]
    .into_iter()
    .map(|(id, name, endpoint)| {
        let found = meeting_notes::local_model_ids(endpoint);
        let models = found.as_ref().cloned().unwrap_or_default();
        Provider {
            id,
            name,
            endpoint,
            reachable: found.is_ok(),
            recommended_installed: installed(&models, recommendation.model),
            models,
        }
    })
    .collect();
    LocalModelSetup {
        total_memory_bytes,
        recommendation,
        providers,
    }
}

/// Ollama model names: registry-style names with an optional tag, nothing else.
pub(crate) fn valid_model_name(name: &str) -> bool {
    (1..=100).contains(&name.len())
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._:-/".contains(&c))
        && name
            .bytes()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && !name.contains("..")
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullProgress {
    model: String,
    /// running | complete | failed | cancelled
    status: String,
    detail: String,
    completed: u64,
    total: u64,
    error: Option<String>,
}

#[derive(Default)]
pub(crate) struct Pulls {
    state: Mutex<Option<PullProgress>>,
    cancel: Mutex<Arc<AtomicBool>>,
}

impl Pulls {
    pub(crate) fn snapshot(&self) -> Option<PullProgress> {
        self.state.lock().ok().and_then(|s| s.clone())
    }

    pub(crate) fn cancel(&self) -> bool {
        let running = self
            .state
            .lock()
            .is_ok_and(|s| s.as_ref().is_some_and(|p| p.status == "running"));
        if running {
            if let Ok(flag) = self.cancel.lock() {
                flag.store(true, Ordering::Release);
            }
        }
        running
    }

    /// Asks the local Ollama to download `model` and follows its progress in the background.
    pub(crate) fn start(self: &Arc<Self>, model: String) -> AppResult<PullProgress> {
        if !valid_model_name(&model) {
            return Err("Ungültiger Modellname.".into());
        }
        let mut state = self.state.lock().map_err(|_| "Download ist gesperrt.")?;
        if state.as_ref().is_some_and(|p| p.status == "running") {
            return Err("Ein Modell-Download läuft bereits.".into());
        }
        let flag = Arc::new(AtomicBool::new(false));
        *self.cancel.lock().map_err(|_| "Download ist gesperrt.")? = Arc::clone(&flag);
        let progress = PullProgress {
            model: model.clone(),
            status: "running".into(),
            ..PullProgress::default()
        };
        *state = Some(progress.clone());
        drop(state);
        let pulls = Arc::clone(self);
        std::thread::Builder::new()
            .name("ollama-pull".into())
            .spawn(move || {
                let result = pull(&model, &flag, |update| pulls.update(update));
                pulls.finish(result, flag.load(Ordering::Acquire));
            })
            .map_err(|_| "Der Download konnte nicht gestartet werden.")?;
        Ok(progress)
    }

    fn update(&self, line: PullLine) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(progress) = state.as_mut().filter(|p| p.status == "running") {
                progress.detail = line.status.chars().take(200).collect();
                if let (Some(completed), Some(total)) = (line.completed, line.total) {
                    progress.completed = completed;
                    progress.total = total;
                }
            }
        }
    }

    fn finish(&self, result: AppResult<()>, cancelled: bool) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(progress) = state.as_mut() {
                match result {
                    Ok(()) => progress.status = "complete".into(),
                    Err(_) if cancelled => progress.status = "cancelled".into(),
                    Err(error) => {
                        progress.status = "failed".into();
                        progress.error = Some(error);
                    }
                }
            }
        }
    }
}

#[derive(serde::Deserialize, Default)]
struct PullLine {
    #[serde(default)]
    status: String,
    #[serde(default)]
    completed: Option<u64>,
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    error: Option<String>,
}

fn pull(model: &str, cancel: &AtomicBool, mut progress: impl FnMut(PullLine)) -> AppResult<()> {
    pull_from(OLLAMA_PULL, model, cancel, &mut progress)
}

/// Follows Ollama's streamed pull answer line by line. Same transport rules as local text
/// models: loopback only, no proxy, no redirects.
fn pull_from(
    url: &str,
    model: &str,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(PullLine),
) -> AppResult<()> {
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(None)
        .build()
        .map_err(|_| "Lokale Verbindung konnte nicht vorbereitet werden.")?;
    let response = client
        .post(url)
        .json(&serde_json::json!({"model": model, "stream": true}))
        .send()
        .map_err(|_| "Ollama ist nicht erreichbar. Bitte Ollama installieren und starten.")?;
    if !response.status().is_success() {
        return Err(format!(
            "Ollama meldet HTTP {} für den Download.",
            response.status().as_u16()
        ));
    }
    let mut reader = BufReader::new(std::io::Read::take(response, 64 * 1024 * 1024));
    let mut line = String::new();
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err("Download abgebrochen.".into());
        }
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|_| "Die Verbindung zu Ollama wurde unterbrochen.")?;
        if read == 0 {
            return Err("Ollama hat den Download ohne Abschluss beendet.".into());
        }
        if line.len() > 64 * 1024 {
            return Err("Ungültige Antwort von Ollama.".into());
        }
        let Ok(update) = serde_json::from_str::<PullLine>(line.trim()) else {
            continue;
        };
        if let Some(error) = &update.error {
            return Err(format!(
                "Ollama meldet: {}",
                error.chars().take(300).collect::<String>()
            ));
        }
        let success = update.status == "success";
        progress(update);
        if success {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn memory_tiers_follow_the_documented_thresholds() {
        assert_eq!(recommend(Some(4 * GIB)).tier, "none");
        assert_eq!(recommend(Some(4 * GIB)).model, None);
        assert_eq!(recommend(Some(8 * GIB)).model, Some(SMALL_MODEL));
        assert_eq!(recommend(Some(16 * GIB)).model, Some(STANDARD_MODEL));
        assert_eq!(recommend(Some(16 * GIB)).tier, "standard");
        assert_eq!(recommend(Some(64 * GIB)).tier, "large");
        assert_eq!(recommend(None).tier, "unknown");
        assert_eq!(
            recommend(Some(8 * GIB)).lm_studio_search,
            Some("Gemma 4 E2B")
        );
    }

    #[test]
    fn meminfo_is_read_in_bytes() {
        let info = "MemTotal:       16303428 kB\nMemFree:  1 kB\n";
        assert_eq!(parse_meminfo(info), Some(16_303_428 * 1024));
        assert_eq!(parse_meminfo("nothing"), None);
    }

    #[test]
    fn only_plain_model_names_are_sent_to_ollama() {
        for name in ["gemma4:e4b", "gemma4:e2b", "library/qwen3:4b", "llama3.2"] {
            assert!(valid_model_name(name), "{name}");
        }
        for name in ["", "Gemma4", "../x", "a b", "gemma4:e4b\n", "-flag", "x\"}"] {
            assert!(!valid_model_name(name), "{name:?}");
        }
    }

    #[test]
    fn a_recommended_model_counts_as_installed_with_or_without_latest() {
        let models = vec!["gemma4:e4b".to_string()];
        assert!(installed(&models, Some("gemma4:e4b")));
        assert!(!installed(&models, Some("gemma4:e2b")));
        assert!(installed(&["gemma4:e2b:latest".into()], Some("gemma4:e2b")));
        assert!(!installed(&models, None));
    }

    /// Synthetic Ollama: streams pull progress lines, then success.
    fn serve(lines: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/api/pull", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = vec![0_u8; 4096];
            let count = stream.read(&mut request).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{lines}",
                lines.len()
            )
            .unwrap();
            String::from_utf8_lossy(&request[..count]).into_owned()
        });
        (url, handle)
    }

    #[test]
    fn pull_progress_is_followed_until_success() {
        let (url, server) = serve(
            "{\"status\":\"pulling manifest\"}\n{\"status\":\"downloading\",\"completed\":5,\"total\":10}\n{\"status\":\"success\"}\n",
        );
        let mut seen = Vec::new();
        pull_from(&url, "gemma4:e4b", &AtomicBool::new(false), &mut |l| {
            seen.push((l.status, l.completed))
        })
        .unwrap();
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[1], ("downloading".into(), Some(5)));
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /api/pull HTTP/1.1"));
        assert!(request.contains("\"model\":\"gemma4:e4b\""));
    }

    #[test]
    fn a_pull_error_or_an_unfinished_stream_is_reported_not_treated_as_success() {
        let (url, server) = serve("{\"error\":\"pull model manifest: file does not exist\"}\n");
        let error =
            pull_from(&url, "gemma4:e9b", &AtomicBool::new(false), &mut |_| {}).unwrap_err();
        assert!(error.contains("file does not exist"), "{error}");
        server.join().unwrap();
        let (url, server) = serve("{\"status\":\"downloading\"}\n");
        assert!(pull_from(&url, "gemma4:e4b", &AtomicBool::new(false), &mut |_| {}).is_err());
        server.join().unwrap();
    }
}
