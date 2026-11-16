//! Creating, editing and deleting stories, chapters and lore entries, plus the trash.

use super::*;

impl FolioApp {
    pub(crate) fn open_story_editor_for(&mut self, story_id: u64) {
        let Some(story) = self.library.story(story_id) else {
            return;
        };
        self.story_editor_story_id = Some(story_id);
        self.story_title_draft = story.title.clone();
        self.story_premise_draft = story.premise.clone();
        self.story_editor_open = true;
    }

    pub(crate) fn ask_delete_story(&mut self, story_id: u64) {
        let Some(story) = self.library.story(story_id) else {
            return;
        };
        self.confirm = Some(Confirm::Story {
            story_id,
            title: story.title.clone(),
            chapters: story.chapters.len(),
            lore: story.lore.len(),
        });
        self.story_editor_open = false;
    }

    pub(crate) fn edit_chapter(&mut self, story_id: u64, chapter_id: u64) {
        let Some(chapter) = self
            .library
            .story(story_id)
            .and_then(|story| story.chapter(chapter_id))
        else {
            return;
        };
        self.chapter_editor = Some(ChapterDraft {
            story_id,
            chapter_id,
            title: chapter.title.clone(),
            summary: chapter.summary.clone(),
        });
    }

    pub(crate) fn ask_delete_chapter(&mut self, story_id: u64, chapter_id: u64) {
        let Some(story) = self.library.story(story_id) else {
            return;
        };
        let Some(number) = story.chapters.iter().position(|c| c.id == chapter_id) else {
            return;
        };
        let Some(chapter) = story.chapter(chapter_id) else {
            return;
        };
        self.confirm = Some(Confirm::Chapter {
            story_id,
            chapter_id,
            story_title: story.title.clone(),
            title: chapter.title.clone(),
            number: number + 1,
            words: chapter.words(),
            summarized: chapter.is_summarized(),
        });
    }

    pub(crate) fn ask_delete_lore(&mut self, story_id: u64, entry_id: u64) {
        let Some(story) = self.library.story(story_id) else {
            return;
        };
        let Some(entry) = story.lore.iter().find(|e| e.id == entry_id) else {
            return;
        };
        self.confirm = Some(Confirm::Lore {
            story_id,
            entry_id,
            story_title: story.title.clone(),
            name: entry.name.clone(),
            category: entry.category.clone(),
            description: entry.description.clone(),
            pinned: entry.pinned,
        });
    }

    pub(crate) fn delete_story_now(&mut self, story_id: u64) {
        if self.job.as_ref().map(|j| j.story_id) == Some(story_id) {
            self.job = None;
        }
        if self
            .draft_live
            .as_ref()
            .is_some_and(|live| live.story_id == story_id)
        {
            self.draft_live = None;
            self.draft_error = None;
        }
        if self.chapter_editor.as_ref().map(|draft| draft.story_id) == Some(story_id) {
            self.chapter_editor = None;
        }
        if self.lore_editor.as_ref().map(|draft| draft.story_id) == Some(story_id) {
            self.lore_editor = None;
        }
        let index = self
            .library
            .stories
            .iter()
            .position(|s| s.id == story_id)
            .unwrap_or(0);
        let Some(story) = self.library.delete_story(story_id) else {
            self.confirm = None;
            return;
        };
        let title = story.title.clone();
        let trash_id = self.next_trash_id();
        self.trash.push(TrashItem::Story {
            id: trash_id,
            index,
            story,
            deleted_at: Instant::now(),
        });
        self.active_story = self.library.active_story;
        self.active_chapter = self
            .library
            .stories
            .get(self.active_story)
            .map(|s| s.open_chapter.min(s.chapters.len().saturating_sub(1)))
            .unwrap_or(0);
        self.library.active_story = self.active_story;
        self.summary_open = false;
        self.editor_selection = None;
        self.confirm = None;
        self.mark_dirty();
        self.notify_deleted(format!("Story “{title}” deleted."), trash_id);
    }

