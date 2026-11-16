//! Chapter summaries, the older-chapter queue and the summary window.

use super::*;

impl FolioApp {
    /// The author pressed Summarize. Every older chapter that would reach the AI
    /// without a usable summary is written first, so the story-so-far block is
    /// complete before the open chapter itself is refreshed.
    pub(crate) fn summarize_chapter(&mut self, story_index: usize, chapter_index: usize) {
        if self.job.is_some() {
            self.notify("Wait for the current reply to finish first.", false);
            return;
        }
        let window = self.library.settings.recent_window();
        let Some(story) = self.library.stories.get(story_index) else {
            return;
        };
        let Some(chapter) = story.chapters.get(chapter_index) else {
            return;
        };
        if chapter.words() == 0 {
            self.notify(
                "Write a little first — there is nothing to summarize yet.",
                false,
            );
            return;
        }
        let story_id = story.id;
        let asked_title = chapter.title.clone();
        let ids = summary_queue(story, chapter_index, window);
        let total = ids.len();
        self.summary_queue = ids.into_iter().map(|id| (story_id, id)).collect();
        self.summary_batch = Some(SummaryBatch { total, asked_title });
        if total > 1 {
            self.notify(
                format!(
                    "Summarizing {} — the story so far is rebuilt from them.",
                    plural(total, "chapter", "chapters")
                ),
                false,
            );
        }
        self.start_next_summary();
    }

    /// Only the older chapters whose summaries the recap is missing. Useful
    /// when the open chapter's own summary is fine but the story so far has
    /// holes in it.
    pub(crate) fn summarize_older(&mut self, story_index: usize, chapter_index: usize) {
        if self.job.is_some() {
            self.notify("Wait for the current reply to finish first.", false);
            return;
        }
        let window = self.library.settings.recent_window();
        let Some(story) = self.library.stories.get(story_index) else {
            return;
        };
        let ids: Vec<u64> = story
            .chapters_needing_summary(chapter_index, window)
            .into_iter()
            .map(|chapter| chapter.id)
            .collect();
        let Some(last) = ids.last().copied() else {
            self.notify("Every older chapter already has a summary.", false);
            return;
        };
        let story_id = story.id;
        // For a run of one, this name is what the toast reports.
        let asked_title = story
            .chapters
            .iter()
            .find(|chapter| chapter.id == last)
            .map(|chapter| chapter.title.clone())
            .unwrap_or_default();
        let total = ids.len();
        self.summary_queue = ids.into_iter().map(|id| (story_id, id)).collect();
        self.summary_batch = Some(SummaryBatch { total, asked_title });
        self.start_next_summary();
    }

    /// Pull the next chapter off the queue and either summarize it locally or
    /// hand it to the endpoint. Ends the batch when the queue runs dry.
    pub(crate) fn start_next_summary(&mut self) {
        if self.job.is_some() {
            return;
        }
        let Some((story_id, chapter_id)) = self.summary_queue.pop_front() else {
            self.finish_summary_batch();
            return;
        };
        if self.library.settings.is_demo() {
            self.write_local_summary(story_id, chapter_id);
            // Local summaries are instant, so the whole batch runs in one frame.
            self.start_next_summary();
            return;
        }
        let Some(story) = self.library.story(story_id) else {
            self.finish_summary_batch();
            return;
        };
        let Some(chapter) = story.chapter(chapter_id) else {
            self.start_next_summary();
            return;
        };
        let prompt = ai::summary_prompt(story, chapter, &self.library.settings);
        // What the summary describes, not what the chapter happens to hold when
        // the answer arrives. An edit during the run must mark it stale.
        let base_len = chapter.body.chars().count();
        let base_hash = crate::model::content_hash(&chapter.body);
        let mut settings = self.library.settings.clone();
        // A summary is a record, not a scene: keep the model close to the facts
        // even when the author likes a lively prose temperature.
        settings.temperature = settings.temperature.min(0.3);
        let words = ai::PromptKind::Summary.words(&settings.prompts);
        // A summary is a fresh, self-contained request; the chapter's
        // conversation is not part of it. The word target keeps a talkative
        // model from wandering, and a reply that still runs into the limit is
        // continued instead of being stored half-finished.
        let (rx, cancel) = start_stream(settings, prompt, Vec::new(), String::new(), words, false);
        self.job = Some(Job {
            kind: JobKind::Summary {
                story_id,
                chapter_id,
                base_len,
                base_hash,
            },
            story_id,
            chapter_id,
            stream: StreamFold::default(),
            rx,
            cancel,
            _started: Instant::now(),
        });
    }

