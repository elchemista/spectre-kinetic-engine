//! Plan orchestrator: tool selection + argument binding.
//!
//! Wires together AL parsing, embedding, tool retrieval, and slot-to-param matching
//! into a single `plan()` call that returns a structured [`CallPlan`].

use crate::al_parser;
use crate::embed::StaticEmbedder;
use crate::error::CoreError;
use crate::matching::{self, SlotAssignment};
use crate::registry::CompiledRegistry;
use crate::similarity;
use crate::types::{CallPlan, ParsedSlot, PlanRequest, PlanStatus};
use ndarray::Array2;
use std::collections::HashMap;
use std::path::Path;

const ACTION_WEIGHT: f32 = 0.65;
const SLOT_COVERAGE_WEIGHT: f32 = 0.20;
const VALUE_SHAPE_WEIGHT: f32 = 0.10;
const REQUIRED_ARG_WEIGHT: f32 = 0.05;

#[derive(Debug, Clone)]
struct CandidateEvaluation {
    tool_index: usize,
    selected_tool: String,
    status: PlanStatus,
    confidence: f32,
    tool_score: f32,
    mapping_score: f32,
    combined_score: f32,
    args: Option<HashMap<String, String>>,
    missing: Vec<String>,
    notes: Vec<String>,
}

impl CandidateEvaluation {
    fn as_candidate(&self) -> crate::types::CandidateTool {
        crate::types::CandidateTool {
            id: self.selected_tool.clone(),
            score: self.combined_score,
            tool_score: Some(self.tool_score),
            mapping_score: Some(self.mapping_score),
            combined_score: Some(self.combined_score),
        }
    }

    fn into_call_plan(
        self,
        active_tool_threshold: f32,
        active_mapping_threshold: f32,
        candidates: Vec<crate::types::CandidateTool>,
    ) -> CallPlan {
        CallPlan {
            status: self.status,
            selected_tool: Some(self.selected_tool),
            confidence: Some(self.confidence),
            tool_score: Some(self.tool_score),
            mapping_score: Some(self.mapping_score),
            combined_score: Some(self.combined_score),
            args: self.args,
            missing: self.missing,
            notes: self.notes,
            active_tool_threshold,
            active_mapping_threshold,
            candidates,
            suggestions: Vec::new(),
        }
    }
}

/// Top-level API for Spectre Dispatcher.
///
/// Holds a loaded model pack (embedder) and compiled registry, and exposes
/// the `plan()` method for processing AL requests.
pub struct SpectreDispatcher {
    embedder: StaticEmbedder,
    registry: CompiledRegistry,
    /// Minimum confidence for the final selected tool (default 0.3).
    tool_threshold: f32,
    /// Minimum cosine similarity for slot-to-param mapping (default 0.4).
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

    /// Override the final planner confidence threshold.
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

        let parsed = al_parser::parse_al(&request.al);
        let normalized_slots = normalize_slots(&request.slots);
        let slot_keys = effective_slot_keys(&parsed, &normalized_slots);
        let query_vec = self.embedder.encode_single(&parsed.action_text);

        // Retrieve the top-K tools from action text without early rejection.
        let sims = similarity::cosine_similarities(&query_vec, self.registry.tool_embeddings.view());
        let candidates = similarity::top_k_above_threshold(&sims, request.top_k, f32::NEG_INFINITY);

        if candidates.is_empty() {
            return CallPlan {
                status: PlanStatus::NoTool,
                selected_tool: None,
                confidence: None,
                tool_score: None,
                mapping_score: None,
                combined_score: None,
                args: None,
                missing: Vec::new(),
                notes: vec!["no action candidates available".into()],
                active_tool_threshold,
                active_mapping_threshold,
                candidates: Vec::new(),
                suggestions: Vec::new(),
            };
        }

        let mut evaluations: Vec<CandidateEvaluation> = candidates
            .iter()
            .map(|&(tool_idx, tool_score)| {
                self.evaluate_candidate(
                    tool_idx,
                    tool_score,
                    &slot_keys,
                    &normalized_slots,
                    active_mapping_threshold,
                )
            })
            .collect();

