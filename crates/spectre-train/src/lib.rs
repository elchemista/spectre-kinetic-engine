//! Spectre Train: ONNX teacher distillation to static token embeddings.

pub mod corpus;
pub mod distill;
pub mod error;
pub mod pack_writer;
pub mod pca;
pub mod teacher;
pub mod weighting;

pub use corpus::{parse_corpus, CorpusEntry};
pub use distill::{distill, DistillConfig, DistillResult};
pub use error::TrainError;
pub use pack_writer::write_pack;
pub use teacher::TeacherModel;
