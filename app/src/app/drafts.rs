//! What happens when the author asks the partner to write: prose drafts, proofreading, brainstorm chat and lore suggestions.

use super::*;

impl FolioApp {
    pub(crate) fn run_action(&mut self, action: Action, ask: String) {
        if self.job.is_some() {
            self.notify("Wait for the current reply to finish first.", false);
            return;
        }
        let demo = self.library.settings.is_demo();
        let (story_id, chapter_id, range, passage, demo_text, prompt, base_hash, base_len) = {
            let Some(story) = self.library.stories.get(self.active_story) else {
                return;
            };
            let Some(chapter) = story.chapters.get(self.active_chapter) else {
                return;
            };
            if matches!(action, Action::Rewrite | Action::Elaborate) && chapter.words() < 3 {
                self.notify("Write a paragraph first, then ask for a rewrite.", false);
                return;
            }
            let body = chapter.body.clone();
            let body_chars = body.chars().count();
            let base_hash = crate::model::content_hash(&body);
            let mut range: Option<Range<usize>> = None;
            if matches!(action, Action::Rewrite | Action::Elaborate) {
                if let Some(selection) = self.editor_selection.clone() {
                    if selection.start < selection.end && selection.end <= body_chars {
                        range = Some(selection);
                    }
                }
                if range.is_none() {
                    range = last_paragraph_range(&body);
                }
            }
            let passage = range
                .as_ref()
                .map(|r| slice_chars(&body, r).to_string())
                .unwrap_or_default();
            let demo_text = if demo {
                Some(ai::demo_reply(action, story, chapter, &ask, &passage))
            } else {
                None
            };
            let prompt = if demo {
                None
            } else {
                Some(ai::chat_prompt(
                    story,
                    chapter,
                    action,
                    &ask,
                    &passage,
                    &self.library.settings,
                ))
            };
            (
                story.id, chapter.id, range, passage, demo_text, prompt, base_hash, body_chars,
            )
        };
        let prose = action.is_prose();
        // Prose gets its own draft block. The brainstorm conversation is a
        // separate session and never feeds a continuation.
        let (history, memory) = if prose {
            (Vec::new(), String::new())
        } else {
            // A long conversation is folded into its memory before it is sent,
            // so the request stays under 85% of the endpoint's window.
            let mut condensed = false;
            if let Some(prompt) = prompt.as_ref() {
                let budget = ai::history_budget(&self.library.settings, prompt);
                if budget != usize::MAX {
                    if let Some(chapter) = self
                        .library
                        .story_mut(story_id)
                        .and_then(|story| story.chapter_mut(chapter_id))
                    {
                        condensed = chapter.compact_conversation(budget);
                    }
                }
            }
            if condensed {
                self.mark_dirty();
                self.notify(
                    "The oldest turns of this conversation were condensed to make room.",
                    false,
                );
            }
            // The conversation belongs to this chapter alone. It is read before
            // the new question lands in it, because the prompt already carries
            // the ask.
            self.library
                .story(story_id)
                .and_then(|story| story.chapter(chapter_id))
                .map(|chapter| (chapter.chat.clone(), chapter.chat_summary.clone()))
                .unwrap_or_default()
        };
        if action == Action::Brainstorm && !ask.trim().is_empty() {
            if let Some(chapter) = self
                .library
                .story_mut(story_id)
                .and_then(|story| story.chapter_mut(chapter_id))
            {
                chapter.chat.push(ChatMessage {
                    role: "user".to_string(),
                    text: ask.trim().to_string(),
                    demo: false,
                });
            }
            self.scroll_chat = true;
            self.chat_follow = true;
        }
        let source = if passage.trim().is_empty() {
            None
        } else {
            Some(passage)
        };
        if prose {
            // A new request replaces any draft that was still waiting.
            let replaced = self
                .library
                .story_mut(story_id)
                .and_then(|story| story.chapter_mut(chapter_id))
                .and_then(|chapter| chapter.draft.take())
                .is_some();
            if replaced {
                self.mark_dirty();
            }
            let draft = Draft {
                action: match action {
                    Action::Rewrite => "rewrite".to_string(),
                    Action::Elaborate => "elaborate".to_string(),
                    _ => "continue".to_string(),
                },
                text: String::new(),
                range: range.as_ref().map(|r| (r.start, r.end)),
                source: source.clone(),
                base_len,
                base_hash,
                stopped: false,
            };
            self.draft_live = Some(LiveDraft {
                story_id,
                chapter_id,
                draft,
            });
            self.draft_error = None;
            self.draft_follow = true;
            self.draft_reveal = true;
        }
        if let Some(text) = demo_text {
            if prose {
                let (rx, cancel) = start_demo_stream(text);
                self.job = Some(Job {
                    kind: JobKind::Action(action),
                    story_id,
                    chapter_id,
                    stream: StreamFold::default(),
                    rx,
                    cancel,
                    _started: Instant::now(),
                });
                return;
            }
            if let Some(chapter) = self
                .library
                .story_mut(story_id)
                .and_then(|story| story.chapter_mut(chapter_id))
            {
                chapter.chat.push(ChatMessage {
                    role: "assistant".to_string(),
                    text,
                    demo: true,
                });
            }
            self.scroll_chat = true;
            self.mark_dirty();
            return;
        }
        let prompt = prompt.unwrap_or_default();
        let settings = self.library.settings.clone();
        // Only Continue and Rewrite carry a word target. A brainstorm answer
        // ends when the question has been answered.
        let words = action.kind().words(&settings.prompts);
        let (rx, cancel) = start_stream(settings, prompt, history, memory, words, true);
        self.job = Some(Job {
            kind: JobKind::Action(action),
            story_id,
            chapter_id,
            stream: StreamFold::default(),
            rx,
            cancel,
            _started: Instant::now(),
        });
        self.scroll_chat = true;
    }

