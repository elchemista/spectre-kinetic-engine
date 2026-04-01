//! Constrained slot-to-param assignment.
//!
//! Given a similarity matrix between slot-card and param-card embeddings,
//! finds a greedy 1-to-1 assignment and returns detailed fit signals that
//! can be used by the planner during late reranking.

use crate::types::{ArgDef, ParsedSlot};
use ndarray::Array2;
use std::collections::HashMap;

/// Result of matching slots to tool parameters.
#[derive(Debug, Clone)]
pub struct SlotAssignment {
    /// Successfully matched pairs: slot_key -> param_name.
    pub mapping: HashMap<String, String>,
    /// Required params that could not be matched to any slot.
    pub unmatched_required: Vec<String>,
    /// Slots that could not be matched to any param.
    pub unmatched_slots: Vec<String>,
    /// Human-readable ambiguity notes gathered during matching.
    pub ambiguity_notes: Vec<String>,
    /// Similarity scores for the matched pairs.
    pub matched_scores: Vec<f32>,
    /// Aggregate slot coverage score in `[0, 1]`.
    pub slot_coverage_score: f32,
}

impl SlotAssignment {
    /// Build an empty assignment when there is no slot-matching work to do.
    pub fn empty(slot_keys: &[ParsedSlot], tool_args: &[ArgDef]) -> Self {
        Self {
            mapping: HashMap::new(),
            unmatched_required: tool_args
                .iter()
                .filter(|arg| arg.required)
                .map(|arg| arg.name.clone())
                .collect(),
            unmatched_slots: slot_keys.iter().map(|slot| slot.key.clone()).collect(),
            ambiguity_notes: Vec::new(),
            matched_scores: Vec::new(),
            slot_coverage_score: 0.0,
        }
    }
}

/// Assign slots to tool parameters using a greedy best-match algorithm.
///
/// # Arguments
/// * `sim_matrix` - Similarity matrix of shape `[num_slots, num_params]`.
/// * `slot_keys` - Parsed slot keys from the AL text.
/// * `tool_args` - Argument definitions for the selected tool.
/// * `threshold` - Minimum similarity for a valid match.
pub fn assign_slots_to_params(
    sim_matrix: &Array2<f32>,
    slot_keys: &[ParsedSlot],
    tool_args: &[ArgDef],
    threshold: f32,
) -> SlotAssignment {
    let num_slots = slot_keys.len();
    let num_params = tool_args.len();

    if num_slots == 0 || num_params == 0 {
        return SlotAssignment::empty(slot_keys, tool_args);
    }

    // Build all (slot_idx, param_idx, score) triples above threshold
    let mut candidates: Vec<(usize, usize, f32)> = Vec::new();
    for s in 0..num_slots {
        for p in 0..num_params {
            let score = sim_matrix[[s, p]];
            if score >= threshold {
                candidates.push((s, p, score));
            }
        }
    }

    // Sort descending by score for greedy assignment
    candidates.sort_unstable_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut assigned_slots = vec![false; num_slots];
    let mut assigned_params = vec![false; num_params];
    let mut mapping = HashMap::new();
    let mut ambiguity_notes = Vec::new();
    let mut matched_scores = Vec::new();

    for &(s_idx, p_idx, score) in &candidates {
        if assigned_slots[s_idx] || assigned_params[p_idx] {
            continue;
        }

        // Check for ambiguity: is there another unassigned param with nearly the same score?
        let ambiguous = candidates.iter().any(|&(s2, p2, score2)| {
            s2 == s_idx && p2 != p_idx && !assigned_params[p2] && (score - score2).abs() < 0.05
        });

        if ambiguous {
            ambiguity_notes.push(format!(
                "slot '{}' has ambiguous match to param '{}'",
                slot_keys[s_idx].key, tool_args[p_idx].name
            ));
        }

        assigned_slots[s_idx] = true;
        assigned_params[p_idx] = true;
        mapping.insert(slot_keys[s_idx].key.clone(), tool_args[p_idx].name.clone());
        matched_scores.push(score);
    }

    let unmatched_required: Vec<String> = tool_args
        .iter()
        .enumerate()
        .filter(|(i, arg)| arg.required && !assigned_params[*i])
        .map(|(_, arg)| arg.name.clone())
        .collect();

    let unmatched_slots: Vec<String> = slot_keys
        .iter()
        .enumerate()
        .filter(|(i, _)| !assigned_slots[*i])
        .map(|(_, slot)| slot.key.clone())
        .collect();

    let slot_coverage_score = slot_coverage_score(num_slots, &matched_scores, ambiguity_notes.is_empty());

    SlotAssignment {
        mapping,
        unmatched_required,
        unmatched_slots,
        ambiguity_notes,
        matched_scores,
        slot_coverage_score,
    }
}

