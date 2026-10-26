//! Thin orchestration around Handy's maintained safe Rust bindings. No local
//! FFI declarations, model parser or recognition algorithm are implemented.
use crate::{BridgeError, Result};
use std::path::Path;
use transcribe_cpp::{
    Backend, BackendMask, Model, ModelOptions, RunOptions, Stream, StreamOptions,
};

pub use transcribe_cpp::{Capabilities, StreamText, StreamUpdate};

pub use crate::RuntimeIdentity;

pub fn runtime_identity() -> RuntimeIdentity {
    RuntimeIdentity {
        compiled_version: transcribe_cpp::compiled_version(),
        loaded_version: transcribe_cpp::version(),
        loaded_commit: transcribe_cpp::version_commit(),
        public_header_hash: transcribe_cpp::header_hash().into(),
    }
}

fn verify_runtime() -> Result<()> {
    let identity = runtime_identity();
    let base = identity
        .loaded_version
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .next()
        .unwrap_or("")
        .trim_end_matches('.');
    if base != identity.compiled_version {
        return Err(BridgeError::Native("The packaged recognition runtime version does not match its Rust bindings. Use a complete matching HandyMeeting installation.".into()));
    }
    Ok(())
}

#[derive(Debug)]
pub struct NativeEngine {
    model: Model,
    capabilities: Capabilities,
}

impl NativeEngine {
    pub fn load(path: &Path, cpu_only: bool, backend_directory: Option<&Path>) -> Result<Self> {
        verify_runtime()?;
        // CPU replacement workers never register GPU modules; a broken GPU
        // driver must not be loaded merely to request Backend::Cpu afterward.
        transcribe_cpp::init_backends_with(
            backend_directory,
            if cpu_only {
                BackendMask::CPU
            } else {
                BackendMask::ALL
            },
        )
        .map_err(native_error)?;
        let model = Model::load_with(
            path,
            &ModelOptions {
                backend: if cpu_only {
                    Backend::Cpu
                } else {
                    Backend::Auto
                },
                ..Default::default()
            },
        )
        .map_err(native_error)?;
        let capabilities = model.capabilities().map_err(native_error)?;
        if !capabilities.supports_streaming || capabilities.native_sample_rate != 16_000 {
            return Err(BridgeError::ModelUnavailable);
        }
        Ok(Self {
            model,
            capabilities,
        })
    }

    pub fn backend(&self) -> String {
        self.model.backend()
    }
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Keeps the model loaded across bounded recognition windows. An active
    /// upstream Stream exclusively leases this model. Two concurrent tracks
    /// therefore use separate worker/model instances, not a cloned Model.
    pub fn with_stream<T>(
        &self,
        run: &RunOptions,
        operation: impl FnOnce(&mut Stream<'_>) -> Result<T>,
    ) -> Result<T> {
        let mut session = self.model.session().map_err(native_error)?;
        let mut stream = session
            .stream(run, &StreamOptions::default())
            .map_err(native_error)?;
        operation(&mut stream)
    }
}

pub fn native_error(error: transcribe_cpp::Error) -> BridgeError {
    // Do not include the load error's path or potentially sensitive input in
    // persisted/public logs. Callers can present the selected model separately.
    let _ = error;
    BridgeError::Native("The installed model or selected compute backend is not supported by this runtime. Audio capture is independent and can continue.".into())
}

pub fn default_run_options() -> RunOptions {
    RunOptions::default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn packaged_runtime_matches_the_pinned_safe_bindings() {
        let identity = super::runtime_identity();
        assert_eq!(identity.compiled_version, "0.4.1");
        super::verify_runtime().unwrap();
        assert!(!identity.public_header_hash.is_empty());
    }
}
