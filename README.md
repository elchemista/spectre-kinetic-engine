# spectre-kinetic

> **Deterministic, zero-LLM action dispatch for agents — powered by static embeddings and a compact Action Language.**
> This library is designed to be used as a FFI binding in Elixir applications as part of the Spectre framework. (Supervised Process Event Controller for Transition & Reasoning with Elixir.) But the framework is not ready yet.

---

## What is spectre-kinetic?

Modern LLM-based agents typically rely on the model itself to decide which tool to call and how to fill in its parameters. This works, but it has real costs: every tool-selection decision burns tokens, adds latency, and introduces non-determinism. If the model hallucinates a tool name or misformats a parameter, the whole action fails silently.

**spectre-kinetic** solves this by moving action dispatch *out* of the LLM entirely.

Instead of asking the model "which function should I call?", you give the agent a small, expressive **Action Language (AL)** — a set of structured verbs and slots like:

```
INSTALL PACKAGE nginx
COMPRESS DIRECTORY /var/log INTO FILE logs.tar.gz
WRITE POST WITH title={title} body={body}
CREATE STRIPE PAYMENT LINK WITH: AMOUNT=5000 CURRENCY='usd' PRODUCT_NAME="Premium Plan"
CALL API WITH: URL="https://api.example.com/users" METHOD=GET
```

spectre-kinetic takes one of these AL statements, uses **cosine similarity over static token embeddings** to match it against a compiled action registry, maps the slots to the correct parameters, and returns a deterministic `CallPlan` JSON — no LLM call required.

The AL parser is **case-insensitive** and **punctuation-tolerant** — it doesn't care if you write `WRITE POST`, `write post`, or `Write Post`, and it handles stray `;`, `,`, `.` and both single and double quotes.

### Why not MCP?

The **Model Context Protocol (MCP)** is designed for LLM tool-calling: the model receives a JSON schema of available tools, reasons about which one to use, and emits a structured tool-call response. This has inherent limitations:

| Concern | MCP (LLM-driven) | spectre-kinetic (AL-driven) |
|---|---|---|
| **Latency** | Every dispatch = full LLM round-trip (hundreds of ms to seconds) | Sub-millisecond: dot products + table lookup |
| **Cost** | Each tool decision burns input + output tokens | Zero marginal cost after initial training |
| **Determinism** | Same prompt can produce different tool calls across runs | Same AL + same registry = identical result, always |
| **Offline** | Requires API/network access for every call | Fully offline — a ~3 MB pack is all you need |
| **Hallucinations** | Model can invent tool names, malform args, skip required params | Impossible — only registered actions are returned |
| **Transparency** | Black box: hard to debug *why* a tool was chosen | `candidates` list shows every action + similarity score |
| **Scaling** | More tools = more tokens in the context = slower + more expensive | More actions = slightly larger registry, same speed |
| **Schema drift** | Schema changes require prompt engineering to handle | Schema changes = rebuild registry (deterministic) |
| **Fallback** | Fails silently or returns garbage | Returns `suggestions` with top-3 pre-filled AL commands for the LLM/user to pick |

MCP is the right choice when the model needs to *reason* about tool selection. spectre-kinetic is the right choice when tool selection is a **structured retrieval problem** that should be fast, reproducible, and free of hallucinations — which is most of the time.

You can use both together: let the LLM produce an AL statement (constrained output), then let spectre-kinetic handle the dispatch. The LLM focuses on reasoning; spectre-kinetic focuses on execution.

### How it works

```
AL statement (text)
       │
       ▼
  AL Parser  ──► normalize + extract verb + slot list
       │              (case-insensitive, punctuation-tolerant)
       ▼
 Static Embeddings  ──► token vectors (distilled from a teacher ONNX model)
       │
       ▼
 Cosine Similarity  ──► ranked action candidates from the compiled registry
       │
       ▼
  Slot Mapper  ──► slots assigned to action parameters (with defaults)
       │
       ▼
  CallPlan JSON  ──► { selected_tool, args, confidence, candidates, suggestions }
```

The embedding table is distilled once from a teacher model (e.g. all-MiniLM-L6-v2) over a training corpus, then frozen. At inference time, no neural network runs — only dot products and a lookup table.

