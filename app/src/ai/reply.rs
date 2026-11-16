//! The finished reply: its seams, its ending and the text inside an answer.

use super::*;

/// One reply, plus the one detail that decides whether it is finished: did the
/// server stop it because the token limit was reached?
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub text: String,
    pub truncated: bool,
}

/// What a streaming request reports while it runs. The worker sends `Delta`s as
/// the text grows, then exactly one `Done` or `Failed`.
#[derive(Clone, Debug, PartialEq)]
pub enum StreamEvent {
    Delta(String),
    /// How far a multi-step job (the spelling check) has got.
    Progress {
        done: usize,
        total: usize,
    },
    Done {
        truncated: bool,
    },
    Failed(String),
}

/// Does the text stop on something that ends a sentence? A closing quote after
/// the full stop counts, and so does a closing bracket.
pub(crate) fn ends_sentence(text: &str) -> bool {
    match text.trim_end().chars().last() {
        Some(ch) => matches!(
            ch,
            '.' | '!' | '?' | '…' | '"' | '\'' | '”' | '’' | '»' | ')' | ']'
        ),
        None => false,
    }
}

/// What a continued reply adds to the text the caller already holds. It keeps
/// the first text byte for byte, so the caller can append the returned suffix
/// and end up with exactly the same words. Models often repeat the last words
/// before carrying on; an overlapping run is dropped instead of appearing
/// twice.
pub(crate) fn continuation_suffix(before: &str, after: &str) -> String {
    let after = after.trim_start();
    if after.is_empty() {
        return String::new();
    }
    if before.trim().is_empty() {
        return after.to_string();
    }
    let head: Vec<&str> = before.split_whitespace().collect();
    let tail: Vec<&str> = after.split_whitespace().collect();
    if tail.is_empty() {
        return String::new();
    }
    let limit = head.len().min(tail.len()).min(40);
    let mut skip = 0;
    for size in (1..=limit).rev() {
        if head[head.len() - size..] == tail[..size] {
            skip = size;
            break;
        }
    }
    if skip == tail.len() {
        return String::new();
    }
    let rest = tail[skip..].join(" ");
    let at_boundary = before.chars().last().is_some_and(|c| c.is_whitespace());
    let glued = skip == 0
        && !at_boundary
        && before.chars().last().is_some_and(|c| c.is_alphanumeric())
        && rest.chars().next().is_some_and(|c| c.is_lowercase());
    if at_boundary || glued {
        rest
    } else {
        format!(" {rest}")
    }
}

/// Guarantee that a summary ends on a finished sentence. A model that was cut
/// and could not be continued is trimmed back to its last full stop, so the
/// text never ends in the middle of a word.
pub fn complete_summary(text: &str) -> (String, bool) {
    let text = text.trim();
    if text.is_empty() || ends_sentence(text) {
        return (text.to_string(), false);
    }
    match text.rfind(['.', '!', '?']) {
        Some(at) => (text[..=at].trim().to_string(), true),
        None => (text.to_string(), false),
    }
}

pub(crate) fn extract_content(value: &Value) -> Option<String> {
    let choice = &value["choices"][0];
    if let Some(text) = choice["message"]["content"].as_str() {
        if !text.trim().is_empty() {
            return Some(text.trim().to_string());
        }
    }
    if let Some(parts) = choice["message"]["content"].as_array() {
        let joined = parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !joined.trim().is_empty() {
            return Some(joined.trim().to_string());
        }
    }
    if let Some(text) = choice["text"].as_str() {
        if !text.trim().is_empty() {
            return Some(text.trim().to_string());
        }
    }
    if let Some(text) = value["output_text"].as_str() {
        if !text.trim().is_empty() {
            return Some(text.trim().to_string());
        }
    }
    None
}
