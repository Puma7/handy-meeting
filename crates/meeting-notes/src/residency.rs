//! Local model lifetime, independent of sessions, transcripts and audio access.
use crate::{AnalysisControl, Connection, TextClient};
use anyhow::{ensure, Context, Result};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalProvider {
    Ollama,
    LmStudio,
}

#[derive(Default)]
pub struct LocalModelPool {
    slots: Arc<Mutex<HashMap<String, Arc<Slot>>>>,
}
struct Slot {
    state: Mutex<State>,
    identity: String,
    auth_digest: [u8; 32],
    provider: LocalProvider,
    model: String,
    users: AtomicUsize,
    cleanup_failed: AtomicBool,
}
struct State {
    client: TextClient,
    provider: LocalProvider,
    loaded: Option<Loaded>,
}
enum Loaded {
    Ollama { release_after: i64 },
    LmStudio { instance: String },
}

/// One logical user's reservation. Arc clones retain an in-flight user's ownership.
pub struct ModelLease {
    slot: Arc<Slot>,
    slots: Arc<Mutex<HashMap<String, Arc<Slot>>>>,
}

impl LocalModelPool {
    /// Reserves only the caller's chosen local route/model, without making network requests.
    pub fn reserve(
        &self,
        connection: &Connection,
        provider: LocalProvider,
    ) -> Result<Arc<ModelLease>> {
        let client = TextClient::new(connection)?;
        valid_model_name(&connection.model)?;
        let identity = client.local_identity()?;
        let auth_digest = client.local_auth_digest();
        let mut slots = self
            .slots
            .lock()
            .map_err(|_| anyhow::anyhow!("Modellreservierungen sind gesperrt."))?;
        ensure!(
            slots.len() < 128 || slots.contains_key(&identity),
            "Zu viele lokale Modellverbindungen; Anwendung vor weiteren Verbindungen neu starten."
        );
        let slot = Arc::clone(slots.entry(identity.clone()).or_insert_with(|| {
            Arc::new(Slot {
                identity,
                auth_digest,
                provider,
                model: connection.model.clone(),
                users: AtomicUsize::new(0),
                cleanup_failed: AtomicBool::new(false),
                state: Mutex::new(State {
                    client,
                    provider,
                    loaded: None,
                }),
            })
        }));
        ensure!(
            slot.provider == provider && slot.auth_digest == auth_digest,
            "Die reservierte Modellverbindung wurde geändert. Anwendung vor der neuen Verbindung neu starten."
        );
        ensure!(!slot.cleanup_failed.load(Ordering::Acquire), "Die vorherige Modellfreigabe konnte nicht bestätigt werden. Lokalen Modelldienst prüfen.");
        slot.users
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                count.checked_add(1)
            })
            .map_err(|_| anyhow::anyhow!("Zu viele Modellreservierungen."))?;
        // Reservation becomes visible while the registry is locked, before any last-user
        // cleanup can run. Actual provider loading never holds this registry lock.
        drop(slots);
        Ok(Arc::new(ModelLease {
            slot,
            slots: Arc::clone(&self.slots),
        }))
    }
}

