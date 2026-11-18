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
        let story_id = story.key.clone();
        let asked_title = chapter.title.clone();
        let ids = summary_queue(story, chapter_index, window);
        let total = ids.len();
        self.summary_queue = ids.into_iter().map(|id| (story_id.clone(), id)).collect();
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
        let ids: Vec<String> = story
            .chapters_needing_summary(chapter_index, window)
            .into_iter()
            .map(|chapter| chapter.key.clone())
            .collect();
        let Some(last) = ids.last().cloned() else {
            self.notify("Every older chapter already has a summary.", false);
            return;
        };
        let story_id = story.key.clone();
        // For a run of one, this name is what the toast reports.
        let asked_title = story
            .chapters
            .iter()
            .find(|chapter| chapter.key == last)
            .map(|chapter| chapter.title.clone())
            .unwrap_or_default();
        let total = ids.len();
        self.summary_queue = ids.into_iter().map(|id| (story_id.clone(), id)).collect();
        self.summary_batch = Some(SummaryBatch { total, asked_title });
        self.start_next_summary();
    }

    /// Pull the next chapter off the queue and either summarize it locally or
    /// start a block-wise run. Ends the batch when the queue runs dry.
    pub(crate) fn start_next_summary(&mut self) {
        if self.job.is_some() {
            return;
        }
        let Some((story_id, chapter_id)) = self.summary_queue.pop_front() else {
            self.finish_summary_batch();
            return;
        };
        self.summary_run = None;
        if self.library.settings.is_demo() {
            self.write_local_summary(story_id, chapter_id);
            // Local summaries are instant, so the whole batch runs in one frame.
            self.start_next_summary();
            return;
        }
        let block_size = ai::summary_block_size(&self.library.settings);
        let Some((source, base_len, base_hash, blocks)) = self
            .library
            .story(&story_id)
            .and_then(|story| story.chapter(&chapter_id))
            .map(|chapter| {
                let source = chapter.body.clone();
                let blocks = ai::summary_blocks(&source, block_size);
                let base_len = source.chars().count();
                let base_hash = crate::model::content_hash(&source);
                (source, base_len, base_hash, blocks)
            })
        else {
            self.start_next_summary();
            return;
        };
        if blocks.is_empty() {
            self.start_next_summary();
            return;
        }
        self.summary_run = Some(SummaryRun {
            story_id,
            chapter_id,
            source,
            blocks,
            next: 0,
            parts: Vec::new(),
            base_len,
            base_hash,
            attempts: 0,
        });
        self.start_next_summary_step();
    }

    /// Start the next request of the current chapter: the next block, or the
    /// final pass that rewrites the block summaries as one text.
    pub(crate) fn start_next_summary_step(&mut self) {
        if self.job.is_some() {
            return;
        }
        let Some(run) = self.summary_run.as_ref() else {
            return;
        };
        let story_id = run.story_id.clone();
        let chapter_id = run.chapter_id.clone();
        let Some(story) = self.library.story(&story_id) else {
            self.abort_summary_run("The story is no longer here.");
            return;
        };
        let Some(chapter) = story.chapter(&chapter_id) else {
            self.abort_summary_run("The chapter is no longer here.");
            return;
        };
        let (step, prompt, words, temperature) = if run.next < run.blocks.len() {
            let range = run.blocks[run.next].clone();
            let passage = run.source.get(range).unwrap_or_default();
            let prompt = ai::summary_block_prompt(
                story,
                chapter,
                passage,
                run.next,
                run.blocks.len(),
                &self.library.settings,
            );
            // The author's word target is the target for one block.
            let words = ai::summary_block_words(&self.library.settings);
            (SummaryStep::Block(run.next), prompt, words, 0.3_f32)
        } else if run.blocks.len() > 1 {
            // Every block is done. Only now are the independent parts joined:
            // the merge prompt reads the whole combined text and rewrites it
            // to the author's whole-chapter target.
            let combined = run.parts.join("\n\n");
            let words = ai::summary_merge_words(&self.library.settings);
            let prompt = ai::summary_merge_prompt(&combined, words);
            (SummaryStep::Merge, prompt, words, 0.2_f32)
        } else {
            // A one-block chapter is already one text; there is nothing to
            // merge. This is reached only by a retry after the block succeeded.
            let summary = run.parts.first().cloned().unwrap_or_default();
            let (base_len, base_hash) = (run.base_len, run.base_hash);
            self.summary_run = None;
            self.store_chapter_summary(story_id, chapter_id, summary, base_len, base_hash, "ai");
            self.start_next_summary();
            return;
        };
        let mut settings = self.library.settings.clone();
        // A summary is a record, not a scene: keep the model close to the facts
        // even when the author likes a lively prose temperature.
        settings.temperature = settings.temperature.min(temperature);
        let (rx, cancel) = start_stream(settings, prompt, Vec::new(), String::new(), words, true);
        self.job = Some(Job {
            kind: JobKind::Summary { step },
            story_id,
            chapter_id,
            stream: StreamFold::default(),
            rx,
            cancel,
            _started: Instant::now(),
        });
    }

    /// Fold one finished summary reply into the run. A block updates the
    /// running summary and starts the next step; the merge step stores the
    /// final text.
    pub(crate) fn apply_summary_reply(
        &mut self,
        step: SummaryStep,
        story_id: String,
        chapter_id: String,
        text: String,
    ) {
        let Some(run) = self.summary_run.as_ref() else {
            return;
        };
        if run.story_id != story_id || run.chapter_id != chapter_id {
            return;
        }
        if expected_summary_step(run) != step {
            return;
        }
        let (summary, _cut) = ai::complete_summary(&text);
        if summary.trim().is_empty() {
            self.summary_failed(step, "The summary reply had no text.".to_string());
            return;
        }
        // A model that ignores "at most N words" does not get to turn a short
        // memory into a retelling. The cut is made at a finished sentence.
        let target = match step {
            SummaryStep::Block(_) => ai::summary_block_words(&self.library.settings),
            SummaryStep::Merge => ai::summary_merge_words(&self.library.settings),
        };
        let (summary, _trimmed) = ai::trim_to_target(&summary, target);
        let (next, total, base_len, base_hash) = {
            let run = self.summary_run.as_ref().expect("checked above");
            (run.next, run.blocks.len(), run.base_len, run.base_hash)
        };
        match step {
            SummaryStep::Block(index) if index == next => {
                let done = {
                    let run = self.summary_run.as_mut().expect("checked above");
                    // Blocks stay independent; the merge pass joins them later.
                    run.parts.push(summary);
                    run.next += 1;
                    run.attempts = 0;
                    run.next >= total
                };
                if done && total <= 1 {
                    let (summary, base_len, base_hash) = {
                        let run = self.summary_run.as_ref().expect("checked above");
                        (
                            run.parts.first().cloned().unwrap_or_default(),
                            run.base_len,
                            run.base_hash,
                        )
                    };
                    self.summary_run = None;
                    self.store_chapter_summary(
                        story_id, chapter_id, summary, base_len, base_hash, "ai",
                    );
                    self.start_next_summary();
                } else {
                    // The next step is the following block or, after the last
                    // block, the merge pass.
                    self.start_next_summary_step();
                }
            }
            SummaryStep::Merge => {
                self.summary_run = None;
                self.store_chapter_summary(
                    story_id, chapter_id, summary, base_len, base_hash, "ai",
                );
                self.start_next_summary();
            }
            _ => {}
        }
    }

    /// A block or merge request failed. Retry the same step once; if it fails
    /// again, keep the old summary. A failed merge after a successful block run
    /// saves the combined block summary rather than throwing the work away.
    pub(crate) fn summary_failed(&mut self, step: SummaryStep, error: String) {
        let Some(run) = self.summary_run.as_mut() else {
            return;
        };
        if expected_summary_step(run) != step {
            return;
        }
        if run.attempts == 0 {
            run.attempts = 1;
            self.start_next_summary_step();
            return;
        }
        let (story_id, chapter_id, combined, base_len, base_hash) = (
            run.story_id.clone(),
            run.chapter_id.clone(),
            run.parts.join("\n\n"),
            run.base_len,
            run.base_hash,
        );
        self.summary_run = None;
        if step == SummaryStep::Merge && !combined.trim().is_empty() {
            self.store_chapter_summary(story_id, chapter_id, combined, base_len, base_hash, "ai");
            self.notify(
                format!(
                    "The final merge failed, so the combined block summary was saved. ({error})"
                ),
                true,
            );
            self.start_next_summary();
            return;
        }
        self.summary_queue.clear();
        self.summary_batch = None;
        self.notify(format!("AI request failed: {error}"), true);
    }

    fn abort_summary_run(&mut self, reason: &str) {
        self.summary_run = None;
        self.summary_queue.clear();
        self.summary_batch = None;
        self.notify(reason, true);
    }

    fn store_chapter_summary(
        &mut self,
        story_id: String,
        chapter_id: String,
        text: String,
        base_len: usize,
        base_hash: u64,
        source: &str,
    ) {
        let (summary, cut) = ai::complete_summary(&text);
        if summary.trim().is_empty() {
            self.notify("The summary reply had no text.", true);
            return;
        }
        if let Some(chapter) = self
            .library
            .story_mut(&story_id)
            .and_then(|story| story.chapter_mut(&chapter_id))
        {
            chapter.summary = summary;
            chapter.summary_source = source.to_string();
            chapter.summary_body_len = base_len;
            chapter.summary_body_hash = base_hash;
            self.mark_dirty();
        }
        if cut {
            self.notify(
                "The summary came back cut off; it was trimmed to its last finished sentence.",
                false,
            );
        }
    }

    /// Write a summary without an endpoint: three picked sentences, free and
    /// instant.
    pub(crate) fn write_local_summary(&mut self, story_id: String, chapter_id: String) {
        let Some(chapter) = self
            .library
            .story(&story_id)
            .and_then(|s| s.chapter(&chapter_id))
        else {
            return;
        };
        let summary = ai::local_summary(chapter);
        let body_len = chapter.body.chars().count();
        let body_hash = crate::model::content_hash(&chapter.body);
        if let Some(story) = self.library.story_mut(&story_id) {
            if let Some(chapter) = story.chapter_mut(&chapter_id) {
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

    /// Which block is being written, if the open chapter has more than one.
    /// The merge pass has no block number of its own; see `summary_merging`.
    pub(crate) fn summary_block_progress(&self) -> Option<(usize, usize)> {
        let run = self.summary_run.as_ref()?;
        if run.blocks.len() <= 1 {
            return None;
        }
        let current = match self.job.as_ref().map(|job| &job.kind) {
            Some(JobKind::Summary {
                step: SummaryStep::Block(index),
            }) => (*index + 1).min(run.blocks.len()),
            _ => run.next.min(run.blocks.len()),
        };
        Some((current, run.blocks.len()))
    }

    /// True while the final pass is reading the combined block summaries.
    pub(crate) fn summary_merging(&self) -> bool {
        self.summary_run
            .as_ref()
            .is_some_and(|run| run.blocks.len() > 1 && run.next >= run.blocks.len())
    }

    /// The text to show in the summary window while it is being written: the
    /// running summary plus the block in flight, or the merge answer alone.
    pub(crate) fn summary_live_text(&self, story_id: String, chapter_id: String) -> Option<String> {
        let job = self.job.as_ref()?;
        if job.story_id != story_id || job.chapter_id != chapter_id {
            return None;
        }
        let JobKind::Summary { step } = &job.kind else {
            return None;
        };
        let run = self.summary_run.as_ref()?;
        if run.story_id != story_id || run.chapter_id != chapter_id {
            return None;
        }
        let mut text = String::new();
        if !matches!(step, SummaryStep::Merge) && !run.parts.is_empty() {
            text.push_str(&run.parts.join("\n\n"));
            text.push_str("\n\n");
        }
        text.push_str(&job.stream.text);
        Some(text)
    }

    /// Stop the whole summary run now. Whatever blocks are already finished
    /// are paid for, so they are saved as one text with the honest source
    /// "partial" rather than thrown away. When no block finished, the old
    /// summary stays and the notice says so.
    pub(crate) fn stop_summary_batch(&mut self) {
        crate::crash::note("stop summary batch");
        self.summary_queue.clear();
        self.summary_batch = None;
        let run = self.summary_run.take();
        if self
            .job
            .as_ref()
            .is_some_and(|job| matches!(job.kind, JobKind::Summary { .. }))
        {
            // Dropping the job sets the worker's cancel flag.
            self.job = None;
        }
        let mut saved = false;
        if let Some(run) = run {
            let combined = run.parts.join("\n\n");
            if !combined.trim().is_empty() {
                self.store_chapter_summary(
                    run.story_id,
                    run.chapter_id,
                    combined,
                    run.base_len,
                    run.base_hash,
                    "partial",
                );
                saved = true;
            }
        }
        self.notify(
            if saved {
                "Summarizing stopped — the finished blocks were saved as a partial summary."
            } else {
                "Summarizing stopped — no block was finished, so the old summary stays."
            },
            false,
        );
    }

    /// The chapter summary in its own window, following the prompts window:
    /// a fixed page with a Done strip that never scrolls away and a body that
    /// scrolls once the summary outgrows it. The editor reserves no room.
    /// The chapter summary as a view of the side panel. The live block, the
    /// block progress and Stop follow a running summary even while the panel
    /// is closed; only the visibility changes.
    pub(crate) fn ui_summary(&mut self, ui: &mut Ui) {
        let story_index = self.active_story;
        let chapter_index = self.active_chapter;
        let Some(story) = self.library.stories.get(story_index) else {
            self.right_open = false;
            return;
        };
        let Some(chapter) = story.chapters.get(chapter_index) else {
            self.right_open = false;
            return;
        };
        let story_id = story.key.clone();
        let chapter_id = chapter.key.clone();
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
        // A summary run may be writing, retrying, or waiting for the next
        // block. In all of those states the primary button is Stop.
        let summary_running = self.summary_batch.is_some()
            && (self
                .job
                .as_ref()
                .is_some_and(|job| matches!(job.kind, JobKind::Summary { .. }))
                || self.summary_run.is_some());
        let block_progress = self.summary_block_progress();
        let merging = self.summary_merging();
        // The live summary, when this chapter is being written right now. Read
        // from the run, so opening the window mid-run shows the text so far.
        let live = self.summary_live_text(story_id, chapter_id);
        let mut close = false;
        let mut do_summary = false;
        let mut do_older = false;
        let mut do_copy = false;
        let mut do_stop = false;

        // The footer keeps "Done" reachable while the summary scrolls.
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
                        RichText::new("The summary is what the AI remembers of this chapter.")
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
                                ui.horizontal_wrapped(|ui| {
                                    let partial = chapter.is_partial_summary();
                                    let dot = if !has_summary {
                                        C::muted()
                                    } else if stale || partial {
                                        C::amber()
                                    } else {
                                        C::green()
                                    };
                                    let (rect, _) =
                                        ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 2.6, dot);
                                    ui.add(
                                        Label::new(caps(&chapter_title, C::muted())).truncate(),
                                    );
                                    if !has_summary {
                                        pill(ui, "Not summarized", C::amber(), C::amber_soft());
                                    } else if partial {
                                        pill(ui, "Partial summary", C::amber(), C::amber_soft());
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
                                            "partial" => "Finished blocks only",
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
                                        .corner_radius(theme::radius(10))
                                        .inner_margin(Margin::symmetric(12, 9))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.horizontal(|ui| {
                                                ui.spinner();
                                                let stage = if merging {
                                                    "Merging the summary".to_string()
                                                } else if let Some((part, total)) = block_progress {
                                                    format!("Writing the summary · part {part}/{total}")
                                                } else {
                                                    "Writing the summary".to_string()
                                                };
                                                ui.label(caps(&stage, C::muted()));
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
                                    if summary_running {
                                        let stop = Button::new(
                                            RichText::new("Stop")
                                                .size(11.5)
                                                .strong()
                                                .color(C::accent_deep()),
                                        )
                                        .fill(C::accent_soft())
                                        .stroke(Stroke::new(1.0_f32, C::accent_soft()))
                                        .corner_radius(theme::radius(8))
                                        .min_size(Vec2::new(86.0, 28.0));
                                        if ui
                                            .add(stop)
                                            .on_hover_text(
                                                "Stop summarizing — the summaries already written stay",
                                            )
                                            .clicked()
                                        {
                                            do_stop = true;
                                        }
                                    } else {
                                        let label = if busy {
                                            "Working…".to_string()
                                        } else {
                                            "Summarize".to_string()
                                        };
                                        let button = Button::new(
                                            RichText::new(label)
                                                .size(11.5)
                                                .color(Color32::WHITE)
                                                .strong(),
                                        )
                                        .fill(C::accent())
                                        .stroke(Stroke::NONE)
                                        .corner_radius(theme::radius(8))
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

        if close {
            self.right_open = false;
        }
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
            ui.ctx().copy_text(copy_summary);
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

/// The step the run is waiting for: the next block, or the merge pass once
/// every block is done.
fn expected_summary_step(run: &SummaryRun) -> SummaryStep {
    if run.next < run.blocks.len() {
        SummaryStep::Block(run.next)
    } else {
        SummaryStep::Merge
    }
}

/// The chapters one Summarize press writes, in order: every missing older
/// summary first, then the open chapter itself.
pub(crate) fn summary_queue(story: &Story, position: usize, window: usize) -> Vec<String> {
    let mut ids: Vec<String> = story
        .chapters_needing_summary(position, window)
        .into_iter()
        .map(|chapter| chapter.key.clone())
        .collect();
    if let Some(chapter) = story.chapters.get(position) {
        ids.push(chapter.key.clone());
    }
    ids
}
