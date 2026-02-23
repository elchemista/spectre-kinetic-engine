//! Core distillation loop: teacher token embeddings to static token table.
//!
//! Processes a corpus through the ONNX teacher model, accumulates
//! contextual token embeddings by token ID, and averages them to produce
//! a static token embedding table.

use crate::corpus::CorpusEntry;
use crate::error::TrainError;
use crate::teacher::TeacherModel;
use crate::weighting;
use std::collections::HashMap;
use tokenizers::Tokenizer;

/// Configuration for the distillation process.
pub struct DistillConfig {
    /// Maximum token sequence length (default: 256).
    pub max_len: usize,
    /// Target embedding dimension (must match teacher output or PCA target).
    pub dim: usize,
    /// Number of corpus entries to process per teacher batch.
    pub batch_size: usize,
    /// Whether to apply Zipf/SIF weighting to the output.
    pub apply_zipf: bool,
    /// SIF coefficient for Zipf weighting (default: 1e-4).
    pub sif_coefficient: f32,
}

impl Default for DistillConfig {
    fn default() -> Self {
        Self {
            max_len: 256,
            dim: 384,
            batch_size: 32,
            apply_zipf: false,
            sif_coefficient: 1e-4,
        }
    }
}

/// Result of the distillation process.
pub struct DistillResult {
    /// Flat token embedding table `[vocab_size * dim]`.
    pub token_embeddings: Vec<f32>,
    /// Vocabulary size (number of rows in the embedding table).
    pub vocab_size: usize,
    /// Embedding dimension (number of columns).
    pub dim: usize,
    /// Optional per-token weights (Zipf/SIF).
    pub weights: Option<Vec<f32>>,
}

/// Distill static token embeddings from a teacher model and corpus.
///
/// # Algorithm
/// 1. For each corpus text, tokenize and run through the teacher.
/// 2. For each output token, accumulate `(sum, count)` keyed by token ID.
/// 3. After all texts, compute `mean = sum / count` per token ID.
/// 4. Optionally compute Zipf/SIF weights.
pub fn distill(
    teacher: &TeacherModel,
    tokenizer: &Tokenizer,
    corpus: &[CorpusEntry],
    config: &DistillConfig,
) -> Result<DistillResult, TrainError> {
    let dim = teacher.dim();
    if dim != config.dim {
        return Err(TrainError::DimMismatch {
            teacher_dim: dim,
            requested_dim: config.dim,
        });
    }

    // Accumulators: token_id -> (sum_vector, count)
    let mut sums: HashMap<u32, Vec<f32>> = HashMap::new();
    let mut counts: HashMap<u32, usize> = HashMap::new();

    // Process corpus in batches
    for batch in corpus.chunks(config.batch_size) {
        let texts: Vec<String> = batch.iter().map(|e| e.text().to_owned()).collect();

        // Tokenize batch
        let encodings = tokenizer
            .encode_batch_fast::<String>(texts, true)
            .map_err(|e| TrainError::Tokenizer(format!("batch tokenization: {e}")))?;

        // Prepare padded input_ids and attention_mask
        let max_seq = encodings
            .iter()
            .map(|e| e.get_ids().len().min(config.max_len))
            .max()
            .unwrap_or(0);

        let mut input_ids: Vec<Vec<i64>> = Vec::with_capacity(encodings.len());
        let mut attention_mask: Vec<Vec<i64>> = Vec::with_capacity(encodings.len());

        for enc in &encodings {
            let ids = enc.get_ids();
            let len = ids.len().min(config.max_len);
            let mut id_vec = vec![0i64; max_seq];
            let mut mask_vec = vec![0i64; max_seq];
            for i in 0..len {
                id_vec[i] = ids[i] as i64;
                mask_vec[i] = 1;
            }
            input_ids.push(id_vec);
            attention_mask.push(mask_vec);
        }

        // Run teacher
        let token_embeddings = teacher.run_batch(&input_ids, &attention_mask)?;

        // Accumulate embeddings by token ID
        for (token_id, emb) in token_embeddings {
            let sum = sums.entry(token_id).or_insert_with(|| vec![0.0; dim]);
            for (i, &v) in emb.iter().enumerate() {
                sum[i] += v;
            }
            *counts.entry(token_id).or_insert(0) += 1;
        }
    }

    // Build static token table: mean of accumulated vectors
    let vocab_size = tokenizer.get_vocab_size(false);
    let mut embeddings = vec![0.0f32; vocab_size * dim];

    for (&token_id, sum) in &sums {
        let idx = token_id as usize;
        if idx >= vocab_size {
            continue;
        }
        let count = counts[&token_id] as f32;
        let offset = idx * dim;
        for (i, &s) in sum.iter().enumerate() {
            embeddings[offset + i] = s / count;
        }
    }

    // Optional Zipf/SIF weighting
    let weights = if config.apply_zipf {
        Some(weighting::compute_sif_weights(
            &counts,
            vocab_size,
            config.sif_coefficient,
        ))
    } else {
        None
    };

    Ok(DistillResult {
        token_embeddings: embeddings,
        vocab_size,
        dim,
        weights,
    })
}