---

## Showcases

### 1. Blog API dispatch

```
AL:  write new blog post with: title='My First Post' text='Hello everyone!'
```

```json
{
  "status": "ok",
  "selected_tool": "Elchemista.Blog.create_post/2",
  "confidence": 0.87,
  "args": {"title": "My First Post", "body": "Hello everyone!"},
  "candidates": [
    {"id": "Elchemista.Blog.create_post/2", "score": 0.87}
  ]
}
```

### 2. Stripe Payment API

```
AL:  CREATE STRIPE PAYMENT LINK WITH: AMOUNT=5000 PRODUCT_NAME="Premium Plan"
```

```json
{
  "status": "ok",
  "selected_tool": "Payments.Stripe.create_payment_link/1",
  "confidence": 0.91,
  "args": {"amount": "5000", "currency": "usd", "name": "Premium Plan"},
  "notes": [],
  "candidates": [...]
}
```

Note: `currency` was filled from the arg default value `"usd"` — no slot needed.

### 3. REST API calls

Registry:
```json
{
  "id": "Http.request/3",
  "module": "Http",
  "name": "request",
  "arity": 3,
  "doc": "Make an HTTP request to a URL with a given method and optional payload",
  "spec": "request(url :: String.t(), method :: String.t(), payload :: String.t())",
  "args": [
    {"name": "url", "type": "String.t()", "required": true, "aliases": ["endpoint","uri"]},
    {"name": "method", "type": "String.t()", "required": false, "aliases": ["verb","http_method"], "default": "GET"},
    {"name": "payload", "type": "String.t()", "required": false, "aliases": ["body","data","json"]}
  ],
  "examples": ["CALL API WITH: URL={url} METHOD={method} PAYLOAD={payload}"]
}
```

AL:
```
CALL API WITH: URL="https://api.example.com/users"
```

Result: dispatches to `Http.request/3` with `method` defaulting to `"GET"`.

### 4. GitHub Issue API

```
AL:  CREATE GITHUB ISSUE WITH: REPO="my-org/my-repo" TITLE="Bug report" BODY="Found a bug in auth"
```

### 5. Slack Webhook

```
AL:  SEND WEBHOOK WITH: URL='https://hooks.slack.com/services/xxx' PAYLOAD='{"text":"deploy done"}'
```

### 6. Linux tools

```
AL:  INSTALL PACKAGE nginx VIA APT
AL:  COMPRESS DIRECTORY /var/log INTO FILE logs.tar.gz
AL:  KILL PROCESS {pid}
AL:  LIST DIRECTORY {path}
```

### 7. Ambiguous input with suggestions

When the input doesn't clearly match any registered action:

```
AL:  do something cool
```

```json
{
  "status": "NO_TOOL",
  "suggestions": [
    {"id": "Blog.create_post/2", "score": 0.18, "al_command": "do something cool WITH: TITLE={title} BODY={body}"},
    {"id": "Http.request/3",     "score": 0.15, "al_command": "do something cool WITH: URL={url} METHOD={method}"},
    {"id": "Stripe.pay/1",       "score": 0.12, "al_command": "do something cool WITH: AMOUNT={amount} NAME={name}"}
  ]
}
```

The LLM can read the suggestions and pick the right one, or ask the user for clarification.

---

## Crates

- **spectre-core** — AL parser, static embedder, cosine similarity, slot->param matching, registry builder, `SpectreDispatcher` planner.
- **spectre-train** — Corpus parsing, ONNX teacher wrapper, distillation loop, pack writer.
- **spectre-kinetic** — CLI with `train`, `build-registry`, `plan`, and `extract-dict` subcommands.
- **spectre-ffi** — C ABI library and optional Rustler NIF bindings for Elixir.

All crates are deterministic and dependency-light.

---

## Installation

### Prerequisites

