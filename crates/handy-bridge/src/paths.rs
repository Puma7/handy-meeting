use crate::{BridgeError, HandyModel, ResolvedModel, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ModelRoots {
    pub hf_caches: Vec<PathBuf>,
    pub models_dirs: Vec<PathBuf>,
}

impl ModelRoots {
    pub fn for_installation(executable: &Path) -> Self {
        let mut roots = Self::default();
        let home = crate::discovery::home();
        if let Some(parent) = executable.parent() {
            let data = parent.join("Data");
            let marker = parent.join("portable");
            let valid_marker = std::fs::read_to_string(&marker)
                .ok()
                .is_some_and(|s| s.trim().starts_with("Handy Portable Mode"));
            if valid_marker || (marker.is_file() && data.is_dir()) {
                roots.hf_caches.push(data.join("huggingface/hub"));
                roots.models_dirs.push(data.join("models"));
            }
        }
        if let Some(cache) = std::env::var_os("HF_HUB_CACHE").filter(|s| !s.is_empty()) {
            roots.hf_caches.push(cache.into());
        }
        if let Some(hf_home) = std::env::var_os("HF_HOME").filter(|s| !s.is_empty()) {
            roots.hf_caches.push(PathBuf::from(hf_home).join("hub"));
        }
        if let Some(home) = &home {
            roots.hf_caches.push(home.join(".cache/huggingface/hub"));
            if cfg!(target_os = "macos") {
                roots
                    .models_dirs
                    .push(home.join("Library/Application Support/com.pais.handy/models"));
            } else if cfg!(target_os = "linux") {
                let data = std::env::var_os("XDG_DATA_HOME")
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".local/share"));
                roots.models_dirs.push(data.join("com.pais.handy/models"));
            }
        }
        if cfg!(windows) {
            for name in ["APPDATA", "LOCALAPPDATA"] {
                if let Some(base) = std::env::var_os(name) {
                    roots
                        .models_dirs
                        .push(PathBuf::from(base).join("com.pais.handy/models"));
                }
            }
        }
        roots.hf_caches.dedup();
        roots.models_dirs.dedup();
        roots
    }

    pub fn resolve(&self, model: &HandyModel) -> Result<ResolvedModel> {
        if !model.ready_for_streaming() {
            return Err(BridgeError::ModelUnavailable);
        }
        self.resolve_downloaded(model)
    }
    pub fn resolve_downloaded(&self, model: &HandyModel) -> Result<ResolvedModel> {
        if !model.is_downloaded || model.is_downloading {
            return Err(BridgeError::ModelUnavailable);
        }
        validate_component(&model.filename)?;
        let source = &model.source;
        if let Some(hf) = source.get("HuggingFace") {
            let repo = hf
                .get("repo_id")
                .and_then(|v| v.as_str())
                .ok_or(BridgeError::UnsupportedSchema)?;
            let revision = hf
                .get("revision")
                .and_then(|v| v.as_str())
                .ok_or(BridgeError::UnsupportedSchema)?;
            let pieces: Vec<_> = repo.split('/').collect();
            if pieces.len() != 2 {
                return Err(BridgeError::UnsafePath);
            }
            for piece in pieces {
                validate_component(piece)?;
            }
            validate_component(revision)?;
            let mut candidates = Vec::new();
            for (cache_rank, cache) in self.hf_caches.iter().enumerate() {
                let repo_dir = cache.join(format!("models--{}", repo.replace('/', "--")));
                let snapshots = repo_dir.join("snapshots");
                let pin = read_ref(&repo_dir.join("refs").join(revision))?;
                let main = read_ref(&repo_dir.join("refs/main"))?;
                let mut revisions = vec![revision.to_owned()];
                if let Some(pin) = &pin {
                    revisions.push(pin.clone());
                }
                if let Some(main) = &main {
                    revisions.push(main.clone());
                }
                if snapshots.is_dir() {
                    let entries = std::fs::read_dir(&snapshots)?;
                    for entry in entries.take(10_001) {
                        let entry = entry?;
                        if revisions.len() >= 10_000 {
                            return Err(BridgeError::OutputLimit);
                        }
                        if entry.path().is_dir() {
                            let name = entry
                                .file_name()
                                .into_string()
                                .map_err(|_| BridgeError::UnsafePath)?;
                            validate_component(&name)?;
                            revisions.push(name);
                        }
                    }
                }
                revisions.sort();
                revisions.dedup();
                for rev in revisions {
                    let rank = if rev == revision || pin.as_ref() == Some(&rev) {
                        0
                    } else if main.as_ref() == Some(&rev) {
                        1
                    } else {
                        2
                    };
                    candidates.push((
                        rank,
                        cache_rank,
                        rev.clone(),
                        snapshots.join(rev).join(&model.filename),
                        repo_dir.clone(),
                    ));
                }
            }
            candidates.sort_by(|a, b| (&a.0, &a.1, &a.2).cmp(&(&b.0, &b.1, &b.2)));
            for (rank, _, revision, candidate, repo_dir) in candidates {
                if candidate.is_file() {
                    let path = contained_file(&candidate, &repo_dir)?;
                    return Ok(ResolvedModel {
                        model_id: model.id.clone(),
                        path,
                        resolution: match rank {
                            0 => "requested-revision",
                            1 => "main-revision",
                            _ => "cached-snapshot",
                        }
                        .into(),
                        revision: Some(revision),
                    });
                }
            }
            // Matches Handy's mirror/manual-drop-in fallback, without deleting
            // stale .partial files or changing Handy's metadata.
            return self.resolve_models_dir(model, true);
        }
        if source == "Local" || source.get("Url").is_some() {
            return self.resolve_models_dir(model, false);
        }
        Err(BridgeError::UnsupportedSource)
    }

    fn resolve_models_dir(&self, model: &HandyModel, hf_fallback: bool) -> Result<ResolvedModel> {
        for root in &self.models_dirs {
            let candidate = root.join(&model.filename);
            if !candidate.is_file() {
                continue;
            }
            if !hf_fallback && root.join(format!("{}.partial", model.filename)).exists() {
                return Err(BridgeError::PartialModel);
            }
            return Ok(ResolvedModel {
                model_id: model.id.clone(),
                path: contained_file(&candidate, root)?,
                resolution: "handy-models-directory".into(),
                revision: None,
            });
        }
        Err(BridgeError::ModelFileMissing)
    }
}

fn read_ref(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > 4096 {
        return Err(BridgeError::UnsafePath);
    }
    let reference = std::fs::read_to_string(path)?.trim().to_owned();
    validate_component(&reference)?;
    Ok(Some(reference))
}

fn contained_file(candidate: &Path, root: &Path) -> Result<PathBuf> {
    let path = candidate.canonicalize()?;
    let root = root.canonicalize()?;
    if !path.starts_with(root) {
        return Err(BridgeError::UnsafePath);
    }
    // Weight files can legitimately be symlinks to the HF repo's blobs.
    if std::fs::metadata(&path)?.len() == 0 {
        return Err(BridgeError::PartialModel);
    }
    Ok(path)
}

fn validate_component(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 4096
        || value == "."
        || value == ".."
        || value.ends_with([' ', '.'])
        || value.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
        })
    {
        return Err(BridgeError::UnsafePath);
    }
    Ok(())
}