    /// Write a summary without an endpoint: three picked sentences, free and
    /// instant.
    pub(crate) fn write_local_summary(&mut self, story_id: u64, chapter_id: u64) {
        let Some(chapter) = self
            .library
            .story(story_id)
            .and_then(|s| s.chapter(chapter_id))
        else {
            return;
        };
        let summary = ai::local_summary(chapter);
        let body_len = chapter.body.chars().count();
        let body_hash = crate::model::content_hash(&chapter.body);
        if let Some(story) = self.library.story_mut(story_id) {
            if let Some(chapter) = story.chapter_mut(chapter_id) {
                chapter.summary = summary;
                chapter.summary_source = "local".to_string();
                chapter.summary_body_len = body_len;
                chapter.summary_body_hash = body_hash;
            }
        }
        self.mark_dirty();
    }

    /// Nothing left to summarize: say what happened and forget the batch.
    pub(crate) fn finish_summary_batch(&mut self) {
        let Some(batch) = self.summary_batch.take() else {
            return;
        };
        self.mark_dirty();
        let note = if batch.total == 1 {
            format!("Summary saved for “{}”.", batch.asked_title)
        } else {
            format!(
                "{} summarized — the story so far is up to date.",
                plural(batch.total, "chapter", "chapters")
            )
        };
        self.notify(note, false);
    }

    /// Progress for the summary card: how many chapters of the run are done.
    pub(crate) fn summary_progress(&self) -> Option<(usize, usize)> {
        let batch = self.summary_batch.as_ref()?;
        let running = usize::from(
            self.job
                .as_ref()
                .is_some_and(|job| matches!(job.kind, JobKind::Summary { .. })),
        );
        let left = self.summary_queue.len() + running;
        Some((batch.total.saturating_sub(left), batch.total))
    }

    /// Stop after the chapter that is already being written.
    pub(crate) fn stop_summary_batch(&mut self) {
        self.summary_queue.clear();
        self.summary_batch = None;
        self.notify(
            "Summarizing stopped — the summaries that were already written are kept.",
            false,
        );
    }

