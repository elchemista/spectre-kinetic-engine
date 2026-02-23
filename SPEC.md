## ModelCaller Spec v0.1 (MiniLM teacher, Rust-only, CLI + Elixir binding)

### Goal

Build a self-contained Rust system that lets any LLM/agent talk in a human-ish **Action Language (AL)** and receive a deterministic JSON “call plan”:

* select the best matching **tool/function/API**
* map AL “slots” (e.g., `TEXT`, `TITLE`) to real argument names (`body`, `post_body`, `content`, …)
* return a JSON plan (or structured errors: no tool / missing args / ambiguous mapping)

This system must not require sending tool specs to the big LLM.

---

## 1) Key design decisions

### 1.1 Teacher model (starting point)

* Teacher: `sentence-transformers/all-MiniLM-L6-v2` (384-d sentence embeddings). ([Hugging Face][1])
* ONNX: use an ONNX-exported package (e.g., `optimum/all-MiniLM-L6-v2`) or export your own. ([Hugging Face][2])
* Pooling used by teacher embedding: mean pooling over token embeddings using attention mask (+ normalization as standard in Sentence-Transformers usage). ([Hugging Face][1])

### 1.2 Tokenizer (paired with teacher; packaged in the distilled model pack)

* Use Hugging Face **`tokenizer.json`** format and load it from Rust via the `tokenizers` crate. ([Docs.rs][3])
* Max length: 256 wordpieces (use this as default truncation for training + runtime). ([Hugging Face][1])

**Rule:** each distilled “pack” is self-consistent: (teacher ONNX + tokenizer + derived static weights). Tokenizer can change between packs; runtime loads the pack’s tokenizer.

---

## 2) High-level architecture

```
LLM/Agent
  |
  |  AL text + slot values
  v
ModelCaller (Rust runtime)
  |
  |-- embed(AL) ------------------> tool retrieval (top-k)
  |-- embed(slot texts) ----------> slot→param mapping for selected tool
  |-- deterministic bind/validate -> call plan JSON
  v
Executor (outside scope): calls the real function/API
```

**Embedding engine**: fork/extend `model2vec-rs` (Rust inference implementation for Model2Vec static embeddings). ([GitHub][4])

Model2Vec context: static embeddings can be far smaller/faster than transformers (up to ~500× faster, per project docs). ([PyPI][5])

---

## 3) Project deliverables (what to build)

### 3.1 Rust workspace (fork of `model2vec-rs`)

Create a workspace with 3 crates:

1. **`modelcaller-core`** (library)

* Loads a ModelCaller **pack** (tokenizer + static token embeddings)
* Loads a compiled tool registry index
* API: `plan(al_text, slots) -> CallPlanJson`

2. **`modelcaller-cli`** (binary)

* `train` (distill pack from teacher ONNX + corpus)
* `build-registry` (compile tool registry to embeddings/index)
* `plan` (read request JSON, output plan JSON)

3. **`modelcaller-train`** (internal lib used by CLI)

* ONNX teacher runner + distillation logic

### 3.2 Elixir exporter

A Mix task that walks modules and exports a **tool registry JSON** from `@doc` and `@spec` plus argument names/types.

---

## 4) Artifacts and formats

### 4.1 Distilled pack format (directory)

A pack is what the runtime loads for embeddings:

```
pack/
  pack.json                 # metadata (teacher id, dim, pooling, tokenizer hash)
  tokenizer.json            # HF tokenizer for this pack
  vocab.txt (optional)
  token_embeddings.bin      # static token vectors (float16 recommended)
  weights.json              # optional weighting params (Zipf/SIF)
```

Runtime embedding = tokenize text → lookup token vectors → mean-pool (Model2Vec-style inference). ([minish][6])

### 4.2 Tool registry JSON (from Elixir)

Minimal required fields:

```json
{
  "version": 1,
  "tools": [
    {
      "id": "MyMod.create_post/2",
      "module": "MyMod",
      "name": "create_post",
      "arity": 2,
      "doc": "...",
      "spec": "...",
      "args": [
        { "name": "title", "type": "String.t()", "required": true, "aliases": ["TITLE","headline"] },
        { "name": "body",  "type": "String.t()", "required": true, "aliases": ["TEXT","content","post_body"] }
      ],
      "examples": [
        "WRITE NEW BLOG POST ... WITH: TITLE={title} TEXT={text}"
      ]
    }
  ]
}
```

### 4.3 Compiled registry index (from `build-registry`)