        evaluations.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    b.tool_score
                        .partial_cmp(&a.tool_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });

        let eval_candidates: Vec<crate::types::CandidateTool> =
            evaluations.iter().map(CandidateEvaluation::as_candidate).collect();

        let best = evaluations[0].clone();
        if best.combined_score < active_tool_threshold {
            return CallPlan {
                status: PlanStatus::NoTool,
                selected_tool: None,
                confidence: None,
                tool_score: None,
                mapping_score: None,
                combined_score: None,
                args: None,
                missing: Vec::new(),
                notes: vec![format!(
                    "no action matched above confidence threshold {:.3}; best combined score was {:.3}",
                    active_tool_threshold, best.combined_score
                )],
                active_tool_threshold,
                active_mapping_threshold,
                candidates: eval_candidates.clone(),
                suggestions: self.build_suggestions(&evaluations, &parsed, 3),
            };
        }

        best.into_call_plan(active_tool_threshold, active_mapping_threshold, eval_candidates)
    }

    /// Convenience: plan directly from a raw AL string, auto-extracting slot values from the WITH section.
    ///
    /// - `al_text`: the Action Language string (may contain placeholders or KEY=value/KEY="quoted value")
    /// - `top_k`: optional number of candidates to consider (default 5)
    /// - `tool_threshold` / `mapping_threshold`: optional threshold overrides
    pub fn plan_al(
        &self,
        al_text: &str,
        top_k: Option<usize>,
        tool_threshold: Option<f32>,
        mapping_threshold: Option<f32>,
    ) -> CallPlan {
        let (_parsed, kv) = al_parser::parse_al_and_slots(al_text);
        let req = crate::types::PlanRequest {
            al: al_text.to_string(),
            slots: kv,
            top_k: top_k.unwrap_or(5),
            tool_threshold,
            mapping_threshold,
        };
        self.plan(&req)
    }

    /// Replace the in-memory registry with a new compiled registry loaded from disk.
    pub fn set_registry(&mut self, mcr_path: &Path) -> Result<(), CoreError> {
        let compiled = CompiledRegistry::load(mcr_path)?;
        if compiled.dims != self.embedder.dim() {
            return Err(CoreError::DimensionMismatch {
                expected: self.embedder.dim(),
                actual: compiled.dims,
            });
        }
        self.registry = compiled;
        Ok(())
    }

    /// Dynamically add an action definition to the active registry.
    pub fn add_action(&mut self, action: crate::types::ToolDef) -> Result<(), CoreError> {
        self.registry.add_action(&self.embedder, action)
    }

    /// Remove an action by ID from the active registry.
    ///
    /// Returns `Ok(true)` if an action was removed.
    pub fn delete_action(&mut self, action_id: &str) -> Result<bool, CoreError> {
        self.registry.delete_action(action_id)
    }

    /// Number of actions currently available in the active registry.
    pub fn action_count(&self) -> usize {
        self.registry.tools.len()
    }

    fn evaluate_candidate(
        &self,
        tool_idx: usize,
        tool_score: f32,
        slot_keys: &[ParsedSlot],
        slots: &HashMap<String, String>,
        mapping_threshold: f32,
    ) -> CandidateEvaluation {
        let tool = &self.registry.tools[tool_idx];
        let assignment = if slot_keys.is_empty() || tool.args.is_empty() {
            SlotAssignment::empty(slot_keys, &tool.args)
        } else {
            self.match_slots_to_params(tool, slot_keys, mapping_threshold)
        };

        let mut args = HashMap::new();
        for (slot_key, param_name) in &assignment.mapping {
            if let Some(value) = slots.get(slot_key) {
                args.insert(param_name.clone(), value.clone());
            }
        }

        apply_defaults(&mut args, &tool.args);

        let missing: Vec<String> = tool
            .args
            .iter()
            .filter(|arg| arg.required && !args.contains_key(&arg.name))
            .map(|arg| arg.name.clone())
            .collect();

        let mut notes = Vec::new();
        if !assignment.unmatched_slots.is_empty() {
            notes.push(format!("unmatched slots: {:?}", assignment.unmatched_slots));
        }
        notes.extend(assignment.ambiguity_notes.iter().cloned());

        let slot_coverage_score = if slot_keys.is_empty() || tool.args.is_empty() {
            0.0
        } else {
            assignment.slot_coverage_score
        };
        let required_arg_satisfaction = required_arg_satisfaction(tool, &args);
        let value_shape_score = value_shape_score(tool, &assignment, slots);
        let mapping_score = mapping_score(slot_coverage_score, value_shape_score, required_arg_satisfaction);
        let combined_score = combined_score(
            tool_score,
            slot_coverage_score,
            value_shape_score,
            required_arg_satisfaction,
        );

        CandidateEvaluation {
            tool_index: tool_idx,
            selected_tool: tool.id.clone(),
            status: if missing.is_empty() {
                PlanStatus::Ok
            } else {
                PlanStatus::MissingArgs
            },
            confidence: combined_score,
            tool_score: clamp_score(tool_score),
            mapping_score,
            combined_score,
            args: if args.is_empty() { None } else { Some(args) },
            missing,
            notes,
        }
    }

    /// Match slot keys to tool params using embedding similarity.
    fn match_slots_to_params(
        &self,
        tool: &crate::types::ToolMeta,
        slot_keys: &[ParsedSlot],
        threshold: f32,
    ) -> SlotAssignment {
        let (param_start, param_end) = tool.param_range;

        let slot_cards: Vec<String> = slot_keys.iter().map(|slot| format!("SLOT {}", slot.key)).collect();
        let slot_refs: Vec<&str> = slot_cards.iter().map(|card| card.as_str()).collect();
        let slot_vecs = self.embedder.encode_batch(&slot_refs);

        let param_slice = self
            .registry
            .param_embeddings
            .slice(ndarray::s![param_start..param_end, ..]);
        let param_vecs: Vec<Vec<f32>> = param_slice.rows().into_iter().map(|row| row.to_vec()).collect();

        let num_slots = slot_vecs.len();
        let num_params = param_vecs.len();
        let mut sim_data = Vec::with_capacity(num_slots * num_params);

        for slot_vec in &slot_vecs {
            for param_vec in &param_vecs {
                sim_data.push(dot_product(slot_vec, param_vec));
            }
        }

        let sim_matrix = Array2::from_shape_vec((num_slots, num_params), sim_data)
            .unwrap_or_else(|_| Array2::zeros((num_slots, num_params)));

        matching::assign_slots_to_params(&sim_matrix, slot_keys, &tool.args, threshold)
    }

    /// Build top-N suggestions with pre-filled AL commands when no tool meets the threshold.
    fn build_suggestions(
        &self,
        evaluations: &[CandidateEvaluation],
        parsed: &al_parser::AlParsed,
        n: usize,
    ) -> Vec<crate::types::ActionSuggestion> {
        evaluations
            .iter()
            .take(n.min(evaluations.len()))
            .map(|evaluation| {
                let tool = &self.registry.tools[evaluation.tool_index];
                let arg_slots: Vec<String> = tool
                    .args
                    .iter()
                    .map(|arg| format!("{}={{{}}}", arg.name.to_uppercase(), arg.name))
                    .collect();

                let al_command = if arg_slots.is_empty() {
                    parsed.action_text.clone()
                } else {
                    format!("{} WITH: {}", parsed.action_text, arg_slots.join(" "))
                };

                crate::types::ActionSuggestion {
                    id: tool.id.clone(),
                    score: evaluation.combined_score,
                    al_command,
                }
            })
            .collect()
    }
}