    /// The chapter summary in its own window, following the prompts window:
    /// a fixed page with a Done strip that never scrolls away and a body that
    /// scrolls once the summary outgrows it. The editor reserves no room.
    pub(crate) fn summary_window(&mut self, ctx: &egui::Context) {
        if !self.summary_open {
            return;
        }
        let story_index = self.active_story;
        let chapter_index = self.active_chapter;
        let Some(story) = self.library.stories.get(story_index) else {
            self.summary_open = false;
            return;
        };
        let Some(chapter) = story.chapters.get(chapter_index) else {
            self.summary_open = false;
            return;
        };
        let story_id = story.id;
        let chapter_id = chapter.id;
        let chapter_title = chapter.title.clone();
        let mut summary = chapter.summary.clone();
        let original_summary = summary.clone();
        let source = chapter.summary_source.clone();
        let body_chars = chapter.body.chars().count();
        let stale = chapter.summary_is_stale();
        let has_summary = chapter.is_summarized();
        let mentioned: Vec<String> = story
            .mentioned_lore(&chapter.body)
            .into_iter()
            .take(6)
            .map(|e| e.name.clone())
            .collect();
        let older_needed = story
            .chapters_needing_summary(chapter_index, self.library.settings.recent_window())
            .len();
        let demo = self.library.settings.is_demo();
        let busy = self.job.is_some();
        let progress = self.summary_progress();
        // The live summary, when this chapter is being written right now. Read
        // from the job, so opening the window mid-run shows the text so far.
        let live = self.job.as_ref().and_then(|job| match job.kind {
            JobKind::Summary {
                story_id: job_story,
                chapter_id: job_chapter,
                ..
            } if job_story == story_id && job_chapter == chapter_id => {
                Some(job.stream.text.clone())
            }
            _ => None,
        });
        let mut open = self.summary_open;
        let mut close = false;
        let mut do_summary = false;
        let mut do_older = false;
        let mut do_copy = false;
        let mut do_stop = false;

        Window::new("Chapter summary")
            .id(Id::new("summary-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(720.0, 560.0))
            .min_size(Vec2::new(560.0, 380.0))
            .show(ctx, |ui| {
                TopBottomPanel::bottom(Id::new("summary-footer"))
                    .frame(Frame::NONE)
                    .show_inside(ui, |ui| {
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            if primary(ui, "Done").clicked() {
                                close = true;
                            }
                            ui.label(
                                RichText::new(
                                    "The summary is what the AI remembers of this chapter.",
                                )
                                .size(9.5)
                                .color(C::muted()),
                            );
                        });
                    });
                CentralPanel::default()
                    .frame(Frame::NONE)
                    .show_inside(ui, |ui| {
                        ScrollArea::vertical()
                            .id_salt("summary-scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let dot = if has_summary {
                                        if stale {
                                            C::amber()
                                        } else {
                                            C::green()
                                        }
                                    } else {
                                        C::muted()
                                    };
                                    let (rect, _) =
                                        ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 2.6, dot);
                                    ui.label(caps(&chapter_title, C::muted()));
                                    if !has_summary {
                                        pill(ui, "Not summarized", C::amber(), C::amber_soft());
                                    } else if stale {
                                        pill(ui, "Needs refresh", C::amber(), C::amber_soft());
                                    } else {
                                        pill(ui, "In AI context", C::green(), C::green_soft());
                                    }
                                    if !source.is_empty() {
                                        let source_label = match source.as_str() {
                                            "ai" => "AI written",
                                            "local" => "Quick offline",
                                            "manual" => "Edited by you",
                                            _ => "Saved",
                                        };
                                        ui.label(
                                            RichText::new(source_label)
                                                .size(9.5)
                                                .color(C::muted()),
                                        );
                                    }
                                });
                                ui.add_space(4.0);
                                if summary.is_empty() && live.is_none() {
                                    ui.add(
                                        Label::new(
                                            RichText::new(if demo {
                                                "Summarize this chapter to keep the AI's memory short. Older chapters that no longer fit the recent window are summarized too, so the story so far stays whole. With no endpoint connected, Folio writes quick extractive summaries."
                                            } else {
                                                "Summarize this chapter to keep the AI's memory short and accurate. Older chapters that no longer fit the recent window are summarized too, so the story so far stays whole."
                                            })
                                            .size(11.5)
                                            .italics()
                                            .color(C::muted()),
                                        )
                                        .wrap(),
                                    );
                                    ui.add_space(6.0);
                                }

                                if let Some(live) = &live {
                                    // The summary grows here while it is written.
                                    Frame::NONE
                                        .fill(C::summary_bg())
                                        .stroke(Stroke::new(1.0_f32, C::line()))
                                        .corner_radius(10)
                                        .inner_margin(Margin::symmetric(12, 9))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.horizontal(|ui| {
                                                ui.spinner();
                                                ui.label(caps("Writing the summary", C::muted()));
                                                ui.with_layout(
                                                    Layout::right_to_left(Align::Center),
                                                    |ui| {
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "{} words",
                                                                live.split_whitespace().count()
                                                            ))
                                                            .size(9.5)
                                                            .color(C::muted()),
                                                        );
                                                    },
                                                );
                                            });
                                            ui.add_space(3.0);
                                            ui.add(
                                                Label::new(
                                                    RichText::new(if live.trim().is_empty() {
                                                        "…"
                                                    } else {
                                                        live.trim()
                                                    })
                                                    .font(serif_id(14.0))
                                                    .color(C::ink_soft())
                                                    .line_height(Some(20.0)),
                                                )
                                                .wrap(),
                                            );
                                        });
                                } else {
                                    TextEdit::multiline(&mut summary)
                                        .id(Id::new("summary-text"))
                                        .font(serif_id(14.0))
                                        .text_color(C::ink_soft())
                                        .frame(true)
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(14)
                                        .show(ui);
                                }
                                ui.add_space(8.0);
                                ui.horizontal_wrapped(|ui| {
                                    let label = match progress {
                                        Some((done, total)) if done < total => {
                                            format!("Summarizing {}/{total}…", done + 1)
                                        }
                                        _ if busy => "Working…".to_string(),
                                        _ => "Summarize".to_string(),
                                    };
                                    let button = Button::new(
                                        RichText::new(label)
                                            .size(11.5)
                                            .color(Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(C::accent())
                                    .stroke(Stroke::NONE)
                                    .corner_radius(8)
                                    .min_size(Vec2::new(86.0, 28.0));
                                    if ui
                                        .add_enabled(!busy, button)
                                        .on_hover_text(
                                            "Write the summaries this chapter needs — including the older chapters that feed the story so far",
                                        )
                                        .clicked()
                                    {
                                        do_summary = true;
                                    }
                                    if progress.is_some()
                                        && ghost(ui, "Stop")
                                            .on_hover_text(
                                                "Stop after the chapter being summarized right now",
                                            )
                                            .clicked()
                                    {
                                        do_stop = true;
                                    }
                                    if older_needed > 0 {
                                        let older_label = format!(
                                            "Summarize {} older {}",
                                            older_needed,
                                            plural(older_needed, "chapter", "chapters")
                                        );
                                        let older = Button::new(
                                            RichText::new(older_label)
                                                .size(10.5)
                                                .color(C::muted()),
                                        )
                                        .frame(false)
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE);
                                        if ui
                                            .add_enabled(!busy, older)
                                            .on_hover_text(
                                                "Write only the older summaries the story-so-far block is missing",
                                            )
                                            .clicked()
                                        {
                                            do_older = true;
                                        }
                                    }
                                    if has_summary {
                                        let copy = Button::new(
                                            RichText::new("Copy").size(10.5).color(C::muted()),
                                        )
                                        .frame(false)
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE);
                                        if ui.add_enabled(!live.is_some(), copy).clicked() {
                                            do_copy = true;
                                        }
                                    }
                                });
                                if !mentioned.is_empty() {
                                    ui.add_space(6.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new("IN THIS CHAPTER")
                                                .size(8.5)
                                                .strong()
                                                .color(C::muted()),
                                        );
                                        for name in &mentioned {
                                            pill(ui, name, C::ink_soft(), C::card());
                                        }
                                    });
                                }
                            });
                    });
            });

        if close {
            open = false;
        }
        self.summary_open = open;
        let copy_summary = summary.clone();
        if summary != original_summary {
            if let Some(chapter) = self
                .library
                .stories
                .get_mut(story_index)
                .and_then(|s| s.chapters.get_mut(chapter_index))
            {
                chapter.summary = summary;
                chapter.summary_source = "manual".to_string();
                chapter.summary_body_len = body_chars;
                chapter.summary_body_hash = crate::model::content_hash(&chapter.body);
            }
            self.mark_dirty();
        }
        if do_copy {
            // Copy what is on screen, including edits made since it opened.
            ctx.copy_text(copy_summary);
            self.notify("Summary copied.", false);
        }
        if do_summary {
            self.summarize_chapter(story_index, chapter_index);
        }
        if do_older {
            self.summarize_older(story_index, chapter_index);
        }
        if do_stop {
            self.stop_summary_batch();
        }
    }

    // ---------------------------------------------------------------- right desk
}

/// The chapters one Summarize press writes, in order: every missing older
/// summary first, then the open chapter itself.
pub(crate) fn summary_queue(story: &Story, position: usize, window: usize) -> Vec<u64> {
    let mut ids: Vec<u64> = story
        .chapters_needing_summary(position, window)
        .into_iter()
        .map(|chapter| chapter.id)
        .collect();
    if let Some(chapter) = story.chapters.get(position) {
        ids.push(chapter.id);
    }
    ids
}
