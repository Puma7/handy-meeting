use anyhow::Result;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub fn new_job_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[derive(Clone, Debug, Default)]
pub struct AnalysisControl {
    cancelled: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct AnalysisCancelled;
impl std::fmt::Display for AnalysisCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Auswertung abgebrochen. Bereits bestätigte Arbeitsschritte bleiben erhalten und können exakt fortgesetzt werden.")
    }
}
impl std::error::Error for AnalysisCancelled {}

impl AnalysisControl {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(AnalysisCancelled.into())
        } else {
            Ok(())
        }
    }
}
pub fn is_cancelled(error: &anyhow::Error) -> bool {
    error.downcast_ref::<AnalysisCancelled>().is_some()
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProgress {
    pub run_id: String,
    pub stage: String,
    pub step_name: Option<String>,
    /// One based during work; zero while preparing.
    pub step_index: usize,
    pub total_steps: usize,
    pub batch_index: usize,
    pub total_batches: usize,
    pub confirmed_tasks: usize,
    pub total_tasks: usize,
}
