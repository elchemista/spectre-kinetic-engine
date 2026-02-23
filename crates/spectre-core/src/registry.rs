//! Compiled registry (.mcr) load and save.
//!
//! The `.mcr` binary format stores tool metadata alongside precomputed
//! embeddings for both tool-cards and param-cards, enabling fast runtime
//! retrieval without re-embedding registry content.

use crate::embed::StaticEmbedder;
use crate::error::CoreError;
use crate::types::{ArgDef, ToolDef, ToolMeta, ToolRegistry};
use half::f16;
use ndarray::Array2;
use std::io::{Read, Write};
use std::path::Path;

/// Magic bytes for the .mcr file format.
const MCR_MAGIC: &[u8; 4] = b"MCR\x01";

/// A compiled registry with precomputed embeddings ready for runtime use.
pub struct CompiledRegistry {
    /// Tool metadata (ids, args, param ranges).
    pub tools: Vec<ToolMeta>,
    /// Embedding dimension.
    pub dims: usize,
    /// Tokenizer hash (for consistency check with pack).
    pub tokenizer_hash: String,
    /// Tool-card embedding matrix `[tool_count, dims]`.
    pub tool_embeddings: Array2<f32>,
    /// Param-card embedding matrix `[total_params, dims]`.
    pub param_embeddings: Array2<f32>,
    /// Optional precomputed slot-card embeddings.
    pub slot_card_embeddings: Option<Array2<f32>>,
    /// Optional slot-card labels corresponding to rows of `slot_card_embeddings`.
    pub slot_card_labels: Option<Vec<String>>,
}

impl CompiledRegistry {
    /// Load a compiled registry from a `.mcr` binary file.
    pub fn load(path: &Path) -> Result<Self, CoreError> {
        let data =
            std::fs::read(path).map_err(|e| CoreError::RegistryLoad(format!("cannot read {}: {e}", path.display())))?;
        Self::from_bytes(&data)
    }

    /// Save this compiled registry to a `.mcr` binary file.
    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        let bytes = self.to_bytes()?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Deserialize from raw bytes.
    fn from_bytes(data: &[u8]) -> Result<Self, CoreError> {
        let mut cursor = std::io::Cursor::new(data);

        // Read and validate magic
        let mut magic = [0u8; 4];
        cursor
            .read_exact(&mut magic)
            .map_err(|e| CoreError::RegistryLoad(format!("failed to read magic: {e}")))?;
        if &magic != MCR_MAGIC {
            return Err(CoreError::RegistryLoad("invalid .mcr magic bytes".into()));
        }

        let version = read_u32(&mut cursor)?;
        if version != 1 {
            return Err(CoreError::RegistryLoad(format!("unsupported .mcr version: {version}")));
        }

        let dims = read_u32(&mut cursor)? as usize;
        let hash_len = read_u32(&mut cursor)? as usize;
        let tokenizer_hash = read_string(&mut cursor, hash_len)?;
        let tool_count = read_u32(&mut cursor)? as usize;
        let param_count = read_u32(&mut cursor)? as usize;
        let slot_card_count = read_u32(&mut cursor)? as usize;

        // Read tool metadata JSON block
        let meta_len = read_u32(&mut cursor)? as usize;
        let meta_json = read_string(&mut cursor, meta_len)?;
        let tools: Vec<ToolMeta> =
            serde_json::from_str(&meta_json).map_err(|e| CoreError::RegistryLoad(format!("tool metadata JSON: {e}")))?;

        // Read tool embedding matrix (f16)
        let tool_embeddings = read_f16_matrix(&mut cursor, tool_count, dims)?;

        // Read param embedding matrix (f16)
        let param_embeddings = read_f16_matrix(&mut cursor, param_count, dims)?;

        // Read optional slot-card embeddings
        let (slot_card_embeddings, slot_card_labels) = if slot_card_count > 0 {
            let emb = read_f16_matrix(&mut cursor, slot_card_count, dims)?;
            let labels_len = read_u32(&mut cursor)? as usize;
            let labels_json = read_string(&mut cursor, labels_len)?;
            let labels: Vec<String> = serde_json::from_str(&labels_json)
                .map_err(|e| CoreError::RegistryLoad(format!("slot card labels JSON: {e}")))?;
            (Some(emb), Some(labels))
        } else {
            (None, None)
        };

        Ok(Self {
            tools,
            dims,
            tokenizer_hash,
            tool_embeddings,
            param_embeddings,
            slot_card_embeddings,
            slot_card_labels,
        })
    }

