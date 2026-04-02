//! Integration tests for runtime registry mutation APIs on `SpectreDispatcher`.

use spectre_core::types::{ArgDef, ToolDef, ToolMeta};
use spectre_core::{CompiledRegistry, SpectreDispatcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn load_dispatcher() -> SpectreDispatcher {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pack_dir = manifest.join("../../packs/minilm");
    let registry_path = manifest.join("tests/test_registry.mcr");

    let (_meta, embedder) = spectre_core::pack::load_pack(&pack_dir).expect("failed to load pack");
    let compiled = CompiledRegistry::load(&registry_path).expect("failed to load test registry");

    SpectreDispatcher::new(embedder, compiled)
}

fn dynamic_action() -> ToolDef {
    ToolDef {
        id: "Dynamic.Echo.say/1".to_string(),
        module: "Dynamic.Echo".to_string(),
        name: "say".to_string(),
        arity: 1,
        doc: "Echo a user message".to_string(),
        spec: "say(message :: String.t()) :: :ok".to_string(),
        args: vec![ArgDef {
            name: "message".to_string(),
            arg_type: "String.t()".to_string(),
            required: true,
            aliases: vec!["text".to_string(), "msg".to_string()],
            default: None,
        }],
        examples: vec!["DYNAMIC ECHO SAY WITH: MESSAGE={message}".to_string()],
    }
}

fn email_action() -> ToolDef {
    ToolDef {
        id: "Dynamic.Email.send/2".to_string(),
        module: "Dynamic.Email".to_string(),
        name: "send".to_string(),
        arity: 2,
        doc: "Send a message to an email recipient".to_string(),
        spec: "send(to :: String.t(), body :: String.t()) :: :ok".to_string(),
        args: vec![
            ArgDef {
                name: "to".to_string(),
                arg_type: "String.t()".to_string(),
                required: true,
                aliases: vec!["recipient".to_string(), "email".to_string()],
                default: None,
            },
            ArgDef {
                name: "body".to_string(),
                arg_type: "String.t()".to_string(),
                required: true,
                aliases: vec!["message".to_string(), "text".to_string()],
                default: None,
            },
        ],
        examples: vec!["SEND MESSAGE WITH: TO={to} BODY={body}".to_string()],
    }
}

fn sms_action() -> ToolDef {
    ToolDef {
        id: "Dynamic.Sms.send/2".to_string(),
        module: "Dynamic.Sms".to_string(),
        name: "send".to_string(),
        arity: 2,
        doc: "Send a message to a phone recipient".to_string(),
        spec: "send(to :: String.t(), body :: String.t()) :: :ok".to_string(),
        args: vec![
            ArgDef {
                name: "to".to_string(),
                arg_type: "String.t()".to_string(),
                required: true,
                aliases: vec!["recipient".to_string(), "phone".to_string(), "number".to_string()],
                default: None,
            },
            ArgDef {
                name: "body".to_string(),
                arg_type: "String.t()".to_string(),
                required: true,
                aliases: vec!["message".to_string(), "text".to_string()],
                default: None,
            },
        ],
        examples: vec!["SEND MESSAGE WITH: TO={to} BODY={body}".to_string()],
    }
}

#[test]
fn add_action_then_delete_action_updates_registry_size() {
    let mut dispatcher = load_dispatcher();
    let base = dispatcher.action_count();

    dispatcher
        .add_action(dynamic_action())
        .expect("add_action should succeed");
    assert_eq!(dispatcher.action_count(), base + 1);

    let duplicate = dispatcher.add_action(dynamic_action());
    assert!(duplicate.is_err(), "duplicate action id should fail");

    let deleted = dispatcher
        .delete_action("Dynamic.Echo.say/1")
        .expect("delete_action should succeed");
    assert!(deleted);
    assert_eq!(dispatcher.action_count(), base);

    let deleted_again = dispatcher
        .delete_action("Dynamic.Echo.say/1")
        .expect("delete_action second call should succeed");
    assert!(!deleted_again);
}

#[test]
fn set_registry_swaps_registry_from_disk() {
    let mut dispatcher = load_dispatcher();

    dispatcher
        .add_action(dynamic_action())
        .expect("add_action should succeed");
    let expanded = dispatcher.action_count();

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let registry_path = manifest.join("tests/test_registry.mcr");
    dispatcher
        .set_registry(&registry_path)
        .expect("set_registry should succeed");

    assert!(dispatcher.action_count() < expanded);
}

#[test]
fn set_registry_rejects_dimension_mismatch() {
    let mut dispatcher = load_dispatcher();

    let bad_registry = CompiledRegistry {
        tools: vec![ToolMeta {
            id: "Bad.Mod.fn/0".to_string(),
            module: "Bad.Mod".to_string(),
            name: "fn".to_string(),
            arity: 0,
            args: Vec::new(),
            param_range: (0, 0),
        }],
        dims: 1,
        tokenizer_hash: "bad".to_string(),
        tool_embeddings: ndarray::Array2::zeros((1, 1)),
        param_embeddings: ndarray::Array2::zeros((0, 1)),
        slot_card_embeddings: None,
        slot_card_labels: None,
    };

    let path = unique_temp_path("bad_registry", "mcr");
    bad_registry.save(&path).expect("failed to write temp mcr");

    let result = dispatcher.set_registry(&path);

    let _ = std::fs::remove_file(&path);

    assert!(matches!(result, Err(spectre_core::CoreError::DimensionMismatch { .. })));
}

#[test]
fn reranking_prefers_email_for_email_like_recipient_values() {
    let mut dispatcher = load_dispatcher();
    dispatcher.add_action(email_action()).expect("add email action");
    dispatcher.add_action(sms_action()).expect("add sms action");

    let plan = dispatcher.plan(&spectre_core::types::PlanRequest {
        al: "SEND MESSAGE WITH: TO=\"dev@example.com\" BODY=\"hello\"".to_string(),
        slots: HashMap::from([
            ("to".to_string(), "dev@example.com".to_string()),
            ("body".to_string(), "hello".to_string()),
        ]),
        top_k: 5,
        tool_threshold: Some(0.0),
        mapping_threshold: Some(0.0),
    });

    assert_eq!(plan.selected_tool.as_deref(), Some("Dynamic.Email.send/2"));
    assert!(plan.tool_score.unwrap_or_default() > 0.0);
    assert!(plan.mapping_score.unwrap_or_default() > 0.0);
    assert_eq!(plan.confidence, plan.combined_score);
}

#[test]
fn reranking_prefers_sms_for_phone_like_recipient_values() {
    let mut dispatcher = load_dispatcher();
    dispatcher.add_action(email_action()).expect("add email action");
    dispatcher.add_action(sms_action()).expect("add sms action");

    let plan = dispatcher.plan(&spectre_core::types::PlanRequest {
        al: "SEND MESSAGE WITH: TO=\"+15551234567\" BODY=\"hello\"".to_string(),
        slots: HashMap::from([
            ("to".to_string(), "+15551234567".to_string()),
            ("body".to_string(), "hello".to_string()),
        ]),
        top_k: 5,
        tool_threshold: Some(0.0),
        mapping_threshold: Some(0.0),
    });

    assert_eq!(plan.selected_tool.as_deref(), Some("Dynamic.Sms.send/2"));
    assert!(plan.tool_score.unwrap_or_default() > 0.0);
    assert!(plan.mapping_score.unwrap_or_default() > 0.0);
    assert_eq!(plan.confidence, plan.combined_score);
}

#[test]
fn plan_al_recovers_inline_email_recipient_without_with_section() {
    let mut dispatcher = load_dispatcher();
    dispatcher.add_action(email_action()).expect("add email action");

    let plan = dispatcher.plan_al("SEND ME EMAIL to yuriy.zhar@gmail.com", None, Some(0.0), Some(0.0));

    assert_eq!(plan.selected_tool.as_deref(), Some("Dynamic.Email.send/2"));
    assert_eq!(
        plan.args.as_ref().and_then(|args| args.get("to")).map(|s| s.as_str()),
        Some("yuriy.zhar@gmail.com")
    );
    assert!(!plan.missing.iter().any(|arg| arg == "to"));
    assert!(
        plan.notes.iter().any(|note| note.contains("recovered inline args")),
        "expected planner notes to mention inline arg recovery, got {:?}",
        plan.notes
    );
}

#[test]
fn plan_al_recovers_inline_email_recipient_with_colon_separator() {
    let mut dispatcher = load_dispatcher();
    dispatcher.add_action(email_action()).expect("add email action");

    let plan = dispatcher.plan_al("SEND ME EMAIL TO: yuriy.zhar@gmail.com", None, Some(0.0), Some(0.0));

    assert_eq!(plan.selected_tool.as_deref(), Some("Dynamic.Email.send/2"));
    assert_eq!(
        plan.args.as_ref().and_then(|args| args.get("to")).map(|s| s.as_str()),
        Some("yuriy.zhar@gmail.com")
    );
    assert!(!plan.missing.iter().any(|arg| arg == "to"));
}

#[test]
fn plan_al_recovers_inline_email_recipient_with_equals_separator() {
    let mut dispatcher = load_dispatcher();
    dispatcher.add_action(email_action()).expect("add email action");

    let plan = dispatcher.plan_al("SEND ME EMAIL TO=yuriy.zhar@gmail.com", None, Some(0.0), Some(0.0));

    assert_eq!(plan.selected_tool.as_deref(), Some("Dynamic.Email.send/2"));
    assert_eq!(
        plan.args.as_ref().and_then(|args| args.get("to")).map(|s| s.as_str()),
        Some("yuriy.zhar@gmail.com")
    );
    assert!(!plan.missing.iter().any(|arg| arg == "to"));
}

fn unique_temp_path(prefix: &str, ext: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{}_{}.{}", prefix, nanos, ext))
}
