//! Plan orchestrator: tool selection + argument binding.
//!
//! Wires together AL parsing, embedding, tool retrieval, and slot-to-param matching
//! into a single `plan()` call that returns a structured [`CallPlan`].

use crate::al_parser;
use crate::embed::StaticEmbedder;
use crate::matching::{self, SlotAssignment};
use crate::registry::CompiledRegistry;
use crate::similarity;
use crate::types::{CallPlan, PlanRequest, PlanStatus};
use ndarray::Array2;
use std::collections::HashMap;

/// Top-level API for Spectre Dispatcher.
///
/// Holds a loaded model pack (embedder) and compiled registry, and exposes
/// the `plan()` method for processing AL requests.
pub struct SpectreDispatcher {
    embedder: StaticEmbedder,
    registry: CompiledRegistry,
    /// Minimum cosine similarity for tool selection (default 0.3).
    tool_threshold: f32,
    /// Minimum cosine similarity for slot-to-param matching (default 0.4).
    mapping_threshold: f32,
}

impl SpectreDispatcher {
    /// Create a new dispatcher from a loaded embedder and compiled registry.
    pub fn new(embedder: StaticEmbedder, registry: CompiledRegistry) -> Self {
        Self {
            embedder,
            registry,
            tool_threshold: 0.3,
            mapping_threshold: 0.35,
        }
    }

    /// Override the tool selection confidence threshold.
    pub fn with_tool_threshold(mut self, threshold: f32) -> Self {
        self.tool_threshold = threshold;
        self
    }

    /// Override the slot-to-param mapping similarity threshold.
    pub fn with_mapping_threshold(mut self, threshold: f32) -> Self {
        self.mapping_threshold = threshold;
        self
    }

    /// Execute a plan request: parse AL, select tool, bind arguments, return result.
    pub fn plan(&self, request: &PlanRequest) -> CallPlan {
        let active_tool_threshold = request.tool_threshold.unwrap_or(self.tool_threshold);
        let active_mapping_threshold = request.mapping_threshold.unwrap_or(self.mapping_threshold);

        // 1. Parse AL text
        let parsed = al_parser::parse_al(&request.al);

        // 2. Embed the action text
        let query_vec = self.embedder.encode_single(&parsed.action_text);

        // 3. Tool selection via cosine similarity
        let sims = similarity::cosine_similarities(&query_vec, self.registry.tool_embeddings.view());
        let candidates = similarity::top_k_above_threshold(&sims, request.top_k, active_tool_threshold);

        // Map top-k candidates to CandidateTool struct
        let eval_candidates: Vec<crate::types::CandidateTool> = candidates
            .iter()
            .map(|&(idx, score)| crate::types::CandidateTool {
                id: self.registry.tools[idx].id.clone(),
                score,
            })
            .collect();

        if candidates.is_empty() {
            return CallPlan {
                status: PlanStatus::NoTool,
                selected_tool: None,
                confidence: None,
                args: None,
                missing: Vec::new(),
                notes: vec!["no tool matched above confidence threshold".into()],
                active_tool_threshold,
                active_mapping_threshold,
                candidates: eval_candidates,
            };
        }

        // 4. Take the best candidate
        let (best_idx, confidence) = candidates[0];
        let tool = &self.registry.tools[best_idx];

        // 5. Slot-to-param matching
        if parsed.slot_keys.is_empty() || tool.args.is_empty() {
            return self.build_result_no_slots(
                tool, confidence, &request.slots, active_tool_threshold, active_mapping_threshold, eval_candidates
            );
        }

        let assignment = self.match_slots_to_params(tool, &parsed, &request.slots, active_mapping_threshold);
        self.build_result(
            tool, confidence, assignment, &request.slots, active_tool_threshold, active_mapping_threshold, eval_candidates
        )
    }

    /// Match slot keys to tool params using embedding similarity.
    fn match_slots_to_params(
        &self,
        tool: &crate::types::ToolMeta,
        parsed: &al_parser::AlParsed,
        _slots: &HashMap<String, String>,
        threshold: f32,
    ) -> Result<SlotAssignment, crate::error::PlanError> {
        let (param_start, param_end) = tool.param_range;

        // Embed slot-card texts
        let slot_cards: Vec<String> = parsed.slot_keys.iter().map(|s| format!("SLOT {}", s.key)).collect();
        let slot_refs: Vec<&str> = slot_cards.iter().map(|s| s.as_str()).collect();
        let slot_vecs = self.embedder.encode_batch(&slot_refs);

        // Extract precomputed param embeddings for this tool
        let param_slice = self.registry.param_embeddings.slice(ndarray::s![param_start..param_end, ..]);
        let param_vecs: Vec<Vec<f32>> = param_slice.rows().into_iter().map(|r| r.to_vec()).collect();

        // Build similarity matrix [num_slots x num_params]
        let num_slots = slot_vecs.len();
        let num_params = param_vecs.len();
        let mut sim_data = Vec::with_capacity(num_slots * num_params);

        for sv in &slot_vecs {
            for pv in &param_vecs {
                sim_data.push(dot_product(sv, pv));
            }
        }

        let sim_matrix = Array2::from_shape_vec((num_slots, num_params), sim_data)
            .unwrap_or_else(|_| Array2::zeros((num_slots, num_params)));

        matching::assign_slots_to_params(&sim_matrix, &parsed.slot_keys, &tool.args, threshold)
    }

