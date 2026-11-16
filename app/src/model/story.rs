//! Story, chapter and draft data, and the fingerprint that notices an edit.

use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct Story {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub premise: String,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
    #[serde(default)]
    pub lore: Vec<LoreEntry>,
    /// Libraries from before chapters had their own conversation kept one
    /// story-wide. It is moved to the open chapter when the library is loaded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chat: Vec<ChatMessage>,
    #[serde(default)]
    pub open_chapter: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub summary_source: String,
    #[serde(default)]
    pub summary_body_len: usize,
    /// Fingerprint of the body the summary was written from. Older libraries
    /// have no fingerprint, so a length check stands in for them.
    #[serde(default)]
    pub summary_body_hash: u64,
    /// The conversation with the AI about this chapter, and nothing else.
    #[serde(default)]
    pub chat: Vec<ChatMessage>,
    /// What the oldest turns of that conversation said, once they had to give
    /// up their place to keep the context window from filling up.
    #[serde(default)]
    pub chat_summary: String,
    /// The author's note for the next stretch of prose. Empty means the model
    /// decides on its own; it is cleared once a draft is accepted.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub steering: String,
    /// A prose draft waiting under the chapter. `body` stays untouched until
    /// the author accepts it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<Draft>,
}

/// One pending AI suggestion, kept as data so it survives a restart.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    /// "continue" or "rewrite".
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub text: String,
    /// The character range the draft replaces, only for rewrites.
    #[serde(default)]
    pub range: Option<(usize, usize)>,
    /// The passage as it stood when the draft was requested, so a rewrite can
    /// notice that the text moved underneath it.
    #[serde(default)]
    pub source: Option<String>,
    /// The chapter as it was when the request started.
    #[serde(default)]
    pub base_len: usize,
    #[serde(default)]
    pub base_hash: u64,
    /// The author stopped the stream before the model was done.
    #[serde(default)]
    pub stopped: bool,
}

/// A small, fast fingerprint of the chapter text. It lets Folio notice a real
/// edit even when the new wording happens to be the same length as the old.
pub fn content_hash(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub text: String,
    #[serde(default)]
    pub demo: bool,
}

impl Story {
    pub fn words(&self) -> usize {
        self.chapters.iter().map(Chapter::words).sum()
    }

    pub fn chapter(&self, id: u64) -> Option<&Chapter> {
        self.chapters.iter().find(|c| c.id == id)
    }

    pub fn chapter_mut(&mut self, id: u64) -> Option<&mut Chapter> {
        self.chapters.iter_mut().find(|c| c.id == id)
    }

    /// Remove one chapter, keeping the rest of the story intact and the same
    /// chapter open when a different one was deleted.
    pub fn delete_chapter(&mut self, chapter_id: u64) -> Option<Chapter> {
        let index = self.chapters.iter().position(|c| c.id == chapter_id)?;
        let chapter = self.chapters.remove(index);
        if index < self.open_chapter {
            self.open_chapter -= 1;
        }
        self.open_chapter = self.open_chapter.min(self.chapters.len().saturating_sub(1));
        Some(chapter)
    }

    /// Put a deleted chapter back where it was, returning its new index.
    pub fn insert_chapter(&mut self, index: usize, chapter: Chapter) -> usize {
        let chapter_id = chapter.id;
        let at = index.min(self.chapters.len());
        self.chapters.insert(at, chapter);
        self.chapters
            .iter()
            .position(|c| c.id == chapter_id)
            .unwrap_or(at)
    }

    /// Remove one lorebook entry, keeping the rest of the book intact.
    pub fn delete_lore(&mut self, entry_id: u64) -> Option<LoreEntry> {
        let index = self.lore.iter().position(|e| e.id == entry_id)?;
        Some(self.lore.remove(index))
    }

    /// Put a deleted lorebook entry back where it was, returning its new index.
    pub fn insert_lore(&mut self, index: usize, entry: LoreEntry) -> usize {
        let entry_id = entry.id;
        let at = index.min(self.lore.len());
        self.lore.insert(at, entry);
        self.lore
            .iter()
            .position(|e| e.id == entry_id)
            .unwrap_or(at)
    }

