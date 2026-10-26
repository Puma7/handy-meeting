//! Versioned, consumer-owned integration description. This is not an upstream
//! Handy handshake and does not imply that its private cache layout is stable.
use crate::{BridgeError, ModelCatalog, Result};
use serde::Serialize;

pub const COMPANION_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionCapabilities {
    pub schema_version: u32,
    pub protocol_owner: &'static str,
    pub upstream_negotiated: bool,
    pub installed_handy_version: Option<String>,
    pub catalog: CatalogCapabilities,
    pub recognition: RecognitionCapabilities,
    pub privacy: PrivacyCapabilities,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCapabilities {
    pub query: [&'static str; 2],
    pub fetched_at_unix_ms: u64,
    pub fingerprint_sha256: String,
    pub entries: usize,
    pub installed_streaming_advertisements: usize,
    pub fresh_query_required_before_capture: bool,
    pub additive_fields_accepted: bool,
    pub missing_required_fields_rejected: bool,
    pub model_path_contract: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionCapabilities {
    pub binding_version: &'static str,
    pub native_api: &'static str,
    pub shares_handy_inference_process: bool,
    pub concurrent_streams_per_model_instance: u8,
    pub model_compatibility: &'static str,
    pub resume_requires_original_identity: bool,
    pub automatic_model_substitution: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyCapabilities {
    pub reads_handy_credentials: bool,
    pub writes_handy_settings_or_weights: bool,
    pub downloads_model_weights: bool,
    pub exports_local_paths: bool,
    pub contains_session_content: bool,
}

pub fn companion_capabilities(
    requested_version: u32,
    catalog: &ModelCatalog,
    installed_handy_version: Option<String>,
) -> Result<CompanionCapabilities> {
    if requested_version != COMPANION_CONTRACT_VERSION {
        return Err(BridgeError::UnsupportedSchema);
    }
    Ok(CompanionCapabilities {
        schema_version: COMPANION_CONTRACT_VERSION,
        protocol_owner: "HandyMeeting",
        upstream_negotiated: false,
        installed_handy_version,
        catalog: CatalogCapabilities {
            query: ["--list-models", "--json"],
            fetched_at_unix_ms: catalog.fetched_at_unix_ms,
            fingerprint_sha256: catalog.fingerprint_sha256.clone(),
            entries: catalog.models.len(),
            installed_streaming_advertisements: catalog.installed_streaming_models().count(),
            fresh_query_required_before_capture: true,
            additive_fields_accepted: true,
            missing_required_fields_rejected: true,
            model_path_contract: "observed-cache-layout; not an upstream stable path API",
        },
        recognition: RecognitionCapabilities {
            binding_version: "0.4.1",
            native_api: "official-safe-transcribe-cpp",
            shares_handy_inference_process: false,
            concurrent_streams_per_model_instance: 1,
            model_compatibility:
                "advertisement is insufficient; validated during native model load",
            resume_requires_original_identity: true,
            automatic_model_substitution: false,
        },
        privacy: PrivacyCapabilities {
            reads_handy_credentials: false,
            writes_handy_settings_or_weights: false,
            downloads_model_weights: false,
            exports_local_paths: false,
            contains_session_content: false,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> ModelCatalog {
        ModelCatalog {
            installation: crate::InstallationIdentity {
                executable: "/private/never-export/handy".into(),
                file_bytes: 99,
                modified_unix_ms: 123,
            },
            fetched_at_unix_ms: 456,
            fingerprint_sha256: "a".repeat(64),
            models: crate::parse_model_list(br#"[
                {"id":"private-model-id","name":"Private model name","filename":"secret.gguf","source":{"Local":"/private/weights.gguf"},"is_downloaded":true,"is_downloading":false,"supports_streaming":true},
                {"id":"second","name":"Second","filename":"model.gguf","source":{},"is_downloaded":false,"is_downloading":false,"supports_streaming":true}
            ]"#).unwrap(),
        }
    }

    #[test]
    fn consumer_description_exports_no_paths_model_names_or_source_objects() {
        let report = companion_capabilities(1, &catalog(), Some("0.9.8".into())).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        for private in [
            "/private",
            "private-model-id",
            "Private model name",
            "secret.gguf",
        ] {
            assert!(!json.contains(private));
        }
        assert_eq!(report.catalog.entries, 2);
        assert_eq!(report.catalog.installed_streaming_advertisements, 1);
        assert!(!report.upstream_negotiated);
        assert!(!report.recognition.shares_handy_inference_process);
        assert!(!report.privacy.reads_handy_credentials);
    }

    #[test]
    fn unknown_consumer_contract_is_rejected_without_guessing() {
        for version in [0, 2, u32::MAX] {
            assert!(matches!(
                companion_capabilities(version, &catalog(), None),
                Err(BridgeError::UnsupportedSchema)
            ));
        }
    }

    #[test]
    fn reports_the_current_observation_instead_of_a_compiled_model_catalog() {
        let mut current = catalog();
        current.models.clear();
        current.fingerprint_sha256 = "b".repeat(64);
        current.fetched_at_unix_ms = 789;
        let report = companion_capabilities(1, &current, None).unwrap();
        assert_eq!(report.catalog.entries, 0);
        assert_eq!(report.catalog.fetched_at_unix_ms, 789);
        assert_eq!(report.catalog.fingerprint_sha256, "b".repeat(64));
        assert!(report.installed_handy_version.is_none());
        assert!(report.catalog.fresh_query_required_before_capture);
        assert!(!report.recognition.automatic_model_substitution);
    }
}
