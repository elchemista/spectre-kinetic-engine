# spectre-kinetic

> **Deterministic, zero-LLM tool selection for agents — powered by static embeddings and a compact Action Language.**

---

## What is spectre-kinetic?

Modern LLM-based agents typically rely on the model itself to decide which tool to call and how to fill in its parameters. This works, but it has real costs: every tool-selection decision burns tokens, adds latency, and introduces non-determinism. If the model hallucinates a tool name or misformats a parameter, the whole action fails silently.

**spectre-kinetic** solves this by moving tool selection *out* of the LLM entirely.

Instead of asking the model "which function should I call?", you give the agent a small, expressive **Action Language (AL)** — a set of structured verbs and slots like:

```
INSTALL PACKAGE nginx
COMPRESS DIRECTORY /var/log INTO FILE logs.tar.gz
WRITE POST WITH title={title} body={body}
```

spectre-kinetic takes one of these AL statements, uses **cosine similarity over static token embeddings** to match it against a compiled tool registry, maps the slots to the correct parameters, and returns a deterministic `CallPlan` JSON — no LLM call required.

### Why was this built?

The problem is this: LLMs are excellent at *reasoning* and *generating text*, but they are overkill — and unreliable — for the mechanical task of "given an intent, select the right function and fill in its arguments". That task is essentially a structured retrieval + mapping problem, and it should be fast, reproducible, and offline-capable.

spectre-kinetic was built to:

- **Offload tool dispatch from the LLM** — the model produces an AL statement, spectre-kinetic handles the rest.
- **Work without a network connection** — all inference uses a tiny pre-distilled static embedding table (a few MB), not a live API.
- **Be fully deterministic** — same AL statement + same registry = same CallPlan, every time.
- **Support any domain through training** — ship it with blog tool definitions today, retrain on Linux CLI commands tomorrow, or mix both in a combined corpus.
- **Give agents transparency** — the `candidates` field in the output shows every evaluated tool and its similarity score, so agents (or humans debugging them) can see exactly why a tool was or wasn't selected.

### How it works

```
AL statement (text)
       │
       ▼
  AL Parser  ──► verb + slot list
       │
       ▼
 Static Embeddings  ──► token vectors (distilled from a teacher ONNX model)
       │
       ▼
 Cosine Similarity  ──► ranked tool candidates from the compiled registry
       │
       ▼
  Slot Mapper  ──► slots assigned to tool parameters
       │
       ▼
  CallPlan JSON  ──► { selected_tool, args, confidence, candidates }
```

The embedding table is distilled once from a teacher model (e.g. all-MiniLM-L6-v2) over a training corpus, then frozen. At inference time, no neural network runs — only dot products and a lookup table.

---

## Crates

- **spectre-core** — AL parser, static embedder, cosine similarity, slot→param matching, registry builder, `SpectreDispatcher` planner.
- **spectre-train** — Corpus parsing, ONNX teacher wrapper, distillation loop, pack writer.
- **spectre-kinetic** — CLI with `train`, `build-registry`, `plan`, and `extract-dict` subcommands.
- **spectre-ffi** — C ABI library and optional Rustler NIF bindings for Elixir.

All crates are deterministic and dependency-light.

---

## Installation

### Prerequisites

