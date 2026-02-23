//! Error types for spectre-train.

use thiserror::Error;

/// Errors that can occur during the training/distillation pipeline.
#[derive(Debug, Error)]
pub enum TrainError {
    /// ONNX Runtime initialization or inference error.
    #[error("ONNX runtime error: {0}")]
    Onnx(String),

    /// Error while parsing the corpus JSONL file.
    #[error("corpus parsing error at line {line}: {message}")]
    Corpus {
        /// 1-based line number where the error occurred.
        line: usize,
        /// Description of the parsing failure.
        message: String,
    },

    /// Tokenizer loading or encoding error.
    #[error("tokenizer error: {0}")]
    Tokenizer(String),

    /// Teacher model output dimension does not match the requested dimension.
    #[error("dimension mismatch: teacher output {teacher_dim} vs requested {requested_dim}")]
    DimMismatch {
        /// Dimension from the teacher model.
        teacher_dim: usize,
        /// Dimension requested in the config.
        requested_dim: usize,
    },

    /// Underlying I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
