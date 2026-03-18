/// Unified error type for the math-sonify engine and plugin.
///
/// All fallible public APIs return `Result<T, SonifyError>`. The audio thread
/// and plugin `process()` callback must never propagate a panic; callers should
/// log the error and return silence rather than unwrapping.
///
/// # Example
///
/// ```rust,ignore
/// use crate::error::SonifyError;
///
/// fn open_device() -> Result<(), SonifyError> {
///     let host = cpal::default_host();
///     host.default_output_device()
///         .ok_or_else(|| SonifyError::AudioDeviceError("no output device found".into()))
/// }
/// ```
use thiserror::Error;

/// All errors that can be produced by the sonification engine.
#[derive(Debug, Error)]
pub enum SonifyError {
    /// The audio host could not open or configure a device.
    ///
    /// Typically caused by the device being in use by another application, an
    /// unsupported sample-rate, or a missing audio driver.
    #[error("audio device error: {0}")]
    AudioDeviceError(String),

    /// The ODE integrator produced a non-finite (NaN or infinite) state.
    ///
    /// This can occur when parameters push the system into a degenerate regime
    /// (e.g., three-body bodies coinciding, Lorenz sigma=0). The engine recovers
    /// by resetting state to the default initial condition.
    #[error("ODE integration error: {0}")]
    OdeIntegrationError(String),

    /// A configuration field could not be parsed or is outside its valid range.
    ///
    /// The engine falls back to `Config::default()` when this error is raised
    /// during config load so that audio is never interrupted by a bad config file.
    #[error("config error: {0}")]
    ConfigError(String),

    /// A VST3/CLAP plugin operation failed.
    ///
    /// Returned from plugin lifecycle methods (`initialize`, `reset`) when the
    /// DSP state cannot be set up correctly. The host should deactivate the
    /// plugin rather than propagating the error into the audio thread.
    #[error("plugin error: {0}")]
    PluginError(String),

    /// Audio rendering failed (e.g., a WAV writer could not be flushed, a
    /// buffer could not be allocated for offline export).
    ///
    /// This error is never raised on the real-time audio thread.
    #[error("render error: {0}")]
    RenderError(String),

    /// A filesystem or I/O operation failed (config read, WAV export, session
    /// log write, PNG phase-portrait export).
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Convenience alias used throughout the codebase.
pub type SonifyResult<T> = Result<T, SonifyError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_variants_display_non_empty() {
        // Every error variant must produce a non-empty Display string.
        let variants: &[SonifyError] = &[
            SonifyError::AudioDeviceError("test".into()),
            SonifyError::OdeIntegrationError("test".into()),
            SonifyError::ConfigError("test".into()),
            SonifyError::PluginError("test".into()),
            SonifyError::RenderError("test".into()),
        ];
        for v in variants {
            let s = v.to_string();
            assert!(!s.is_empty(), "Error variant {:?} produced an empty Display string", v);
        }
    }

    #[test]
    fn test_io_error_from_impl() {
        // The From<std::io::Error> impl must produce an IoError variant.
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let sonify_err: SonifyError = io_err.into();
        match sonify_err {
            SonifyError::IoError(_) => {}
            other => panic!("Expected IoError, got {:?}", other),
        }
    }

    #[test]
    fn test_error_display_contains_message() {
        let err = SonifyError::AudioDeviceError("no output device".into());
        assert!(err.to_string().contains("no output device"),
            "Display should contain the original message, got: {}", err);
    }

    #[test]
    fn test_config_error_display() {
        let err = SonifyError::ConfigError("invalid sigma".into());
        assert!(err.to_string().contains("invalid sigma"),
            "ConfigError display should contain 'invalid sigma', got: {}", err);
    }
}
