//! Spectre Dispatcher CLI.
//!
//! Provides three subcommands:
//! - `train`: Distill a static embedding pack from an ONNX teacher model
//! - `build-registry`: Compile a tool registry JSON into a binary .mcr file
//! - `plan`: Read a plan request from stdin and output a call plan JSON

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;

#[derive(Parser)]
#[command(name = "spectre-dispatcher", version, about = "Spectre Dispatcher - static embedding tool selection")]
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
    let teacher = spectre_train::TeacherModel::load(Path::new(teacher_onnx))
        .context("failed to load teacher model")?;

    eprintln!("Loading tokenizer...");
    let tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path)
        .map_err(|e| anyhow::anyhow!("failed to load tokenizer: {e}"))?;

    eprintln!("Parsing corpus...");
    let corpus = spectre_train::parse_corpus(Path::new(corpus_path))
        .context("failed to parse corpus")?;
    eprintln!("  {} entries loaded", corpus.len());

    let config = spectre_train::DistillConfig {
        max_len,
        dim,
        batch_size: 32,
        apply_zipf,
        sif_coefficient: 1e-4,
    };

    eprintln!("Distilling static embeddings...");
    let result = spectre_train::distill(&teacher, &tokenizer, &corpus, &config)
        .context("distillation failed")?;
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
    let (meta, embedder) = spectre_core::pack::load_pack(Path::new(model_dir))
        .context("failed to load model pack")?;

    eprintln!("Loading tool registry...");
    let registry_json = std::fs::read_to_string(registry_path)
        .context("failed to read registry JSON")?;
    let registry: spectre_core::types::ToolRegistry = serde_json::from_str(&registry_json)
        .context("failed to parse registry JSON")?;
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
    let (_meta, embedder) = spectre_core::pack::load_pack(Path::new(model_dir))
        .context("failed to load model pack")?;
    let compiled = spectre_core::CompiledRegistry::load(Path::new(registry_path))
        .context("failed to load compiled registry")?;

    let dispatcher = spectre_core::SpectreDispatcher::new(embedder, compiled);

    // Read request JSON from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).context("failed to read stdin")?;
    let request: spectre_core::PlanRequest = serde_json::from_str(&input)
        .context("failed to parse request JSON")?;

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
