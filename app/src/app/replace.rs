//! Find and replace across the open chapter or the whole book.

use super::*;

impl FolioApp {
    pub(crate) fn open_find(&mut self) {
        if self.library.stories.is_empty() {
            self.notify(
                "Start a story first — then its chapters can be searched.",
                false,
            );
            return;
        }
        self.find_open = true;
        self.find_focus = true;
        self.find_replace = false;
        self.find_index = None;
    }

    /// Replace every match in the open chapter (or the whole book when
    /// `only_chapter` is None), keeping the old text so the change can be taken
    /// back from the notice.
    pub(crate) fn replace_in_book(
        &mut self,
        query: &str,
        replacement: &str,
        match_case: bool,
        only_chapter: Option<u64>,
    ) {
        if query.is_empty() {
            return;
        }
        let story_id = self.story_id();
        let mut total = 0usize;
        if let Some(story) = self.library.story(story_id) {
            for chapter in &story.chapters {
                if only_chapter.is_some_and(|id| chapter.id != id) {
                    continue;
                }
                total += find_char_ranges(&chapter.body, query, match_case).len();
            }
        }
        if total == 0 {
            self.notify("There are no matches left to replace.", false);
            return;
        }
        let mut saved: Vec<(u64, String)> = Vec::new();
        let mut touched = 0usize;
        if let Some(story) = self.library.story_mut(story_id) {
            for chapter in &mut story.chapters {
                if only_chapter.is_some_and(|id| chapter.id != id) {
                    continue;
                }
                let (new_body, count) =
                    replace_all_in_text(&chapter.body, query, replacement, match_case);
                if count > 0 {
                    saved.push((chapter.id, chapter.body.clone()));
                    chapter.body = new_body;
                    touched += 1;
                }
            }
        }
        self.editor_selection = None;
        self.pending_selection = None;
        self.mark_dirty();
        self.notify_replaced(
            format!(
                "Replaced {} in {}. Chapter summaries now ask for a refresh.",
                plural(total, "match", "matches"),
                plural(touched, "chapter", "chapters")
            ),
            ReplaceUndo {
                story_id,
                chapters: saved,
            },
        );
    }

    pub(crate) fn restore_replacement(&mut self, undo: ReplaceUndo) {
        let Some(story) = self.library.story_mut(undo.story_id) else {
            self.notify("That story is no longer open, so the change stayed.", true);
            return;
        };
        let mut restored = 0usize;
        for (chapter_id, body) in undo.chapters {
            if let Some(chapter) = story.chapter_mut(chapter_id) {
                chapter.body = body;
                restored += 1;
            }
        }
        self.editor_selection = None;
        self.pending_selection = None;
        self.mark_dirty();
        self.notify(
            format!(
                "Replacement undone — {} put back.",
                plural(restored, "chapter", "chapters")
            ),
            false,
        );
    }
}

/// Character ranges (not bytes) of every non-overlapping match, so a result can
/// be selected in the editor and replaced without breaking on accented text.
pub(crate) fn find_char_ranges(text: &str, query: &str, match_case: bool) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if match_case {
        for (byte, matched) in text.match_indices(query) {
            let start = text[..byte].chars().count();
            let end = start + matched.chars().count();
            out.push(start..end);
        }
    } else {
        let (lower, map) = lowercase_with_map(text);
        let needle = query.to_lowercase();
        for (byte, matched) in lower.match_indices(&needle) {
            let Some(start) = map.get(byte).copied() else {
                continue;
            };
            let end = map
                .get(byte + matched.len().saturating_sub(1))
                .copied()
                .unwrap_or(start)
                + 1;
            out.push(start..end);
        }
    }
    out
}

/// Lowercase the text while remembering which original character each byte of
/// the lowercase text came from.
pub(crate) fn lowercase_with_map(text: &str) -> (String, Vec<usize>) {
    let mut lower = String::with_capacity(text.len());
    let mut map = Vec::with_capacity(text.len());
    for (index, character) in text.chars().enumerate() {
        let folded: String = character.to_lowercase().collect();
        for _ in folded.bytes() {
            map.push(index);
        }
        lower.push_str(&folded);
    }
    (lower, map)
}

pub(crate) fn replace_all_in_text(
    text: &str,
    query: &str,
    replacement: &str,
    match_case: bool,
) -> (String, usize) {
    let ranges = find_char_ranges(text, query, match_case);
    if ranges.is_empty() {
        return (text.to_string(), 0);
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for range in &ranges {
        if range.start > cursor {
            out.extend(chars[cursor..range.start].iter());
        }
        out.push_str(replacement);
        cursor = range.end;
    }
    if cursor < chars.len() {
        out.extend(chars[cursor..].iter());
    }
    (out, ranges.len())
}

/// The next or previous match, wrapping around. `None` starts at the first
/// (or the last, when going back).
pub(crate) fn step_find_hit(current: Option<usize>, len: usize, forward: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match current {
        Some(index) if index < len => {
            if forward {
                (index + 1) % len
            } else {
                (index + len - 1) % len
            }
        }
        _ => {
            if forward {
                0
            } else {
                len - 1
            }
        }
    })
}