fn normalize_slots(slots: &HashMap<String, String>) -> HashMap<String, String> {
    slots
        .iter()
        .map(|(key, value)| (key.to_lowercase(), value.clone()))
        .collect()
}

fn effective_slot_keys(parsed: &al_parser::AlParsed, slots: &HashMap<String, String>) -> Vec<ParsedSlot> {
    if !parsed.slot_keys.is_empty() {
        return parsed.slot_keys.clone();
    }

    let mut keys: Vec<ParsedSlot> = slots
        .keys()
        .map(|key| ParsedSlot {
            key: key.to_lowercase(),
            placeholder: false,
        })
        .collect();
    keys.sort_by(|a, b| a.key.cmp(&b.key));
    keys
}

fn combined_score(
    tool_score: f32,
    slot_coverage_score: f32,
    value_shape_score: f32,
    required_arg_satisfaction: f32,
) -> f32 {
    (ACTION_WEIGHT * clamp_score(tool_score)
        + SLOT_COVERAGE_WEIGHT * clamp_score(slot_coverage_score)
        + VALUE_SHAPE_WEIGHT * clamp_score(value_shape_score)
        + REQUIRED_ARG_WEIGHT * clamp_score(required_arg_satisfaction))
    .clamp(0.0, 1.0)
}

fn mapping_score(slot_coverage_score: f32, value_shape_score: f32, required_arg_satisfaction: f32) -> f32 {
    let mapping_weight = SLOT_COVERAGE_WEIGHT + VALUE_SHAPE_WEIGHT + REQUIRED_ARG_WEIGHT;
    if mapping_weight == 0.0 {
        return 0.0;
    }

    ((SLOT_COVERAGE_WEIGHT * clamp_score(slot_coverage_score)
        + VALUE_SHAPE_WEIGHT * clamp_score(value_shape_score)
        + REQUIRED_ARG_WEIGHT * clamp_score(required_arg_satisfaction))
        / mapping_weight)
        .clamp(0.0, 1.0)
}