    /// Serialize to raw bytes.
    fn to_bytes(&self) -> Result<Vec<u8>, CoreError> {
        let mut buf = Vec::new();

        buf.write_all(MCR_MAGIC)?;
        write_u32(&mut buf, 1)?; // version
        write_u32(&mut buf, self.dims as u32)?;

        let hash_bytes = self.tokenizer_hash.as_bytes();
        write_u32(&mut buf, hash_bytes.len() as u32)?;
        buf.write_all(hash_bytes)?;

        write_u32(&mut buf, self.tools.len() as u32)?;
        write_u32(&mut buf, self.param_embeddings.nrows() as u32)?;

        let slot_count = self.slot_card_embeddings.as_ref().map_or(0, |e| e.nrows());
        write_u32(&mut buf, slot_count as u32)?;

        // Tool metadata JSON
        let meta_json = serde_json::to_string(&self.tools)?;
        write_u32(&mut buf, meta_json.len() as u32)?;
        buf.write_all(meta_json.as_bytes())?;

        // Tool embeddings (f16)
        write_f16_matrix(&mut buf, &self.tool_embeddings)?;

        // Param embeddings (f16)
        write_f16_matrix(&mut buf, &self.param_embeddings)?;

        // Optional slot-card embeddings
        if let (Some(emb), Some(labels)) = (&self.slot_card_embeddings, &self.slot_card_labels) {
            write_f16_matrix(&mut buf, emb)?;
            let labels_json = serde_json::to_string(labels)?;
            write_u32(&mut buf, labels_json.len() as u32)?;
            buf.write_all(labels_json.as_bytes())?;
        }

        Ok(buf)
    }
}

/// Build a compiled registry from a tool registry JSON and a model pack's embedder.
///
/// Generates tool-card and param-card text strings, embeds them via the
/// `StaticEmbedder`, and assembles the result into a `CompiledRegistry`.
pub fn build_registry(
    embedder: &StaticEmbedder,
    registry: &ToolRegistry,
    tokenizer_hash: &str,
) -> Result<CompiledRegistry, CoreError> {
    let dims = embedder.dim();

    // Build tool-card texts and embed them
    let tool_cards: Vec<String> = registry.tools.iter().map(build_tool_card).collect();
    let tool_card_refs: Vec<&str> = tool_cards.iter().map(|s| s.as_str()).collect();
    let tool_vecs = embedder.encode_batch(&tool_card_refs);

    let tool_embeddings = vecs_to_array2(&tool_vecs, dims)?;

    // Build param-card texts and embed them, tracking ranges per tool
    let mut param_cards: Vec<String> = Vec::new();
    let mut tools_meta: Vec<ToolMeta> = Vec::new();

    for tool_def in &registry.tools {
        let param_start = param_cards.len();
        for arg in &tool_def.args {
            param_cards.push(build_param_card(&tool_def.name, arg));
        }
        let param_end = param_cards.len();

        tools_meta.push(ToolMeta {
            id: tool_def.id.clone(),
            module: tool_def.module.clone(),
            name: tool_def.name.clone(),
            arity: tool_def.arity,
            args: tool_def.args.clone(),
            param_range: (param_start, param_end),
        });
    }

    let param_card_refs: Vec<&str> = param_cards.iter().map(|s| s.as_str()).collect();
    let param_vecs = if param_card_refs.is_empty() {
        Vec::new()
    } else {
        embedder.encode_batch(&param_card_refs)
    };

    let param_embeddings = vecs_to_array2(&param_vecs, dims)?;

    Ok(CompiledRegistry {
        tools: tools_meta,
        dims,
        tokenizer_hash: tokenizer_hash.to_string(),
        tool_embeddings,
        param_embeddings,
        slot_card_embeddings: None,
        slot_card_labels: None,
    })
}

/// Build a tool-card text string for embedding.
fn build_tool_card(tool: &ToolDef) -> String {
    let mut card = format!("{}.{}/{}\nDOC: {}\nSPEC: {}", tool.module, tool.name, tool.arity, tool.doc, tool.spec);
    if let Some(example) = tool.examples.first() {
        card.push_str("\nEX: ");
        card.push_str(example);
    }
    card
}

/// Build a param-card text string for embedding.
fn build_param_card(tool_name: &str, arg: &ArgDef) -> String {
    let aliases = if arg.aliases.is_empty() {
        String::new()
    } else {
        format!("\nALIASES {}", arg.aliases.join(","))
    };
    format!(
        "PARAM {}\nTYPE {}{}\nDOC {} argument",
        arg.name, arg.arg_type, aliases, tool_name
    )
}

/// Convert a `Vec<Vec<f32>>` to an `Array2<f32>`.
fn vecs_to_array2(vecs: &[Vec<f32>], dims: usize) -> Result<Array2<f32>, CoreError> {
    if vecs.is_empty() {
        return Ok(Array2::zeros((0, dims)));
    }
    let flat: Vec<f32> = vecs.iter().flat_map(|v| v.iter().copied()).collect();
    Array2::from_shape_vec((vecs.len(), dims), flat)
        .map_err(|e| CoreError::RegistryLoad(format!("failed to build embedding matrix: {e}")))
}

