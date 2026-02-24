//! Spectre Dispatcher CLI.
//!
//! Provides three subcommands:
//! - `train`: Distill a static embedding pack from an ONNX teacher model
//! - `build-registry`: Compile a tool registry JSON into a binary .mcr file
//! - `plan`: Read a plan request from stdin and output a call plan JSON

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;

#[derive(Parser)]
#[command(
    name = "spectre-kinetic",
    version,
    about = "Spectre Kinetic - static embedding tool selection"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Distill a static embedding pack from an ONNX teacher model
    Train {
        /// Path to the teacher ONNX model file
        #[arg(long)]
        teacher_onnx: String,
        /// Path to the HuggingFace tokenizer.json
        #[arg(long)]
        tokenizer: String,
        /// Path to the corpus.jsonl file
        #[arg(long)]
        corpus: String,
        /// Output directory for the pack
        #[arg(long)]
        out: String,
        /// Maximum token length (default: 256)
        #[arg(long, default_value = "256")]
        max_len: usize,
        /// Embedding dimension (default: 384, teacher's native)
        #[arg(long, default_value = "384")]
        dim: usize,
        /// Apply Zipf/SIF weighting
        #[arg(long)]
        zipf: bool,
    },

    /// Compile a tool registry JSON into a binary .mcr file with precomputed embeddings
    BuildRegistry {
        /// Path to the model pack directory
        #[arg(long)]
        model: String,
        /// Path to the tool registry JSON file
        #[arg(long)]
        registry: String,
        /// Output path for the compiled .mcr file
        #[arg(long)]
        out: String,
    },

    /// Read a plan request from stdin and output a call plan JSON to stdout
    Plan {
        /// Path to the model pack directory
        #[arg(long)]
        model: String,
        /// Path to the compiled registry .mcr file
        #[arg(long)]
        registry: String,
        /// Read JSON from stdin (required)
        #[arg(long)]
        stdin_json: bool,
    },

    /// Extract a dictionary file from a corpus JSONL and optional registry
    ExtractDict {
        /// Path to the corpus.jsonl (or combined corpus) file
        #[arg(long)]
        corpus: String,
        /// Optional path to the tool registry JSON file
        #[arg(long)]
        registry: Option<String>,
        /// Optional path to a seed wordlist file (whitespace-separated or one per line)
        #[arg(long)]
        seed: Option<String>,
        /// Output path for the generated DICTIONARY.txt
        #[arg(long, default_value = "DICTIONARY.txt")]
        out: String,
        /// Number of most common words to include from the corpus/registry
        #[arg(long, default_value = "500")]
        top_n: usize,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Commands::Train {
            teacher_onnx,
            tokenizer,
            corpus,
            out,
            max_len,
            dim,
            zipf,
        } => cmd_train(&teacher_onnx, &tokenizer, &corpus, &out, max_len, dim, zipf),
        Commands::BuildRegistry { model, registry, out } => cmd_build_registry(&model, &registry, &out),
        Commands::Plan {
            model,
            registry,
            stdin_json: _,
        } => cmd_plan(&model, &registry),
        Commands::ExtractDict {
            corpus,
            registry,
            seed,
            out,
            top_n,
        } => cmd_extract_dict(&corpus, registry.as_deref(), seed.as_deref(), &out, top_n),
    }
}

/// Train: distill a static embedding pack from an ONNX teacher.
fn cmd_train(
    teacher_onnx: &str,
    tokenizer_path: &str,
    corpus_path: &str,
    out_dir: &str,
    max_len: usize,
    dim: usize,
    apply_zipf: bool,
) -> Result<()> {
    eprintln!("Loading teacher ONNX model...");
    let mut teacher =
        spectre_train::TeacherModel::load(Path::new(teacher_onnx)).context("failed to load teacher model")?;

    eprintln!("Teacher dim: {}", teacher.dim());

    eprintln!("Parsing corpus...");
    let corpus = spectre_train::parse_corpus(Path::new(corpus_path)).context("failed to load corpus")?;

    eprintln!("Corpus entries: {}", corpus.len());

    let tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path)
        .map_err(|e| anyhow::anyhow!("failed to load tokenizer: {e}"))?;

    let config = spectre_train::DistillConfig {
        max_len,
        dim,
        apply_zipf: apply_zipf,
        ..Default::default()
    };

    eprintln!("Distilling embeddings...");
    let result = spectre_train::distill(&mut teacher, &tokenizer, &corpus, &config).context("distillation failed")?;
    eprintln!("  vocab_size={}, dim={}", result.vocab_size, result.dim);

    // Compute a simple tokenizer hash for the pack metadata
    let tok_hash = format!("{:x}", simple_hash(tokenizer_path));

    let metadata = spectre_core::types::PackMetadata {
        teacher_id: teacher_onnx.to_string(),
        dim: result.dim,
        pooling: "mean".to_string(),
        tokenizer_hash: tok_hash,
        max_len,
        apply_pca: None,
        apply_zipf: if apply_zipf { Some(true) } else { None },
    };

    eprintln!("Writing pack to {out_dir}...");
    spectre_train::write_pack(Path::new(out_dir), &metadata, Path::new(tokenizer_path), &result)
        .context("failed to write pack")?;

    eprintln!("Done.");
    Ok(())
}