Outputs a single file (JSON or binary) containing:

* tool metadata
* tool embedding vectors for retrieval
* param embedding vectors for slot→param matching
* optional inverted indices / normalization constants

---

## 5) Runtime request/response contract (for Elixir binding)

### 5.1 Input request JSON

```json
{
  "al": "WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE={title} TEXT={text}",
  "slots": { "title": "My Title", "text": "Body content..." },
  "top_k": 5
}
```

### 5.2 Output plan JSON

Success:

```json
{
  "status": "ok",
  "selected_tool": "MyMod.create_post/2",
  "confidence": 0.87,
  "args": { "title": "My Title", "body": "Body content..." },
  "missing": [],
  "notes": []
}
```

Errors:

* `NO_TOOL` (no candidate above threshold)
* `MISSING_ARGS` (selected tool requires args not provided)
* `AMBIGUOUS_MAPPING` (slot→param mapping uncertain)

---

## 6) Core algorithms

### 6.1 Tool selection (AL → tool)

During `build-registry`, generate “tool-card texts” and embed them:

**Tool-card template (short, consistent):**

```
{module}.{name}/{arity}
DOC: {doc}
SPEC: {spec}
EX: {example_1}
```

At runtime:

1. embed AL string
2. cosine similarity vs tool embeddings
3. choose top-k + threshold

### 6.2 Slot extraction (AL → slot keys)

Do not use ML for extracting slot keys. Parse deterministically:

* `WITH:` section keys (e.g., `TITLE=...`)
* `{slot_name}` placeholders
* optionally allow `KEY=literal` as well

### 6.3 Argument binding (slot keys → real param names)

Problem: AL uses `TEXT`, tool uses `body/post_body/content`.

Solution: embed and match.

During `build-registry`:

* **Param-card text** per tool param:

  ```
  PARAM {param_name}
  TYPE {type}
  ALIASES {aliases...}
  DOC {short_doc}
  ```
* **Slot-card text** per canonical slot (small global list):

  ```
  SLOT TEXT: main content/body
  SLOT TITLE: headline/title
  SLOT URL: a URL
  ...
  ```

At runtime:

1. embed slot-cards for slots present in AL
2. embed param-cards for the selected tool
3. compute similarity matrix
4. solve constrained matching:

   * all required params must be matched
   * one slot maps to at most one param (unless allowed)
   * type compatibility heuristics (optional v1)

Then fill args from `slots` input and validate required params.

---

## 7) Rust-only distillation pipeline (`modelcaller train`)

### Inputs

* teacher ONNX model (MiniLM)
* teacher tokenizer.json (same repo)
* domain corpus:

  * AL sentences
  * tool docs/spec strings
  * param/slot cards
  * examples

### Steps

1. **Load tokenizer** with `tokenizers` crate from `tokenizer.json`. ([Docs.rs][3])
2. **Run teacher ONNX** in Rust (ONNX Runtime) to produce embeddings. Sentence-Transformers documents ONNX backend usage and ONNX conversion workflows. ([sbert.net][7])
3. **Build static token embeddings** (domain-aligned):

   * sample sentences from corpus
   * get teacher token-level outputs
   * for each token id: accumulate sum/count → mean token vector