fn required_arg_satisfaction(tool: &crate::types::ToolMeta, args: &HashMap<String, String>) -> f32 {
    let required_count = tool.args.iter().filter(|arg| arg.required).count();
    if required_count == 0 {
        return 1.0;
    }

    let satisfied = tool
        .args
        .iter()
        .filter(|arg| arg.required && args.contains_key(&arg.name))
        .count();

    satisfied as f32 / required_count as f32
}

fn value_shape_score(
    tool: &crate::types::ToolMeta,
    assignment: &SlotAssignment,
    slots: &HashMap<String, String>,
) -> f32 {
    let mut scores = Vec::new();

    for (slot_key, param_name) in &assignment.mapping {
        let Some(value) = slots.get(slot_key) else {
            continue;
        };
        let Some(arg) = tool.args.iter().find(|arg| arg.name == *param_name) else {
            continue;
        };

        let expected_shapes = expected_shapes(tool, arg);
        if expected_shapes.is_empty() {
            continue;
        }

        scores.push(shape_match_score(value, &expected_shapes));
    }

    if scores.is_empty() {
        0.0
    } else {
        (scores.iter().copied().sum::<f32>() / scores.len() as f32).clamp(0.0, 1.0)
    }
}

fn expected_shapes(tool: &crate::types::ToolMeta, arg: &crate::types::ArgDef) -> Vec<ValueShape> {
    let arg_text = format!("{} {}", arg.name, arg.aliases.join(" ")).to_lowercase();
    let tool_text = format!("{} {} {}", tool.id, tool.module, tool.name).to_lowercase();
    let recipient_like = contains_any(&arg_text, &["to", "recipient", "target", "contact", "user"]);

    let mut shapes = Vec::new();

    if contains_any(&arg_text, &["email", "mail"]) {
        push_shape(&mut shapes, ValueShape::Email);
    }
    if contains_any(&arg_text, &["phone", "mobile", "tel", "number"]) {
        push_shape(&mut shapes, ValueShape::Phone);
    }
    if contains_any(&arg_text, &["url", "link", "uri", "website"]) {
        push_shape(&mut shapes, ValueShape::Url);
    }
    if contains_any(&arg_text, &["path", "file", "dir", "directory"]) {
        push_shape(&mut shapes, ValueShape::Path);
    }
    if contains_any(&arg_text, &["date", "due_on", "birthday"]) {
        push_shape(&mut shapes, ValueShape::IsoDate);
    }
    if contains_any(
        &arg_text,
        &["datetime", "timestamp", "scheduled_at", "start_at", "end_at"],
    ) {
        push_shape(&mut shapes, ValueShape::IsoDateTime);
    }
    if contains_any(&arg_text, &["time", "at", "when", "start", "end", "schedule"]) {
        push_shape(&mut shapes, ValueShape::IsoDateTime);
        push_shape(&mut shapes, ValueShape::IsoTime);
    }

    if recipient_like && contains_any(&tool_text, &["email", "mail"]) {
        push_shape(&mut shapes, ValueShape::Email);
    }
    if recipient_like
        && contains_any(
            &tool_text,
            &[
                "sms", "phone", "text", "message", "chat", "whatsapp", "telegram", "signal", "call",
            ],
        )
    {
        push_shape(&mut shapes, ValueShape::Phone);
    }

    shapes
}