/// Build a compiled registry with precomputed embeddings.
fn cmd_build_registry(model_dir: &str, registry_path: &str, out_path: &str) -> Result<()> {
    eprintln!("Loading model pack...");
    let (meta, embedder) = spectre_core::pack::load_pack(Path::new(model_dir)).context("failed to load model pack")?;

    eprintln!("Loading tool registry...");
    let registry_json = std::fs::read_to_string(registry_path).context("failed to read registry JSON")?;
    let registry: spectre_core::types::ToolRegistry =
        serde_json::from_str(&registry_json).context("failed to parse registry JSON")?;
    eprintln!("  {} tools loaded", registry.tools.len());

    eprintln!("Building compiled registry...");
    let compiled = spectre_core::registry::build_registry(&embedder, &registry, &meta.tokenizer_hash)
        .context("failed to build registry")?;

    eprintln!("Saving to {out_path}...");
    compiled.save(Path::new(out_path)).context("failed to save .mcr")?;

    eprintln!("Done.");
    Ok(())
}

/// Plan: read request JSON from stdin, output plan JSON to stdout.
fn cmd_plan(model_dir: &str, registry_path: &str) -> Result<()> {
    // Load model and registry
    let (_meta, embedder) = spectre_core::pack::load_pack(Path::new(model_dir)).context("failed to load model pack")?;
    let compiled =
        spectre_core::CompiledRegistry::load(Path::new(registry_path)).context("failed to load compiled registry")?;

    let dispatcher = spectre_core::SpectreDispatcher::new(embedder, compiled);

    // Read request JSON from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).context("failed to read stdin")?;
    let request: spectre_core::PlanRequest = serde_json::from_str(&input).context("failed to parse request JSON")?;

    // Execute plan
    let plan = dispatcher.plan(&request);

    // Write result to stdout
    let stdout = io::stdout();
    let writer = BufWriter::new(stdout.lock());
    serde_json::to_writer_pretty(writer, &plan).context("failed to write plan JSON")?;
    io::stdout().lock().write_all(b"\n")?;

    Ok(())
}

/// Simple non-cryptographic hash for generating tokenizer_hash values.
fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for b in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

// ---------------------------------------------------------------------------
// extract-dict implementation
// ---------------------------------------------------------------------------

