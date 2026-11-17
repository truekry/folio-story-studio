//! Splitting a chapter into blocks for a block-wise summary.

use super::*;
use std::ops::Range;

/// How many words one block should aim for. A chapter is cut at paragraph
/// boundaries where possible, so a block is usually a little under this.
pub const SUMMARY_BLOCK_WORDS: usize = 1200;

/// A single paragraph longer than this is split even if there is no paragraph
/// boundary to use. Nothing is ever cut at a character.
pub const SUMMARY_BLOCK_MAX_WORDS: usize = 2000;

/// Byte ranges of the chapter's blocks, in reading order. Every word appears
/// exactly once; the blank lines between paragraphs may sit outside the ranges.
pub fn summary_blocks(body: &str, target_words: usize) -> Vec<Range<usize>> {
    let target = target_words.clamp(300, SUMMARY_BLOCK_MAX_WORDS);
    let mut blocks = Vec::new();
    let mut start: Option<usize> = None;
    let mut end = 0usize;
    let mut words = 0usize;

    for range in paragraph_ranges(body) {
        let count = body[range.clone()].split_whitespace().count();
        if count == 0 {
            continue;
        }
        // A giant paragraph has no usable paragraph boundary. Flush what is
        // waiting, then split the paragraph itself at word/sentence boundaries.
        if count > SUMMARY_BLOCK_MAX_WORDS {
            if let Some(begin) = start.take() {
                blocks.push(begin..end);
            }
            blocks.extend(word_blocks(body, range, target));
            words = 0;
            continue;
        }
        if start.is_some() && words + count > target {
            blocks.push(start.take().unwrap_or(0)..end);
            words = 0;
        }
        if start.is_none() {
            start = Some(range.start);
            end = range.end;
        } else {
            end = range.end;
        }
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
/// not to the merged text.
pub fn summary_block_words(settings: &AiSettings) -> u32 {
    PromptKind::Summary.words(&settings.prompts)
}

/// Word ranges inside one paragraph, used when it is too long to be a block.
fn word_blocks(body: &str, range: Range<usize>, target: usize) -> Vec<Range<usize>> {
    let words = word_ranges(body, range);
    if words.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < words.len() {
        let mut end = (start + target).min(words.len());
        if end < words.len() {
            // Prefer a sentence end near the target. The lower bound keeps
            // very short sentence fragments from becoming their own block.
            let lower = start + target * 4 / 5;
            for candidate in (lower..end).rev() {
                if ends_sentence(&body[words[candidate].clone()]) {
                    end = candidate + 1;
                    break;
                }
            }
        }
        if end <= start {
            end = (start + 1).min(words.len());
        }
        out.push(words[start].start..words[end - 1].end);
        start = end;
    }
    out
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

/// Byte ranges of the non-empty paragraphs, trimmed at both ends. This is the
/// same paragraph rule the editor uses, kept here so the summariser can work
/// without the app layer.
fn paragraph_ranges(body: &str) -> Vec<Range<usize>> {
    let chars: Vec<char> = body.chars().collect();
    let mut byte_of = Vec::with_capacity(chars.len() + 1);
    let mut byte = 0usize;
    for ch in &chars {
        byte_of.push(byte);
        byte += ch.len_utf8();
    }
    byte_of.push(byte);

    let mut ranges = Vec::new();
    let mut start: Option<usize> = None;
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] == '\n' {
            let mut next = index + 1;
            while next < chars.len() && matches!(chars[next], ' ' | '\t' | '\r') {
                next += 1;
            }
            if next < chars.len() && chars[next] == '\n' {
                if let Some(begin) = start.take() {
                    let mut end = index;
                    while end > begin && chars[end - 1].is_whitespace() {
                        end -= 1;
                    }
                    if end > begin {
                        ranges.push(byte_of[begin]..byte_of[end]);
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
            ranges.push(byte_of[begin]..byte_of[end]);
        }
    }
    ranges
}
