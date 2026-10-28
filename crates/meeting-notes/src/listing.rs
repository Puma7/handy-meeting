//! Per-file discovery of stored results: one unreadable file never hides the others.
use crate::service::{is_linked, UnsupportedFormat};
use crate::{read_analysis, Analysis, PendingAnalysis, PROMPT_VERSION};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{fs, path::Path};

/// A verified finished result. `current_prompt` is false for results of an earlier prompt
/// version: they stay readable as they were saved, but are never resumed or extended.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedAnalysis {
    #[serde(flatten)]
    pub analysis: Analysis,
    pub current_prompt: bool,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    /// Unreadable, linked, or failing the integrity and source checks.
    Damaged,
    /// Written in a storage format this build does not know.
    NewerFormat,
    /// Belongs to a different meeting than the folder it was found in.
    OtherMeeting,
}

/// A stored work state that was left untouched and not shown.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkippedAnalysis {
    pub id: String,
    pub reason: SkipReason,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisListing {
    pub analyses: Vec<ListedAnalysis>,
    pub skipped: Vec<SkippedAnalysis>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingListing {
    pub items: Vec<PendingAnalysis>,
    pub skipped: Vec<SkippedAnalysis>,
}

/// Separates unsupported saved schemas from damaged results without changing either file.
pub fn skip_reason(error: &anyhow::Error) -> SkipReason {
    if error.downcast_ref::<UnsupportedFormat>().is_some() {
        SkipReason::NewerFormat
    } else {
        SkipReason::Damaged
    }
}

/// Accepts only fixed-length hexadecimal IDs before considering a result-directory entry.
pub fn is_run_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|c| c.is_ascii_hexdigit())
}

