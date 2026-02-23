//! Static token embedding engine (Model2Vec-style inference).
//!
//! Follows the Model2Vec pattern: tokenize -> lookup static token vectors -> mean-pool.
//! Adapted from `model2vec-rs` for the Spectre pack format.

use crate::error::CoreError;
use ndarray::Array2;
use serde_json::Value;
use tokenizers::Tokenizer;

/// Static token embedding model for Spectre Dispatcher.
///
/// Loads a distilled pack (tokenizer + static token vectors) and produces
/// fixed-dimension embeddings for arbitrary text via mean pooling.
pub struct StaticEmbedder {
    tokenizer: Tokenizer,
    embeddings: Array2<f32>,
    weights: Option<Vec<f32>>,
    normalize: bool,
    dim: usize,
    max_len: usize,
    median_token_length: usize,
    unk_token_id: Option<usize>,
}

impl StaticEmbedder {
    /// Construct from owned data.
    ///
    /// The caller is responsible for decoding the raw token embeddings
    /// into `f32` and providing the tokenizer. See [`crate::pack::load_pack`]
    /// for the standard loading path.
    pub fn new(
        tokenizer: Tokenizer,
        embeddings: Vec<f32>,
        vocab_size: usize,
        dim: usize,
        max_len: usize,
        normalize: bool,
        weights: Option<Vec<f32>>,
    ) -> Result<Self, CoreError> {
        if embeddings.len() != vocab_size * dim {
            return Err(CoreError::DimensionMismatch {
                expected: vocab_size * dim,
                actual: embeddings.len(),
            });
        }

        let (median_token_length, unk_token_id) = Self::compute_metadata(&tokenizer)?;

        let embeddings = Array2::from_shape_vec((vocab_size, dim), embeddings)
            .map_err(|e| CoreError::PackLoad(format!("failed to build embeddings array: {e}")))?;

        Ok(Self {
            tokenizer,
            embeddings,
            weights,
            normalize,
            dim,
            max_len,
            median_token_length,
            unk_token_id,
        })
    }

    /// Embed a single text string into a fixed-dimension vector.
    pub fn encode_single(&self, text: &str) -> Vec<f32> {
        self.encode_batch(&[text])
            .into_iter()
            .next()
            .unwrap_or_else(|| vec![0.0; self.dim])
    }

    /// Embed a batch of text strings into vectors.
    pub fn encode_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        let mut result = Vec::with_capacity(texts.len());

        let truncated: Vec<String> = texts
            .iter()
            .map(|t| Self::truncate_str(t, self.max_len, self.median_token_length).into())
            .collect();

        let encodings = self
            .tokenizer
            .encode_batch_fast::<String>(truncated, false)
            .unwrap_or_default();

        for encoding in encodings {
            let mut ids = encoding.get_ids().to_vec();
            if let Some(unk_id) = self.unk_token_id {
                ids.retain(|&id| id as usize != unk_id);
            }
            ids.truncate(self.max_len);
            result.push(self.pool_ids(&ids));
        }

        result
    }

    /// Return the embedding dimension.
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Mean-pool token IDs into a single embedding vector.
    fn pool_ids(&self, ids: &[u32]) -> Vec<f32> {
        let mut sum = vec![0.0f32; self.dim];
        let mut count = 0usize;

        for &id in ids {
            let row_idx = id as usize;
            if row_idx >= self.embeddings.nrows() {
                continue;
            }

            let scale = self
                .weights
                .as_ref()
                .and_then(|w| w.get(row_idx))
                .copied()
                .unwrap_or(1.0);

            let row = self.embeddings.row(row_idx);
            for (i, &v) in row.iter().enumerate() {
                sum[i] += v * scale;
            }
            count += 1;
        }

        let denom = count.max(1) as f32;
        for x in &mut sum {
            *x /= denom;
        }

        if self.normalize {
            l2_normalize(&mut sum);
        }

        sum
    }

    /// Compute median token length and optional unk_token_id from the tokenizer.
    fn compute_metadata(tokenizer: &Tokenizer) -> Result<(usize, Option<usize>), CoreError> {
        let mut lens: Vec<usize> = tokenizer.get_vocab(false).keys().map(|tk| tk.len()).collect();
        lens.sort_unstable();
        let median_token_length = lens.get(lens.len() / 2).copied().unwrap_or(1);

        let spec_json = tokenizer
            .to_string(false)
            .map_err(|e| CoreError::Tokenizer(format!("tokenizer serialization failed: {e}")))?;
        let spec: Value =
            serde_json::from_str(&spec_json).map_err(|e| CoreError::Tokenizer(format!("tokenizer JSON parse: {e}")))?;

        let unk_token_id = spec
            .get("model")
            .and_then(|m| m.get("unk_token"))
            .and_then(Value::as_str)
            .and_then(|tok| tokenizer.token_to_id(tok))
            .map(|id| id as usize);

        Ok((median_token_length, unk_token_id))
    }

    /// Character-level pre-truncation heuristic.
    fn truncate_str(s: &str, max_tokens: usize, median_len: usize) -> &str {
        let max_chars = max_tokens.saturating_mul(median_len);
        match s.char_indices().nth(max_chars) {
            Some((byte_idx, _)) => &s[..byte_idx],
            None => s,
        }
    }
}

/// L2-normalize a vector in place, guarding against zero-length.
pub(crate) fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|&x| x * x).sum::<f32>().sqrt().max(1e-12);
    for x in v.iter_mut() {
        *x /= norm;
    }
}
