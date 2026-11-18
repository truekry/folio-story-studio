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

/// The bin belongs to the newest question or answer only: a system note has
/// nothing to undo, and a running answer is not part of the history yet.
pub(crate) fn removable_last(chapter: &Chapter, busy: bool) -> bool {
    !busy
        && matches!(
            chapter.chat.last().map(|message| message.role.as_str()),
            Some("user" | "assistant")
        )
}

/// Remove exactly one line of the conversation, the newest. Question and answer
/// are never grouped, so the author can step back entry by entry.
pub(crate) fn pop_last_message(chapter: &mut Chapter) -> Option<ChatMessage> {
    chapter.chat.pop()
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
    // Only a continuation uses up the steering note. A rewrite or an
    // elaboration leaves the author's note for the next stretch of prose.
    if draft.action == "continue" {
        chapter.steering.clear();
    }
    Ok(())
}

/// The paragraph before and after a passage, for a rewrite or an elaboration.
/// Both are shortened: the model should see the join, not a second page.
pub(crate) fn neighbour_paragraphs(
    body: &str,
    range: &Range<usize>,
) -> (Option<String>, Option<String>) {
    let paragraphs = paragraph_ranges(body);
    let before = paragraphs
        .iter()
        .take_while(|paragraph| paragraph.end <= range.start)
        .last()
        .map(|paragraph| slice_chars(body, paragraph));
    let after = paragraphs
        .iter()
        .find(|paragraph| paragraph.start >= range.end)
        .map(|paragraph| slice_chars(body, paragraph));
    (
        before.map(|text| crate::ai::trim_chars(text.trim(), 600)),
        after.map(|text| crate::ai::trim_chars(text.trim(), 600)),
    )
}

/// True when a draft hands back a long piece of the read-only context around
/// the passage. The author is told; nothing is blocked or retried.
pub(crate) fn draft_repeats_neighbour(
    draft: &str,
    before: Option<&str>,
    after: Option<&str>,
) -> bool {
    let draft = draft.trim();
    if draft.is_empty() {
        return false;
    }
    [before, after].into_iter().flatten().any(|neighbour| {
        let neighbour = neighbour.trim();
        if neighbour.chars().count() < 40 {
            return false;
        }
        let head: String = neighbour.chars().take(40).collect();
        draft.contains(&head)
    })
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

/// How much of a rewrite is just the passage again. Words are compared in
/// order, ignoring case and punctuation, so "The sea was very beautiful." and
/// "the sea was very beautiful" count as the same text. A true rewrite shares
/// names and a few nouns, not most of its positions.
pub(crate) fn echo_similarity(answer: &str, source: &str) -> f32 {
    fn words(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut word = String::new();
        for ch in text.chars() {
            if ch.is_alphanumeric() {
                word.extend(ch.to_lowercase());
            } else if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
        }
        if !word.is_empty() {
            out.push(word);
        }
        out
    }
    let answer = words(answer);
    let source = words(source);
    let longest = answer.len().max(source.len());
    if longest == 0 {
        return 0.0;
    }
    if answer == source {
        return 1.0;
    }
    let same = answer
        .iter()
        .zip(source.iter())
        .filter(|(a, b)| a == b)
        .count();
    same as f32 / longest as f32
}

/// Splice paragraph corrections into a body. Ranges are char indices into the
/// same text the correction was made from; they are applied bottom-up so each
/// one keeps addressing the words it was measured against. Out-of-range values
/// are clamped instead of panicking: a half-written chapter must never take the
/// window down.
pub(crate) fn apply_corrections(body: &str, corrections: &[(Range<usize>, String)]) -> String {
    let mut chars: Vec<char> = body.chars().collect();
    for (range, text) in corrections.iter().rev() {
        let start = range.start.min(chars.len());
        let end = range.end.clamp(start, chars.len());
        chars.splice(start..end, text.chars());
    }
    chars.into_iter().collect()
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