    pub(crate) fn delete_chapter_now(&mut self, story_id: u64, chapter_id: u64) {
        if self.job.as_ref().map(|j| j.chapter_id) == Some(chapter_id) {
            self.job = None;
        }
        if self
            .draft_live
            .as_ref()
            .is_some_and(|live| live.chapter_id == chapter_id)
        {
            self.draft_live = None;
            self.draft_error = None;
        }
        if self.chapter_editor.as_ref().map(|draft| draft.chapter_id) == Some(chapter_id) {
            self.chapter_editor = None;
        }
        let index = self
            .library
            .story(story_id)
            .and_then(|story| story.chapters.iter().position(|c| c.id == chapter_id))
            .unwrap_or(0);
        let story_title = self
            .library
            .story(story_id)
            .map(|story| story.title.clone())
            .unwrap_or_default();
        let open_in_story = self.library.story_mut(story_id).and_then(|story| {
            story
                .delete_chapter(chapter_id)
                .map(|chapter| (story.open_chapter, chapter))
        });
        let Some((open_chapter, chapter)) = open_in_story else {
            self.confirm = None;
            return;
        };
        let title = chapter.title.clone();
        let trash_id = self.next_trash_id();
        self.trash.push(TrashItem::Chapter {
            id: trash_id,
            story_id,
            story_title,
            index,
            chapter,
            deleted_at: Instant::now(),
        });
        if self.story_id() == story_id {
            self.active_chapter = open_chapter;
        }
        self.summary_open = false;
        self.editor_selection = None;
        self.confirm = None;
        self.mark_dirty();
        self.notify_deleted(format!("Chapter “{title}” deleted."), trash_id);
    }

    pub(crate) fn delete_lore_now(&mut self, story_id: u64, entry_id: u64) {
        if self.lore_editor.as_ref().and_then(|draft| draft.entry_id) == Some(entry_id) {
            self.lore_editor = None;
        }
        let Some(story) = self.library.story(story_id) else {
            self.confirm = None;
            return;
        };
        let story_title = story.title.clone();
        let index = story
            .lore
            .iter()
            .position(|entry| entry.id == entry_id)
            .unwrap_or(0);
        let Some(entry) = self
            .library
            .story_mut(story_id)
            .and_then(|story| story.delete_lore(entry_id))
        else {
            self.confirm = None;
            return;
        };
        let name = entry.name.clone();
        let trash_id = self.next_trash_id();
        self.trash.push(TrashItem::Lore {
            id: trash_id,
            story_id,
            story_title,
            index,
            entry,
            deleted_at: Instant::now(),
        });
        self.confirm = None;
        self.mark_dirty();
        self.notify_deleted(format!("Lore entry “{name}” deleted."), trash_id);
    }

    /// Empty this chapter's brainstorm. The chapter text, its summary and the
    /// lorebook are untouched; only the conversation and its memory go.
    pub(crate) fn clear_chat_now(&mut self, story_id: u64, chapter_id: u64) {
        if let Some(chapter) = self
            .library
            .story_mut(story_id)
            .and_then(|story| story.chapter_mut(chapter_id))
        {
            clear_chapter_chat(chapter);
        }
        self.confirm = None;
        self.mark_dirty();
        self.notify("Chat cleared.", false);
    }

    pub(crate) fn restore_trash(&mut self, trash_id: u64) {
        let Some(position) = self.trash.iter().position(|item| item.id() == trash_id) else {
            return;
        };
        if let Some(story_id) = self.trash[position].story_id() {
            if self.library.story(story_id).is_none() {
                self.notify(
                    "The story this belonged to is still deleted. Put the story back first.",
                    true,
                );
                return;
            }
        }
        let item = self.trash.remove(position);
        match item {
            TrashItem::Story { index, story, .. } => {
                let open_chapter = story.open_chapter;
                let title = story.title.clone();
                let at = self.library.insert_story(index, story);
                self.active_story = at;
                self.library.active_story = self.active_story;
                self.active_chapter = self.library.stories[self.active_story]
                    .chapters
                    .len()
                    .saturating_sub(1)
                    .min(open_chapter);
                self.summary_open = false;
                self.editor_selection = None;
                self.mark_dirty();
                self.notify(format!("Story “{title}” put back."), false);
            }
            TrashItem::Chapter {
                story_id,
                index,
                chapter,
                ..
            } => {
                let title = chapter.title.clone();
                if let Some(story) = self.library.story_mut(story_id) {
                    let at = story.insert_chapter(index, chapter);
                    story.open_chapter = at;
                    if self.story_id() == story_id {
                        self.active_chapter = at;
                    }
                    self.summary_open = false;
                    self.editor_selection = None;
                    self.mark_dirty();
                    self.notify(format!("Chapter “{title}” put back."), false);
                }
            }
            TrashItem::Lore {
                story_id,
                index,
                entry,
                ..
            } => {
                let name = entry.name.clone();
                if let Some(story) = self.library.story_mut(story_id) {
                    story.insert_lore(index, entry);
                    self.mark_dirty();
                    self.notify(format!("Lore entry “{name}” put back."), false);
                }
            }
        }
    }

