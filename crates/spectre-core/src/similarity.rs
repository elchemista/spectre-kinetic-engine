//! Cosine similarity and top-k retrieval.
//!
//! When both the query and matrix rows are L2-normalized,
//! cosine similarity reduces to the dot product.

use ndarray::ArrayView2;

/// Compute cosine similarity between a query vector and each row of a matrix.
///
/// Both `query` and rows of `matrix` are assumed to be L2-normalized,
/// so similarity = dot product.
///
/// Returns one similarity score per matrix row, in row order.
pub fn cosine_similarities(query: &[f32], matrix: ArrayView2<f32>) -> Vec<f32> {
    let dim = query.len();
    matrix
        .rows()
        .into_iter()
        .map(|row| {
            debug_assert_eq!(row.len(), dim);
            row.iter().zip(query.iter()).map(|(&a, &b)| a * b).sum()
        })
        .collect()
}

/// Return the top-k indices sorted by descending similarity, filtered by threshold.
///
/// Returns pairs of `(index, similarity_score)`.
pub fn top_k_above_threshold(similarities: &[f32], k: usize, threshold: f32) -> Vec<(usize, f32)> {
    let mut indexed: Vec<(usize, f32)> = similarities
        .iter()
        .enumerate()
        .filter(|(_, &sim)| sim >= threshold)
        .map(|(i, &sim)| (i, sim))
        .collect();

    // Sort descending by similarity
    indexed.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    indexed.truncate(k);
    indexed
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[test]
    fn cosine_similarities_should_return_correct_scores() {
        // 3 rows, dim=2, all unit vectors
        let matrix = Array2::from_shape_vec(
            (3, 2),
            vec![
                1.0,
                0.0, // row 0: pointing right
                0.0,
                1.0, // row 1: pointing up
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2, // row 2: 45 degrees
            ],
        )
        .unwrap();

        let query = vec![1.0, 0.0]; // pointing right
        let sims = cosine_similarities(&query, matrix.view());

        assert!((sims[0] - 1.0).abs() < 1e-5); // identical
        assert!((sims[1] - 0.0).abs() < 1e-5); // orthogonal
        assert!((sims[2] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3); // 45 degrees
    }

    #[test]
    fn top_k_should_filter_by_threshold() {
        let sims = vec![0.9, 0.1, 0.5, 0.8, 0.2];
        let result = top_k_above_threshold(&sims, 3, 0.4);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, 0); // highest: 0.9
        assert_eq!(result[1].0, 3); // second: 0.8
        assert_eq!(result[2].0, 2); // third: 0.5
    }

    #[test]
    fn top_k_should_return_empty_when_all_below_threshold() {
        let sims = vec![0.1, 0.2, 0.3];
        let result = top_k_above_threshold(&sims, 5, 0.5);
        assert!(result.is_empty());
    }

    #[test]
    fn top_k_should_respect_k_limit() {
        let sims = vec![0.9, 0.8, 0.7, 0.6, 0.5];
        let result = top_k_above_threshold(&sims, 2, 0.0);
        assert_eq!(result.len(), 2);
    }
}