fn slot_coverage_score(num_slots: usize, matched_scores: &[f32], unambiguous: bool) -> f32 {
    if num_slots == 0 || matched_scores.is_empty() {
        return 0.0;
    }

    let coverage_ratio = matched_scores.len() as f32 / num_slots as f32;
    let average_similarity = matched_scores.iter().copied().sum::<f32>() / matched_scores.len() as f32;
    let ambiguity_factor = if unambiguous { 1.0 } else { 0.85 };

    (coverage_ratio * average_similarity.clamp(0.0, 1.0) * ambiguity_factor).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_slot(key: &str) -> ParsedSlot {
        ParsedSlot {
            key: key.to_string(),
            placeholder: true,
        }
    }

    fn make_arg(name: &str, required: bool) -> ArgDef {
        ArgDef {
            name: name.to_string(),
            arg_type: "String.t()".to_string(),
            required,
            aliases: Vec::new(),
            default: None,
        }
    }

    #[test]
    fn assign_should_match_perfect_diagonal() {
        let sim = Array2::from_shape_vec((2, 2), vec![0.9, 0.1, 0.1, 0.9]).unwrap();
        let slots = vec![make_slot("title"), make_slot("text")];
        let args = vec![make_arg("title", true), make_arg("body", true)];

        let result = assign_slots_to_params(&sim, &slots, &args, 0.05);
        assert_eq!(result.mapping.get("title").unwrap(), "title");
        assert_eq!(result.mapping.get("text").unwrap(), "body");
        assert!(result.unmatched_required.is_empty());
        assert!(result.slot_coverage_score > 0.8);
    }

    #[test]
    fn assign_should_track_missing_required_when_unmatched() {
        let sim = Array2::from_shape_vec((1, 2), vec![0.9, 0.1]).unwrap();
        let slots = vec![make_slot("title")];
        let args = vec![make_arg("title", true), make_arg("body", true)];

        let result = assign_slots_to_params(&sim, &slots, &args, 0.05);
        assert_eq!(result.unmatched_required, vec!["body"]);
    }

    #[test]
    fn assign_should_handle_optional_params() {
        let sim = Array2::from_shape_vec((1, 2), vec![0.9, 0.1]).unwrap();
        let slots = vec![make_slot("title")];
        let args = vec![make_arg("title", true), make_arg("tags", false)];

        let result = assign_slots_to_params(&sim, &slots, &args, 0.05);
        assert_eq!(result.mapping.get("title").unwrap(), "title");
        assert!(result.unmatched_required.is_empty());
    }

    #[test]
    fn assign_should_filter_by_threshold() {
        let sim = Array2::from_shape_vec((1, 1), vec![0.1]).unwrap();
        let slots = vec![make_slot("text")];
        let args = vec![make_arg("body", true)];

        let result = assign_slots_to_params(&sim, &slots, &args, 0.5);
        assert_eq!(result.unmatched_required, vec!["body"]);
        assert_eq!(result.unmatched_slots, vec!["text"]);
        assert_eq!(result.slot_coverage_score, 0.0);
    }
}