// --- Binary I/O helpers ---

fn read_u32(cursor: &mut std::io::Cursor<&[u8]>) -> Result<u32, CoreError> {
    let mut buf = [0u8; 4];
    cursor
        .read_exact(&mut buf)
        .map_err(|e| CoreError::RegistryLoad(format!("read u32: {e}")))?;
    Ok(u32::from_le_bytes(buf))
}

fn read_string(cursor: &mut std::io::Cursor<&[u8]>, len: usize) -> Result<String, CoreError> {
    let mut buf = vec![0u8; len];
    cursor
        .read_exact(&mut buf)
        .map_err(|e| CoreError::RegistryLoad(format!("read string: {e}")))?;
    String::from_utf8(buf).map_err(|e| CoreError::RegistryLoad(format!("invalid UTF-8: {e}")))
}

fn read_f16_matrix(cursor: &mut std::io::Cursor<&[u8]>, rows: usize, cols: usize) -> Result<Array2<f32>, CoreError> {
    let count = rows * cols;
    let mut raw = vec![0u8; count * 2];
    cursor
        .read_exact(&mut raw)
        .map_err(|e| CoreError::RegistryLoad(format!("read f16 matrix: {e}")))?;

    let floats: Vec<f32> = raw
        .chunks_exact(2)
        .map(|b| f16::from_le_bytes([b[0], b[1]]).to_f32())
        .collect();

    Array2::from_shape_vec((rows, cols), floats)
        .map_err(|e| CoreError::RegistryLoad(format!("f16 matrix shape: {e}")))
}

fn write_u32(buf: &mut Vec<u8>, val: u32) -> Result<(), CoreError> {
    buf.write_all(&val.to_le_bytes())?;
    Ok(())
}

fn write_f16_matrix(buf: &mut Vec<u8>, matrix: &Array2<f32>) -> Result<(), CoreError> {
    for &val in matrix.iter() {
        let h = f16::from_f32(val);
        buf.write_all(&h.to_le_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcr_round_trip_should_preserve_data() {
        let tool_emb = Array2::from_shape_vec((2, 4), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]).unwrap();
        let param_emb = Array2::from_shape_vec((3, 4), (1..=12).map(|x| x as f32).collect()).unwrap();

        let registry = CompiledRegistry {
            tools: vec![
                ToolMeta {
                    id: "Mod.func/1".into(),
                    module: "Mod".into(),
                    name: "func".into(),
                    arity: 1,
                    args: vec![ArgDef {
                        name: "arg1".into(),
                        arg_type: "String.t()".into(),
                        required: true,
                        aliases: vec!["TEXT".into()],
                    }],
                    param_range: (0, 1),
                },
                ToolMeta {
                    id: "Mod.other/2".into(),
                    module: "Mod".into(),
                    name: "other".into(),
                    arity: 2,
                    args: vec![
                        ArgDef {
                            name: "a".into(),
                            arg_type: "integer()".into(),
                            required: true,
                            aliases: Vec::new(),
                        },
                        ArgDef {
                            name: "b".into(),
                            arg_type: "String.t()".into(),
                            required: false,
                            aliases: Vec::new(),
                        },
                    ],
                    param_range: (1, 3),
                },
            ],
            dims: 4,
            tokenizer_hash: "abc123".into(),
            tool_embeddings: tool_emb,
            param_embeddings: param_emb,
            slot_card_embeddings: None,
            slot_card_labels: None,
        };

        let bytes = registry.to_bytes().unwrap();
        let loaded = CompiledRegistry::from_bytes(&bytes).unwrap();

        assert_eq!(loaded.tools.len(), 2);
        assert_eq!(loaded.dims, 4);
        assert_eq!(loaded.tokenizer_hash, "abc123");
        assert_eq!(loaded.tools[0].id, "Mod.func/1");
        assert_eq!(loaded.tools[1].param_range, (1, 3));

        // f16 round-trip introduces small error
        assert!((loaded.tool_embeddings[[0, 0]] - 1.0).abs() < 0.01);
        assert!((loaded.param_embeddings[[2, 3]] - 12.0).abs() < 0.01);
    }

    #[test]
    fn build_tool_card_should_include_doc_spec_example() {
        let tool = ToolDef {
            id: "Mod.create/2".into(),
            module: "Mod".into(),
            name: "create".into(),
            arity: 2,
            doc: "Creates a thing".into(),
            spec: "create(a, b) :: :ok".into(),
            args: Vec::new(),
            examples: vec!["CREATE THING WITH: A={a}".into()],
        };
        let card = build_tool_card(&tool);
        assert!(card.contains("DOC: Creates a thing"));
        assert!(card.contains("SPEC: create(a, b) :: :ok"));
        assert!(card.contains("EX: CREATE THING WITH: A={a}"));
    }
}
