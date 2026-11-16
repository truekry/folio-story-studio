//! The library: every story, the settings and the next free id.

use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct Library {
    pub stories: Vec<Story>,
    #[serde(default)]
    pub settings: AiSettings,
    #[serde(default = "one")]
    pub next_id: u64,
    #[serde(default)]
    pub active_story: usize,
    /// Whether the chapter header carries the Summary… button. Off, the page
    /// shows only the state dot; the window stays reachable from the top bar.
    #[serde(default = "yes")]
    pub show_summary: bool,
    /// Path of the installed font used for the story text. Empty means the
    /// system serif Folio finds by default; Folio ships no font of its own.
    #[serde(default)]
    pub writing_font: String,
    /// Size of the chapter text in points, chosen with the type bar.
    #[serde(default = "default_writing_size")]
    pub writing_size: f32,
    /// Where the last export went, so the next one starts there.
    #[serde(default)]
    pub export_dir: String,
    /// True for one PDF of the whole story, false for one text file per chapter.
    #[serde(default = "yes")]
    pub export_pdf: bool,
}

impl Library {
    pub fn sample() -> Self {
        Self {
            stories: vec![sample_lantern_coast(), sample_orchard()],
            settings: AiSettings::default(),
            next_id: 100,
            active_story: 0,
            show_summary: true,
            writing_font: String::new(),
            writing_size: DEFAULT_WRITING_SIZE,
            export_dir: String::new(),
            export_pdf: true,
        }
    }

    pub fn next_id(&mut self) -> u64 {
        self.next_id = self.next_id.saturating_add(1);
        self.next_id
    }

    pub fn story(&self, id: u64) -> Option<&Story> {
        self.stories.iter().find(|s| s.id == id)
    }

    /// Move a story-wide conversation from an older library into the chapter
    /// that was open at the time, so every chapter has its own from now on.
    pub fn adopt_story_chats(&mut self) {
        for story in &mut self.stories {
            if story.chat.is_empty() {
                continue;
            }
            let open = story
                .open_chapter
                .min(story.chapters.len().saturating_sub(1));
            match story.chapters.get_mut(open) {
                Some(chapter) if chapter.chat.is_empty() => {
                    chapter.chat = std::mem::take(&mut story.chat);
                }
                _ => story.chat.clear(),
            }
        }
    }

    pub fn story_mut(&mut self, id: u64) -> Option<&mut Story> {
        self.stories.iter_mut().find(|s| s.id == id)
    }

    pub fn total_words(&self) -> u64 {
        self.stories
            .iter()
            .map(|s| s.chapters.iter().map(|c| c.words() as u64).sum::<u64>())
            .sum()
    }

    /// Remove a whole story (chapters, lorebook and notes) by id. The same
    /// story stays open when another one was deleted.
    pub fn delete_story(&mut self, story_id: u64) -> Option<Story> {
        let index = self.stories.iter().position(|s| s.id == story_id)?;
        let story = self.stories.remove(index);
        if index < self.active_story {
            self.active_story -= 1;
        }
        if self.active_story >= self.stories.len() {
            self.active_story = self.stories.len().saturating_sub(1);
        }
        Some(story)
    }

    /// Put a deleted story back where it was, returning its new index.
    pub fn insert_story(&mut self, index: usize, story: Story) -> usize {
        let story_id = story.id;
        let at = index.min(self.stories.len());
        self.stories.insert(at, story);
        self.stories
            .iter()
            .position(|s| s.id == story_id)
            .unwrap_or(at)
    }
}

pub(crate) fn one() -> u64 {
    1
}

pub(crate) fn yes() -> bool {
    true
}

/// The size the chapter text has always had; the type bar can change it.
pub const DEFAULT_WRITING_SIZE: f32 = 17.5;

pub(crate) fn default_writing_size() -> f32 {
    DEFAULT_WRITING_SIZE
}
