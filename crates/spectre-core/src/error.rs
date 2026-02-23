//! Error types for spectre-core.

use thiserror::Error;

/// Infrastructure errors for loading packs, registries, and embedding operations.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Failed to load a model pack from disk.
    #[error("pack loading failed: {0}")]
    PackLoad(String),

    /// Failed to load or parse a compiled registry.
    #[error("registry loading failed: {0}")]
    RegistryLoad(String),

    /// Tokenizer initialization or encoding error.
    #[error("tokenizer error: {0}")]
    Tokenizer(String),

    /// Embedding matrix dimensions do not match expected values.
    #[error("embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension.
        expected: usize,
        /// Actual dimension found.
        actual: usize,
    },

    /// Underlying I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Domain-level plan errors that become structured fields in the response JSON.
///
/// These are not propagated via `Result`; instead they drive the `PlanStatus`
/// variant in the [`CallPlan`](crate::types::CallPlan) output.
#[derive(Debug, Error)]
pub enum PlanError {
    /// No tool matched above the confidence threshold.
    #[error("NO_TOOL: no tool matched above confidence threshold")]
    NoTool,

    /// The selected tool requires arguments that were not provided.
    #[error("MISSING_ARGS: required arguments not provided: {missing:?}")]
    MissingArgs {
        /// Names of the missing required arguments.
        missing: Vec<String>,
    },

    /// Slot-to-param mapping could not be resolved unambiguously.
    #[error("AMBIGUOUS_MAPPING: could not uniquely map slots to params: {details}")]
    AmbiguousMapping {
        /// Human-readable description of the ambiguity.
        details: String,
    },
}