/// Lists every finished result below `root` (one folder per run). Folders without a result
/// are unfinished runs and belong to pending discovery. Nothing is modified or deleted.
pub fn list_analyses(root: &Path, meeting_id: &str) -> Result<AnalysisListing> {
    list_with_result_metadata(root, meeting_id, |path| fs::symlink_metadata(path))
}
/// Discovers finished runs with injectable metadata reads and reports individually unreadable entries.
fn list_with_result_metadata(
    root: &Path,
    meeting_id: &str,
    mut result_metadata: impl FnMut(&Path) -> std::io::Result<fs::Metadata>,
) -> Result<AnalysisListing> {
    let mut listing = AnalysisListing::default();
    for entry in
        fs::read_dir(root).context("Gespeicherte Auswertungen konnten nicht gelesen werden.")?
    {
        let entry = entry.context("Gespeicherte Auswertungen konnten nicht gelesen werden.")?;
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_run_id(&id) {
            continue;
        }
        let folder = entry.path();
        let skip = |listing: &mut AnalysisListing, reason| {
            listing.skipped.push(SkippedAnalysis {
                id: id.clone(),
                reason,
            })
        };
        match fs::symlink_metadata(&folder) {
            Ok(metadata) if metadata.is_dir() && !is_linked(&metadata) => {}
            _ => {
                skip(&mut listing, SkipReason::Damaged);
                continue;
            }
        }
        let path = folder.join("result.json");
        match result_metadata(&path) {
            Ok(metadata) if metadata.is_file() && !is_linked(&metadata) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            _ => {
                skip(&mut listing, SkipReason::Damaged);
                continue;
            }
        }
        match read_analysis(&path) {
            Ok(analysis) if analysis.id != id => skip(&mut listing, SkipReason::Damaged),
            Ok(analysis) if analysis.meeting_id != meeting_id => {
                skip(&mut listing, SkipReason::OtherMeeting)
            }
            Ok(analysis) => listing.analyses.push(ListedAnalysis {
                current_prompt: analysis.prompt_version == PROMPT_VERSION,
                analysis,
            }),
            Err(error) => skip(&mut listing, skip_reason(&error)),
        }
    }
    listing
        .analyses
        .sort_by(|a, b| b.analysis.created_at.cmp(&a.analysis.created_at));
    listing.skipped.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(listing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{source_hash, Kind, Section, Source, Step, Template, RESULT_SCHEMA};

    /// Writes a structurally valid synthetic finished result and returns its exact original bytes.
    fn finished(root: &Path, id: &str) -> Vec<u8> {
        let sources = vec![Source {
            id: "S00001".into(),
            source: "system".into(),
            start_ms: 0,
            end_ms: 1000,
            text: "Invented source text for a local saved-result fixture.".into(),
        }];
        let result = Analysis {
            id: id.into(),
            meeting_id: "synthetic-meeting".into(),
            source_hash: source_hash(&sources).unwrap(),
            template: Template {
                id: "fixture".into(),
                name: "Synthetic template".into(),
                steps: vec![Step {
                    id: "summary".into(),
                    name: "Summary".into(),
                    kind: Kind::Summary,
                    prompt: "Summarize with exact quotes.".into(),
                }],
            },
            sections: vec![Section {
                id: "summary".into(),
                name: "Summary".into(),
                kind: Kind::Summary,
                items: vec![],
                rejected: Default::default(),
            }],
            sources,
            created_at: "2026-10-27T00:00:00Z".into(),
            model: "synthetic".into(),
            mode: "local".into(),
            endpoint: "http://127.0.0.1:1/v1/".into(),
            prompt_version: PROMPT_VERSION.into(),
            result_schema: RESULT_SCHEMA,
        };
        let folder = root.join(id);
        fs::create_dir(&folder).unwrap();
        let bytes = serde_json::to_vec(&result).unwrap();
        fs::write(folder.join("result.json"), &bytes).unwrap();
        bytes
    }

    #[test]
    /// Checks synthetic future schemas are identified before payload decoding and all stored bytes stay intact.
    fn incompatible_newer_result_shapes_are_reported_as_newer_and_left_untouched() {
        let root = tempfile::tempdir().unwrap();
        let valid = "11111111111111111111111111111111";
        let future = "22222222222222222222222222222222";
        let damaged = "33333333333333333333333333333333";
        let original = finished(root.path(), valid);
        fs::create_dir(root.path().join(future)).unwrap();
        fs::create_dir(root.path().join(damaged)).unwrap();
        // Valid JSON from a hypothetical future format, deliberately incompatible
        // with every current Analysis field. No model or transport is involved.
        let future_bytes=serde_json::to_vec(&serde_json::json!({"resultSchema":RESULT_SCHEMA+1,"sources":{"newShape":true},"template":7,"sections":null})).unwrap();
        let damaged_bytes=serde_json::to_vec(&serde_json::json!({"resultSchema":RESULT_SCHEMA,"sources":{"newShape":true},"template":7,"sections":null})).unwrap();
        let future_path = root.path().join(future).join("result.json");
        let damaged_path = root.path().join(damaged).join("result.json");
        fs::write(&future_path, &future_bytes).unwrap();
        fs::write(&damaged_path, &damaged_bytes).unwrap();
        assert_eq!(
            skip_reason(&read_analysis(&future_path).unwrap_err()),
            SkipReason::NewerFormat
        );
        assert_eq!(
            skip_reason(&read_analysis(&damaged_path).unwrap_err()),
            SkipReason::Damaged
        );
        let listing = list_analyses(root.path(), "synthetic-meeting").unwrap();
        assert_eq!(listing.analyses.len(), 1);
        assert_eq!(listing.analyses[0].analysis.id, valid);
        assert_eq!(
            listing.skipped,
            vec![
                SkippedAnalysis {
                    id: future.into(),
                    reason: SkipReason::NewerFormat
                },
                SkippedAnalysis {
                    id: damaged.into(),
                    reason: SkipReason::Damaged
                }
            ]
        );
        assert_eq!(
            fs::read(root.path().join(valid).join("result.json")).unwrap(),
            original
        );
        assert_eq!(fs::read(&future_path).unwrap(), future_bytes);
        assert_eq!(fs::read(&damaged_path).unwrap(), damaged_bytes);
    }

    #[test]
    /// Checks injected metadata failures and non-file results remain reported alongside valid synthetic runs.
    fn result_metadata_errors_and_non_regular_entries_are_reported_without_hiding_valid_runs() {
        let root = tempfile::tempdir().unwrap();
        let valid = "11111111111111111111111111111111";
        let denied = "22222222222222222222222222222222";
        let missing = "33333333333333333333333333333333";
        let directory = "44444444444444444444444444444444";
        let original = finished(root.path(), valid);
        let denied_original = finished(root.path(), denied);
        fs::create_dir(root.path().join(missing)).unwrap();
        fs::create_dir_all(root.path().join(directory).join("result.json")).unwrap();
        // Deterministic PermissionDenied fault injection avoids changing host ACLs
        // or relying on which account executes the test. All other metadata is real.
        let listing = list_with_result_metadata(root.path(), "synthetic-meeting", |path| {
            if path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                == Some(denied)
            {
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
            } else {
                fs::symlink_metadata(path)
            }
        })
        .unwrap();
        assert_eq!(listing.analyses.len(), 1);
        assert_eq!(listing.analyses[0].analysis.id, valid);
        assert_eq!(
            listing.skipped,
            vec![
                SkippedAnalysis {
                    id: denied.into(),
                    reason: SkipReason::Damaged
                },
                SkippedAnalysis {
                    id: directory.into(),
                    reason: SkipReason::Damaged
                }
            ]
        );
        assert_eq!(
            fs::read(root.path().join(valid).join("result.json")).unwrap(),
            original
        );
        assert_eq!(
            fs::read(root.path().join(denied).join("result.json")).unwrap(),
            denied_original
        );
        assert!(!root.path().join(missing).join("result.json").exists());
        assert!(root.path().join(directory).join("result.json").is_dir());
    }

    #[cfg(unix)]
    #[test]
    /// Checks synthetic linked results are rejected while unrelated valid results and link targets stay unchanged.
    fn linked_results_are_not_followed_and_other_saved_runs_stay_visible() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let valid = "11111111111111111111111111111111";
        let linked = "22222222222222222222222222222222";
        let original = finished(root.path(), valid);
        let outside_bytes = finished(outside.path(), linked);
        fs::create_dir(root.path().join(linked)).unwrap();
        let target = outside.path().join(linked).join("result.json");
        let link = root.path().join(linked).join("result.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(
            read_analysis(&link).is_err(),
            "A structurally valid linked result must not be read"
        );
        let listing = list_analyses(root.path(), "synthetic-meeting").unwrap();
        assert_eq!(listing.analyses.len(), 1);
        assert_eq!(listing.analyses[0].analysis.id, valid);
        assert_eq!(
            listing.skipped,
            vec![SkippedAnalysis {
                id: linked.into(),
                reason: SkipReason::Damaged
            }]
        );
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&target).unwrap(), outside_bytes);
        assert_eq!(
            fs::read(root.path().join(valid).join("result.json")).unwrap(),
            original
        );
    }
}
