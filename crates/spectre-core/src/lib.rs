//! Spectre Core: static embedding-based tool selection and argument binding.

pub mod al_parser;
pub mod embed;
pub mod error;
pub mod matching;
pub mod pack;
pub mod planner;
pub mod registry;
pub mod similarity;
pub mod types;

pub use embed::StaticEmbedder;
pub use error::{CoreError, PlanError};
pub use planner::SpectreDispatcher;
pub use registry::CompiledRegistry;
pub use types::{CallPlan, PlanRequest, PlanStatus};