    /// Send the question in the field. The panel has no other way in, so an
    /// empty field sends nothing.
    pub(crate) fn send_brainstorm(&mut self) {
        let Some(text) = brainstorm_send_text(&self.ai_input) else {
            return;
        };
        self.ai_input.clear();
        self.run_action(Action::Brainstorm, text);
    }

    /// Read a marked passage against the lorebook. The reply is parsed into a
    /// proposal; nothing changes until the author saves it in the window.
    pub(crate) fn start_lore_entry(&mut self, story_id: u64, chapter_id: u64, passage: String) {
        if self.job.is_some() {
            self.notify("Wait for the current reply to finish first.", false);
            return;
        }
        let passage = passage.trim().to_string();
        if passage.is_empty() {
            self.notify("Select a passage first.", false);
            return;
        }
        let demo = self.library.settings.is_demo();
        let Some(story) = self.library.story(story_id) else {
            return;
        };
        if story.chapter(chapter_id).is_none() {
            return;
        }
        if demo {
            let reply = ai::demo_lore_reply(story, &passage);
            let suggestion = ai::parse_lore_reply(&reply, story);
            self.open_lore_proposal(story_id, suggestion);
            return;
        }
        let prompt = ai::lore_prompt(story, &passage);
        let mut settings = self.library.settings.clone();
        // Facts, not atmosphere: the lorebook pass reads the page like a record.
        settings.temperature = settings.temperature.min(0.3);
        let (rx, cancel) = start_stream(settings, prompt, Vec::new(), String::new(), 320, false);
        self.job = Some(Job {
            kind: JobKind::Lore,
            story_id,
            chapter_id,
            stream: StreamFold::default(),
            rx,
            cancel,
            _started: Instant::now(),
        });
        self.notify("Reading the selection for the lorebook…", false);
    }

    pub(crate) fn open_lore_proposal(&mut self, story_id: u64, suggestion: ai::LoreSuggestion) {
        if suggestion.text.trim().is_empty() {
            self.notify("The lorebook pass came back with no text to save.", true);
            return;
        }
        if !suggestion.well_formed {
            self.notify(
                "The reply did not follow the lorebook shape; please check the fields.",
                false,
            );
        }
        self.lore_proposal = Some(LoreProposal {
            story_id,
            target: suggestion.target,
            name: if suggestion.name.is_empty() {
                suggestion.target_name
            } else {
                suggestion.name
            },
            category: suggestion.category,
            aliases: suggestion.aliases,
            triggers: suggestion.triggers,
            text: suggestion.text,
            pinned: false,
            well_formed: suggestion.well_formed,
        });
    }

