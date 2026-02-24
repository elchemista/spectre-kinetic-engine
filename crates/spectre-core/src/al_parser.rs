//! Deterministic Action Language (AL) text parser.
//!
//! Extracts the action portion and slot keys from an AL string.
//! No ML involved -- pure string parsing per the SPEC:
//! - `WITH:` section splitting
//! - `{slot_name}` placeholder extraction
//! - `KEY=value` parsing from the WITH: section
//!
//! The parser is **case-insensitive** and **punctuation-tolerant**: it normalizes
//! the input before parsing so that `write post with: title='hello'` works just
//! as well as `WRITE POST WITH: TITLE="hello"`.

use crate::types::ParsedSlot;
use std::collections::HashMap;

/// Result of parsing an AL text string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlParsed {
    /// The action portion (text before `WITH:`), with `{placeholder}` names inlined.
    pub action_text: String,
    /// Extracted slot keys in order of appearance.
    pub slot_keys: Vec<ParsedSlot>,
}

/// Normalize an AL input string before parsing.
///
/// - Converts single quotes (`'`) used as value delimiters to double quotes (`"`)
/// - Strips trailing punctuation noise (`;`, `,`, `.`) from the overall string
/// - Collapses multiple spaces into one
///
/// The strategy for single-to-double quote conversion: we only convert `'` when
/// it appears to delimit a value in a `KEY='value'` or `KEY = 'value'` context,
/// or when it appears after `=` at the start of a value. We do this by scanning
/// for balanced pairs of `'` that contain at least one non-quote character.
fn normalize_al_input(input: &str) -> String {
    let trimmed = input.trim();
    // Strip trailing punctuation noise (;,.)
    let trimmed = trimmed.trim_end_matches([';', ',', '.']);
    let trimmed = trimmed.trim();

    // Convert single-quoted values to double-quoted values.
    // We look for patterns like: ='value' or = 'value'
    let mut result = String::with_capacity(trimmed.len());
    let chars: Vec<char> = trimmed.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '\'' {
            // Check if this looks like a value delimiter
            // (preceded by `=` possibly with whitespace, or start of a quoted value)
            let last_significant = result.trim_end().chars().last();
            if last_significant == Some('=') || is_after_equals_whitespace(&result) {
                // Convert to double quote, find the matching closing single quote
                result.push('"');
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    result.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() && chars[i] == '\'' {
                    result.push('"');
                    i += 1;
                } else {
                    // Unmatched single quote, close with double
                    result.push('"');
                }
                continue;
            }
            // Otherwise keep the single quote as-is (it might be an apostrophe)
            result.push(chars[i]);
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }

    // Collapse multiple spaces
    let mut collapsed = String::with_capacity(result.len());
    let mut prev_space = false;
    for ch in result.chars() {
        if ch.is_ascii_whitespace() {
            if !prev_space {
                collapsed.push(' ');
            }
            prev_space = true;
        } else {
            collapsed.push(ch);
            prev_space = false;
        }
    }

    collapsed.trim().to_string()
}

/// Check if the string ends with `= ` (equals followed by optional whitespace).
fn is_after_equals_whitespace(s: &str) -> bool {
    let trimmed = s.trim_end();
    trimmed.ends_with('=')
}

/// Strip stray punctuation from a token that isn't inside quotes or braces.
/// Removes leading/trailing `;`, `:`, `,`, `.` but preserves internal ones.
fn strip_token_punctuation(token: &str) -> &str {
    // Preserve ':' for URLs like https://example.com, but strip trailing ':' on regular tokens.
    let t = if token.contains("://") {
        token.trim_end_matches([';', ',', '.'])
    } else {
        token.trim_end_matches([';', ',', '.', ':'])
    };
    t.trim_start_matches([';', ',', ':'])
}