impl ModelLease {
    pub(crate) fn matches_client(&self, client: &TextClient) -> bool {
        client
            .local_identity()
            .is_ok_and(|identity| identity == self.slot.identity)
            && client.local_auth_digest() == self.slot.auth_digest
    }
    /// Checks reuse without waiting for provider loading or exposing the credential.
    pub fn matches_connection(
        &self,
        connection: &Connection,
        provider: LocalProvider,
    ) -> Result<bool> {
        Ok(self.slot.provider == provider && self.matches_client(&TextClient::new(connection)?))
    }
    /// Preloads once. The caller runs this outside recording locks and retains the lease
    /// through final persistence; failed/partial jobs may retain it for an explicit retry.
    pub fn prepare(&self, control: &AnalysisControl) -> Result<()> {
        control.check()?;
        let mut state = self
            .slot
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Lokales Modell ist gesperrt."))?;
        control.check()?;
        if state.loaded.is_some() {
            return Ok(());
        }
        let client = &state.client;
        let loaded = match state.provider {
            LocalProvider::Ollama => {
                let before =
                    client.model_request("api/ps", None, control, Duration::from_secs(10))?;
                let models = before["models"]
                    .as_array()
                    .context("Ungültige geladene Modellliste.")?;
                ensure!(models.len() <= 10_000, "Geladene Modellliste ist zu groß.");
                let found = models
                    .iter()
                    .find(|m| model_name_matches(m["name"].as_str(), &client.model));
                let release_after = match found {
                    None => 0,
                    Some(model) => {
                        let expiry = model["expires_at"]
                            .as_str()
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok());
                        match expiry {
                            Some(at)
                                if at.signed_duration_since(chrono::Utc::now()).num_days()
                                    <= 365 =>
                            {
                                at.signed_duration_since(chrono::Utc::now())
                                    .num_seconds()
                                    .max(300)
                            }
                            _ => -1,
                        }
                    }
                };
                let response = client.model_request(
                    "api/generate",
                    Some(&json!({"model":client.model,"stream":false,"keep_alive":-1})),
                    // A native load creates server state. Once submitted, read its bounded
                    // acknowledgement even after cancellation so the owned instance remains
                    // known and can be released. Inference itself remains cancellable.
                    &AnalysisControl::default(),
                    Duration::from_secs(300),
                )?;
                ensure!(
                    response["done"] == true,
                    "Textmodell wurde nicht als geladen bestätigt."
                );
                Loaded::Ollama { release_after }
            }
            LocalProvider::LmStudio => {
                let before = client.model_request(
                    "api/v1/models",
                    None,
                    control,
                    Duration::from_secs(10),
                )?;
                let models = before["models"]
                    .as_array()
                    .context("Ungültige lokale Modellliste.")?;
                ensure!(models.len() <= 10_000, "Lokale Modellliste ist zu groß.");
                let model = models
                    .iter()
                    .find(|m| {
                        m["type"] == "llm"
                            && (m["key"].as_str() == Some(client.model.as_str())
                                || m["loaded_instances"].as_array().is_some_and(|instances| {
                                    instances
                                        .iter()
                                        .any(|i| i["id"].as_str() == Some(client.model.as_str()))
                                }))
                    })
                    .context("Das gewählte Textmodell ist im lokalen Dienst nicht vorhanden.")?;
                let key = model["key"]
                    .as_str()
                    .context("Lokale Modellkennung fehlt.")?;
                valid_model_name(key)?;
                let existing: Vec<&str> = models
                    .iter()
                    .filter_map(|m| m["loaded_instances"].as_array())
                    .flatten()
                    .filter_map(|i| i["id"].as_str())
                    .collect();
                let response = client.model_request(
                    "api/v1/models/load",
                    Some(&json!({"model":key})),
                    &AnalysisControl::default(),
                    Duration::from_secs(300),
                )?;
                let instance = response["instance_id"]
                    .as_str()
                    .context("Geladene Modellinstanz fehlt.")?;
                valid_model_name(instance)?;
                ensure!(
                    response["status"] == "loaded" && response["type"] == "llm",
                    "Textmodell wurde nicht als geladen bestätigt."
                );
                ensure!(!existing.contains(&instance), "Der Dienst hat eine bereits vorhandene Modellinstanz zurückgegeben; deren dauerhafte Reservierung ist nicht bestätigt und sie wird nicht entladen.");
                Loaded::LmStudio {
                    instance: instance.to_owned(),
                }
            }
        };
        state.loaded = Some(loaded);
        control.check()
    }
    /// Returns only routing policy for the selected model; never session or audio data.
    pub(crate) fn inference(&self) -> Result<(LocalProvider, String)> {
        let state = self
            .slot
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Lokales Modell ist gesperrt."))?;
        let name = match state
            .loaded
            .as_ref()
            .context("Lokales Modell wurde noch nicht geladen.")?
        {
            Loaded::Ollama { .. } => state.client.model.clone(),
            Loaded::LmStudio { instance } => instance.clone(),
        };
        Ok((state.provider, name))
    }
    pub fn same_model(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.slot, &other.slot)
    }
}

impl Drop for ModelLease {
    fn drop(&mut self) {
        if self.slot.users.fetch_sub(1, Ordering::AcqRel) != 1 {
            return;
        }
        let Ok(slots) = self.slots.lock() else {
            return;
        };
        // Different loopback spellings can reach the same provider. Conservatively defer
        // cleanup across all routes to that provider/model, without merging their routes,
        // credentials or owned instance IDs. A distinct server may simply stay warm longer.
        let same_model = |slot: &&Arc<Slot>| {
            slot.provider == self.slot.provider
                && if slot.provider == LocalProvider::Ollama {
                    model_name_matches(Some(&slot.model), &self.slot.model)
                } else {
                    slot.model == self.slot.model
                }
        };
        if slots
            .values()
            .filter(same_model)
            .any(|slot| slot.users.load(Ordering::Acquire) > 0)
        {
            return;
        }
        for slot in slots.values().filter(same_model) {
            slot.release();
        }
    }
}
impl Slot {
    fn release(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Some(loaded) = state.loaded.take() else {
            return;
        };
        let expected_instance = match &loaded {
            Loaded::LmStudio { instance } => Some(instance.clone()),
            _ => None,
        };
        let control = AnalysisControl::default();
        let result = match loaded {
            Loaded::Ollama { release_after: -1 } => return,
            Loaded::Ollama { release_after } => state.client.model_request(
                "api/generate",
                Some(
                    &json!({"model":state.client.model,"stream":false,"keep_alive":release_after}),
                ),
                &control,
                Duration::from_secs(10),
            ),
            Loaded::LmStudio { instance } => state.client.model_request(
                "api/v1/models/unload",
                Some(&json!({"instance_id":instance})),
                &control,
                Duration::from_secs(10),
            ),
        };
        self.cleanup_failed.store(
            match result {
                Ok(response) => match state.provider {
                    LocalProvider::Ollama => response["done"] != true,
                    LocalProvider::LmStudio => {
                        response["instance_id"].as_str() != expected_instance.as_deref()
                    }
                },
                Err(_) => true,
            },
            Ordering::Release,
        );
    }
}

fn valid_model_name(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= 300 && !value.chars().any(char::is_control),
        "Ungültige lokale Modellkennung."
    );
    Ok(())
}
fn model_name_matches(value: Option<&str>, model: &str) -> bool {
    value.is_some_and(|value| {
        value == model
            || value.strip_suffix(":latest") == Some(model)
            || model.strip_suffix(":latest") == Some(value)
    })
}
