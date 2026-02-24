//! Integration tests that validate AL parsing against real example files.

use spectre_core::al_parser::{parse_al, parse_al_and_slots};
use spectre_core::types::ToolRegistry;
use std::path::Path;

#[test]
fn parse_all_al_like_lines_from_example_corpus() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let corpus_path = manifest.join("../../example/combined_corpus.jsonl");
    let raw = std::fs::read_to_string(&corpus_path)
        .unwrap_or_else(|e| panic!("failed reading {}: {e}", corpus_path.display()));

    let mut parsed_count = 0usize;

    for (idx, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let v: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("invalid JSONL at line {}: {e}", idx + 1));

        let kind = v
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or_else(|| panic!("missing type at line {}", idx + 1));

        if kind == "al" || kind == "example" {
            let text = v
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or_else(|| panic!("missing text at line {}", idx + 1));

            // Must parse without panics and produce an action text.
            let parsed = parse_al(text);
            assert!(
                !parsed.action_text.trim().is_empty(),
                "empty action_text for line {}: {}",
                idx + 1,
                text
            );

            // Also run value extraction path (covers quote/punctuation/case normalization behavior).
            let _ = parse_al_and_slots(text);
            parsed_count += 1;
        }
    }

    assert!(
        parsed_count >= 100,
        "expected broad coverage from example corpus, got {parsed_count}"
    );
}

#[test]
fn tools_json_uses_actions_and_has_defaults() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tools_path = manifest.join("../../example/tools.json");
    let raw =
        std::fs::read_to_string(&tools_path).unwrap_or_else(|e| panic!("failed reading {}: {e}", tools_path.display()));

    let registry: ToolRegistry = serde_json::from_str(&raw).unwrap_or_else(|e| panic!("invalid tools.json: {e}"));

    assert!(!registry.actions.is_empty(), "example/tools.json has no actions");

    let stripe = registry
        .actions
        .iter()
        .find(|a| a.id == "Payments.Stripe.create_payment_link/1")
        .expect("missing Payments.Stripe.create_payment_link/1 action");

    let currency = stripe
        .args
        .iter()
        .find(|a| a.name == "currency")
        .expect("missing currency arg");

    assert_eq!(currency.default.as_deref(), Some("usd"));
}

#[test]
fn all_examples_in_tools_json_are_parseable() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tools_path = manifest.join("../../example/tools.json");
    let raw =
        std::fs::read_to_string(&tools_path).unwrap_or_else(|e| panic!("failed reading {}: {e}", tools_path.display()));

    let registry: ToolRegistry = serde_json::from_str(&raw).unwrap_or_else(|e| panic!("invalid tools.json: {e}"));

    let mut example_count = 0usize;
    for action in &registry.actions {
        for ex in &action.examples {
            let parsed = parse_al(ex);
            assert!(
                !parsed.action_text.trim().is_empty(),
                "example failed to parse for action {}: {}",
                action.id,
                ex
            );
            let _ = parse_al_and_slots(ex);
            example_count += 1;
        }
    }

    assert!(example_count > 0, "no examples found in tools.json");
}