    /// Put the waiting draft into the chapter. The draft is taken out first, so
    /// a double click can never append the same text twice.
    pub(crate) fn accept_draft(&mut self, story_id: u64, chapter_id: u64) {
        let draft = self
            .library
            .story_mut(story_id)
            .and_then(|story| story.chapter_mut(chapter_id))
            .and_then(|chapter| chapter.draft.take());
        let Some(draft) = draft else {
            return;
        };
        let replaces = matches!(draft.action.as_str(), "rewrite" | "elaborate");
        let outcome = (|| -> Result<(), String> {
            let chapter = self
                .library
                .story_mut(story_id)
                .ok_or_else(|| "That story is no longer here.".to_string())?
                .chapter_mut(chapter_id)
                .ok_or_else(|| "That chapter is no longer here.".to_string())?;
            accept_into(chapter, &draft)?;
            Ok(())
        })();
        match outcome {
            Ok(()) => {
                self.mark_dirty();
                self.notify(
                    if replaces {
                        "Replaced the passage with the AI draft."
                    } else {
                        "Added the AI draft to the end of the chapter."
                    },
                    false,
                );
            }
            Err(error) => {
                // Keep the draft so the author can retry or delete it.
                if let Some(chapter) = self
                    .library
                    .story_mut(story_id)
                    .and_then(|story| story.chapter_mut(chapter_id))
                {
                    if chapter.draft.is_none() {
                        chapter.draft = Some(draft);
                    }
                }
                self.notify(error, true);
            }
        }
    }

