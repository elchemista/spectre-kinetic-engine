//! Zipf/SIF token weighting.
//!
//! Tokens that appear more frequently get lower weight, following the SIF formula:
//! `weight = a / (a + p(token))` where `a` is a small constant and `p(token)` is
//! the empirical token frequency.

use std::collections::HashMap;

/// Compute SIF (Smooth Inverse Frequency) weights for all tokens in the vocabulary.
///
/// # Arguments
/// * `token_counts` - Observed counts of each token ID in the corpus.
/// * `vocab_size` - Total vocabulary size (weights vector length).
/// * `sif_coefficient` - The `a` parameter in `a / (a + p(token))`. Typical: 1e-4.
///
/// # Returns
/// A `Vec<f32>` of length `vocab_size` with per-token weights.
/// Tokens not seen in the corpus get a weight of 1.0.
pub fn compute_sif_weights(token_counts: &HashMap<u32, usize>, vocab_size: usize, sif_coefficient: f32) -> Vec<f32> {
    let total: f64 = token_counts.values().map(|&c| c as f64).sum();
    let a = sif_coefficient as f64;

    let mut weights = vec![1.0f32; vocab_size];

    if total == 0.0 {
        return weights;
    }

    for (&token_id, &count) in token_counts {
        let idx = token_id as usize;
        if idx >= vocab_size {
            continue;
        }
        let p = count as f64 / total;
        weights[idx] = (a / (a + p)) as f32;
    }

    weights
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sif_weights_should_downweight_frequent_tokens() {
        let mut counts = HashMap::new();
        counts.insert(0, 1000); // very frequent
        counts.insert(1, 10); // rare
        counts.insert(2, 1); // very rare

        let weights = compute_sif_weights(&counts, 4, 1e-4);

        // Frequent tokens should have lower weight
        assert!(weights[0] < weights[1]);
        assert!(weights[1] < weights[2]);
        // Unseen token gets weight 1.0
        assert!((weights[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn sif_weights_should_handle_empty_counts() {
        let counts = HashMap::new();
        let weights = compute_sif_weights(&counts, 5, 1e-4);
        assert!(weights.iter().all(|&w| (w - 1.0).abs() < 1e-6));
    }
}
