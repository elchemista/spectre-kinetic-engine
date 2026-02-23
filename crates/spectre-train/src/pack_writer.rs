//! Pack directory writer.
//!
//! Writes a distilled pack to a directory on disk:
//! - `pack.json` (metadata)
//! - `tokenizer.json` (copied from source)
//! - `token_embeddings.bin` (f16 binary)
//! - `weights.json` (optional)

use crate::distill::DistillResult;
use crate::error::TrainError;
use half::f16;
use spectre_core::types::PackMetadata;
use std::path::Path;

/// Write a distilled pack to the given output directory.
///
/// Creates the directory if it does not exist. Copies the tokenizer from
/// `tokenizer_path` and converts embeddings to f16 binary format.
pub fn write_pack(
    output_dir: &Path,
    metadata: &PackMetadata,
    tokenizer_path: &Path,
    result: &DistillResult,
) -> Result<(), TrainError> {
    std::fs::create_dir_all(output_dir)?;

    // Write pack.json
    let meta_json = serde_json::to_string_pretty(metadata)?;
    std::fs::write(output_dir.join("pack.json"), meta_json)?;

    // Copy tokenizer.json
    std::fs::copy(tokenizer_path, output_dir.join("tokenizer.json"))?;

    // Write token_embeddings.bin as f16 little-endian
    write_f16_embeddings(output_dir, &result.token_embeddings)?;

    // Write weights.json if present
    if let Some(weights) = &result.weights {
        let weights_json = serde_json::to_string(weights)?;
        std::fs::write(output_dir.join("weights.json"), weights_json)?;
    }

    Ok(())
}

/// Convert f32 embeddings to f16 and write as raw binary.
fn write_f16_embeddings(output_dir: &Path, embeddings: &[f32]) -> Result<(), TrainError> {
    let path = output_dir.join("token_embeddings.bin");
    let mut bytes = Vec::with_capacity(embeddings.len() * 2);

    for &val in embeddings {
        let h = f16::from_f32(val);
        bytes.extend_from_slice(&h.to_le_bytes());
    }

    std::fs::write(path, bytes)?;
    Ok(())
}
