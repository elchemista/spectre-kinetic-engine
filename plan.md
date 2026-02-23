ModelCaller Rust Workspace Implementation Plan
Context
Build the ModelCaller system from SPEC.md: a Rust workspace with 3 crates that lets LLM/agents talk in Action Language (AL) and receive deterministic JSON "call plans" for tool selection and argument binding. The embedding engine follows the model2vec-rs reference implementation pattern (static token embeddings with mean pooling). Code must follow the Rust best practices in SKILLS.md.

Workspace Structure

specter-toolchain/
  Cargo.toml              # workspace root (replaces current package)
  rustfmt.toml
  model2vec-rs/            # reference only, NOT a workspace member
  crates/
    modelcaller-core/      # library: embedding, registry, planning
    modelcaller-train/     # library: ONNX distillation
    modelcaller-cli/       # binary: CLI with train/build-registry/plan
Implementation Phases
Phase 1: Workspace Skeleton
Convert root Cargo.toml to workspace with resolver = "2", workspace deps, workspace lints
Remove existing src/main.rs placeholder
Create all 3 crate Cargo.tomls with proper deps (thiserror for libs, anyhow for CLI only)
Add rustfmt.toml (max_width = 120)
Create stub lib.rs/main.rs files
Verify cargo check
Phase 2: Core Types & Errors (modelcaller-core)
error.rs: CoreError (thiserror) + PlanError (NO_TOOL, MISSING_ARGS, AMBIGUOUS_MAPPING)
types.rs: PackMetadata, ToolRegistry, ToolDef, ArgDef, PlanRequest, CallPlan, PlanStatus, McrHeader, ToolMeta, ParsedSlot
Phase 3: Embedding Engine (modelcaller-core)
embed.rs: StaticEmbedder adapted from model2vec-rs StaticModel pattern
from_pack(), encode_single(), encode_batch(), pool_ids()
Uses &[&str] params (not &[String]), thiserror, no HF Hub
pack.rs: load_pack() reads pack/ dir (pack.json, tokenizer.json, token_embeddings.bin as f16)
Reference: model2vec-rs/src/model.rs lines 229-376 for pool_ids, compute_metadata, truncation
Phase 4: AL Parser (modelcaller-core)
al_parser.rs: deterministic parsing, no ML
Split on WITH:, extract {slot_name} placeholders, parse KEY=value
Thorough unit tests
Phase 5: Similarity & Matching (modelcaller-core)
similarity.rs: cosine similarity, top-k with threshold
matching.rs: greedy slot-to-param assignment using similarity matrix
Phase 6: Registry (modelcaller-core)
registry.rs: CompiledRegistry struct, build_registry(), .mcr binary save/load
Binary format: header + tool metadata JSON + tool embeddings f16 + param embeddings f16 + optional slot card embeddings
Phase 7: Planner (modelcaller-core)
planner.rs: ModelCaller struct with plan() method orchestrating:
Parse AL -> embed action -> cosine sim vs tool embeddings -> top-k
For selected tool: embed slot cards -> similarity vs param embeddings -> greedy matching
Validate required args -> return CallPlan JSON
Phase 8: Training Library (modelcaller-train)
corpus.rs: JSONL parsing with serde tagged enum
teacher.rs: ONNX session wrapper via ort crate
distill.rs: accumulation loop (token-level teacher outputs -> mean per token ID)
pca.rs: stub for v1 (passthrough), real PCA later
weighting.rs: Zipf/SIF weights
pack_writer.rs: writes pack/ directory
Phase 9: CLI (modelcaller-cli)
main.rs: clap derive with Train/BuildRegistry/Plan subcommands
Each command dispatches to core/train library functions
Key Dependencies
Dep	Version	Crate	Purpose
tokenizers	0.21	core, train	HF tokenizer
ndarray	0.15	core, train	Embedding matrices
half	2.0	core, train	f16 encode/decode
serde/serde_json	1.0	all	Serialization
thiserror	2.0	core, train	Library errors
anyhow	1.0	cli only	Binary errors
clap	4.0	cli only	CLI args
ort	2.0	train only	ONNX Runtime
Verification
cargo check --workspace passes after each phase
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace with unit tests for al_parser, embed, matching, registry round-trip, planner integration
End-to-end: build a tiny test pack + registry, run modelcaller plan with sample AL input, verify JSON output