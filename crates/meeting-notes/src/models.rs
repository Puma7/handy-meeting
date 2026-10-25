use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub source: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Summary,
    Decisions,
    Tasks,
    Analysis,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub prompt: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub steps: Vec<Step>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    pub source_id: String,
    pub quote: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fact {
    pub text: String,
    #[serde(deserialize_with = "nullable")]
    pub owner: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub due: Option<String>,
    pub evidence: Vec<Evidence>,
}
fn nullable<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Answer {
    pub(crate) items: Vec<Fact>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub items: Vec<Fact>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub id: String,
    pub meeting_id: String,
    pub source_hash: String,
    pub template: Template,
    pub sections: Vec<Section>,
    pub sources: Vec<Source>,
    pub created_at: String,
    pub model: String,
    pub mode: String,
    pub endpoint: String,
    pub prompt_version: String,
}

/// Deliberately not serializable: an API key must never enter a stored run snapshot.
pub struct Connection {
    pub endpoint: String,
    pub model: String,
    pub mode: String,
    pub cloud_consent: bool,
    pub api_key: String,
}