4. Optional: **PCA** to reduce dimension (e.g., 256) and weighting (Zipf/SIF-style) to downweight common tokens (optional v1; can add later). ([PyPI][5])
5. Write **pack/** directory.

---

## 8) CLI specification

### `modelcaller train`

* Inputs: `--teacher-onnx`, `--tokenizer`, `--corpus`, `--out pack_dir`, optional `--dim`, `--max-len`
* Output: `pack_dir/` (distilled static model)

### `modelcaller build-registry`

* Inputs: `--model pack_dir`, `--registry registry.json`, `--out registry.compiled`
* Output: compiled registry index

### `modelcaller plan`

* Inputs: `--model pack_dir`, `--compiled registry.compiled`, `--stdin-json`
* Output: plan JSON to stdout

---

## 9) Elixir integration

* Use a Port or `System.cmd` to call `modelcaller plan`
* Send request JSON on stdin, read response JSON from stdout
* On `NO_TOOL` / `MISSING_ARGS`, the caller can:

  * ask the LLM to rephrase AL
  * request missing slots
  * or fall back to normal LLM response

---

## 10) v1 acceptance criteria

1. Given a registry with 50+ tools, `plan` selects the correct tool for AL examples with >90% accuracy (on your test set).
2. Slot→param mapping correctly maps common synonyms (TEXT→body/content/post_body; TITLE→title/headline) without LLM tool specs.
3. Errors are deterministic and machine-parseable (`NO_TOOL`, `MISSING_ARGS`, `AMBIGUOUS_MAPPING`).
4. Entire pipeline runs without Python (teacher ONNX + tokenizer in Rust). ([sbert.net][7])

[1]: https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2?utm_source=chatgpt.com "sentence-transformers/all-MiniLM-L6-v2 · Hugging Face"
[2]: https://huggingface.co/optimum/all-MiniLM-L6-v2?utm_source=chatgpt.com "optimum/all-MiniLM-L6-v2 · Hugging Face"
[3]: https://docs.rs/tokenizers/latest/tokenizers/?utm_source=chatgpt.com "tokenizers - Rust - Docs.rs"
[4]: https://github.com/MinishLab/model2vec-rs?utm_source=chatgpt.com "MinishLab/model2vec-rs: Official Rust Implementation of Model2Vec - GitHub"
[5]: https://pypi.org/project/model2vec/?utm_source=chatgpt.com "model2vec · PyPI"
[6]: https://minishlab.github.io/hf_blogpost/?utm_source=chatgpt.com "Model2Vec Introduction blogpost – minish – Soooooooooooo fast"
[7]: https://www.sbert.net/docs/sentence_transformer/usage/efficiency.html?utm_source=chatgpt.com "Speeding up Inference — Sentence Transformers documentation"


## Update: registry embeddings must be persisted and loadable

### Requirement

`modelcaller plan` must take **one file** that already contains:

* the tool registry (tools, params, metadata)
* the **precomputed embeddings** used for retrieval and slot→param matching

So runtime does **no embedding work on registry content**, only embeds the AL query + slot/param “cards” as needed (or even those can be precomputed too).

This matches the “static embedding + fast lookup” design of Model2Vec: at inference you just embed the input and do similarity; precomputing everything else is the point. ([minish][1])

---

## Spec: files on disk

### A) Model pack (for embedding AL and card strings)

Directory produced by `modelcaller train`:

```
pack/
  pack.json
  tokenizer.json
  vocab.txt (optional)
  token_embeddings.bin   # static token table (float16 recommended)
  weights.json (optional) # weighting params if used
```

Model2Vec distillation concept: forward vocabulary through a teacher, reduce with PCA, apply Zipf weighting; inference = mean of token embeddings. ([minish][1])

### B) Registry+Embeddings file (single file input for runtime)

One file produced by `modelcaller build-registry`, e.g.:

* `registry.mcr` (recommended: binary) OR
* `registry.with_embeddings.json` (human-readable, larger)

#### Recommended: `registry.mcr` (binary container)

Contains:

* header: version, dims, tokenizer hash, counts
* tool metadata block
* tool embedding matrix (float16)
* param metadata block
* param embedding matrix (float16)
* optional: precomputed “slot card” embeddings (for your canonical slot set)

Rationale: JSON for large float arrays is slow and huge. Binary gives fast load and mmap-friendly.

#### Acceptable fallback: `registry.with_embeddings.json`

Same information, but embeddings stored as:

* base64-encoded float16 bytes, or
* quantized int8 arrays + scale

---

## Runtime API contract (unchanged, but now `--registry` already includes embeddings)

### `modelcaller plan`

Inputs:

* `--model pack/`
* `--registry registry.mcr` (or `.json`)
* request JSON on stdin

Request JSON:

```json
{
  "al": "WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE={title} TEXT={text}",
  "slots": { "title": "My Title", "text": "Body..." },
  "top_k": 5
}
```

Output JSON:

```json
{
  "status": "ok",
  "selected_tool": "Elchemista.Blog.create_post/2",
  "confidence": 0.87,
  "args": { "title": "My Title", "body": "Body..." },
  "missing": [],
  "notes": []
}
```

---

## Training and datasets (Rust-only, JSON formats)

You need **two kinds of data**:

1. **Distillation corpus** (to build the static token table for your AL/tool domain)
2. **Supervised examples** (to validate/optionally train a small reranker later)

Model2Vec can be distilled without labeled datasets, but you are explicitly choosing to add domain data (AL + docs/specs) to shape the embeddings. ([minish][1])

### 1) Distillation corpus format (JSONL recommended)

File: `corpus.jsonl`
Each line is one JSON object:

```json
{"type":"al","text":"WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE={title} TEXT={text}"}
{"type":"tool_doc","tool_id":"Elchemista.Blog.create_post/2","text":"Creates a new blog post on elchemista.com ..."}
{"type":"tool_spec","tool_id":"Elchemista.Blog.create_post/2","text":"create_post(title :: String.t(), body :: String.t()) :: {:ok, Post.t()} | {:error, term()}"}
{"type":"param_card","tool_id":"Elchemista.Blog.create_post/2","text":"PARAM body TYPE String.t() ALIASES content,text,post_body DOC Main post content"}
{"type":"param_card","tool_id":"Elchemista.Blog.create_post/2","text":"PARAM title TYPE String.t() ALIASES headline,subject DOC Post title"}
```

How it’s used in `modelcaller train`:

* tokenize+run teacher ONNX on these texts
* accumulate contextual token vectors by token-id
* average to produce static token vectors
* optional PCA/weighting
* write `pack/`

Teacher model: `all-MiniLM-L6-v2` is a sentence-transformers embedding model (384-d). ([Hugging Face][2])
Mean-pooling usage is standard in the model’s examples. ([Hugging Face][2])

### 2) Supervised “planning” examples format (JSON)

File: `examples.json` (array) or `examples.jsonl`

Each item specifies the expected tool and argument mapping:

```json
{
  "version": 1,
  "examples": [
    {
      "al": "WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE={title} TEXT={text}",
      "slots": { "title": "Hello", "text": "World" },
      "expected_tool": "Elchemista.Blog.create_post/2",
      "expected_args": { "title": "title", "body": "text" }
    },
    {
      "al": "CREATE STRIPE PAYMENT LINK WITH: AMOUNT={amount} CURRENCY={currency} PRODUCT_NAME={name}",
      "slots": { "amount": "1200", "currency": "EUR", "name": "T-shirt" },
      "expected_tool": "Payments.Stripe.create_payment_link/1",
      "expected_args": { "amount": "amount", "currency": "currency", "line_items[0].price_data.product_data.name": "name" }
    }
  ]
}
```

Notes:

* `expected_args` maps **tool param path** → **slot key** (slot keys are stable, tool args can be nested).
* This dataset is used for:

  * evaluation of retrieval accuracy and slot→param matching
  * future optional supervised reranker training (if needed)

---

## CLI commands (final)

### 1) Train pack (Rust-only, teacher ONNX)

```
modelcaller train \
  --teacher-onnx teacher.onnx \
  --tokenizer tokenizer.json \
  --corpus corpus.jsonl \
  --out pack/ \
  --max-len 256 \
  --dim 384
```

Implementation uses:

* Rust `tokenizers` crate for tokenization ([Docs.rs][3])
* ONNX Runtime via Rust `ort` binding ([GitHub][4])

### 2) Build registry with embeddings persisted

```
modelcaller build-registry \
  --model pack/ \
  --registry registry.json \
  --out registry.mcr
```

Produces the single file you’ll pass to `plan`.

### 3) Plan (single-shot for Elixir Port)

```
modelcaller plan \
  --model pack/ \
  --registry registry.mcr \
  --stdin-json
```

---

## How the registry embeddings are computed (build-registry)

1. For each tool, build a short **tool-card string** (`DOC`, `SPEC`, `EXAMPLES`) and embed it.
2. For each param, build a **param-card string** (`PARAM`, `TYPE`, `ALIASES`, short doc) and embed it.
3. Optionally embed canonical **slot-card strings** once and store them too.
4. Save all embeddings into `registry.mcr`.

At runtime:

* embed AL query (only)
* retrieve top-k tools by cosine similarity
* for selected tool, match slots↔params using stored param embeddings (and stored slot embeddings if present)
* validate required args / types → return plan or error

This is consistent with Model2Vec inference being mean pooling over token embeddings (fast), and shifting everything else to precomputation. ([minish][1])

[1]: https://minishlab.github.io/hf_blogpost/?utm_source=chatgpt.com "Model2Vec Introduction blogpost – minish – Soooooooooooo fast"
[2]: https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2?utm_source=chatgpt.com "sentence-transformers/all-MiniLM-L6-v2 · Hugging Face"
[3]: https://docs.rs/ort/latest/ort/?utm_source=chatgpt.com "ort - Rust - Docs.rs"
[4]: https://github.com/pykeio/ort?utm_source=chatgpt.com "Fast ML inference & training for ONNX models in Rust - GitHub"