fn shape_match_score(value: &str, expected_shapes: &[ValueShape]) -> f32 {
    let actual_shapes = detect_shapes(value);
    if actual_shapes.is_empty() {
        return 0.0;
    }

    if actual_shapes.iter().any(|shape| expected_shapes.contains(shape)) {
        return 1.0;
    }

    0.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueShape {
    Email,
    Phone,
    IsoDate,
    IsoTime,
    IsoDateTime,
    Url,
    Path,
}

fn detect_shapes(value: &str) -> Vec<ValueShape> {
    let trimmed = value.trim();
    let mut shapes = Vec::new();

    if is_email_like(trimmed) {
        shapes.push(ValueShape::Email);
    }
    if is_phone_like(trimmed) {
        shapes.push(ValueShape::Phone);
    }
    if is_iso_datetime_like(trimmed) {
        shapes.push(ValueShape::IsoDateTime);
    }
    if is_iso_date_like(trimmed) {
        shapes.push(ValueShape::IsoDate);
    }
    if is_iso_time_like(trimmed) {
        shapes.push(ValueShape::IsoTime);
    }
    if is_url_like(trimmed) {
        shapes.push(ValueShape::Url);
    }
    if is_path_like(trimmed) {
        shapes.push(ValueShape::Path);
    }

    shapes
}

fn is_email_like(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

fn is_phone_like(value: &str) -> bool {
    let digits = value.chars().filter(|ch| ch.is_ascii_digit()).count();
    digits >= 7
        && value
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '+' | '-' | ' ' | '(' | ')' | '.'))
}

fn is_iso_date_like(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

fn is_iso_time_like(value: &str) -> bool {
    let bytes = value.as_bytes();
    (bytes.len() == 5 || bytes.len() == 8)
        && bytes[2] == b':'
        && bytes[0..2].iter().all(u8::is_ascii_digit)
        && bytes[3..5].iter().all(u8::is_ascii_digit)
        && (bytes.len() == 5 || (bytes[5] == b':' && bytes[6..8].iter().all(u8::is_ascii_digit)))
}

fn is_iso_datetime_like(value: &str) -> bool {
    if let Some((date, time)) = value.split_once('T') {
        return is_iso_date_like(date) && is_iso_time_like(time.trim_end_matches('Z'));
    }
    if let Some((date, time)) = value.split_once(' ') {
        return is_iso_date_like(date) && is_iso_time_like(time.trim_end_matches('Z'));
    }
    false
}

fn is_url_like(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn is_path_like(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with("./")
        || value.starts_with("../")
        || value.contains(std::path::MAIN_SEPARATOR)
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn push_shape(shapes: &mut Vec<ValueShape>, shape: ValueShape) {
    if !shapes.contains(&shape) {
        shapes.push(shape);
    }
}

fn clamp_score(score: f32) -> f32 {
    score.clamp(0.0, 1.0)
}

/// Apply default values from arg definitions for any args not yet in the map.
fn apply_defaults(args: &mut HashMap<String, String>, arg_defs: &[crate::types::ArgDef]) {
    for arg in arg_defs {
        if !args.contains_key(&arg.name) {
            if let Some(ref default) = arg.default {
                args.insert(arg.name.clone(), default.clone());
            }
        }
    }
}

/// Simple dot product of two vectors.
fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_shape_detection_covers_email_phone_and_dates() {
        assert!(detect_shapes("dev@example.com").contains(&ValueShape::Email));
        assert!(detect_shapes("+39 333 123 4567").contains(&ValueShape::Phone));
        assert!(detect_shapes("2026-04-01").contains(&ValueShape::IsoDate));
        assert!(detect_shapes("2026-04-01T12:30:00Z").contains(&ValueShape::IsoDateTime));
    }

    #[test]
    fn combined_score_weights_tool_and_mapping_signals() {
        let score = combined_score(0.4, 1.0, 1.0, 1.0);
        assert!((score - 0.61).abs() < 1e-5);
    }
}
