//! Splitting a chapter into blocks for a block-wise summary.

use super::*;
use std::ops::Range;

/// The default size of one block, in words of the chapter. The author can
/// change it in the settings; tests use this as the fresh-library value.
#[cfg(test)]
pub const SUMMARY_BLOCK_WORDS: usize = crate::model::DEFAULT_SUMMARY_BLOCK_SIZE;

/// The smallest block the settings allow. Below this, a summary would have to
/// ask the model to compress a whole scene into a sentence.
pub const SUMMARY_BLOCK_MIN_WORDS: usize = 300;

/// The largest block the settings allow.
pub const SUMMARY_BLOCK_MAX_WORDS: usize = 2000;

/// Byte ranges of the chapter's blocks, in reading order. Every word appears
/// exactly once; a block ends at a sentence end, never in the middle of one.
/// When the next sentence would push the block over the target, the block ends
/// at the previous sentence — so it can run slightly past the target only when
/// a single sentence is longer than the target itself.
pub fn summary_blocks(body: &str, target_words: usize) -> Vec<Range<usize>> {
    let target = target_words.clamp(SUMMARY_BLOCK_MIN_WORDS, SUMMARY_BLOCK_MAX_WORDS);
    let mut blocks = Vec::new();
    let mut start: Option<usize> = None;
    let mut end = 0usize;
    let mut words = 0usize;

    for range in sentence_ranges(body) {
        let count = body[range.clone()].split_whitespace().count();
        if count == 0 {
            continue;
        }
        if start.is_some() && words + count > target {
            blocks.push(start.take().unwrap_or(0)..end);
            words = 0;
        }
        if start.is_none() {
            start = Some(range.start);
        }
        end = range.end;
        words += count;
    }
    if let Some(begin) = start {
        if end > begin {
            blocks.push(begin..end);
        }
    }
    blocks
}

/// The word target of one block summary. In the prompt settings this is the
/// "aim for X words" value of the chapter summary; it applies to every block,
/// not to the merged text. The merged text has its own target, see
/// [`summary_merge_words`].
pub fn summary_block_words(settings: &AiSettings) -> u32 {
    PromptKind::Summary.words(&settings.prompts)
}

/// The default length of the merged chapter summary, in words. A merge only
/// happens for chapters that were summarized in more than one block.
pub const DEFAULT_SUMMARY_MERGE_WORDS: u32 = 150;

/// The word target of the merged chapter summary. Zero means no target.
pub fn summary_merge_words(settings: &AiSettings) -> u32 {
    settings
        .prompts
        .merge_words
        .unwrap_or(DEFAULT_SUMMARY_MERGE_WORDS)
}

/// The block size the author chose, kept inside what the summariser can use.
pub fn summary_block_size(settings: &AiSettings) -> usize {
    settings
        .summary_block_size
        .clamp(SUMMARY_BLOCK_MIN_WORDS, SUMMARY_BLOCK_MAX_WORDS)
}

/// Trim a summary back to the last finished sentence that still fits the
/// target, with a 20% tolerance. A model that ignores the word limit is not
/// allowed to turn a 90-word memory into a 330-word retelling, but a sentence
/// is never cut in half: when the first sentence alone is longer than the
/// limit it is kept whole. A target of zero means no limit and no change.
pub fn trim_to_target(text: &str, target_words: u32) -> (String, bool) {
    let text = text.trim();
    if target_words == 0 || text.is_empty() {
        return (text.to_string(), false);
    }
    let limit = ((target_words as f64) * 1.2).ceil() as usize;
    if text.split_whitespace().count() <= limit {
        return (text.to_string(), false);
    }
    let mut best: Option<usize> = None;
    for range in sentence_ranges(text) {
        let ends = ends_sentence(text[..range.end].trim_end());
        let words_to_end = text[..range.end].split_whitespace().count();
        if words_to_end <= limit {
            if ends {
                best = Some(range.end);
            }
        } else if best.is_none() {
            // The first sentence already runs past the limit. Keep it whole
            // rather than cutting through a word or a clause.
            best = ends.then_some(range.end);
            break;
        } else {
            break;
        }
    }
    match best {
        Some(end) if end < text.len() => (text[..end].trim_end().to_string(), true),
        _ => (text.to_string(), false),
    }
}

/// Sentence ranges of the chapter, trimmed, in reading order. A sentence ends
/// at `.`, `!` or `?`, even when a closing quote or bracket follows it. A
/// paragraph break without punctuation does not end a sentence.
fn sentence_ranges(body: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut end = 0usize;
    for range in word_ranges(body, 0..body.len()) {
        if start.is_none() {
            start = Some(range.start);
        }
        end = range.end;
        if ends_sentence_text(&body[range.clone()]) {
            out.push(start.take().unwrap_or(range.start)..end);
        }
    }
    if let Some(begin) = start {
        if end > begin {
            out.push(begin..end);
        }
    }
    out
}

/// True when a word ends a sentence. Closing quotes and brackets after the
/// punctuation still count; a closing quote alone does not.
fn ends_sentence_text(text: &str) -> bool {
    let mut chars = text.trim_end().chars().rev().peekable();
    while let Some(&ch) = chars.peek() {
        if matches!(ch, '"' | '\'' | '”' | '’' | ')' | ']' | '»') {
            chars.next();
        } else {
            break;
        }
    }
    matches!(chars.next(), Some('.' | '!' | '?' | '…'))
}

fn word_ranges(body: &str, range: Range<usize>) -> Vec<Range<usize>> {
    let text = &body[range.clone()];
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (offset, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if let Some(begin) = start.take() {
                out.push(range.start + begin..range.start + offset);
            }
        } else if start.is_none() {
            start = Some(offset);
        }
    }
    if let Some(begin) = start {
        out.push(range.start + begin..range.start + text.len());
    }
    out
}
