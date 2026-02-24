//! Pack loading from a directory on disk.
//!
//! A pack contains:
//! - `pack.json` (metadata)
//! - `tokenizer.json` (HuggingFace tokenizer)
//! - `token_embeddings.bin` (raw little-endian f16 vectors)
//! - `weights.json` (optional per-token weights)

use crate::embed::StaticEmbedder;
use crate::error::CoreError;
use crate::types::PackMetadata;
use half::f16;
use std::path::Path;
use tokenizers::Tokenizer;

/// Load a distilled model pack from a directory.
///
/// Returns the parsed metadata and a ready-to-use [`StaticEmbedder`].
pub fn load_pack(pack_dir: &Path) -> Result<(PackMetadata, StaticEmbedder), CoreError> {
    let meta = load_metadata(pack_dir)?;
    let tokenizer = load_tokenizer(pack_dir)?;
    let embeddings = load_embeddings(pack_dir, &meta)?;
    let weights = load_weights(pack_dir)?;

    let vocab_size = embeddings.len() / meta.dim;
    let embedder = StaticEmbedder::new(tokenizer, embeddings, vocab_size, meta.dim, meta.max_len, true, weights)?;

    Ok((meta, embedder))
}

/// Read and parse `pack.json`.
fn load_metadata(pack_dir: &Path) -> Result<PackMetadata, CoreError> {
    let path = pack_dir.join("pack.json");
    let data = std::fs::read_to_string(&path)
        .map_err(|e| CoreError::PackLoad(format!("cannot read {}: {e}", path.display())))?;
    let meta: PackMetadata = serde_json::from_str(&data)?;
    Ok(meta)
}

/// Load the HuggingFace tokenizer from `tokenizer.json`.
fn load_tokenizer(pack_dir: &Path) -> Result<Tokenizer, CoreError> {
    let path = pack_dir.join("tokenizer.json");
    Tokenizer::from_file(&path).map_err(|e| CoreError::Tokenizer(format!("cannot load {}: {e}", path.display())))
}

/// Read `token_embeddings.bin` as raw little-endian f16 and convert to f32.
fn load_embeddings(pack_dir: &Path, meta: &PackMetadata) -> Result<Vec<f32>, CoreError> {
    let path = pack_dir.join("token_embeddings.bin");
    let raw = std::fs::read(&path).map_err(|e| CoreError::PackLoad(format!("cannot read {}: {e}", path.display())))?;

    if raw.len() % 2 != 0 {
        return Err(CoreError::PackLoad(format!(
            "token_embeddings.bin has odd byte count ({})",
            raw.len()
        )));
    }

    let floats: Vec<f32> = raw
        .chunks_exact(2)
        .map(|b| {
            let bytes: [u8; 2] = [b[0], b[1]];
            f16::from_le_bytes(bytes).to_f32()
        })
        .collect();

    if meta.dim > 0 && !floats.len().is_multiple_of(meta.dim) {
        return Err(CoreError::DimensionMismatch {
            expected: meta.dim,
            actual: floats.len(),
        });
    }

    Ok(floats)
}

/// Optionally load per-token weights from `weights.json`.
fn load_weights(pack_dir: &Path) -> Result<Option<Vec<f32>>, CoreError> {
    let path = pack_dir.join("weights.json");
    if !path.exists() {
        return Ok(None);
    }
    let data = std::fs::read_to_string(&path)
        .map_err(|e| CoreError::PackLoad(format!("cannot read {}: {e}", path.display())))?;
    let weights: Vec<f32> = serde_json::from_str(&data)?;
    Ok(Some(weights))
}
