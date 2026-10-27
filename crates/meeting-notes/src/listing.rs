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

pub fn skip_reason(error: &anyhow::Error) -> SkipReason {
    if error.downcast_ref::<UnsupportedFormat>().is_some() {
        SkipReason::NewerFormat
    } else {
        SkipReason::Damaged
    }
}

pub fn is_run_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|c| c.is_ascii_hexdigit())
}

/// Lists every finished result below `root` (one folder per run). Folders without a result
/// are unfinished runs and belong to pending discovery. Nothing is modified or deleted.
pub fn list_analyses(root: &Path, meeting_id: &str) -> Result<AnalysisListing> {
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
        if fs::symlink_metadata(&path).is_err() {
            continue;
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
