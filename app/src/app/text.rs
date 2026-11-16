//! Pure text helpers: character ranges, paragraph splitting and draft application.

use super::*;

pub(crate) fn slice_chars<'a>(text: &'a str, range: &Range<usize>) -> &'a str {
    let start = byte_index(text, range.start);
    let end = byte_index(text, range.end).min(text.len());
    if start <= end {
        &text[start..end]
    } else {
        ""
    }
}

pub(crate) fn byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

pub(crate) fn replace_char_range(text: &str, range: &Range<usize>, replacement: &str) -> String {
    let start = byte_index(text, range.start);
    let end = byte_index(text, range.end).min(text.len());
    let mut out = String::with_capacity(text.len() + replacement.len());
    out.push_str(&text[..start]);
    out.push_str(replacement);
    out.push_str(&text[end..]);
    out
}

/// Add a paragraph to the chapter without ever leaving more than one blank
/// line between the old text and the new.
pub(crate) fn append_draft(body: &str, text: &str) -> String {
    let text = text.trim();
    if body.trim().is_empty() {
        text.to_string()
    } else {
        format!("{}\n\n{text}", body.trim_end())
    }
}

/// Drop a chapter's brainstorm: every message and the condensed memory that
/// stood in for the oldest ones.
pub(crate) fn clear_chapter_chat(chapter: &mut Chapter) {
    chapter.chat.clear();
    chapter.chat_summary.clear();
}

/// Add new facts to a lore entry without losing what was already known.
pub(crate) fn append_lore(existing: &str, facts: &str) -> String {
    let existing = existing.trim();
    let facts = facts.trim();
    if existing.is_empty() {
        facts.to_string()
    } else if facts.is_empty() {
        existing.to_string()
    } else {
        format!("{existing}\n\n{facts}")
    }
}

/// Put a draft into the chapter. Choosing Accept is the only place this
/// happens, and it refuses when the chapter moved since the draft was asked
/// for, rather than replacing text the model never saw.
pub(crate) fn apply_draft(draft: &Draft, body: &str) -> Result<String, String> {
    if draft.base_hash != crate::model::content_hash(body) {
        return Err(
            "The chapter changed since this draft was made. Retry the draft on the current text."
                .to_string(),
        );
    }
    let text = draft.text.trim();
    if text.is_empty() {
        return Err("The draft is empty.".to_string());
    }
    if matches!(draft.action.as_str(), "rewrite" | "elaborate") {
        let (start, end) = draft
            .range
            .ok_or_else(|| "This draft has no passage to replace.".to_string())?;
        if start >= end || end > body.chars().count() {
            return Err("The passage this draft rewrites is no longer in the chapter.".to_string());
        }
        let range = start..end;
        if let Some(source) = &draft.source {
            if slice_chars(body, &range) != source {
                return Err(
                    "The passage changed since this draft was made. Retry the draft.".to_string(),
                );
            }
        }
        Ok(replace_char_range(body, &range, text))
    } else {
        Ok(append_draft(body, text))
    }
}

/// Accepting is the only way a draft reaches the chapter, and only then does
/// the note that produced it retire. A deleted or failed draft leaves it for
/// the next try.
pub(crate) fn accept_into(chapter: &mut Chapter, draft: &Draft) -> Result<(), String> {
    chapter.body = apply_draft(draft, &chapter.body)?;
    chapter.steering.clear();
    Ok(())
}

pub(crate) fn last_paragraph_range(body: &str) -> Option<Range<usize>> {
    let trimmed = body.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    let start_byte = trimmed.rfind("\n\n").map(|i| i + 2).unwrap_or(0);
    let start = trimmed[..start_byte].chars().count();
    let end = trimmed.chars().count();
    if start >= end {
        None
    } else {
        Some(start..end)
    }
}

/// Char ranges of every non-empty paragraph. A blank line separates
/// paragraphs; the ranges exclude surrounding whitespace, so corrections can
/// be spliced back without touching the page's spacing.
pub(crate) fn paragraph_ranges(body: &str) -> Vec<Range<usize>> {
    let chars: Vec<char> = body.chars().collect();
    let mut ranges = Vec::new();
    let mut start: Option<usize> = None;
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '\n' {
            let mut next = index + 1;
            while next < chars.len()
                && (chars[next] == ' ' || chars[next] == '\t' || chars[next] == '\r')
            {
                next += 1;
            }
            if next < chars.len() && chars[next] == '\n' {
                if let Some(begin) = start.take() {
                    let mut end = index;
                    while end > begin && chars[end - 1].is_whitespace() {
                        end -= 1;
                    }
                    if end > begin {
                        ranges.push(begin..end);
                    }
                }
                index = next + 1;
                continue;
            }
        }
        if !chars[index].is_whitespace() && start.is_none() {
            start = Some(index);
        }
        index += 1;
    }
    if let Some(begin) = start {
        let mut end = chars.len();
        while end > begin && chars[end - 1].is_whitespace() {
            end -= 1;
        }
        if end > begin {
            ranges.push(begin..end);
        }
    }
    ranges
}

/// A blank field sends nothing.
pub(crate) fn brainstorm_send_text(input: &str) -> Option<String> {
    let text = input.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}