- **Rust (stable ≥ 1.93)** — install or update via [rustup](https://rustup.rs):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  # or update an existing installation:
  rustup update stable
  ```

- **ONNX Runtime 1.23 shared library** — required at runtime for the `train` subcommand (not needed for `plan` or `build-registry`). See the section below.

### Install the CLI

```bash
git clone https://github.com/your-org/spectre-kinetic
cd spectre-kinetic
cargo install --path crates/spectre-cli
```

This puts `spectre-kinetic` in `~/.cargo/bin/`. Make sure that directory is in your `PATH`.

Verify:

```bash
spectre-kinetic --version
# spectre-kinetic 0.1.0
```

---

## Setting up the ONNX Runtime

`spectre-kinetic train` uses the ONNX Runtime to run the teacher model. The `ort` crate loads the runtime **dynamically at runtime** — you need to provide the shared library yourself.

### Download ONNX Runtime 1.23

```bash
# Linux x64
curl -L https://github.com/microsoft/onnxruntime/releases/download/v1.23.0/onnxruntime-linux-x64-1.23.0.tgz \
  -o onnxruntime-linux-x64-1.23.0.tgz
tar -xzf onnxruntime-linux-x64-1.23.0.tgz

# macOS arm64
curl -L https://github.com/microsoft/onnxruntime/releases/download/v1.23.0/onnxruntime-osx-arm64-1.23.0.tgz \
  -o onnxruntime-osx-arm64-1.23.0.tgz
tar -xzf onnxruntime-osx-arm64-1.23.0.tgz
```

### Point spectre-kinetic at it

Set the `ORT_DYLIB_PATH` environment variable to the `.so` / `.dylib` file:

```bash
# Linux
export ORT_DYLIB_PATH=/path/to/onnxruntime-linux-x64-1.23.0/lib/libonnxruntime.so.1.23.0

# macOS
export ORT_DYLIB_PATH=/path/to/onnxruntime-osx-arm64-1.23.0/lib/libonnxruntime.1.23.0.dylib
```

Add this to your `~/.bashrc` / `~/.zshrc` to make it permanent.

---

## Downloading a Teacher Model

Any ONNX model that outputs `last_hidden_state` of shape `[batch, seq_len, dim]` works. The recommended default is **all-MiniLM-L6-v2** (dim=384).

### Download with huggingface-cli

```bash
pip install huggingface_hub
huggingface-cli download sentence-transformers/all-MiniLM-L6-v2 \
  --include "*.onnx" "tokenizer.json" \
  --local-dir models/all-MiniLM-L6-v2
```

### Manual download

Go to: https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/tree/main

Download:
- `onnx/model.onnx` → save as `models/all-MiniLM-L6-v2/model.onnx`
- `tokenizer.json` → save as `models/all-MiniLM-L6-v2/tokenizer.json`

---

## Training a Model Pack

### 1. Prepare a corpus

A `corpus.jsonl` file with one JSON object per line. Supported types:

```jsonl
{"type":"al","text":"WRITE POST WITH title={title} body={body}"}
{"type":"tool_doc","tool_id":"Blog.write/2","text":"Writes a blog post"}
{"type":"tool_spec","tool_id":"Blog.write/2","text":"title: string, body: string"}
{"type":"param_card","tool_id":"Blog.write/2","text":"title: string (required)"}
{"type":"slot_card","text":"title={title}"}
{"type":"example","tool_id":"Blog.write/2","text":"WRITE POST WITH title={title} body={body}"}
```

For Linux CLI tools, AL verbs map like:

```jsonl
{"type":"al","text":"INSTALL PACKAGE nginx"}
{"type":"tool_doc","tool_id":"apt/install","text":"Install a package using apt"}
{"type":"example","tool_id":"apt/install","text":"INSTALL PACKAGE {package}"}
```

### 2. Run training

```bash
ORT_DYLIB_PATH=/path/to/libonnxruntime.so.1.23.0 \
spectre-kinetic train \
  --teacher-onnx models/all-MiniLM-L6-v2/model.onnx \
  --tokenizer models/all-MiniLM-L6-v2/tokenizer.json \
  --corpus corpus.jsonl \
  --out packs/minilm \
  --max-len 256 \
  --dim 384 \
  --zipf
```

Outputs written to `packs/minilm/`:

| File | Description |
|---|---|
| `pack.json` | Metadata (vocab size, dim, teacher ID, timestamp) |
| `tokenizer.json` | Copy of the tokenizer |
| `token_embeddings.bin` | Static token table (f16 little-endian, `[vocab_size × dim]`) |
| `weights.bin` | Optional Zipf/SIF per-token weights (when `--zipf`) |

The pack is self-contained — copy the `packs/minilm/` directory anywhere to use it.

---

## Using the Trained Model

### Build a compiled registry

Define your tools in a `tools.json` file (see schema below), then compile it:

```bash
spectre-kinetic build-registry \
  --model packs/minilm \
  --registry tools.json \
  --out registry.mcr
```

This embeds each tool card (doc + spec + examples) and individual parameter cards into a compact binary registry using f16 cosine-ready embeddings.

### Plan a tool call

Send a `PlanRequest` JSON on stdin:

```json
{
  "al": "INSTALL PACKAGE nginx",
  "slots": {"package": "nginx"},
  "top_k": 3,
  "tool_threshold": 0.45,
  "mapping_threshold": 0.35
}
```

```bash
cat plan_request.json | spectre-kinetic plan \
  --model packs/minilm \
  --registry registry.mcr \
  --stdin-json
```

Example output:

```json
{
  "status": "ok",
  "selected_tool": "apt/install",
  "confidence": 0.82,
  "args": {
    "package": "nginx"
  },
  "missing": [],
  "active_tool_threshold": 0.45,
  "active_mapping_threshold": 0.35,
  "candidates": [
    {"id": "apt/install",  "score": 0.82},
    {"id": "pacman/install", "score": 0.71},
    {"id": "dnf/install",  "score": 0.69}
  ]
}
```

- **`tool_threshold`** — minimum cosine similarity for a tool to be considered (default: `0.40`).
- **`mapping_threshold`** — minimum similarity for a slot to be mapped to a parameter (default: `0.35`).
- **`candidates`** — all evaluated tools and their scores, in descending order.

---

## Registry Schema (`tools.json`)

```json
{
  "version": 1,
  "tools": [
    {
      "id": "Elchemista.Blog.create_post/2",
      "module": "Elchemista.Blog",
      "name": "create_post",
      "arity": 2,
      "doc": "Creates a new blog post on elchemista.com with a given title and content body",
      "spec": "create_post(title :: String.t(), body :: String.t()) :: {:ok, Post.t()} | {:error, term()}",
      "args": [
        {"name": "title", "type": "String.t()", "required": true, "aliases": ["headline", "subject"]},
        {"name": "body",  "type": "String.t()", "required": true, "aliases": ["content", "text", "post_body"]}
      ],
      "examples": ["WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE={title} TEXT={text}"]
    }
  ]
}
```

---

## Extracting a Dictionary for Agents

`extract-dict` generates a compact `DICTIONARY.txt` in three lines for zero/low-shot prompts:

- Line 1: Uppercase tokens — seed words (from `--seed`) + most frequent words from corpus/registry, space-separated.
- Line 2: Slot keys — lowercase slot names, space-separated.
- Line 3: Examples — AL examples joined by ` | `.

Examples:

```bash
spectre-kinetic extract-dict \
  --corpus combined_corpus.jsonl \
  --out DICTIONARY.txt

spectre-kinetic extract-dict \
  --corpus combined_corpus.jsonl \
  --registry tools.json \
  --seed example/AL_DICTIONARY.txt \
  --out DICTIONARY.txt \
  --top-n 800
```

Notes:

- `--seed` is optional. Provide a whitespace- or line-separated wordlist.
- `--top-n` caps uppercase common tokens. Slot keys and examples are appended separately.

---

## FFI Bindings (C ABI + Elixir Rustler)

Build the shared library:

```bash
cargo build -p spectre-ffi --release
# → target/release/libspectre_ffi.so (Linux), .dylib (macOS), .dll (Windows)
```

C ABI (JSON in/out):

```c
struct Spectre; // opaque
struct Spectre* spectre_open(const char* model_dir, const char* registry_mcr, char** err_msg);
void spectre_close(struct Spectre* h);
int spectre_plan_json(struct Spectre* h, const char* request_json, char** out_plan_json, char** err_msg);
int spectre_plan_al(struct Spectre* h, const char* al_text, char** out_plan_json, char** err_msg);
void spectre_free_string(char* p);
char* spectre_version(void);
```

Elixir (Rustler feature): add a Rustler crate entry pointing to `crates/spectre-ffi` with `features = ["rustler"]`. Exposed functions:

- `Spectre.FFI.open(model_dir, registry_mcr) :: resource`
- `Spectre.FFI.plan_json(handle, request_json) :: String`
- `Spectre.FFI.plan_al(handle, al_text) :: String`

---

## Complete Workflow Example (Linux tools)

```bash
# 1. Update Rust
rustup update stable

# 2. Install spectre-kinetic
cargo install --path crates/spectre-cli

# 3. Download ONNX Runtime 1.23
curl -L https://github.com/microsoft/onnxruntime/releases/download/v1.23.0/onnxruntime-linux-x64-1.23.0.tgz | tar -xz
export ORT_DYLIB_PATH=$PWD/onnxruntime-linux-x64-1.23.0/lib/libonnxruntime.so.1.23.0

# 4. Download teacher model
huggingface-cli download sentence-transformers/all-MiniLM-L6-v2 \
  --include "*.onnx" "tokenizer.json" --local-dir models/all-MiniLM-L6-v2

# 5. Train on the linux + generic corpus
spectre-kinetic train \
  --teacher-onnx models/all-MiniLM-L6-v2/model.onnx \
  --tokenizer models/all-MiniLM-L6-v2/tokenizer.json \
  --corpus combined_corpus.jsonl \
  --out packs/minilm --zipf

# 6. Build registry
spectre-kinetic build-registry \
  --model packs/minilm \
  --registry tools.json \
  --out registry.mcr

# 7. Plan
echo '{"al":"INSTALL PACKAGE nginx","slots":{"package":"nginx"}}' | \
  spectre-kinetic plan --model packs/minilm --registry registry.mcr --stdin-json
```

---

## Developer Notes

- **Tests:** `cargo test --workspace`
- **Lint:** `cargo clippy --workspace --all-targets -- -D warnings`
- **Format:** `rustfmt` (config included)
- The PCA module truncates dimensions (no eigendecomposition yet).
- The teacher wrapper uses the first ONNX output (typically `last_hidden_state`).
- Distillation uses mean pooling + optional SIF (Zipf) weighting per token.

## Crate Structure

| Crate | Role |
|---|---|
| `crates/spectre-core` | AL parser, embedder, similarity, matching, registry, planner |
| `crates/spectre-train` | Corpus parsing, ONNX teacher wrapper, distillation, pack writer |
| `crates/spectre-cli` | `spectre-kinetic` binary |
| `crates/spectre-ffi` | C ABI library and optional Rustler NIF bindings |

## License

Apache License 2.0
