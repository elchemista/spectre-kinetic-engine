//! Deterministic Action Language (AL) text parser.
//!
//! Extracts the action portion and slot keys from an AL string.
//! No ML involved -- pure string parsing per the SPEC:
//! - `WITH:` section splitting
//! - `{slot_name}` placeholder extraction
//! - `KEY=value` parsing from the WITH: section

use crate::types::ParsedSlot;

/// Result of parsing an AL text string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlParsed {
    /// The action portion (text before `WITH:`), with `{placeholder}` names inlined.
    pub action_text: String,
    /// Extracted slot keys in order of appearance.
    pub slot_keys: Vec<ParsedSlot>,
}

/// Parse an AL text string to extract the action text and slot keys.
///
/// # Examples
///
/// ```
/// use spectre_core::al_parser::parse_al;
///
/// let result = parse_al("WRITE POST WITH: TITLE={title} TEXT={text}");
/// assert_eq!(result.action_text, "WRITE POST");
/// assert_eq!(result.slot_keys.len(), 2);
/// assert_eq!(result.slot_keys[0].key, "title");
/// assert_eq!(result.slot_keys[1].key, "text");
/// ```
pub fn parse_al(al_text: &str) -> AlParsed {
    let (action_part, with_part) = split_on_with(al_text);

    let mut slot_keys = Vec::new();

    // Extract {placeholder} slots from the action portion
    let action_text = extract_placeholders(action_part, &mut slot_keys);

    // Parse KEY=value or KEY={value} from the WITH: section
    if let Some(with_section) = with_part {
        parse_with_section(with_section, &mut slot_keys);
    }

    AlParsed { action_text, slot_keys }
}

/// Split on the first occurrence of `WITH:` or `WITH ` (case-insensitive).
/// Returns (before, Some(after)) or (full_text, None).
fn split_on_with(text: &str) -> (&str, Option<&str>) {
    let upper = text.to_uppercase();
    if let Some(pos) = upper.find("WITH:") {
        let before = text[..pos].trim();
        let after = text[pos + 5..].trim();
        (before, Some(after))
    } else if let Some(pos) = upper.find("WITH ") {
        let before = text[..pos].trim();
        let after = text[pos + 5..].trim();
        (before, Some(after))
    } else {
        (text.trim(), None)
    }
}

/// Scan for `{slot_name}` patterns in the action text.
/// Returns a cleaned action string with placeholders replaced by their key names.
fn extract_placeholders(text: &str, slots: &mut Vec<ParsedSlot>) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            let mut name = String::new();
            let mut found_close = false;
            for inner in chars.by_ref() {
                if inner == '}' {
                    found_close = true;
                    break;
                }
                name.push(inner);
            }
            if found_close && !name.is_empty() {
                let key = name.trim().to_lowercase();
                if !slots.iter().any(|s| s.key == key) {
                    slots.push(ParsedSlot {
                        key,
                        placeholder: true,
                    });
                }
                result.push_str(name.trim());
            } else {
                // Malformed placeholder, keep as-is
                result.push('{');
                result.push_str(&name);
            }
        } else {
            result.push(ch);
        }
    }

    result.trim().to_string()
}

/// Parse `KEY=value` or `KEY={slot}` entries from the WITH: section.
fn parse_with_section(section: &str, slots: &mut Vec<ParsedSlot>) {
    for token in section.split_whitespace() {
        if let Some(eq_pos) = token.find('=') {
            let key_raw = &token[..eq_pos];
            let val_raw = &token[eq_pos + 1..];

            let key = key_raw.trim().to_lowercase();
            if key.is_empty() {
                continue;
            }

            let is_placeholder = val_raw.starts_with('{') && val_raw.ends_with('}');
            let slot_key = if is_placeholder {
                val_raw[1..val_raw.len() - 1].trim().to_lowercase()
            } else {
                key.clone()
            };

            if !slots.iter().any(|s| s.key == slot_key) {
                slots.push(ParsedSlot {
                    key: slot_key,
                    placeholder: is_placeholder,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_al_should_split_on_with_section() {
        let result = parse_al("CREATE POST WITH: TITLE={title} TEXT={text}");
        assert_eq!(result.action_text, "CREATE POST");
        assert_eq!(result.slot_keys.len(), 2);
    }

    #[test]
    fn parse_al_should_extract_placeholders_from_action() {
        let result = parse_al("WRITE {title} FOR site.com");
        assert_eq!(result.slot_keys.len(), 1);
        assert_eq!(result.slot_keys[0].key, "title");
        assert!(result.slot_keys[0].placeholder);
        assert_eq!(result.action_text, "WRITE title FOR site.com");
    }

    #[test]
    fn parse_al_should_handle_no_with_section() {
        let result = parse_al("SEND EMAIL TO {recipient}");
        assert_eq!(result.action_text, "SEND EMAIL TO recipient");
        assert_eq!(result.slot_keys.len(), 1);
        assert_eq!(result.slot_keys[0].key, "recipient");
    }

    #[test]
    fn parse_al_should_handle_key_equals_value() {
        let result = parse_al("DO THING WITH: MODE=fast COUNT=10");
        assert_eq!(result.action_text, "DO THING");
        assert_eq!(result.slot_keys.len(), 2);
        assert_eq!(result.slot_keys[0].key, "mode");
        assert_eq!(result.slot_keys[1].key, "count");
    }

    #[test]
    fn parse_al_should_handle_empty_input() {
        let result = parse_al("");
        assert_eq!(result.action_text, "");
        assert!(result.slot_keys.is_empty());
    }

    #[test]
    fn parse_al_should_be_case_insensitive_for_with() {
        let result = parse_al("DO THING with: KEY={val}");
        assert_eq!(result.action_text, "DO THING");
        assert_eq!(result.slot_keys.len(), 1);
    }

    #[test]
    fn parse_al_should_deduplicate_slot_keys() {
        let result = parse_al("USE {title} WITH: TITLE={title}");
        assert_eq!(result.slot_keys.len(), 1);
        assert_eq!(result.slot_keys[0].key, "title");
    }

    #[test]
    fn parse_al_should_handle_spec_example() {
        let result = parse_al("WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE={title} TEXT={text}");
        assert_eq!(result.action_text, "WRITE NEW BLOG POST FOR elchemista.com");
        assert_eq!(result.slot_keys.len(), 2);
        assert_eq!(result.slot_keys[0].key, "title");
        assert_eq!(result.slot_keys[1].key, "text");
    }

    #[test]
    fn parse_al_should_handle_with_without_colon() {
        let result = parse_al("WRITE POST WITH TITLE={title} TEXT={text}");
        assert_eq!(result.action_text, "WRITE POST");
        assert_eq!(result.slot_keys.len(), 2);
        assert_eq!(result.slot_keys[0].key, "title");
        assert_eq!(result.slot_keys[1].key, "text");
    }
}
