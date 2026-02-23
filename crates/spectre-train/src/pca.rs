//! Optional PCA dimension reduction.
//!
//! Stub for v1: returns the input unchanged. A real PCA implementation
//! (power iteration or full eigendecomposition) can be added later.

use ndarray::Array2;

/// Reduce the dimensionality of embeddings using PCA.
///
/// Currently a pass-through stub. When `target_dim == input dim`,
/// this is a no-op. Future versions will implement actual PCA projection.
pub fn reduce_dimensions(embeddings: &Array2<f32>, target_dim: usize) -> Array2<f32> {
    let current_dim = embeddings.ncols();
    if target_dim >= current_dim {
        return embeddings.clone();
    }

    // Stub: just truncate columns (not true PCA, but preserves dimensions)
    embeddings.slice(ndarray::s![.., ..target_dim]).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduce_dimensions_should_passthrough_when_target_equals_source() {
        let emb = Array2::from_shape_vec((3, 4), (0..12).map(|x| x as f32).collect()).unwrap();
        let result = reduce_dimensions(&emb, 4);
        assert_eq!(result.shape(), &[3, 4]);
    }

    #[test]
    fn reduce_dimensions_should_truncate_when_target_smaller() {
        let emb = Array2::from_shape_vec((3, 4), (0..12).map(|x| x as f32).collect()).unwrap();
        let result = reduce_dimensions(&emb, 2);
        assert_eq!(result.shape(), &[3, 2]);
    }
}
