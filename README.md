# Spectre Toolchain

Static-embedding tool selection and planning for agents using a compact Action Language (AL).

This repository provides:

- spectre-core: Core library for static text embeddings, AL parsing, similarity, slot→param matching, registry build, and the SpectreDispatcher planner.
- spectre-train: Training library to distill a static token embedding table from an ONNX teacher model using a JSONL corpus.
- spectre-dispatcher: CLI to train packs, build compiled registries (.mcr), and run planning from stdin JSON.

All crates follow a small, dependency-light design and deterministic behavior suitable for tool-calling.

## Status

- All unit tests pass: `cargo test --workspace`.
- Clippy clean: `cargo clippy --workspace --all-targets -- -D warnings`.
- ONNX inference uses `ort = 2.0.0-rc.9` with dynamic loading.

## Quickstart

### 1) Build

```
cargo build --workspace
```

Run tests:

```
cargo test --workspace
```

### 2) Distill a model pack

You need:

- Teacher ONNX (e.g. a MiniLM variant that outputs `last_hidden_state` of shape [batch, seq_len, dim]).
- HuggingFace `tokenizer.json` matching the teacher.
- A `corpus.jsonl` with one JSON object per line; see "Corpus format" below.

Command:

```
spectre-dispatcher train \
  --teacher-onnx /path/to/teacher.onnx \
  --tokenizer /path/to/tokenizer.json \
  --corpus /path/to/corpus.jsonl \
  --out packs/minilm \
  --max-len 256 \
  --dim 384 \
  --zipf
```

Outputs in `packs/minilm`:

- pack.json (metadata)
- tokenizer.json (copied)
- token_embeddings.bin (f16 little-endian)
- weights.json (optional, when `--zipf`)

Note: The ONNX runtime is loaded dynamically. If needed, set `ORT_DYLIB_PATH` to the directory containing the ONNX Runtime shared library.

### 3) Build a compiled registry (.mcr)

Given a tool registry JSON (see schema summary below):

```
spectre-dispatcher build-registry \
  --model packs/minilm \
  --registry tools.json \
  --out registry.mcr
```

### 4) Plan a call from AL + slots

Pipe a `PlanRequest` JSON to stdin:

```
echo '{
  "al": "WRITE POST WITH title={title} body={body}",
  "slots": {"title": "Hello", "body": "World"},
  "top_k": 3
}' | spectre-dispatcher plan \
  --model packs/minilm \
  --registry registry.mcr \
  --stdin-json
```

The CLI prints a `CallPlan` JSON with selected tool, confidence, mapped args, and status.

## Corpus format (corpus.jsonl)

One JSON object per line with a `type` field (snake_case):

- {"type":"al","text":"WRITE POST WITH title={title}"}
- {"type":"tool_doc","tool_id":"Blog.write/2","text":"Writes a blog post"}
- {"type":"tool_spec","tool_id":"Blog.write/2","text":"title: string, body: string"}
- {"type":"param_card","tool_id":"Blog.write/2","text":"title: string (required)"}
- {"type":"slot_card","text":"title={title}"}
- {"type":"example","tool_id":"Blog.write/2","text":"WRITE POST WITH title={title} body={body}"}

The distiller tokenizes each text, runs the teacher, accumulates token embeddings by token ID, and averages them to produce a static token table.

## Registry schema summary (tools.json)

```
{
  "version": 1,
  "tools": [
    {
      "id": "Blog.write/2",
      "module": "Blog",
      "name": "write",
      "arity": 2,
      "doc": "Writes a blog post",
      "spec": "title: string, body: string",
      "args": [
        {"name":"title", "type":"string", "required":true, "aliases":["subject"]},
        {"name":"body",  "type":"string", "required":true}
      ],
      "examples": ["WRITE POST WITH title={title} body={body}"]
    }
  ]
}
```

The CLI will embed a tool card (doc+spec+examples) and per-parameter cards, storing them in `registry.mcr` with f16 embeddings.

## Developer notes

- Formatting: `rustfmt` (config included).
- Linting: `cargo clippy --workspace --all-targets -- -D warnings`.
- Tests: `cargo test --workspace`.
- The PCA module is a stub that truncates dimensions (no eigendecomposition yet).
- The teacher wrapper expects an output named `last_hidden_state` or uses the first output.
- Distillation uses mean pooling and optional SIF (Zipf) weighting.

## Crate structure

- crates/spectre-core: library with AL parser, embedder, similarity, matching, registry, planner.
- crates/spectre-train: library with corpus parsing, ONNX teacher wrapper, distillation, pack writer.
- crates/spectre-cli: `spectre-dispatcher` binary (train, build-registry, plan).

## License

TBD.
