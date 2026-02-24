//! Integration tests for runtime registry mutation APIs on `SpectreDispatcher`.

use spectre_core::types::{ArgDef, ToolDef, ToolMeta};
use spectre_core::{CompiledRegistry, SpectreDispatcher};
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

#[test]
fn add_action_then_delete_action_updates_registry_size() {
    let mut dispatcher = load_dispatcher();
    let base = dispatcher.action_count();

    dispatcher.add_action(dynamic_action()).expect("add_action should succeed");
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

    dispatcher.add_action(dynamic_action()).expect("add_action should succeed");
    let expanded = dispatcher.action_count();

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let registry_path = manifest.join("tests/test_registry.mcr");
    dispatcher.set_registry(&registry_path).expect("set_registry should succeed");

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

fn unique_temp_path(prefix: &str, ext: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{}_{}.{}", prefix, nanos, ext))
}