    /// Lore that is either pinned or mentioned in the text.
    pub fn lore_for_text<'a>(&'a self, text: &str) -> Vec<&'a LoreEntry> {
        let haystack = text.to_lowercase();
        self.lore
            .iter()
            .filter(|entry| entry.pinned || entry.matches(&haystack))
            .collect()
    }

    pub fn mentioned_lore<'a>(&'a self, text: &str) -> Vec<&'a LoreEntry> {
        let haystack = text.to_lowercase();
        self.lore
            .iter()
            .filter(|entry| entry.matches(&haystack))
            .collect()
    }

    /// Where the "recent" summaries begin: everything before this index is
    /// condensed into the story-so-far block instead of being sent as itself.
    pub fn recent_start(&self, position: usize, window: usize) -> usize {
        position.saturating_sub(window).min(self.chapters.len())
    }

    /// The earlier chapters whose own summaries still travel one by one.
    pub fn recent_summaries<'a>(&'a self, position: usize, window: usize) -> Vec<&'a Chapter> {
        let start = self.recent_start(position, window);
        let end = position.min(self.chapters.len());
        self.chapters[start.min(end)..end]
            .iter()
            .filter(|chapter| chapter.is_summarized())
            .collect()
    }

    /// Everything older than the recent window, folded into one block so a long
    /// book does not send twenty summaries where a paragraph will do. Every
    /// chapter keeps a share of the budget, so none of them disappears.
    pub fn story_so_far(&self, position: usize, window: usize) -> String {
        let end = self.recent_start(position, window);
        let summaries: Vec<&Chapter> = self.chapters[..end]
            .iter()
            .filter(|chapter| chapter.is_summarized())
            .collect();
        if summaries.is_empty() {
            return String::new();
        }
        let share = (1200 / summaries.len()).clamp(90, 280);
        summaries
            .iter()
            .map(|chapter| clip(chapter.summary.trim(), share))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Earlier chapters that would otherwise be missing from the AI's memory.
    /// Outside the window a stale summary also counts as missing, because the
    /// story-so-far block is built from it.
    pub fn chapters_needing_summary<'a>(
        &'a self,
        position: usize,
        window: usize,
    ) -> Vec<&'a Chapter> {
        let end = position.min(self.chapters.len());
        let window_start = self.recent_start(end, window);
        self.chapters[..end]
            .iter()
            .enumerate()
            .filter(|(index, chapter)| {
                if chapter.words() == 0 {
                    return false;
                }
                if !chapter.is_summarized() {
                    return true;
                }
                // A summary inside the window is sent as itself, so a stale one
                // is still usable; outside the window it feeds the recap.
                *index < window_start && chapter.summary_is_stale()
            })
            .map(|(_, chapter)| chapter)
            .collect()
    }
}

/// Shorten to at most `max` characters, ending on a word boundary if it can.
pub(crate) fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    match cut.rfind(' ') {
        Some(at) if at > max / 2 => format!("{}…", cut[..at].trim_end()),
        _ => format!("{}…", cut.trim_end()),
    }
}

impl Chapter {
    pub fn words(&self) -> usize {
        self.body.split_whitespace().count()
    }

    pub fn is_summarized(&self) -> bool {
        !self.summary.trim().is_empty()
    }

    pub fn summary_is_stale(&self) -> bool {
        if !self.is_summarized() {
            return false;
        }
        if self.summary_body_hash != 0 {
            return self.summary_body_hash != content_hash(&self.body);
        }
        let body_len = self.body.chars().count();
        let drift = (body_len as i64 - self.summary_body_len as i64).abs();
        drift > 40
    }

    pub fn summary_state(&self) -> &'static str {
        if !self.is_summarized() {
            "not summarized"
        } else if self.summary_is_stale() {
            "summary needs refreshing"
        } else {
            "summary ready"
        }
    }

    /// Rough token count of this chapter's conversation, using the same
    /// four-characters-per-token estimate as the request budget.
    pub fn conversation_tokens(&self) -> usize {
        let turns: usize = self
            .chat
            .iter()
            .map(|message| message.text.chars().count() / 4 + 8)
            .sum();
        turns + self.chat_summary.chars().count() / 4
    }

    /// Fold the oldest turns into a short memory until the conversation fits
    /// `budget` tokens. The newest turns always stay verbatim; when the memory
    /// itself grows too long its oldest part fades. Returns true when a turn
    /// moved, so the caller can save the library.
    pub fn compact_conversation(&mut self, budget: usize) -> bool {
        let mut moved = false;
        while self.conversation_tokens() > budget && self.chat.len() > 2 {
            let message = self.chat.remove(0);
            let line = clip(message.text.trim(), 200);
            if !line.is_empty() {
                let who = if message.role == "user" {
                    "Author"
                } else {
                    "Partner"
                };
                if !self.chat_summary.is_empty() {
                    self.chat_summary.push(' ');
                }
                self.chat_summary.push_str(&format!("{who}: {line}"));
            }
            moved = true;
        }
        if self.chat_summary.chars().count() > CHAT_MEMORY_CHARS {
            let chars: Vec<char> = self.chat_summary.chars().collect();
            let mut from = chars.len() - CHAT_MEMORY_CHARS;
            // Start at a word so the memory does not open mid-sentence.
            while from < chars.len() && !chars[from].is_whitespace() {
                from += 1;
            }
            self.chat_summary =
                format!("…{}", chars[from..].iter().collect::<String>().trim_start());
        }
        moved
    }
}

/// How much of the condensed conversation stays in the request. At about 300
/// tokens it is a paragraph of memory, not a second story.
pub(crate) const CHAT_MEMORY_CHARS: usize = 1200;
