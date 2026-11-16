//! Window state, saving, story/chapter switching and shortcuts.

use super::*;

impl FolioApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        theme::install(&cc.egui_ctx);
        let shot = screenshot.map(|path| Shot {
            path,
            frames: 0,
            requested: false,
        });
        let screenshot_mode = shot.is_some();
        let (library, store_path) = if shot.is_some() {
            (Library::sample(), None)
        } else {
            crate::model::load_library()
        };
        // The library may name an installed font; install let the default
        // system serif stand in until the file is known.
        if !library.writing_font.is_empty() {
            let path = std::path::Path::new(&library.writing_font);
            if path.exists() {
                theme::use_writing_font(&cc.egui_ctx, Some(path));
            }
        }
        let active_story = library
            .active_story
            .min(library.stories.len().saturating_sub(1));
        let active_chapter = library
            .stories
            .get(active_story)
            .map(|s| s.open_chapter.min(s.chapters.len().saturating_sub(1)))
            .unwrap_or(0);
        // Handy for checking a particular view during development.
        let debug_view = std::env::var("FOLIO_VIEW").unwrap_or_default();
        let mut library = library;
        if debug_view == "empty" {
            library.stories.clear();
        } else if debug_view == "no-chapters" {
            if let Some(story) = library.stories.first_mut() {
                story.chapters.clear();
            }
        }
        let confirm = match debug_view.as_str() {
            "delete-story" => library.stories.first().map(|story| Confirm::Story {
                story_id: story.id,
                title: story.title.clone(),
                chapters: story.chapters.len(),
                lore: story.lore.len(),
            }),
            "delete-chapter" => library
                .stories
                .get(active_story)
                .and_then(|story| story.chapters.get(active_chapter).map(|c| (story, c)))
                .map(|(story, chapter)| Confirm::Chapter {
                    story_id: story.id,
                    chapter_id: chapter.id,
                    story_title: story.title.clone(),
                    title: chapter.title.clone(),
                    number: active_chapter + 1,
                    words: chapter.words(),
                    summarized: chapter.is_summarized(),
                }),
            "chat-confirm" => library
                .stories
                .get(active_story)
                .and_then(|story| story.chapters.get(active_chapter).map(|c| (story, c)))
                .map(|(story, chapter)| Confirm::Chat {
                    story_id: story.id,
                    chapter_id: chapter.id,
                }),
            _ => None,
        };
        let (story_title_draft, story_premise_draft, story_editor_story_id) =
            if debug_view == "story-editor" {
                match library.stories.get(active_story) {
                    Some(story) => (story.title.clone(), story.premise.clone(), Some(story.id)),
                    None => (String::new(), String::new(), None),
                }
            } else {
                (String::new(), String::new(), None)
            };
        let chapter_editor = if debug_view == "chapter-editor" {
            library
                .stories
                .get(active_story)
                .and_then(|story| story.chapters.get(active_chapter))
                .map(|chapter| ChapterDraft {
                    story_id: library.stories[active_story].id,
                    chapter_id: chapter.id,
                    title: chapter.title.clone(),
                    summary: chapter.summary.clone(),
                })
        } else {
            None
        };
        let export_dir = if library.export_dir.trim().is_empty() {
            crate::model::default_export_dir().display().to_string()
        } else {
            library.export_dir.clone()
        };
        let export_name = library
            .stories
            .get(active_story)
            .map(|story| story.title.clone())
            .unwrap_or_default();
        let mut app = Self {
            library,
            store_path,
            shot,
            active_story,
            active_chapter,
            right_open: true,
            right_tab: if debug_view == "lore" {
                Tab::Lore
            } else {
                Tab::Ai
            },
            settings_open: debug_view == "settings",
            prompts_open: debug_view == "prompts",
            show_key: false,
            new_story_open: debug_view == "new-story",
            new_story_title: if debug_view == "new-story" {
                "The Salt Orchard".to_string()
            } else {
                String::new()
            },
            new_story_premise: String::new(),
            new_chapter_open: false,
            new_chapter_title: String::new(),
            story_editor_open: debug_view == "story-editor",
            story_editor_story_id,
            story_title_draft,
            story_premise_draft,
            chapter_editor,
            confirm,
            trash: Vec::new(),
            trash_next_id: 1,
            trash_open: debug_view == "trash",
            lore_editor: None,
            lore_proposal: None,
            lore_search: String::new(),
            system_fonts: crate::fonts::system_fonts(),
            lore_filter: "All".to_string(),
            ai_input: String::new(),
            job: None,
            summary_queue: VecDeque::new(),
            summary_batch: None,
            models: Vec::new(),
            models_from: String::new(),
            models_rx: None,
            models_error: None,
            test_rx: None,
            test_status: None,
            probe: Probe::Idle,
            probe_from: String::new(),
            toast: None,
            dirty: false,
            last_edit: Instant::now(),
            summary_open: false,
            find_open: debug_view == "find" || debug_view == "find-replace",
            find_query: if debug_view == "find" || debug_view == "find-replace" {
                "door".to_string()
            } else {
                String::new()
            },
            find_replacement: if debug_view == "find" || debug_view == "find-replace" {
                "gate".to_string()
            } else {
                String::new()
            },
            find_match_case: false,
            find_replace: debug_view == "find-replace" || debug_view == "replace-confirm",
            find_index: None,
            find_focus: false,
            pending_selection: None,
            editor_selection: None,
            draft_live: None,
            draft_error: None,
            draft_follow: true,
            draft_reveal: false,
            scroll_chat: false,
            chat_follow: true,
            export_open: debug_view == "export"
                || debug_view == "export-txt"
                || debug_view == "export-done"
                || debug_view == "export-pdf"
                || debug_view == "export-overwrite",
            export_kind: if debug_view == "export-txt" || debug_view == "export-done" {
                ExportKind::Chapters
            } else {
                ExportKind::Pdf
            },
            export_dir,
            export_name,
            export_names: HashMap::new(),
            export_existing: None,
            export_status: None,
            export_written: None,
        };
        if debug_view == "settings-lan" {
            // A typical home setup: a LAN address, no key, and
            // the model list the endpoint reported.
            app.settings_open = true;
            app.library.settings.base_url = "192.168.10.20:5001/v1".to_string();
            app.library.settings.api_key.clear();
            app.library.settings.model = "local-model".to_string();
            app.library.settings.context_limit = 32768;
            app.models_from = app.library.settings.base_url.clone();
            app.models = vec![app.library.settings.model.clone()];
        }
        if debug_view == "settings-key" {
            // A key already typed, so the Clear key button is on screen.
            app.settings_open = true;
            app.library.settings.api_key = "sk-example-key-that-should-not-be-leaked".to_string();
        }
        if debug_view == "undo-toast" {
            if let Some(story) = app.library.stories.get(app.active_story) {
                if let Some(chapter) = story.chapters.get(app.active_chapter) {
                    app.delete_chapter_now(story.id, chapter.id);
                }
            }
        }
        if debug_view == "find-jump" {
            if let Some(story) = app.library.stories.get(app.active_story) {
                if let Some(chapter) = story.chapters.get(app.active_chapter) {
                    if let Some(range) = find_char_ranges(&chapter.body, "door", false).first() {
                        app.pending_selection = Some((chapter.id, range.clone()));
                    }
                }
            }
        }
        if debug_view == "replace-toast" {
            app.replace_in_book("door", "gate", false, None);
        }
        if debug_view == "replace-confirm" {
            app.find_open = true;
            app.find_query = "door".to_string();
            app.find_replacement = "gate".to_string();
            if let Some(story) = app.library.stories.get(app.active_story) {
                let query = app.find_query.clone();
                let matches: usize = story
                    .chapters
                    .iter()
                    .map(|chapter| find_char_ranges(&chapter.body, &query, false).len())
                    .sum();
                let chapters = story
                    .chapters
                    .iter()
                    .filter(|chapter| !find_char_ranges(&chapter.body, &query, false).is_empty())
                    .count();
                app.confirm = Some(Confirm::Replace {
                    query,
                    replacement: app.find_replacement.clone(),
                    match_case: false,
                    matches,
                    chapters,
                    story_title: story.title.clone(),
                    chapter_id: None,
                    chapter_title: String::new(),
                });
            }
        }
        if debug_view == "replace-undo" {
            app.replace_in_book("door", "gate", false, None);
            if let Some(Toast {
                undo: Some(UndoAction::Replace(snapshot)),
                ..
            }) = &app.toast
            {
                let snapshot = snapshot.clone();
                app.restore_replacement(snapshot);
            }
        }
        if debug_view == "trash" {
            let story_id = app.story_id();
            if let Some(chapter_id) = app
                .library
                .story(story_id)
                .and_then(|story| story.chapters.get(1))
                .map(|chapter| chapter.id)
            {
                app.delete_chapter_now(story_id, chapter_id);
            }
            if let Some(entry_id) = app
                .library
                .story(story_id)
                .and_then(|story| story.lore.get(2))
                .map(|entry| entry.id)
            {
                app.delete_lore_now(story_id, entry_id);
            }
        }
        if debug_view == "lore-editor" {
            if let Some(entry_id) = app
                .library
                .stories
                .get(app.active_story)
                .and_then(|story| story.lore.first())
                .map(|entry| entry.id)
            {
                app.edit_lore(Some(entry_id));
            }
        }
        if debug_view == "summary-hidden" {
            app.library.show_summary = false;
        }
        if debug_view == "summary-popup" || debug_view == "summary-streaming" {
            app.summary_open = true;
        }
        if debug_view == "lore-proposal" {
            // The shape the review window takes after a real pass: an existing
            // entry in the sample library, with facts to add.
            let story = app.library.stories.get(app.active_story);
            if let Some(story) = story {
                if let Some(entry) = story.lore.first() {
                    app.lore_proposal = Some(LoreProposal {
                        story_id: story.id,
                        target: Some(entry.id),
                        name: entry.name.clone(),
                        category: entry.category.clone(),
                        aliases: String::new(),
                        triggers: String::new(),
                        text: "The brass key is stamped with the initials E. VALE, and the salt on it never dries.".to_string(),
                        pinned: false,
                        well_formed: true,
                    });
                }
            }
        }
        if debug_view == "chat-streaming" {
            // A half-written brainstorm answer and a job that never ends, so the
            // live bubble and its follow-scroll are visible.
            if let Some(story) = app.library.stories.get_mut(app.active_story) {
                if let Some(chapter) = story.chapters.get_mut(app.active_chapter) {
                    let story_id = story.id;
                    let chapter_id = chapter.id;
                    chapter.chat = vec![ChatMessage {
                        role: "user".to_string(),
                        text: "What should Fluttershy do about the creature?".to_string(),
                        demo: false,
                    }];
                    let (tx, rx) = mpsc::channel::<StreamEvent>();
                    // Keep the sender alive: no Done is ever sent.
                    std::mem::forget(tx);
                    let mut stream = StreamFold::default();
                    stream.text = "Since Fluttershy has never seen the creature before, I would suggest she asks Lyra first. Lyra is the one who knows the town's rules, and the question puts her in a position where she has to choose between explaining and deflecting.".to_string();
                    app.job = Some(Job {
                        kind: JobKind::Action(Action::Brainstorm),
                        story_id,
                        chapter_id,
                        stream,
                        rx,
                        cancel: Arc::new(AtomicBool::new(false)),
                        _started: Instant::now(),
                    });
                    app.chat_follow = true;
                }
            }
        }
        if debug_view == "proofread-streaming" {
            // A spelling pass halfway through: both Stop buttons and the
            // progress hint are visible.
            if let Some(story) = app.library.stories.get(app.active_story) {
                if let Some(chapter) = story.chapters.get(app.active_chapter) {
                    let story_id = story.id;
                    let chapter_id = chapter.id;
                    let base_hash = crate::model::content_hash(&chapter.body);
                    let (tx, rx) = mpsc::channel::<StreamEvent>();
                    // Keep the sender alive: no Done is ever sent.
                    std::mem::forget(tx);
                    let mut stream = StreamFold::default();
                    stream.progress = Some((3, 12));
                    app.job = Some(Job {
                        kind: JobKind::Proofread { base_hash },
                        story_id,
                        chapter_id,
                        stream,
                        rx,
                        cancel: Arc::new(AtomicBool::new(false)),
                        _started: Instant::now(),
                    });
                }
            }
        }
        if debug_view == "summary-streaming" {
            // A half-written summary and a job that never ends, so the live
            // block, the word count and Stop are visible.
            if let Some(story) = app.library.stories.get(app.active_story) {
                if let Some(chapter) = story.chapters.get(app.active_chapter) {
                    let story_id = story.id;
                    let chapter_id = chapter.id;
                    let asked_title = chapter.title.clone();
                    let mut stream = StreamFold::default();
                    stream.text = "Mira finds her mother's house sealed with a wall of warm \
stones, and a boy at the gate hands her a key that is too large for any door. The key \
opens the garden gate instead."
                        .to_string();
                    let (tx, rx) = mpsc::channel::<StreamEvent>();
                    // Keep the sender alive: no Done is ever sent.
                    std::mem::forget(tx);
                    app.summary_batch = Some(SummaryBatch {
                        total: 1,
                        asked_title,
                    });
                    app.job = Some(Job {
                        kind: JobKind::Summary {
                            story_id,
                            chapter_id,
                            base_len: chapter.body.chars().count(),
                            base_hash: crate::model::content_hash(&chapter.body),
                        },
                        story_id,
                        chapter_id,
                        stream,
                        rx,
                        cancel: Arc::new(AtomicBool::new(false)),
                        _started: Instant::now(),
                    });
                }
            }
        }
        if debug_view == "recap" {
            // A window narrow enough that the story-so-far block appears next to
            // the recent summaries.
            app.library.settings.summary_window = 1;
            if let Some(story) = app.library.stories.get_mut(app.active_story) {
                if story.chapters.len() > 2 {
                    story.open_chapter = 2;
                    app.active_chapter = 2;
                }
            }
        }
        if debug_view == "summary-batch" {
            // Three unsummarized chapters and one press: the whole run has to
            // finish in a single frame with the offline writer.
            app.library.settings.summary_window = 1;
            let ready = match app.library.stories.get_mut(app.active_story) {
                Some(story) if story.chapters.len() > 2 => {
                    for chapter in story.chapters.iter_mut() {
                        chapter.summary.clear();
                    }
                    story.open_chapter = 2;
                    true
                }
                _ => false,
            };
            if ready {
                app.active_chapter = 2;
                app.summarize_chapter(app.active_story, 2);
            }
        }
        if debug_view == "chapter-chat" || debug_view == "chapter-chat-other" {
            // One conversation per chapter: each chapter keeps its own, and a
            // long one has already folded its oldest turns into a memory.
            app.library.settings.base_url = "192.168.10.20:5001/v1".to_string();
            app.library.settings.api_key.clear();
            app.library.settings.model = "local-model".to_string();
            app.library.settings.context_limit = 32768;
            let other = debug_view == "chapter-chat-other";
            if let Some(story) = app.library.stories.get_mut(app.active_story) {
                if story.chapters.len() > 1 {
                    story.chapters[0].chat = vec![
                        ChatMessage {
                            role: "user".to_string(),
                            text: "What is Wren hiding about the harbour lights?".to_string(),
                            demo: false,
                        },
                        ChatMessage {
                            role: "assistant".to_string(),
                            text: "Wren keeps two lanterns because one of them is not for the harbour. Have them answer your question with a silence and a change of subject — the coast knows more than they will say on the pier.".to_string(),
                            demo: false,
                        },
                    ];
                    story.chapters[1].chat = vec![
                        ChatMessage {
                            role: "user".to_string(),
                            text: "Who is E. Vale, and why is the key too large for any door?".to_string(),
                            demo: false,
                        },
                        ChatMessage {
                            role: "assistant".to_string(),
                            text: "The key is too large because the door it opens is not a door in this house yet. Let Mira test it against the garden gate first, so the reader feels the size of it before it matters.".to_string(),
                            demo: false,
                        },
                    ];
                    story.chapters[1].chat_summary = "Author: Should the wall of stones be warm or cold? Partner: Warm — it is the first sign that the house is not empty. Author: And the boy at the gate? Partner: Give him the key without explaining it; he is a messenger, not a guide. Write the discovery slowly and let the tea still be steaming when Mira reaches the map room.".to_string();
                    let open = if other { 0 } else { 1 };
                    story.open_chapter = open;
                    app.active_chapter = open;
                }
            }
        }
        if debug_view == "export-txt" || debug_view == "export-done" {
            // A name the author typed, to show the file name field at work.
            app.export_name = "Moonlight Draft".to_string();
        }
        if debug_view == "export-overwrite" {
            // Files from an earlier export, so the dialog has something to ask
            // about.
            let dir = std::env::temp_dir().join("folio-export-shot");
            if let Some(story) = app.library.stories.get(app.active_story).cloned() {
                app.export_kind = ExportKind::Chapters;
                app.export_name = "Moonlight Draft".to_string();
                app.export_dir = dir.display().to_string();
                let name = app.export_file_stem();
                let _ = crate::model::export_chapters(&story, &name, &dir);
                let targets = export_targets(&story, &name, &dir, app.export_kind);
                app.export_existing = Some(existing_names(&targets));
            }
        }
        if debug_view == "export-pdf" || debug_view == "export-done" {
            // A real run into a throwaway folder, so the success line on the
            // screenshot tells the truth.
            let dir = std::env::temp_dir().join("folio-export-shot");
            if let Some(story) = app.library.stories.get(app.active_story).cloned() {
                if debug_view == "export-pdf" {
                    app.export_kind = ExportKind::Pdf;
                    let name = app.export_file_stem();
                    if let Ok(path) = crate::model::export_pdf(&story, &name, &dir) {
                        app.export_dir = dir.display().to_string();
                        app.export_status = Some((
                            true,
                            format!(
                                "Wrote {} to {}",
                                path.file_name().unwrap_or_default().to_string_lossy(),
                                dir.display()
                            ),
                        ));
                        app.export_written = Some(dir);
                    }
                } else {
                    let name = app.export_file_stem();
                    let _ = crate::model::export_chapters(&story, &name, &dir);
                    app.export_dir = dir.display().to_string();
                    app.export_status = Some((
                        true,
                        format!(
                            "Wrote {} to {}",
                            plural(story.chapters.len(), "chapter file", "chapter files"),
                            dir.display()
                        ),
                    ));
                    app.export_written = Some(dir);
                }
            }
        }
        // The lamp: a fixed answer while drawing a screenshot, a real question
        // at startup. The demo writer has no endpoint to ask, so the dot stays
        // grey and says why on hover.
        let probe_base = "192.168.10.20:5001/v1".to_string();
        match debug_view.as_str() {
            "probe-online" => {
                app.library.settings.base_url = probe_base.clone();
                app.library.settings.api_key.clear();
                app.library.settings.model = "local-model".to_string();
                app.library.settings.context_limit = 32768;
                app.models_from = probe_base.clone();
                app.models = vec!["local-model".to_string()];
                app.probe = Probe::Online {
                    model: "local-model".to_string(),
                    context: Some(32768),
                    note: "Connected (HTTP 200). 1 model is listed.".to_string(),
                };
                app.probe_from = probe_base;
            }
            "probe-offline" => {
                app.library.settings.base_url = probe_base.clone();
                app.library.settings.api_key.clear();
                app.library.settings.model = "local-model".to_string();
                app.probe = Probe::Offline(
                    "Could not reach the endpoint. Check the address and your connection. (connection refused)"
                        .to_string(),
                );
                app.probe_from = probe_base;
            }
            "probe-checking" => {
                app.library.settings.base_url = probe_base.clone();
                app.library.settings.api_key.clear();
                app.library.settings.model = "local-model".to_string();
                app.models_from = probe_base.clone();
                app.probe = Probe::Checking;
                app.probe_from = probe_base;
            }
            // A real run against a throwaway local endpoint, for testing the
            // lamp without touching the saved library.
            "probe-live" => {
                app.library.settings.base_url = std::env::var("FOLIO_PROBE_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:5099/v1".to_string());
                app.library.settings.api_key = "test-key".to_string();
                app.library.settings.model = "mock-model".to_string();
            }
            _ => {}
        }
        // The draft block, drawn without an endpoint. A fixed ready draft, or a
        // half-written one with a job that never finishes, so the spinner and
        // Stop button are visible.
        if matches!(debug_view.as_str(), "draft-ready" | "draft-streaming") {
            let target = app.library.stories.get(app.active_story).and_then(|story| {
                story.chapters.get(app.active_chapter).map(|chapter| {
                    (
                        story.id,
                        chapter.id,
                        chapter.body.chars().count(),
                        crate::model::content_hash(&chapter.body),
                    )
                })
            });
            if let Some((story_id, chapter_id, base_len, base_hash)) = target {
                let text = "The wind changed before Mira reached the door. It came off the water \
carrying the smell of cold iron, and the house settled around her as if it had been waiting."
                    .to_string();
                let draft = Draft {
                    action: "continue".to_string(),
                    text: if debug_view == "draft-streaming" {
                        "The wind changed before Mira reached the door.".to_string()
                    } else {
                        text
                    },
                    range: None,
                    source: None,
                    base_len,
                    base_hash,
                    stopped: false,
                };
                if debug_view == "draft-ready" {
                    if let Some(chapter) = app
                        .library
                        .story_mut(story_id)
                        .and_then(|story| story.chapter_mut(chapter_id))
                    {
                        chapter.draft = Some(draft);
                    }
                } else {
                    let (tx, rx) = mpsc::channel::<StreamEvent>();
                    // Keep the sender alive: no Done is ever sent, so the
                    // block stays in its "writing" state.
                    std::mem::forget(tx);
                    app.job = Some(Job {
                        kind: JobKind::Action(Action::Continue),
                        story_id,
                        chapter_id,
                        stream: StreamFold::default(),
                        rx,
                        cancel: Arc::new(AtomicBool::new(false)),
                        _started: Instant::now(),
                    });
                    app.draft_live = Some(LiveDraft {
                        story_id,
                        chapter_id,
                        draft,
                    });
                }
                app.draft_reveal = true;
            }
        }
        // The steering bar with a note in it, scrolled into view without a draft.
        if debug_view == "steering" {
            let story_index = app.active_story;
            let chapter_index = app.active_chapter;
            if let Some(chapter) = app
                .library
                .stories
                .get_mut(story_index)
                .and_then(|story| story.chapters.get_mut(chapter_index))
            {
                chapter.steering =
                    "The storm reaches the harbour before dawn, and Wren is already on the pier."
                        .to_string();
            }
            app.draft_reveal = true;
        }
        if (!screenshot_mode || debug_view == "probe-live") && !app.library.settings.is_demo() {
            // The very run the Settings window makes when someone presses
            // "Test connection" — once, so the lamp is honest from the start.
            app.start_test();
        }
        app
    }

    pub(crate) fn clamp(&mut self) {
        if self.library.stories.is_empty() {
            self.active_story = 0;
            self.active_chapter = 0;
            self.library.active_story = 0;
            return;
        }
        self.active_story = self
            .active_story
            .min(self.library.stories.len().saturating_sub(1));
        let max = self.library.stories[self.active_story]
            .chapters
            .len()
            .saturating_sub(1);
        self.active_chapter = self.active_chapter.min(max);
        self.library.active_story = self.active_story;
    }

    pub(crate) fn story_id(&self) -> u64 {
        self.library
            .stories
            .get(self.active_story)
            .map(|s| s.id)
            .unwrap_or(0)
    }

    pub(crate) fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.toast = Some(Toast {
            text: text.into(),
            error,
            born: Instant::now(),
            undo: None,
        });
    }

    /// A delete notice stays a little longer and carries an Undo button. The
    /// deleted item itself stays in the session trash until Folio closes.
    pub(crate) fn notify_deleted(&mut self, text: impl Into<String>, undo_id: u64) {
        self.toast = Some(Toast {
            text: text.into(),
            error: false,
            born: Instant::now(),
            undo: Some(UndoAction::Delete(undo_id)),
        });
    }

    pub(crate) fn notify_replaced(&mut self, text: impl Into<String>, undo: ReplaceUndo) {
        self.toast = Some(Toast {
            text: text.into(),
            error: false,
            born: Instant::now(),
            undo: Some(UndoAction::Replace(undo)),
        });
    }

    pub(crate) fn next_trash_id(&mut self) -> u64 {
        let id = self.trash_next_id;
        self.trash_next_id = self.trash_next_id.saturating_add(1);
        id
    }

    pub(crate) fn save_now(&mut self) {
        if self.shot.is_some() {
            return;
        }
        let Some(path) = self.store_path.clone() else {
            return;
        };
        match crate::model::save_library(&path, &self.library) {
            Ok(()) => {
                self.dirty = false;
            }
            Err(error) => {
                self.notify(error, true);
            }
        }
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
        self.last_edit = Instant::now();
    }

    pub(crate) fn switch_story(&mut self, index: usize) {
        if index == self.active_story || index >= self.library.stories.len() {
            return;
        }
        self.active_story = index;
        self.library.active_story = index;
        self.active_chapter = self.library.stories[index]
            .open_chapter
            .min(self.library.stories[index].chapters.len().saturating_sub(1));
        self.summary_open = false;
        self.editor_selection = None;
        self.find_index = None;
        self.ai_input.clear();
        // The new chapter brings its own conversation; show the newest turns.
        self.scroll_chat = true;
        self.chat_follow = true;
        self.mark_dirty();
    }

    pub(crate) fn switch_chapter(&mut self, index: usize) {
        if index == self.active_chapter
            || index >= self.library.stories[self.active_story].chapters.len()
        {
            return;
        }
        self.active_chapter = index;
        self.library.stories[self.active_story].open_chapter = index;
        self.summary_open = false;
        self.editor_selection = None;
        self.find_index = None;
        self.scroll_chat = true;
        self.chat_follow = true;
        self.mark_dirty();
    }

    pub(crate) fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::S)) {
            self.save_now();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::F)) {
            self.open_find();
        }
        if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(Key::N)) {
            self.new_story_open = true;
        } else if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::N)) {
            if self.library.stories.is_empty() {
                self.new_story_open = true;
            } else {
                self.new_chapter_open = true;
                let count = self
                    .library
                    .stories
                    .get(self.active_story)
                    .map(|s| s.chapters.len() + 1)
                    .unwrap_or(1);
                self.new_chapter_title = format!("Chapter {count}");
            }
        }
    }

    pub(crate) fn autosave(&mut self, ctx: &egui::Context) {
        if self.dirty && self.shot.is_none() {
            if self.last_edit.elapsed() >= Duration::from_millis(700) {
                self.save_now();
            } else {
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        }
    }

    pub(crate) fn handle_screenshot(&mut self, ctx: &egui::Context) {
        if self.shot.is_none() {
            return;
        }
        ctx.request_repaint();
        let captured = ctx.input(|input| {
            input.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(image) = captured {
            let path = self
                .shot
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_default();
            match theme::save_png(&path, &image) {
                Ok(()) => println!("screenshot saved to {}", path.display()),
                Err(error) => eprintln!("screenshot failed: {error}"),
            }
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        let mut request = false;
        let mut give_up = false;
        if let Some(shot) = &mut self.shot {
            shot.frames += 1;
            // A picture of the lamp should show its answer, not a spinner:
            // wait for a running check to finish. The test gives up on its own
            // after twenty seconds, so this cannot hang forever.
            let settling =
                self.probe.is_checking() && (self.test_rx.is_some() || self.models_rx.is_some());
            if shot.frames >= 120 && !shot.requested && !settling {
                shot.requested = true;
                request = true;
            }
            if (shot.frames > 900 && !settling) || shot.frames > 4000 {
                give_up = true;
            }
        }
        if request {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::UserData::default()));
        }
        if give_up {
            eprintln!("screenshot did not arrive; closing");
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    // ---------------------------------------------------------------- top bar
}