fn cmd_extract_dict(
    corpus_path: &str,
    registry_path: Option<&str>,
    seed_path: Option<&str>,
    out_path: &str,
    top_n: usize,
) -> Result<()> {
    eprintln!("Parsing corpus...");
    let entries = spectre_train::parse_corpus(Path::new(corpus_path)).context("failed to load corpus")?;

    let mut upper_counts: HashMap<String, usize> = HashMap::new();
    let mut slot_keys: HashSet<String> = HashSet::new();
    let mut special_upper: HashSet<String> = HashSet::new();
    let mut examples: Vec<String> = Vec::new();
    let mut examples_seen: HashSet<String> = HashSet::new();

    // Seed with user-provided dictionary tokens (optional)
    if let Some(path) = seed_path {
        let seed = std::fs::read_to_string(path).with_context(|| format!("failed to read seed file {}", path))?;
        for tok in split_tokens(&seed).into_iter().map(|t| t.to_uppercase()) {
            if tok.len() >= 2 {
                special_upper.insert(tok);
            }
        }
    }

    // From corpus
    for entry in entries.iter() {
        match entry {
            spectre_train::CorpusEntry::Al { text }
            | spectre_train::CorpusEntry::ToolDoc { text, .. }
            | spectre_train::CorpusEntry::ToolSpec { text, .. }
            | spectre_train::CorpusEntry::ParamCard { text, .. }
            | spectre_train::CorpusEntry::SlotCard { text }
            | spectre_train::CorpusEntry::Example { text, .. } => {
                for w in split_tokens(text) {
                    let uw = w.to_uppercase();
                    if uw.len() >= 2 && uw.chars().any(|c| c.is_ascii_alphabetic()) {
                        *upper_counts.entry(uw).or_insert(0) += 1;
                    }
                }
            }
        }

        if let spectre_train::CorpusEntry::Al { text } = entry {
            let parsed = spectre_core::al_parser::parse_al(text);
            for s in parsed.slot_keys {
                if !s.key.is_empty() {
                    slot_keys.insert(s.key);
                }
            }
            let ex = text.trim();
            if !ex.is_empty() && examples_seen.insert(ex.to_string()) {
                examples.push(ex.to_string());
            }
        }
        if let spectre_train::CorpusEntry::SlotCard { text } = entry {
            let parsed = spectre_core::al_parser::parse_al(text);
            for s in parsed.slot_keys {
                if !s.key.is_empty() {
                    slot_keys.insert(s.key);
                }
            }
        }
        if let spectre_train::CorpusEntry::Example { text, .. } = entry {
            let parsed = spectre_core::al_parser::parse_al(text);
            for s in parsed.slot_keys {
                if !s.key.is_empty() {
                    slot_keys.insert(s.key);
                }
            }
            let ex = text.trim();
            if !ex.is_empty() && examples_seen.insert(ex.to_string()) {
                examples.push(ex.to_string());
            }
        }
    }

    // From registry (optional)
    if let Some(registry_path) = registry_path {
        eprintln!("Loading registry JSON...");
        let registry_json = std::fs::read_to_string(registry_path).context("failed to read registry JSON")?;
        let registry: spectre_core::types::ToolRegistry =
            serde_json::from_str(&registry_json).context("failed to parse registry JSON")?;

        for tool in registry.tools.iter() {
            for part in split_tokens(&tool.module) {
                let uw = part.to_uppercase();
                if uw.len() >= 2 {
                    *upper_counts.entry(uw).or_insert(0) += 1;
                }
            }
            for part in split_tokens(&tool.name) {
                let uw = part.to_uppercase();
                if uw.len() >= 2 {
                    *upper_counts.entry(uw).or_insert(0) += 1;
                }
            }

            for arg in tool.args.iter() {
                if !arg.name.is_empty() {
                    slot_keys.insert(arg.name.to_lowercase());
                }
                for al in arg.aliases.iter() {
                    if !al.is_empty() {
                        slot_keys.insert(al.to_lowercase());
                    }
                }
            }

            for ex in tool.examples.iter() {
                for w in split_tokens(ex) {
                    let uw = w.to_uppercase();
                    if uw.len() >= 2 && uw.chars().any(|c| c.is_ascii_alphabetic()) {
                        *upper_counts.entry(uw).or_insert(0) += 1;
                    }
                }

                let parsed = spectre_core::al_parser::parse_al(ex);
                for s in parsed.slot_keys {
                    if !s.key.is_empty() {
                        slot_keys.insert(s.key);
                    }
                }
                let ex_clean = ex.trim();
                if !ex_clean.is_empty() && examples_seen.insert(ex_clean.to_string()) {
                    examples.push(ex_clean.to_string());
                }
            }

            for w in split_tokens(&tool.doc) {
                let uw = w.to_uppercase();
                if uw.len() >= 2 && uw.chars().any(|c| c.is_ascii_alphabetic()) {
                    *upper_counts.entry(uw).or_insert(0) += 1;
                }
            }
            for w in split_tokens(&tool.spec) {
                let uw = w.to_uppercase();
                if uw.len() >= 2 && uw.chars().any(|c| c.is_ascii_alphabetic()) {
                    *upper_counts.entry(uw).or_insert(0) += 1;
                }
            }
        }
    }

    // Merge: always include special AL dictionary tokens first
    let mut final_upper: Vec<String> = special_upper.into_iter().collect();
    final_upper.sort();

    // Top-N common words (excluding ones already included)
    let mut freq: Vec<(String, usize)> = upper_counts.into_iter().collect();
    freq.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut seen_upper: HashSet<String> = final_upper.iter().cloned().collect();
    for (w, _) in freq.into_iter() {
        if seen_upper.contains(&w) {
            continue;
        }
        if w.len() < 2 {
            continue;
        }
        if !w.chars().any(|c| c.is_ascii_alphabetic()) {
            continue;
        }
        final_upper.push(w.clone());
        seen_upper.insert(w);
        if final_upper.len() >= top_n {
            break;
        }
    }

    // Slot keys (lowercase, sorted)
    let mut final_slots: Vec<String> = slot_keys.into_iter().collect();
    final_slots.sort();

    // Write DICTIONARY.txt as 3 compact lines
    eprintln!("Writing {}...", out_path);
    let line1 = final_upper.join(" ");
    let line2 = final_slots.join(" ");
    let line3 = if examples.is_empty() {
        String::new()
    } else {
        examples.join(" | ")
    };
    let out = if line3.is_empty() {
        format!("{}\n{}\n", line1, line2)
    } else {
        format!("{}\n{}\n{}\n", line1, line2, line3)
    };
    std::fs::write(out_path, out).with_context(|| format!("failed to write {}", out_path))?;

    eprintln!("Done.");
    Ok(())
}

fn split_tokens(text: &str) -> Vec<String> {
    let mut toks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        let is_ok = ch.is_ascii_alphanumeric() || ch == '_' || ch == '-';
        if is_ok {
            cur.push(ch);
        } else if !cur.is_empty() {
            toks.push(cur.clone());
            cur.clear();
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks
}
