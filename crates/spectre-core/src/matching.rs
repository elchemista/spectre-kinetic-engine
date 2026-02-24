//! Constrained slot-to-param assignment.
//!
//! Given a similarity matrix between slot-card and param-card embeddings,
//! finds the optimal 1-to-1 assignment using a greedy algorithm.

use crate::error::PlanError;
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
}

/// Assign slots to tool parameters using a greedy best-match algorithm.
///
/// # Arguments
/// * `sim_matrix` - Similarity matrix of shape `[num_slots, num_params]`.
/// * `slot_keys` - Parsed slot keys from the AL text.
/// * `tool_args` - Argument definitions for the selected tool.
/// * `threshold` - Minimum similarity for a valid match.
///
/// # Errors
/// Returns `PlanError::MissingArgs` if required params remain unmatched,
/// or `PlanError::AmbiguousMapping` if two matches are indistinguishably close.
pub fn assign_slots_to_params(
    sim_matrix: &Array2<f32>,
    slot_keys: &[ParsedSlot],
    tool_args: &[ArgDef],
    threshold: f32,
) -> Result<SlotAssignment, PlanError> {
    let num_slots = slot_keys.len();
    let num_params = tool_args.len();

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
        .map(|(_, s)| s.key.clone())
        .collect();

    // Return error if required params are missing
    if !unmatched_required.is_empty() {
        return Err(PlanError::MissingArgs {
            missing: unmatched_required,
        });
    }

    // Return error if there were unresolvable ambiguities
    if !ambiguity_notes.is_empty() && !unmatched_slots.is_empty() {
        return Err(PlanError::AmbiguousMapping {
            details: ambiguity_notes.join("; "),
        });
    }

    Ok(SlotAssignment {
        mapping,
        unmatched_required: Vec::new(),
        unmatched_slots,
    })
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
        // 2 slots, 2 params, perfect diagonal match
        let sim = Array2::from_shape_vec((2, 2), vec![0.9, 0.1, 0.1, 0.9]).unwrap();
        let slots = vec![make_slot("title"), make_slot("text")];
        let args = vec![make_arg("title", true), make_arg("body", true)];

        let result = assign_slots_to_params(&sim, &slots, &args, 0.05).unwrap();
        assert_eq!(result.mapping.get("title").unwrap(), "title");
        assert_eq!(result.mapping.get("text").unwrap(), "body");
        assert!(result.unmatched_required.is_empty());
    }

    #[test]
    fn assign_should_return_missing_args_when_required_unmatched() {
        // 1 slot, 2 required params -> one must be missing
        let sim = Array2::from_shape_vec((1, 2), vec![0.9, 0.1]).unwrap();
        let slots = vec![make_slot("title")];
        let args = vec![make_arg("title", true), make_arg("body", true)];

        let err = assign_slots_to_params(&sim, &slots, &args, 0.05).unwrap_err();
        match err {
            PlanError::MissingArgs { missing } => assert_eq!(missing, vec!["body"]),
            _ => panic!("expected MissingArgs"),
        }
    }

    #[test]
    fn assign_should_handle_optional_params() {
        // 1 slot, 2 params (one optional) -> should succeed
        let sim = Array2::from_shape_vec((1, 2), vec![0.9, 0.1]).unwrap();
        let slots = vec![make_slot("title")];
        let args = vec![make_arg("title", true), make_arg("tags", false)];

        let result = assign_slots_to_params(&sim, &slots, &args, 0.05).unwrap();
        assert_eq!(result.mapping.get("title").unwrap(), "title");
    }

    #[test]
    fn assign_should_filter_by_threshold() {
        let sim = Array2::from_shape_vec((1, 1), vec![0.1]).unwrap();
        let slots = vec![make_slot("text")];
        let args = vec![make_arg("body", true)];

        let err = assign_slots_to_params(&sim, &slots, &args, 0.5).unwrap_err();
        match err {
            PlanError::MissingArgs { missing } => assert_eq!(missing, vec!["body"]),
            _ => panic!("expected MissingArgs"),
        }
    }
}