    pub(crate) fn discard_draft(&mut self, story_id: u64, chapter_id: u64) {
        let mut had = false;
        if self
            .draft_live
            .as_ref()
            .is_some_and(|live| live.story_id == story_id && live.chapter_id == chapter_id)
        {
            self.draft_live = None;
            self.draft_error = None;
            // If the job is still running, stop it; its text has no home now.
            if self
                .job
                .as_ref()
                .is_some_and(|job| job.story_id == story_id && job.chapter_id == chapter_id)
            {
                if let Some(job) = self.job.as_ref() {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            }
            had = true;
        }
        if let Some(chapter) = self
            .library
            .story_mut(story_id)
            .and_then(|story| story.chapter_mut(chapter_id))
        {
            had |= chapter.draft.take().is_some();
        }
        if had {
            self.mark_dirty();
            self.notify("Draft deleted.", false);
        }
    }

    pub(crate) fn retry_draft(&mut self, story_id: u64, chapter_id: u64) {
        let action = self
            .library
            .story(story_id)
            .and_then(|story| story.chapter(chapter_id))
            .and_then(|chapter| chapter.draft.as_ref())
            .map(|draft| draft.action.clone())
            .or_else(|| {
                self.draft_live.as_ref().and_then(|live| {
                    (live.story_id == story_id && live.chapter_id == chapter_id)
                        .then(|| live.draft.action.clone())
                })
            });
        let action = match action.as_deref() {
            Some("rewrite") => Action::Rewrite,
            Some("elaborate") => Action::Elaborate,
            Some(_) => Action::Continue,
            None => return,
        };
        self.discard_draft_quiet(story_id, chapter_id);
        self.run_action(action, String::new());
    }

    /// Drop a draft without a notice, used when Retry replaces it at once.
    pub(crate) fn discard_draft_quiet(&mut self, story_id: u64, chapter_id: u64) {
        if self
            .draft_live
            .as_ref()
            .is_some_and(|live| live.story_id == story_id && live.chapter_id == chapter_id)
        {
            self.draft_live = None;
            self.draft_error = None;
        }
        if let Some(chapter) = self
            .library
            .story_mut(story_id)
            .and_then(|story| story.chapter_mut(chapter_id))
        {
            chapter.draft = None;
        }
    }

    pub(crate) fn stop_draft(&mut self, story_id: u64, chapter_id: u64) {
        let running = self.job.as_ref().is_some_and(|job| {
            job.story_id == story_id
                && job.chapter_id == chapter_id
                && matches!(&job.kind, JobKind::Action(action) if action.is_prose())
        });
        if !running {
            return;
        }
        if let Some(job) = self.job.as_ref() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        if let Some(live) = self.draft_live.as_mut() {
            if live.story_id == story_id && live.chapter_id == chapter_id {
                live.draft.stopped = true;
            }
        }
        self.notify("Stopping the draft — the text so far is kept.", false);
    }

    pub(crate) fn stop_chat(&mut self, story_id: u64, chapter_id: u64) {
        let running = self.job.as_ref().is_some_and(|job| {
            job.story_id == story_id
                && job.chapter_id == chapter_id
                && matches!(
                    job.kind,
                    JobKind::Action(Action::Brainstorm) | JobKind::Proofread { .. }
                )
        });
        if !running {
            return;
        }
        let proofread = self
            .job
            .as_ref()
            .is_some_and(|job| matches!(job.kind, JobKind::Proofread { .. }));
        if let Some(job) = self.job.as_ref() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.notify(
            if proofread {
                "Stopping the check — the chapter stays unchanged."
            } else {
                "Stopping the answer — the text that arrived is kept."
            },
            false,
        );
    }

    pub(crate) fn stop_proofread(&mut self, story_id: u64, chapter_id: u64) {
        let running = self.job.as_ref().is_some_and(|job| {
            job.story_id == story_id
                && job.chapter_id == chapter_id
                && matches!(job.kind, JobKind::Proofread { .. })
        });
        if !running {
            return;
        }
        if let Some(job) = self.job.as_ref() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.notify("Stopping the check — the chapter stays unchanged.", false);
    }

    /// Proofread the chapter one paragraph at a time. The worker sends a
    /// Progress event after each paragraph and the finished body as one delta;
    /// nothing is written until the whole pass is done.
    pub(crate) fn start_proofread(&mut self, story_id: u64, chapter_id: u64) {
        if self.job.is_some() {
            self.notify("Wait for the current reply to finish first.", false);
            return;
        }
        if self.library.settings.is_demo() {
            self.notify("Connect an endpoint to check spelling and grammar.", false);
            return;
        }
        let Some(chapter) = self
            .library
            .story(story_id)
            .and_then(|story| story.chapter(chapter_id))
        else {
            return;
        };
        if chapter.words() == 0 {
            self.notify("Write a little first — there is nothing to check.", false);
            return;
        }
        let body = chapter.body.clone();
        let base_hash = crate::model::content_hash(&body);
        let ranges = paragraph_ranges(&body);
        if ranges.is_empty() {
            self.notify("There is nothing to check.", false);
            return;
        }
        let paragraphs: Vec<String> = ranges
            .iter()
            .map(|range| slice_chars(&body, range).to_string())
            .collect();
        let total = paragraphs.len();
        let settings = self.library.settings.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut corrections: Vec<(Range<usize>, String)> = Vec::new();
            for (index, (range, original)) in
                ranges.into_iter().zip(paragraphs.into_iter()).enumerate()
            {
                if worker_cancel.load(Ordering::Relaxed) {
                    let _ = tx.send(StreamEvent::Done { truncated: false });
                    return;
                }
                match ai::proofread_paragraph(&settings, &original, &worker_cancel) {
                    Ok(corrected) => {
                        let corrected = corrected.trim().to_string();
                        if !corrected.is_empty() && corrected != original {
                            corrections.push((range, corrected));
                        }
                        let _ = tx.send(StreamEvent::Progress {
                            done: index + 1,
                            total,
                        });
                    }
                    Err(error) => {
                        let _ = tx.send(StreamEvent::Failed(error));
                        return;
                    }
                }
            }
            if worker_cancel.load(Ordering::Relaxed) {
                let _ = tx.send(StreamEvent::Done { truncated: false });
                return;
            }
            let mut chars: Vec<char> = body.chars().collect();
            for (range, text) in corrections.iter().rev() {
                chars.splice(range.clone(), text.chars());
            }
            let corrected: String = chars.into_iter().collect();
            let _ = tx.send(StreamEvent::Delta(corrected));
            let _ = tx.send(StreamEvent::Done { truncated: false });
        });
        self.job = Some(Job {
            kind: JobKind::Proofread { base_hash },
            story_id,
            chapter_id,
            stream: StreamFold::default(),
            rx,
            cancel,
            _started: Instant::now(),
        });
        self.notify(
            format!("Checking {}…", plural(total, "paragraph", "paragraphs")),
            false,
        );
    }

    /// Write the checked body back, unless the chapter moved while the pass
    /// ran. The old body goes into the undo notice.
    pub(crate) fn apply_proofread(
        &mut self,
        story_id: u64,
        chapter_id: u64,
        base_hash: u64,
        text: String,
    ) {
        if text.trim().is_empty() {
            self.notify("Proofreading stopped — the chapter is unchanged.", false);
            return;
        }
        let Some(chapter) = self
            .library
            .story_mut(story_id)
            .and_then(|story| story.chapter_mut(chapter_id))
        else {
            return;
        };
        if crate::model::content_hash(&chapter.body) != base_hash {
            self.notify(
                "The chapter changed while it was being checked. Nothing was applied.",
                true,
            );
            return;
        }
        if chapter.body == text {
            self.notify("No spelling or grammar corrections were needed.", false);
            return;
        }
        let old = chapter.body.clone();
        let title = chapter.title.clone();
        chapter.body = text;
        self.editor_selection = None;
        self.pending_selection = None;
        self.mark_dirty();
        self.notify_replaced(
            format!("Spelling and grammar checked for “{title}”."),
            ReplaceUndo {
                story_id,
                chapters: vec![(chapter_id, old)],
            },
        );
    }
}
