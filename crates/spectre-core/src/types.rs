//! Domain types for spectre-core.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Pack metadata (pack.json)
// ---------------------------------------------------------------------------

/// Metadata stored in `pack.json` inside a distilled model pack directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackMetadata {
    /// Identifier of the teacher model used for distillation.
    pub teacher_id: String,
    /// Embedding dimension of the static token vectors.
    pub dim: usize,
    /// Pooling strategy used at inference (always "mean" for now).
    pub pooling: String,
    /// Hash of the tokenizer.json bundled with this pack.
    pub tokenizer_hash: String,
    /// Maximum token length used during distillation.
    pub max_len: usize,
    /// Whether PCA was applied during distillation (and target dim if so).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_pca: Option<usize>,
    /// Whether Zipf/SIF weighting was applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_zipf: Option<bool>,
}

// ---------------------------------------------------------------------------
// Tool registry JSON (input from Elixir exporter)
// ---------------------------------------------------------------------------

/// Top-level tool registry as exported by the Elixir Mix task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRegistry {
    /// Schema version (currently 1).
    pub version: u32,
    /// List of tool definitions.
    pub tools: Vec<ToolDef>,
}

/// Definition of a single tool/function/API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    /// Fully-qualified tool identifier (e.g. "MyMod.create_post/2").
    pub id: String,
    /// Module name.
    pub module: String,
    /// Function name.
    pub name: String,
    /// Function arity.
    pub arity: u32,
    /// Documentation string from `@doc`.
    pub doc: String,
    /// Type specification from `@spec`.
    pub spec: String,
    /// Argument definitions with type/alias metadata.
    pub args: Vec<ArgDef>,
    /// Example AL strings for this tool.
    #[serde(default)]
    pub examples: Vec<String>,
}

/// Definition of a single function argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgDef {
    /// Canonical parameter name (e.g. "body").
    pub name: String,
    /// Elixir type string (e.g. "String.t()").
    #[serde(rename = "type")]
    pub arg_type: String,
    /// Whether this argument is required for a valid call.
    pub required: bool,
    /// Known aliases that may appear in AL slot names.
    #[serde(default)]
    pub aliases: Vec<String>,
}

// ---------------------------------------------------------------------------
// Plan request / response
// ---------------------------------------------------------------------------

/// Input request for the `plan` command, received as JSON on stdin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRequest {
    /// The Action Language text to parse and match.
    pub al: String,
    /// Slot values extracted by the caller (slot_key -> value).
    pub slots: HashMap<String, String>,
    /// Number of top candidate tools to consider.
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// Optional override for the tool selection similarity threshold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_threshold: Option<f32>,
    /// Optional override for the slot-to-param matching similarity threshold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapping_threshold: Option<f32>,
}

fn default_top_k() -> usize {
    5
}

/// Output call plan returned as JSON on stdout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallPlan {
    /// Plan outcome status.
    pub status: PlanStatus,
    /// The selected tool ID (present when status is Ok).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_tool: Option<String>,
    /// Cosine similarity confidence score for the selected tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    /// Bound arguments (param_name -> value) after slot-to-param mapping.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<HashMap<String, String>>,
    /// Names of required arguments that could not be matched.
    #[serde(default)]
    pub missing: Vec<String>,
    /// Informational notes about the plan.
    #[serde(default)]
    pub notes: Vec<String>,
    /// Tool selection similarity threshold that was applied (useful for tuning).
    pub active_tool_threshold: f32,
    /// Slot mapping similarity threshold that was applied (useful for tuning).
    pub active_mapping_threshold: f32,
    /// All evaluated tool candidates and their similarity scores (sorted descending).
    #[serde(default)]
    pub candidates: Vec<CandidateTool>,
}

/// A tool candidate evaluated during the tool selection phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateTool {
    /// Fully-qualified tool ID.
    pub id: String,
    /// Cosine similarity score.
    pub score: f32,
}

/// Outcome status of a plan request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlanStatus {
    /// Tool was selected and arguments were successfully bound.
    #[serde(rename = "ok")]
    Ok,
    /// No tool matched above the confidence threshold.
    NoTool,
    /// Tool was selected but required arguments are missing.
    MissingArgs,
    /// Slot-to-param mapping was ambiguous.
    AmbiguousMapping,
}

// ---------------------------------------------------------------------------
// Compiled registry structures (for .mcr binary file)
// ---------------------------------------------------------------------------

/// Header of the `.mcr` compiled registry binary file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McrHeader {
    /// Magic bytes identifying the file format.
    pub magic: [u8; 4],
    /// File format version.
    pub version: u32,
    /// Embedding dimension.
    pub dims: usize,
    /// Hash of the tokenizer used to build the embeddings.
    pub tokenizer_hash: String,
    /// Number of tools in the registry.
    pub tool_count: usize,
    /// Total number of param cards across all tools.
    pub param_count: usize,
    /// Number of precomputed canonical slot card embeddings.
    pub slot_card_count: usize,
}

/// Metadata for a single tool stored inside the compiled registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMeta {
    /// Fully-qualified tool ID.
    pub id: String,
    /// Module name.
    pub module: String,
    /// Function name.
    pub name: String,
    /// Function arity.
    pub arity: u32,
    /// Argument definitions.
    pub args: Vec<ArgDef>,
    /// Index range `[start, end)` into the param embedding matrix.
    pub param_range: (usize, usize),
}

// ---------------------------------------------------------------------------
// AL parser types
// ---------------------------------------------------------------------------

/// A slot key extracted from an AL text string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSlot {
    /// The slot key name (lowercased).
    pub key: String,
    /// Whether this slot came from a `{placeholder}` syntax (vs `KEY=value`).
    pub placeholder: bool,
}