    /// Build CallPlan when there are no slots to match (just return the tool).
    fn build_result_no_slots(
        &self,
        tool: &crate::types::ToolMeta,
        confidence: f32,
        slots: &HashMap<String, String>,
        active_tool_threshold: f32,
        active_mapping_threshold: f32,
        candidates: Vec<crate::types::CandidateTool>,
    ) -> CallPlan {
        // Check if required args are missing
        let missing: Vec<String> = tool
            .args
            .iter()
            .filter(|a| a.required)
            .filter(|a| !slots.contains_key(&a.name))
            .map(|a| a.name.clone())
            .collect();

        if !missing.is_empty() {
            return CallPlan {
                status: PlanStatus::MissingArgs,
                selected_tool: Some(tool.id.clone()),
                confidence: Some(confidence),
                args: None,
                missing,
                notes: Vec::new(),
                active_tool_threshold,
                active_mapping_threshold,
                candidates,
            };
        }

        CallPlan {
            status: PlanStatus::Ok,
            selected_tool: Some(tool.id.clone()),
            confidence: Some(confidence),
            args: Some(slots.clone()),
            missing: Vec::new(),
            notes: Vec::new(),
            active_tool_threshold,
            active_mapping_threshold,
            candidates,
        }
    }

    /// Build CallPlan from a slot assignment result.
    fn build_result(
        &self,
        tool: &crate::types::ToolMeta,
        confidence: f32,
        assignment: Result<SlotAssignment, crate::error::PlanError>,
        slots: &HashMap<String, String>,
        active_tool_threshold: f32,
        active_mapping_threshold: f32,
        candidates: Vec<crate::types::CandidateTool>,
    ) -> CallPlan {
        match assignment {
            Err(crate::error::PlanError::MissingArgs { missing }) => CallPlan {
                status: PlanStatus::MissingArgs,
                selected_tool: Some(tool.id.clone()),
                confidence: Some(confidence),
                args: None,
                missing,
                notes: Vec::new(),
                active_tool_threshold,
                active_mapping_threshold,
                candidates,
            },
            Err(crate::error::PlanError::AmbiguousMapping { details }) => CallPlan {
                status: PlanStatus::AmbiguousMapping,
                selected_tool: Some(tool.id.clone()),
                confidence: Some(confidence),
                args: None,
                missing: Vec::new(),
                notes: vec![details],
                active_tool_threshold,
                active_mapping_threshold,
                candidates,
            },
            Err(_) => CallPlan {
                status: PlanStatus::NoTool,
                selected_tool: None,
                confidence: None,
                args: None,
                missing: Vec::new(),
                notes: vec!["unexpected error during matching".into()],
                active_tool_threshold,
                active_mapping_threshold,
                candidates,
            },
            Ok(assignment) => {
                // Build args by mapping slot values through the assignment
                let mut args = HashMap::new();
                for (slot_key, param_name) in &assignment.mapping {
                    if let Some(value) = slots.get(slot_key) {
                        args.insert(param_name.clone(), value.clone());
                    }
                }

                // Check for any required args still missing
                let missing: Vec<String> = tool
                    .args
                    .iter()
                    .filter(|a| a.required && !args.contains_key(&a.name))
                    .map(|a| a.name.clone())
                    .collect();

                if !missing.is_empty() {
                    return CallPlan {
                        status: PlanStatus::MissingArgs,
                        selected_tool: Some(tool.id.clone()),
                        confidence: Some(confidence),
                        args: Some(args),
                        missing,
                        notes: Vec::new(),
                        active_tool_threshold,
                        active_mapping_threshold,
                        candidates,
                    };
                }

                let mut notes = Vec::new();
                if !assignment.unmatched_slots.is_empty() {
                    notes.push(format!("unmatched slots: {:?}", assignment.unmatched_slots));
                }

                CallPlan {
                    status: PlanStatus::Ok,
                    selected_tool: Some(tool.id.clone()),
                    confidence: Some(confidence),
                    args: Some(args),
                    missing: Vec::new(),
                    notes,
                    active_tool_threshold,
                    active_mapping_threshold,
                    candidates,
                }
            }
        }
    }
}

/// Simple dot product of two vectors.
fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum()
}