- **Rust (stable >= 1.93)** -- install or update via [rustup](https://rustup.rs):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  # or update an existing installation:
  rustup update stable
  ```

- **ONNX Runtime 1.23 shared library** -- required at runtime for the `train` subcommand (not needed for `plan` or `build-registry`). See the section below.

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

`spectre-kinetic train` uses the ONNX Runtime to run the teacher model. The `ort` crate loads the runtime **dynamically at runtime** -- you need to provide the shared library yourself.

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
- `onnx/model.onnx` -> save as `models/all-MiniLM-L6-v2/model.onnx`
- `tokenizer.json` -> save as `models/all-MiniLM-L6-v2/tokenizer.json`

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

For API actions:

```jsonl
{"type":"al","text":"CALL API WITH: URL={url} METHOD={method}"}
{"type":"tool_doc","tool_id":"Http.request/3","text":"Make an HTTP request to a URL"}
{"type":"example","tool_id":"Http.request/3","text":"CALL API WITH: URL={url} METHOD={method} PAYLOAD={payload}"}
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
| `token_embeddings.bin` | Static token table (f16 little-endian, `[vocab_size x dim]`) |
| `weights.bin` | Optional Zipf/SIF per-token weights (when `--zipf`) |

The pack is self-contained -- copy the `packs/minilm/` directory anywhere to use it.

---

## Using the Trained Model

### Build a compiled registry

Define your actions in a `tools.json` file (see schema below), then compile it:

```bash
spectre-kinetic build-registry \
  --model packs/minilm \
  --registry tools.json \
  --out registry.mcr
```

This embeds each action card (doc + spec + examples) and individual parameter cards into a compact binary registry using f16 cosine-ready embeddings.

### Plan an action call

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

- **`tool_threshold`** -- minimum cosine similarity for an action to be considered (default: `0.40`).
- **`mapping_threshold`** -- minimum similarity for a slot to be mapped to a parameter (default: `0.35`).
- **`candidates`** -- all evaluated actions and their scores, in descending order.
- **`suggestions`** -- when no action meets the threshold, the top-3 closest matches with pre-filled AL commands.

---

## Registry Schema (`tools.json`)

The registry uses `"actions"` as the top-level key (the legacy `"tools"` key is still accepted for backwards compatibility):

```json
{
  "version": 1,
  "actions": [
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
    },
    {
      "id": "Payments.Stripe.create_payment_link/1",
      "module": "Payments.Stripe",
      "name": "create_payment_link",
      "arity": 3,
      "doc": "Generates a new checkout session on Stripe",
      "spec": "create_payment_link(amount :: integer(), currency :: String.t(), name :: String.t())",
      "args": [
        {"name": "amount",   "type": "integer()",  "required": true,  "aliases": ["price","cost"]},
        {"name": "currency", "type": "String.t()", "required": true,  "aliases": ["coin"], "default": "usd"},
        {"name": "name",     "type": "String.t()", "required": true,  "aliases": ["item","product_name"]}
      ],
      "examples": ["CREATE STRIPE PAYMENT LINK WITH: AMOUNT={amount} CURRENCY={currency} PRODUCT_NAME={name}"]
    }
  ]
}
```

### Arg fields

| Field | Type | Description |
|---|---|---|
| `name` | string | Canonical parameter name |
| `type` | string | Type string (e.g. `"String.t()"`, `"integer()"`) |
| `required` | bool | Whether the arg must be provided |
| `aliases` | string[] | Alternate names the AL slot mapper will consider |
| `default` | string? | Default value used when no slot matches this arg |

---

## AL Parser Features

The parser is designed to handle messy, real-world input from LLMs and humans:

- **Case-insensitive**: `WRITE POST`, `write post`, `Write Post` all work identically
- **WITH keyword**: splits on `WITH:`, `WITH `, `with:`, `With` (any case)
- **Quote-agnostic**: both `TITLE="hello"` and `TITLE='hello'` work
- **Punctuation-tolerant**: trailing `;`, `,`, `.` are stripped automatically
- **Whitespace-tolerant**: extra spaces are collapsed
- **Placeholders**: `{slot_name}` in the action or WITH section
- **Literal values**: `KEY=value`, `KEY="quoted value"`, `KEY='single quoted'`

Examples of equivalent inputs that all parse identically:

```
WRITE POST WITH: TITLE={title} TEXT={text}
write post with: title={title} text={text}
Write Post With: Title={title}, Text={text};
WRITE POST WITH TITLE={title}; TEXT={text}
```

---

## Extracting a Dictionary for Agents

`extract-dict` generates a compact `DICTIONARY.txt` in three lines for zero/low-shot prompts:

- Line 1: Uppercase tokens -- seed words (from `--seed`) + most frequent words from corpus/registry, space-separated.
- Line 2: Slot keys -- lowercase slot names, space-separated.
- Line 3: Examples -- AL examples joined by ` | `.

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

## FFI Bindings (C ABI + Elixir Rustler, AL-first)

Build the shared library:

```bash
cargo build -p spectre-ffi --release
# -> target/release/libspectre_ffi.so (Linux), .dylib (macOS), .dll (Windows)
```

C ABI:

```c
struct Spectre; // opaque
struct Spectre* spectre_open(const char* model_dir, const char* registry_mcr, char** err_msg);
void spectre_close(struct Spectre* h);
int spectre_plan(struct Spectre* h, const char* al_text, char** out_plan_json, char** err_msg);
int spectre_plan_json(struct Spectre* h, const char* request_json, char** out_plan_json, char** err_msg);
int spectre_plan_al(struct Spectre* h, const char* al_text, char** out_plan_json, char** err_msg);
int spectre_add_action(struct Spectre* h, const char* action_json, char** err_msg);
int spectre_delete_action(struct Spectre* h, const char* action_id, int* out_deleted, char** err_msg);
int spectre_load_registry(struct Spectre* h, const char* registry_mcr, char** err_msg);
void spectre_free_string(char* p);
char* spectre_version(void);
```

Preferred AL-only call path:

- `spectre_plan(...)` (alias) or `spectre_plan_al(...)`
- Pass a single AL string like:
  - `WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE="New Day" TEXT="Today i want to speak about ..."`

`spectre_plan_json(...)` is kept for compatibility, but you do not need to build a JSON `PlanRequest` if you use AL-only APIs.

Runtime registry mutation APIs:

- `spectre_add_action(...)`: register a new action at runtime from a `ToolDef` JSON payload
- `spectre_delete_action(...)`: remove an action by id (`out_deleted=1` when removed)
- `spectre_load_registry(...)`: hot-swap the active `.mcr` registry without reopening the model pack

Example `action_json` payload:

```json
{
  "id": "Dynamic.Echo.say/1",
  "module": "Dynamic.Echo",
  "name": "say",
  "arity": 1,
  "doc": "Echo a user message",
  "spec": "say(message :: String.t()) :: :ok",
  "args": [
    {"name": "message", "type": "String.t()", "required": true, "aliases": ["text", "msg"]}
  ],
  "examples": ["DYNAMIC ECHO SAY WITH: MESSAGE={message}"]
}
```

Elixir (Rustler feature): add a Rustler crate entry pointing to `crates/spectre-ffi` with `features = ["rustler"]`. Exposed functions:

- `Spectre.FFI.open(model_dir, registry_mcr) :: resource`
- `Spectre.FFI.plan(handle, al_text) :: String` (AL-only alias)
- `Spectre.FFI.plan_json(handle, request_json) :: String`
- `Spectre.FFI.plan_al(handle, al_text) :: String`
- `Spectre.FFI.add_action(handle, action_json) :: boolean`
- `Spectre.FFI.delete_action(handle, action_id) :: boolean`
- `Spectre.FFI.load_registry(handle, registry_mcr) :: boolean`

---

## Complete Workflow Example (Linux + API tools)

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

# 7. Plan a Linux action
echo '{"al":"INSTALL PACKAGE nginx","slots":{"package":"nginx"}}' | \
  spectre-kinetic plan --model packs/minilm --registry registry.mcr --stdin-json

# 8. Plan an API action
echo '{"al":"CALL API WITH: URL=\"https://api.example.com/users\" METHOD=GET","slots":{"url":"https://api.example.com/users","method":"GET"}}' | \
  spectre-kinetic plan --model packs/minilm --registry registry.mcr --stdin-json
```

---

## Developer Notes

- **Tests:** `cargo test --workspace` (70+ tests covering parser, similarity, matching, serialization, and real-world messy inputs)
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
