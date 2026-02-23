//! Corpus JSONL parsing.
//!
//! Parses the `corpus.jsonl` file used for distillation training.
//! Each line is a JSON object with a `type` field that determines the variant.

use crate::error::TrainError;
use serde::Deserialize;
use std::io::BufRead;
use std::path::Path;

/// A single entry from the distillation corpus.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CorpusEntry {
    /// An Action Language sentence.
    Al {
        /// The AL text to embed.
        text: String,
    },
    /// A tool documentation string.
    ToolDoc {
        /// The tool this doc belongs to.
        tool_id: String,
        /// The documentation text.
        text: String,
    },
    /// A tool type specification string.
    ToolSpec {
        /// The tool this spec belongs to.
        tool_id: String,
        /// The spec text.
        text: String,
    },
    /// A parameter card text.
    ParamCard {
        /// The tool this param belongs to.
        tool_id: String,
        /// The param card text.
        text: String,
    },
    /// A slot card text.
    SlotCard {
        /// The slot card text.
        text: String,
    },
    /// An example AL sentence for a specific tool.
    Example {
        /// The tool this example demonstrates.
        tool_id: String,
        /// The example text.
        text: String,
    },
}

impl CorpusEntry {
    /// Get the text content of this entry, regardless of variant.
    pub fn text(&self) -> &str {
        match self {
            Self::Al { text }
            | Self::ToolDoc { text, .. }
            | Self::ToolSpec { text, .. }
            | Self::ParamCard { text, .. }
            | Self::SlotCard { text }
            | Self::Example { text, .. } => text,
        }
    }
}

/// Parse a corpus JSONL file, returning all valid entries.
///
/// Each line is independently parsed; invalid lines produce errors
/// with the 1-based line number.
pub fn parse_corpus(path: &Path) -> Result<Vec<CorpusEntry>, TrainError> {
    let file = std::fs::File::open(path).map_err(|e| TrainError::Corpus {
        line: 0,
        message: format!("cannot open {}: {e}", path.display()),
    })?;
    let reader = std::io::BufReader::new(file);

    let mut entries = Vec::new();
    for (idx, line_result) in reader.lines().enumerate() {
        let line = line_result.map_err(|e| TrainError::Corpus {
            line: idx + 1,
            message: format!("read error: {e}"),
        })?;

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let entry: CorpusEntry = serde_json::from_str(trimmed).map_err(|e| TrainError::Corpus {
            line: idx + 1,
            message: format!("JSON parse error: {e}"),
        })?;

        entries.push(entry);
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_corpus_should_parse_valid_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corpus.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"al","text":"WRITE POST"}}"#).unwrap();
        writeln!(f, r#"{{"type":"tool_doc","tool_id":"Mod.f/1","text":"Does stuff"}}"#).unwrap();
        writeln!(f, r#"{{"type":"param_card","tool_id":"Mod.f/1","text":"PARAM body"}}"#).unwrap();
        drop(f);

        let entries = parse_corpus(&path).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].text(), "WRITE POST");
        assert_eq!(entries[1].text(), "Does stuff");
    }

    #[test]
    fn parse_corpus_should_skip_empty_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corpus.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"al","text":"hello"}}"#).unwrap();
        writeln!(f).unwrap();
        writeln!(f, r#"{{"type":"al","text":"world"}}"#).unwrap();
        drop(f);

        let entries = parse_corpus(&path).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn parse_corpus_should_report_line_number_on_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corpus.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"al","text":"ok"}}"#).unwrap();
        writeln!(f, "not json at all").unwrap();
        drop(f);

        let err = parse_corpus(&path).unwrap_err();
        match err {
            TrainError::Corpus { line, .. } => assert_eq!(line, 2),
            _ => panic!("expected Corpus error"),
        }
    }
}