    pub(crate) fn edit_lore(&mut self, entry_id: Option<u64>) {
        let Some(story) = self.library.stories.get(self.active_story) else {
            return;
        };
        let story_id = story.id;
        let entry = entry_id.and_then(|id| story.lore.iter().find(|e| e.id == id));
        self.lore_editor = Some(LoreDraft {
            story_id,
            entry_id,
            name: entry.map(|e| e.name.clone()).unwrap_or_default(),
            category: entry
                .map(|e| e.category.clone())
                .unwrap_or_else(|| "Character".to_string()),
            aliases: entry.map(|e| e.aliases.clone()).unwrap_or_default(),
            triggers: entry.map(|e| e.triggers.clone()).unwrap_or_default(),
            description: entry.map(|e| e.description.clone()).unwrap_or_default(),
            pinned: entry.map(|e| e.pinned).unwrap_or(false),
        });
    }

    pub(crate) fn create_story(&mut self) {
        let title = if self.new_story_title.trim().is_empty() {
            "Untitled story".to_string()
        } else {
            self.new_story_title.trim().to_string()
        };
        let story_id = self.library.next_id();
        let chapter_id = self.library.next_id();
        let story = Story {
            id: story_id,
            title,
            premise: self.new_story_premise.trim().to_string(),
            chapters: vec![Chapter {
                id: chapter_id,
                title: "Chapter One".to_string(),
                body: String::new(),
                summary: String::new(),
                summary_source: String::new(),
                summary_body_len: 0,
                summary_body_hash: 0,
                chat: Vec::new(),
                chat_summary: String::new(),
                steering: String::new(),
                draft: None,
            }],
            lore: Vec::new(),
            chat: Vec::new(),
            open_chapter: 0,
        };
        self.library.stories.push(story);
        self.active_story = self.library.stories.len() - 1;
        self.active_chapter = 0;
        self.library.active_story = self.active_story;
        self.new_story_open = false;
        self.new_story_title.clear();
        self.new_story_premise.clear();
        self.summary_open = false;
        self.mark_dirty();
        self.notify("New story created. The first chapter is waiting.", false);
    }

    pub(crate) fn create_chapter(&mut self) {
        let Some(story) = self.library.stories.get(self.active_story) else {
            self.new_chapter_open = false;
            return;
        };
        let count = story.chapters.len() + 1;
        let title = if self.new_chapter_title.trim().is_empty() {
            format!("Chapter {count}")
        } else {
            self.new_chapter_title.trim().to_string()
        };
        let id = self.library.next_id();
        if let Some(story) = self.library.stories.get_mut(self.active_story) {
            story.chapters.push(Chapter {
                id,
                title,
                body: String::new(),
                summary: String::new(),
                summary_source: String::new(),
                summary_body_len: 0,
                summary_body_hash: 0,
                chat: Vec::new(),
                chat_summary: String::new(),
                steering: String::new(),
                draft: None,
            });
            let index = story.chapters.len() - 1;
            story.open_chapter = index;
            self.active_chapter = index;
        }
        self.new_chapter_open = false;
        self.new_chapter_title.clear();
        self.summary_open = false;
        self.mark_dirty();
    }
}