/// Parse an AL text string to extract the action text and slot keys.
///
/// The parser is case-insensitive and punctuation-tolerant. Both `WITH:` and
/// `with` (or any mixed case) are recognized. Stray `;`, `,`, `.` around
/// tokens are stripped. Single-quoted values are treated like double-quoted.
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
    let normalized = normalize_al_input(al_text);
    let (action_part, with_part) = split_on_with(&normalized);

    let mut slot_keys = Vec::new();

    // Extract {placeholder} slots from the action portion
    let action_text = extract_placeholders(action_part, &mut slot_keys);

    // Parse KEY=value or KEY={value} from the WITH: section
    if let Some(with_section) = with_part {
        parse_with_section(with_section, &mut slot_keys);
    }

    AlParsed { action_text, slot_keys }
}

/// Parse AL and also extract literal KEY=value pairs (including quoted values) from the WITH section.
/// Returns the usual AlParsed and a lowercase key -> value map for any literal assignments found.
///
/// The parser is case-insensitive and punctuation-tolerant.
pub fn parse_al_and_slots(al_text: &str) -> (AlParsed, HashMap<String, String>) {
    let normalized = normalize_al_input(al_text);
    let (action_part, with_part) = split_on_with(&normalized);

    let mut slot_keys = Vec::new();
    let action_text = extract_placeholders(action_part, &mut slot_keys);

    let mut kv = HashMap::new();
    if let Some(with_section) = with_part {
        parse_with_section_values(with_section, &mut slot_keys, &mut kv);
    }

    (AlParsed { action_text, slot_keys }, kv)
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

/// Parse KEY=value entries from the WITH: section, supporting quoted values with spaces and placeholders.
fn parse_with_section_values(section: &str, slots: &mut Vec<ParsedSlot>, kv_out: &mut HashMap<String, String>) {
    let bytes = section.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // skip whitespace
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        // parse key until '='
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        // allow whitespace before '='
        let key_end = i;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            // Not a key=value, skip token
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            continue;
        }
        // consume '='
        i += 1;

        let key_raw = std::str::from_utf8(&bytes[key_start..key_end]).unwrap_or("").trim();
        let key = strip_token_punctuation(key_raw).to_lowercase();
        if key.is_empty() {
            continue;
        }

        // skip whitespace before value
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        // parse value (double-quoted, single-quoted, placeholder, or bare token)
        let (value, is_placeholder) = if bytes[i] == b'"' || bytes[i] == b'\'' {
            // quoted string (double or single quotes)
            let quote_char = bytes[i];
            i += 1; // skip opening quote
            let val_start = i;
            while i < bytes.len() && bytes[i] != quote_char {
                i += 1;
            }
            let val_end = i.min(bytes.len());
            let val = std::str::from_utf8(&bytes[val_start..val_end])
                .unwrap_or("")
                .to_string();
            if i < bytes.len() && bytes[i] == quote_char {
                i += 1;
            }
            (val, false)
        } else if bytes[i] == b'{' {
            // placeholder {slot}
            i += 1; // skip '{'
            let val_start = i;
            while i < bytes.len() && bytes[i] != b'}' {
                i += 1;
            }
            let val_end = i.min(bytes.len());
            let slot_key = std::str::from_utf8(&bytes[val_start..val_end])
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if i < bytes.len() && bytes[i] == b'}' {
                i += 1;
            }
            (slot_key, true)
        } else {
            // unquoted token until whitespace, strip trailing punctuation
            let val_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            let val_end = i.min(bytes.len());
            let val = std::str::from_utf8(&bytes[val_start..val_end]).unwrap_or("");
            let val = strip_token_punctuation(val).to_string();
            (val, false)
        };

        // record slot key
        let slot_key = if is_placeholder { value.clone() } else { key.clone() };
        if !slots.iter().any(|s| s.key == slot_key) {
            slots.push(ParsedSlot {
                key: slot_key,
                placeholder: is_placeholder,
            });
        }
        // record value only for literal (non-placeholder)
        if !is_placeholder {
            kv_out.insert(key, value);
        }
        // Skip trailing punctuation noise (;,.) after the value
        while i < bytes.len() && (bytes[i] == b';' || bytes[i] == b',' || bytes[i] == b'.') {
            i += 1;
        }
        // continue loop (i already past end of value + punctuation)
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
                    slots.push(ParsedSlot { key, placeholder: true });
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
/// Punctuation-tolerant: strips stray `;`, `,`, `.` from keys/values.
fn parse_with_section(section: &str, slots: &mut Vec<ParsedSlot>) {
    for token in section.split_whitespace() {
        let token = strip_token_punctuation(token);
        if let Some(eq_pos) = token.find('=') {
            let key_raw = &token[..eq_pos];
            let val_raw = &token[eq_pos + 1..];
            // Strip quotes from value if present
            let val_raw = strip_quotes(val_raw);

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

/// Strip surrounding double or single quotes from a value string.
fn strip_quotes(s: &str) -> &str {
    if s.len() >= 2 && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))) {
        return &s[1..s.len() - 1];
    }
    s
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

    #[test]
    fn parse_al_and_slots_should_extract_quoted_values() {
        let (parsed, kv) = parse_al_and_slots(
            "WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE=\"New Day\" TEXT=\"Today i want to speak about ...\"",
        );
        assert_eq!(parsed.action_text, "WRITE NEW BLOG POST FOR elchemista.com");
        assert!(parsed.slot_keys.iter().any(|s| s.key == "title" && !s.placeholder));
        assert!(parsed.slot_keys.iter().any(|s| s.key == "text" && !s.placeholder));
        assert_eq!(kv.get("title").map(|s| s.as_str()), Some("New Day"));
        assert_eq!(
            kv.get("text").map(|s| s.as_str()),
            Some("Today i want to speak about ...")
        );
    }

    #[test]
    fn parse_al_and_slots_should_extract_unquoted_and_placeholders() {
        let (parsed, kv) = parse_al_and_slots("DO THING WITH: MODE=fast COUNT=10 KEY={slot}");
        // action text preserved
        assert_eq!(parsed.action_text, "DO THING");
        // slots include mode/count as literals and slot as placeholder
        assert!(parsed.slot_keys.iter().any(|s| s.key == "mode" && !s.placeholder));
        assert!(parsed.slot_keys.iter().any(|s| s.key == "count" && !s.placeholder));
        assert!(parsed.slot_keys.iter().any(|s| s.key == "slot" && s.placeholder));
        // kv contains only literal assignments
        assert_eq!(kv.get("mode").map(|s| s.as_str()), Some("fast"));
        assert_eq!(kv.get("count").map(|s| s.as_str()), Some("10"));
        assert!(!kv.contains_key("slot"));
    }

    // -----------------------------------------------------------------------
    // Case-insensitivity tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_al_lowercase_input() {
        let result = parse_al("write new blog post with: title={title} text={text}");
        assert_eq!(result.action_text, "write new blog post");
        assert_eq!(result.slot_keys.len(), 2);
        assert_eq!(result.slot_keys[0].key, "title");
        assert_eq!(result.slot_keys[1].key, "text");
    }

    #[test]
    fn parse_al_mixed_case_input() {
        let result = parse_al("Write New Blog Post With: Title={title} Text={text}");
        assert_eq!(result.action_text, "Write New Blog Post");
        assert_eq!(result.slot_keys.len(), 2);
        assert_eq!(result.slot_keys[0].key, "title");
    }

    #[test]
    fn parse_al_mixed_case_with_keyword() {
        let result = parse_al("CREATE POST WiTh: TITLE={t}");
        assert_eq!(result.action_text, "CREATE POST");
        assert_eq!(result.slot_keys.len(), 1);
    }

    #[test]
    fn parse_al_uppercase_slot_keys_normalized() {
        let result = parse_al("DO THING WITH: TITLE={TITLE} TEXT={TEXT}");
        assert_eq!(result.slot_keys[0].key, "title");
        assert_eq!(result.slot_keys[1].key, "text");
    }

    // -----------------------------------------------------------------------
    // Punctuation tolerance tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_al_trailing_semicolon() {
        let result = parse_al("WRITE POST WITH: TITLE={title} TEXT={text};");
        assert_eq!(result.action_text, "WRITE POST");
        assert_eq!(result.slot_keys.len(), 2);
    }

    #[test]
    fn parse_al_trailing_comma() {
        let result = parse_al("WRITE POST WITH: TITLE={title}, TEXT={text},");
        assert_eq!(result.action_text, "WRITE POST");
        assert_eq!(result.slot_keys.len(), 2);
    }

    #[test]
    fn parse_al_values_with_trailing_punctuation() {
        let (parsed, kv) = parse_al_and_slots("DO THING WITH: MODE=fast; COUNT=10,");
        assert_eq!(parsed.action_text, "DO THING");
        assert_eq!(kv.get("mode").map(|s| s.as_str()), Some("fast"));
        assert_eq!(kv.get("count").map(|s| s.as_str()), Some("10"));
    }

    #[test]
    fn parse_al_values_with_colon_punctuation() {
        let (parsed, kv) = parse_al_and_slots("DO THING WITH: MODE=fast: COUNT=10:");
        assert_eq!(parsed.action_text, "DO THING");
        assert_eq!(kv.get("mode").map(|s| s.as_str()), Some("fast"));
        assert_eq!(kv.get("count").map(|s| s.as_str()), Some("10"));
    }

    // -----------------------------------------------------------------------
    // Single-quote support tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_al_and_slots_single_quoted_values() {
        let (parsed, kv) = parse_al_and_slots("WRITE POST WITH: TITLE='My Title' TEXT='Hello world'");
        assert_eq!(parsed.action_text, "WRITE POST");
        assert_eq!(kv.get("title").map(|s| s.as_str()), Some("My Title"));
        assert_eq!(kv.get("text").map(|s| s.as_str()), Some("Hello world"));
    }

    #[test]
    fn parse_al_and_slots_mixed_quotes() {
        let (parsed, kv) = parse_al_and_slots("WRITE POST WITH: TITLE=\"Double\" TEXT='Single'");
        assert_eq!(parsed.action_text, "WRITE POST");
        assert_eq!(kv.get("title").map(|s| s.as_str()), Some("Double"));
        assert_eq!(kv.get("text").map(|s| s.as_str()), Some("Single"));
    }

    // -----------------------------------------------------------------------
    // Example-based tests (from example/corpus.jsonl and example/tools.json)
    // -----------------------------------------------------------------------

    #[test]
    fn example_blog_post_with_placeholders() {
        let result = parse_al("WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE={title} TEXT={text}");
        assert_eq!(result.action_text, "WRITE NEW BLOG POST FOR elchemista.com");
        assert_eq!(result.slot_keys.len(), 2);
        assert!(result.slot_keys.iter().any(|s| s.key == "title" && s.placeholder));
        assert!(result.slot_keys.iter().any(|s| s.key == "text" && s.placeholder));
    }

    #[test]
    fn example_blog_post_with_literal_values() {
        let (parsed, kv) = parse_al_and_slots(
            "WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE=\"My First Post\" TEXT=\"Hello everyone!\"",
        );
        assert_eq!(parsed.action_text, "WRITE NEW BLOG POST FOR elchemista.com");
        assert_eq!(kv.get("title").map(|s| s.as_str()), Some("My First Post"));
        assert_eq!(kv.get("text").map(|s| s.as_str()), Some("Hello everyone!"));
    }

    #[test]
    fn example_stripe_payment_link() {
        let result =
            parse_al("CREATE STRIPE PAYMENT LINK WITH: AMOUNT={amount} CURRENCY={currency} PRODUCT_NAME={name}");
        assert_eq!(result.action_text, "CREATE STRIPE PAYMENT LINK");
        assert_eq!(result.slot_keys.len(), 3);
        assert!(result.slot_keys.iter().any(|s| s.key == "amount"));
        assert!(result.slot_keys.iter().any(|s| s.key == "currency"));
        assert!(result.slot_keys.iter().any(|s| s.key == "name"));
    }

    #[test]
    fn example_stripe_with_values() {
        let (parsed, kv) = parse_al_and_slots(
            "CREATE STRIPE PAYMENT LINK WITH: AMOUNT=5000 CURRENCY=usd PRODUCT_NAME=\"Premium Plan\"",
        );
        assert_eq!(parsed.action_text, "CREATE STRIPE PAYMENT LINK");
        assert_eq!(kv.get("amount").map(|s| s.as_str()), Some("5000"));
        assert_eq!(kv.get("currency").map(|s| s.as_str()), Some("usd"));
        assert_eq!(kv.get("product_name").map(|s| s.as_str()), Some("Premium Plan"));
    }

    // -----------------------------------------------------------------------
    // Linux CLI corpus examples
    // -----------------------------------------------------------------------

    #[test]
    fn example_install_package_via_apt() {
        let result = parse_al("INSTALL PACKAGE {package} VIA APT");
        assert_eq!(result.action_text, "INSTALL PACKAGE package VIA APT");
        assert_eq!(result.slot_keys.len(), 1);
        assert_eq!(result.slot_keys[0].key, "package");
    }

    #[test]
    fn example_list_directory() {
        let result = parse_al("LIST DIRECTORY {path}");
        assert_eq!(result.action_text, "LIST DIRECTORY path");
        assert_eq!(result.slot_keys.len(), 1);
        assert_eq!(result.slot_keys[0].key, "path");
    }

    #[test]
    fn example_delete_file() {
        let result = parse_al("DELETE FILE {path}");
        assert_eq!(result.action_text, "DELETE FILE path");
        assert_eq!(result.slot_keys.len(), 1);
        assert_eq!(result.slot_keys[0].key, "path");
    }

    #[test]
    fn example_compress_directory() {
        let result = parse_al("COMPRESS DIRECTORY /var/log INTO FILE logs.tar.gz");
        assert_eq!(result.action_text, "COMPRESS DIRECTORY /var/log INTO FILE logs.tar.gz");
        assert!(result.slot_keys.is_empty());
    }

    #[test]
    fn example_install_package_lowercase() {
        let result = parse_al("install package {package} via apt");
        assert_eq!(result.action_text, "install package package via apt");
        assert_eq!(result.slot_keys[0].key, "package");
    }

    // -----------------------------------------------------------------------
    // API calling examples
    // -----------------------------------------------------------------------

    #[test]
    fn example_api_call_with_values() {
        let (parsed, kv) = parse_al_and_slots("CALL API WITH: URL=\"https://api.example.com/users\" METHOD=GET");
        assert_eq!(parsed.action_text, "CALL API");
        assert_eq!(kv.get("url").map(|s| s.as_str()), Some("https://api.example.com/users"));
        assert_eq!(kv.get("method").map(|s| s.as_str()), Some("GET"));
    }

    #[test]
    fn example_send_webhook() {
        let (parsed, kv) = parse_al_and_slots(
            "SEND WEBHOOK WITH: URL='https://hooks.slack.com/services/xxx' PAYLOAD='{\"text\":\"hello\"}'",
        );
        assert_eq!(parsed.action_text, "SEND WEBHOOK");
        assert!(kv.contains_key("url"));
        assert!(kv.contains_key("payload"));
    }

    #[test]
    fn example_create_github_issue() {
        let (parsed, kv) = parse_al_and_slots(
            "CREATE GITHUB ISSUE WITH: REPO=\"my-org/my-repo\" TITLE=\"Bug report\" BODY=\"Found a bug\"",
        );
        assert_eq!(parsed.action_text, "CREATE GITHUB ISSUE");
        assert_eq!(kv.get("repo").map(|s| s.as_str()), Some("my-org/my-repo"));
        assert_eq!(kv.get("title").map(|s| s.as_str()), Some("Bug report"));
        assert_eq!(kv.get("body").map(|s| s.as_str()), Some("Found a bug"));
    }

    // -----------------------------------------------------------------------
    // Edge cases and robustness
    // -----------------------------------------------------------------------

    #[test]
    fn parse_al_extra_whitespace() {
        let result = parse_al("  WRITE   POST   WITH:   TITLE={title}   TEXT={text}  ");
        assert_eq!(result.action_text, "WRITE POST");
        assert_eq!(result.slot_keys.len(), 2);
    }

    #[test]
    fn parse_al_only_action_no_slots() {
        let result = parse_al("RESTART SERVER");
        assert_eq!(result.action_text, "RESTART SERVER");
        assert!(result.slot_keys.is_empty());
    }

    #[test]
    fn parse_al_multiple_placeholders_in_action() {
        let result = parse_al("COPY {source} TO {destination}");
        assert_eq!(result.action_text, "COPY source TO destination");
        assert_eq!(result.slot_keys.len(), 2);
        assert_eq!(result.slot_keys[0].key, "source");
        assert_eq!(result.slot_keys[1].key, "destination");
    }

    #[test]
    fn parse_al_and_slots_space_around_equals() {
        // The normalization handles extra spaces; the parser should still work
        let (parsed, kv) = parse_al_and_slots("DO THING WITH: KEY = value");
        assert_eq!(parsed.action_text, "DO THING");
        // The key parsing tolerates whitespace around '='
        assert!(kv.contains_key("key") || parsed.slot_keys.iter().any(|s| s.key == "key"));
    }

    #[test]
    fn parse_al_with_colon_in_url_value() {
        let (parsed, kv) = parse_al_and_slots("FETCH DATA WITH: URL=\"https://api.example.com:8080/data\"");
        assert_eq!(parsed.action_text, "FETCH DATA");
        assert_eq!(
            kv.get("url").map(|s| s.as_str()),
            Some("https://api.example.com:8080/data")
        );
    }

    #[test]
    fn parse_al_all_lowercase_natural_style() {
        let result = parse_al("write a new blog post with title={title} body={body}");
        assert_eq!(result.action_text, "write a new blog post");
        assert_eq!(result.slot_keys.len(), 2);
    }

    #[test]
    fn parse_al_with_numbers_in_values() {
        let (parsed, kv) = parse_al_and_slots("SET CONFIG WITH: PORT=8080 WORKERS=4 DEBUG=true");
        assert_eq!(parsed.action_text, "SET CONFIG");
        assert_eq!(kv.get("port").map(|s| s.as_str()), Some("8080"));
        assert_eq!(kv.get("workers").map(|s| s.as_str()), Some("4"));
        assert_eq!(kv.get("debug").map(|s| s.as_str()), Some("true"));
    }

    // -----------------------------------------------------------------------
    // Normalization unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn normalize_strips_trailing_semicolons() {
        let result = normalize_al_input("WRITE POST WITH: TITLE={title};");
        assert!(result.ends_with("TITLE={title}"));
    }

    #[test]
    fn normalize_converts_single_quotes_to_double() {
        let result = normalize_al_input("DO THING WITH: KEY='hello world'");
        assert!(result.contains("KEY=\"hello world\""));
    }

    #[test]
    fn normalize_collapses_spaces() {
        let result = normalize_al_input("WRITE   POST    WITH:    TITLE={title}");
        assert!(!result.contains("  "));
    }

    #[test]
    fn normalize_preserves_double_quotes() {
        let result = normalize_al_input("WRITE POST WITH: TITLE=\"hello\"");
        assert!(result.contains("TITLE=\"hello\""));
    }

    // -----------------------------------------------------------------------
    // Registry JSON deserialization (actions key)
    // -----------------------------------------------------------------------

    #[test]
    fn registry_accepts_actions_key() {
        let json = r#"{"version": 1, "actions": [{"id":"A.b/1","module":"A","name":"b","arity":1,"doc":"d","spec":"s","args":[],"examples":[]}]}"#;
        let reg: crate::types::ToolRegistry = serde_json::from_str(json).unwrap();
        assert_eq!(reg.actions.len(), 1);
        assert_eq!(reg.actions[0].id, "A.b/1");
    }

    #[test]
    fn registry_accepts_legacy_tools_key() {
        let json = r#"{"version": 1, "tools": [{"id":"A.b/1","module":"A","name":"b","arity":1,"doc":"d","spec":"s","args":[],"examples":[]}]}"#;
        let reg: crate::types::ToolRegistry = serde_json::from_str(json).unwrap();
        assert_eq!(reg.actions.len(), 1);
    }

    #[test]
    fn argdef_with_default_value() {
        let json = r#"{"name":"currency","type":"String.t()","required":true,"aliases":["coin"],"default":"usd"}"#;
        let arg: crate::types::ArgDef = serde_json::from_str(json).unwrap();
        assert_eq!(arg.default, Some("usd".to_string()));
    }

    #[test]
    fn argdef_without_default_value() {
        let json = r#"{"name":"title","type":"String.t()","required":true,"aliases":[]}"#;
        let arg: crate::types::ArgDef = serde_json::from_str(json).unwrap();
        assert_eq!(arg.default, None);
    }

    // -----------------------------------------------------------------------
    // CallPlan suggestions field
    // -----------------------------------------------------------------------

    #[test]
    fn callplan_suggestions_serialization() {
        let plan = crate::types::CallPlan {
            status: crate::types::PlanStatus::NoTool,
            selected_tool: None,
            confidence: None,
            args: None,
            missing: Vec::new(),
            notes: vec!["no action matched".into()],
            active_tool_threshold: 0.5,
            active_mapping_threshold: 0.35,
            candidates: Vec::new(),
            suggestions: vec![crate::types::ActionSuggestion {
                id: "Blog.write/2".into(),
                score: 0.25,
                al_command: "WRITE POST WITH: TITLE={title} BODY={body}".into(),
            }],
        };
        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("suggestions"));
        assert!(json.contains("Blog.write/2"));
        assert!(json.contains("al_command"));
    }

    #[test]
    fn callplan_suggestions_empty_skipped_in_json() {
        let plan = crate::types::CallPlan {
            status: crate::types::PlanStatus::Ok,
            selected_tool: Some("Blog.write/2".into()),
            confidence: Some(0.9),
            args: Some(HashMap::new()),
            missing: Vec::new(),
            notes: Vec::new(),
            active_tool_threshold: 0.5,
            active_mapping_threshold: 0.35,
            candidates: Vec::new(),
            suggestions: Vec::new(),
        };
        let json = serde_json::to_string(&plan).unwrap();
        // suggestions should be skipped when empty
        assert!(!json.contains("suggestions"));
    }

    // -----------------------------------------------------------------------
    // Real-world messy input tests
    // -----------------------------------------------------------------------

    #[test]
    fn messy_input_semicolons_and_lowercase() {
        let (parsed, kv) = parse_al_and_slots("write post with: title='My Post'; text='Hello';");
        assert_eq!(parsed.action_text, "write post");
        assert_eq!(kv.get("title").map(|s| s.as_str()), Some("My Post"));
        assert_eq!(kv.get("text").map(|s| s.as_str()), Some("Hello"));
    }

    #[test]
    fn messy_input_mixed_case_and_quotes() {
        let (parsed, kv) =
            parse_al_and_slots("Create Stripe Payment Link WITH: Amount=5000, Currency='usd', Product_Name=\"Widget\"");
        assert_eq!(parsed.action_text, "Create Stripe Payment Link");
        assert_eq!(kv.get("amount").map(|s| s.as_str()), Some("5000"));
        assert_eq!(kv.get("currency").map(|s| s.as_str()), Some("usd"));
        assert_eq!(kv.get("product_name").map(|s| s.as_str()), Some("Widget"));
    }

    #[test]
    fn messy_input_extra_spaces_everywhere() {
        let result = parse_al("  INSTALL   PACKAGE   {package}   VIA   APT  ");
        assert_eq!(result.action_text, "INSTALL PACKAGE package VIA APT");
        assert_eq!(result.slot_keys[0].key, "package");
    }
}
