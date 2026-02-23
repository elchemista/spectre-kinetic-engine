//! ONNX teacher model wrapper.
//!
//! Wraps an ONNX teacher model (e.g. all-MiniLM-L6-v2) and runs inference
//! to produce token-level contextual embeddings for distillation.

use crate::error::TrainError;
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;

/// Wrapper around an ONNX teacher model for producing contextual embeddings.
pub struct TeacherModel {
    session: Session,
    dim: usize,
}

impl TeacherModel {
    /// Load an ONNX model from a file path.
    pub fn load(onnx_path: &Path) -> Result<Self, TrainError> {
        let session = Session::builder()
            .map_err(|e| TrainError::Onnx(format!("session builder: {e}")))?
            .commit_from_file(onnx_path)
            .map_err(|e| TrainError::Onnx(format!("load model {}: {e}", onnx_path.display())))?;

        // Infer dimension from output shape metadata (default to 384 for MiniLM)
        let dim = session
            .outputs
            .first()
            .and_then(|o| o.output_type.tensor_dimensions())
            .and_then(|dims| dims.last().copied())
            .unwrap_or(384) as usize;

        Ok(Self { session, dim })
    }

    /// Return the embedding dimension of the teacher model.
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Run the teacher on a batch of token sequences and return token-level embeddings.
    ///
    /// # Arguments
    /// * `input_ids` - Token IDs for each sequence, shape `[batch, seq_len]`.
    /// * `attention_mask` - Attention masks, shape `[batch, seq_len]`.
    ///
    /// # Returns
    /// A vector of `(token_id, embedding)` pairs extracted from the last hidden state.
    /// Only tokens where `attention_mask == 1` are included.
    pub fn run_batch(
        &self,
        input_ids: &[Vec<i64>],
        attention_mask: &[Vec<i64>],
    ) -> Result<Vec<(u32, Vec<f32>)>, TrainError> {
        let batch_size = input_ids.len();
        if batch_size == 0 {
            return Ok(Vec::new());
        }

        let seq_len = input_ids[0].len();

        let ids_tensor = self.build_tensor(input_ids, batch_size, seq_len, "input_ids")?;
        let mask_tensor = self.build_tensor(attention_mask, batch_size, seq_len, "attention_mask")?;

        let type_ids_flat: Vec<i64> = vec![0i64; batch_size * seq_len];
        let type_tensor = Tensor::from_array(([batch_size, seq_len], type_ids_flat))
            .map_err(|e| TrainError::Onnx(format!("token_type_ids tensor: {e}")))?;

        let inputs = ort::inputs![
            "input_ids" => ids_tensor,
            "attention_mask" => mask_tensor,
            "token_type_ids" => type_tensor,
        ]
        .map_err(|e| TrainError::Onnx(format!("building inputs: {e}")))?;

        let outputs = self
            .session
            .run(inputs)
            .map_err(|e| TrainError::Onnx(format!("ONNX run: {e}")))?;

        // Extract last_hidden_state [batch, seq_len, dim]
        let hidden = outputs
            .iter()
            .next()
            .ok_or_else(|| TrainError::Onnx("no output tensor found".into()))?;

        let (_, value_ref) = hidden;
        let tensor = value_ref
            .try_extract_raw_tensor::<f32>()
            .map_err(|e| TrainError::Onnx(format!("extract tensor: {e}")))?;
        let data = tensor.1;

        self.collect_token_embeddings(data, input_ids, attention_mask, batch_size, seq_len)
    }

    /// Build an i64 tensor from a slice of sequences.
    fn build_tensor(
        &self,
        seqs: &[Vec<i64>],
        batch_size: usize,
        seq_len: usize,
        name: &str,
    ) -> Result<Tensor<i64>, TrainError> {
        let flat: Vec<i64> = seqs.iter().flat_map(|s| s.iter().copied()).collect();
        Tensor::from_array(([batch_size, seq_len], flat))
            .map_err(|e| TrainError::Onnx(format!("{name} tensor: {e}")))
    }

    /// Collect (token_id, embedding) pairs from the raw output data.
    fn collect_token_embeddings(
        &self,
        data: &[f32],
        input_ids: &[Vec<i64>],
        attention_mask: &[Vec<i64>],
        batch_size: usize,
        seq_len: usize,
    ) -> Result<Vec<(u32, Vec<f32>)>, TrainError> {
        let mut results = Vec::new();
        for b in 0..batch_size {
            for s in 0..seq_len {
                if attention_mask[b][s] == 1 {
                    let token_id = input_ids[b][s] as u32;
                    let offset = (b * seq_len + s) * self.dim;
                    let emb = data[offset..offset + self.dim].to_vec();
                    results.push((token_id, emb));
                }
            }
        }
        Ok(results)
    }
}
