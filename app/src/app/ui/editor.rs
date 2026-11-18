//! The chapter desk: the editor, the steering bar and the draft block.

use super::*;

impl FolioApp {
    pub(crate) fn ui_center(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                Frame::NONE
                    .fill(C::paper())
                    .inner_margin(Margin::symmetric(34, 16)),
            )
            .show(ctx, |ui| {
                if self.library.stories.is_empty() {
                    ui.add_space(90.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("A blank shelf.")
                                .font(serif_id(34.0))
                                .color(C::ink()),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(
                                "Every story begins with a title, a premise and a first chapter.",
                            )
                            .font(serif_id(14.0))
                            .color(C::muted()),
                        );
                        ui.add_space(14.0);
                        if primary(ui, "Start a story").clicked() {
                            self.new_story_title.clear();
                            self.new_story_premise.clear();
                            self.new_story_open = true;
                        }
                    });
                    return;
                }
                let Some(story) = self.library.stories.get(self.active_story) else {
                    ui.label("No story open.");
                    return;
                };
                if story.chapters.is_empty() {
                    let count = story.chapters.len() + 1;
                    ui.add_space(90.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("The first page is missing.")
                                .font(serif_id(32.0))
                                .color(C::ink()),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("This story has no chapters yet.")
                                .font(serif_id(14.0))
                                .color(C::muted()),
                        );
                        ui.add_space(14.0);
                        if primary(ui, "Add a chapter").clicked() {
                            self.new_chapter_title = format!("Chapter {count}");
                            self.new_chapter_open = true;
                        }
                    });
                    return;
                }
                let Some(chapter) = story.chapters.get(self.active_chapter) else {
                    ui.label("No chapter open.");
                    return;
                };
                let story_title = story.title.clone();
                let story_id = story.key.clone();
                let chapter_id = chapter.key.clone();
                let mut title = chapter.title.clone();
                let mut body = chapter.body.clone();
                let mut steering = chapter.steering.clone();
                let original_title = title.clone();
                let original_body = body.clone();
                let original_steering = steering.clone();
                let words = chapter.words();
                // What the draft block should show right now: the live one
                // while it grows, otherwise the saved draft under this chapter.
                let prose_streaming = self.job.as_ref().is_some_and(
                    |job| matches!(&job.kind, JobKind::Action(action) if action.is_prose()),
                );
                let busy = self.job.is_some();
                let live_match = self
                    .draft_live
                    .as_ref()
                    .filter(|live| live.story_id == story_id && live.chapter_id == chapter_id);
                let (draft_shown, draft_error, draft_streaming) = match live_match {
                    Some(live) => (
                        Some(live.draft.clone()),
                        self.draft_error.clone(),
                        prose_streaming,
                    ),
                    None => (chapter.draft.clone(), None, false),
                };
                let user_scrolled = ui.input(|i| {
                    i.raw_scroll_delta.y.abs() > 0.5 || i.smooth_scroll_delta.y.abs() > 0.5
                });
                let follow = self.draft_follow && !user_scrolled;
                let reveal = self.draft_reveal;
                let mut draft_action = DraftAction::None;
                let mut menu_action: Option<SelectionAction> = None;
                let mut start_action: Option<Action> = None;

                // Type bar: the installed face the story is set in, and the
                // body size. The choice is applied after the editor borrows end.
                let mut font_path = self.library.writing_font.clone();
                let mut font_size = self.library.writing_size.clamp(10.0, 40.0);
                let mut font_changed = false;
                let mut want_proofread = false;
                let system_fonts = &self.system_fonts;
                ui.horizontal_wrapped(|ui| {
                    ui.label(caps("Type", C::muted()));
                    let selected = if font_path.is_empty() {
                        "System default".to_string()
                    } else {
                        system_fonts
                            .iter()
                            .find(|font| font.path.display().to_string() == font_path)
                            .map(|font| font.name.clone())
                            .or_else(|| {
                                Path::new(&font_path)
                                    .file_stem()
                                    .and_then(|stem| stem.to_str())
                                    .map(str::to_string)
                            })
                            .unwrap_or_else(|| "System default".to_string())
                    };
                    // White and Corpo set prose in their own sans face, so a
                    // chosen writing font cannot show. Say so instead of
                    // letting the picker do nothing.
                    let paper_look = theme::look() == crate::theme::Look::Paper;
                    ui.add_enabled_ui(paper_look, |ui| {
                        egui::ComboBox::from_id_salt("writing-font")
                            .selected_text(RichText::new(selected).size(11.0))
                            .width(210.0)
                            .show_ui(ui, |ui| {
                                ScrollArea::vertical()
                                    .id_salt("writing-fonts")
                                    .max_height(300.0)
                                    .show(ui, |ui| {
                                        if ui
                                            .selectable_label(font_path.is_empty(), "System default")
                                            .clicked()
                                        {
                                            font_path.clear();
                                            font_changed = true;
                                            ui.close();
                                        }
                                        for font in system_fonts {
                                            let path = font.path.display().to_string();
                                            if ui
                                                .selectable_label(font_path == path, &font.name)
                                                .clicked()
                                            {
                                                font_path = path;
                                                font_changed = true;
                                                ui.close();
                                            }
                                        }
                                    });
                            });
                    });
                    if !paper_look {
                        ui.label(RichText::new("Paper look only").size(9.0).color(C::muted()))
                            .on_hover_text(
                                "White and Corpo set the prose in their own sans face. Choose Paper in Settings to use a writing font.",
                            );
                    }
                    let size = ui.add(
                        egui::DragValue::new(&mut font_size)
                            .range(10.0..=40.0)
                            .speed(0.5)
                            .max_decimals(1)
                            .suffix(" px"),
                    );
                    if size.changed() {
                        font_changed = true;
                    }
                    if ghost(ui, "Reset").clicked() {
                        font_path.clear();
                        font_size = crate::model::DEFAULT_WRITING_SIZE;
                        font_changed = true;
                    }
                    let check = Button::new(
                        RichText::new("Check spelling")
                            .size(10.5)
                            .color(C::ink_soft()),
                    )
                    .fill(C::soft())
                    .stroke(Stroke::new(1.0_f32, C::line()))
                    .corner_radius(theme::radius(8))
                    .min_size(Vec2::new(0.0, 24.0));
                    if ui
                        .add_enabled(!busy, check)
                        .on_hover_text(
                            "Proofread this chapter with the AI, paragraph by paragraph",
                        )
                        .clicked()
                    {
                        want_proofread = true;
                    }
                });
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    let left = format!(
                        "{}   /   CHAPTER {} OF {}",
                        story_title.to_uppercase(),
                        self.active_chapter + 1,
                        story.chapters.len()
                    );
                    let right = format!("{words} words  ·  {} min read", (words / 230).max(1));
                    // The word count keeps its room; a long story title is cut
                    // at the last word that fits instead of running under it.
                    let right_width = ui.fonts(|fonts| {
                        fonts
                            .layout_no_wrap(
                                right.clone(),
                                FontId::proportional(10.5),
                                C::muted(),
                            )
                            .size()
                            .x
                    });
                    let room = (ui.available_width() - right_width - 8.0).max(48.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(room, 14.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.add(
                                Label::new(
                                    RichText::new(left).size(9.5).strong().color(C::muted()),
                                )
                                .truncate(),
                            );
                        },
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(right).size(10.5).color(C::muted()));
                    });
                });
                ui.add_space(5.0);

                let title_response = ui.add(
                    TextEdit::singleline(&mut title)
                        .id(Id::new(("chapter-title", chapter_id.clone())))
                        .font(serif_id(31.0))
                        .text_color(C::ink())
                        .frame(false)
                        .desired_width(f32::INFINITY)
                );
                let _ = title_response;
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let summarized = chapter.is_summarized();
                    let stale = chapter.summary_is_stale();
                    let partial = chapter.is_partial_summary();
                    let (dot, label) = summary_lamp(summarized, stale, partial);
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                    ui.painter().circle_filled(rect.center(), 2.6, dot);
                    if !label.is_empty() {
                        ui.label(RichText::new(label).size(10.0).color(C::muted()));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.job.is_some() {
                            ui.spinner();
                        }
                    });
                });
                ui.add_space(6.0);

                // The note lives below the scrolling page, so it stays in sight
                // while the chapter scrolls and while a draft grows.
                // The Continue button becomes Stop while a prose draft for this
                // chapter is being written.
                let proofread_here = self.job.as_ref().is_some_and(|job| {
                    job.story_id == story_id
                        && job.chapter_id == chapter_id
                        && matches!(job.kind, JobKind::Proofread { .. })
                });
                let steering_streaming = proofread_here
                    || self.job.as_ref().is_some_and(|job| {
                        job.story_id == story_id
                            && job.chapter_id == chapter_id
                            && matches!(&job.kind, JobKind::Action(action) if action.is_prose())
                    });
                let steering_hint = if proofread_here {
                    let progress = self
                        .job
                        .as_ref()
                        .and_then(|job| job.stream.progress)
                        .map(|(done, total)| format!("{done}/{total}"))
                        .unwrap_or_else(|| "…".to_string());
                    format!(
                        "Proofreading {progress} — Stop leaves the chapter unchanged  ·  Select text and right-click for more"
                    )
                } else if steering_streaming {
                    "Writing — Stop keeps what arrived  ·  Select text and right-click for more"
                        .to_string()
                } else {
                    "Ctrl+Enter to continue  ·  Select text and right-click to rewrite, elaborate or add lore"
                        .to_string()
                };
                let mut stop_steering = false;
                TopBottomPanel::bottom(Id::new("steering-bar"))
                    .frame(Frame::NONE)
                    .show_inside(ui, |ui| {
                        ui.add_space(6.0);
                        if let Some(action) = steering_bar(
                            ui,
                            &mut steering,
                            busy,
                            steering_streaming,
                            &steering_hint,
                        ) {
                            match action {
                                SteeringAction::Continue => start_action = Some(Action::Continue),
                                SteeringAction::Stop => stop_steering = true,
                            }
                        }
                    });

                let body_height = (ui.available_height() - 12.0).max(180.0);
                let editor_id = Id::new(("chapter-body", chapter_id.clone()));
                // While Find is open, matching words in this chapter carry a
                // background colour; the current one is stronger.
                let find_query = if self.showing(Tab::Find) {
                    self.find_query.trim().to_string()
                } else {
                    String::new()
                };
                let find_match_case = self.find_match_case;
                let find_ranges = if find_query.is_empty() {
                    Vec::new()
                } else {
                    find_char_ranges(&body, &find_query, find_match_case)
                };
                let find_active = self.find_index.filter(|index| *index < find_ranges.len());
                if let Some((pending_id, range)) = self.pending_selection.clone() {
                    if pending_id == chapter_id {
                        let mut state = egui::text_edit::TextEditState::load(ui.ctx(), editor_id)
                            .unwrap_or_default();
                        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(range.start),
                            egui::text::CCursor::new(range.end),
                        )));
                        state.store(ui.ctx(), editor_id);
                        ui.memory_mut(|memory| memory.request_focus(editor_id));
                        self.pending_selection = None;
                    }
                }
                let scroll_out = ScrollArea::vertical()
                    .id_salt(("chapter-scroll", chapter_id.clone()))
                    .max_height(body_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let had_focus = ui.memory(|memory| memory.has_focus(editor_id));
                        let edit = TextEdit::multiline(&mut body)
                            .id(editor_id)
                            .font(serif_id(font_size))
                            .text_color(C::ink())
                            .frame(false)
                            .desired_width(ui.available_width())
                            .desired_rows(18);
                        let output = if find_query.is_empty() {
                            edit.show(ui)
                        } else {
                            let mut layouter = |ui: &Ui,
                                                text: &dyn egui::TextBuffer,
                                                wrap_width: f32|
                             -> Arc<egui::Galley> {
                                let content = text.as_str();
                                let mut job = egui::text::LayoutJob::default();
                                job.wrap.max_width = wrap_width;
                                let normal = TextFormat {
                                    font_id: serif_id(font_size),
                                    color: C::ink(),
                                    ..Default::default()
                                };
                                let marked = TextFormat {
                                    background: C::find_bg(),
                                    ..normal.clone()
                                };
                                let current = TextFormat {
                                    background: C::find_active_bg(),
                                    ..normal.clone()
                                };
                                let ranges =
                                    find_char_ranges(content, &find_query, find_match_case);
                                let mut cursor = 0usize;
                                for (index, range) in ranges.iter().enumerate() {
                                    let start = byte_index(content, range.start);
                                    let end =
                                        byte_index(content, range.end).min(content.len());
                                    if start > cursor {
                                        job.append(
                                            &content[cursor..start],
                                            0.0,
                                            normal.clone(),
                                        );
                                    }
                                    let format = if Some(index) == find_active {
                                        current.clone()
                                    } else {
                                        marked.clone()
                                    };
                                    job.append(&content[start..end], 0.0, format);
                                    cursor = end;
                                }
                                if cursor < content.len() {
                                    job.append(&content[cursor..], 0.0, normal);
                                }
                                ui.fonts(|fonts| fonts.layout_job(job))
                            };
                            edit.layouter(&mut layouter).show(ui)
                        };
                        let secondary_active = keep_selection_on_secondary(
                            ui,
                            editor_id,
                            had_focus,
                            &mut self.editor_selection,
                        );
                        if output.response.has_focus() && !secondary_active {
                            if let Some(cursor) = output.cursor_range {
                                self.editor_selection = Some(cursor.as_sorted_char_range());
                            }
                        }
                        // Prose actions on a marked passage. Right-click keeps
                        // the selection in egui, so the same range can be used
                        // for all three.
                        let has_selection = self.editor_selection.as_ref().is_some_and(|range| {
                            range.start < range.end && range.end <= body.chars().count()
                        });
                        if has_selection {
                            output.response.context_menu(|ui| {
                                ui.set_min_width(204.0);
                                ui.label(caps("Selected passage", C::muted()));
                                ui.add_space(2.0);
                                if menu_item(ui, "Rewrite", false).clicked() {
                                    menu_action = Some(SelectionAction::Rewrite);
                                    ui.close();
                                }
                                if menu_item(ui, "Elaborate", false).clicked() {
                                    menu_action = Some(SelectionAction::Elaborate);
                                    ui.close();
                                }
                                ui.separator();
                                if menu_item(ui, "Make lorebook entry", false).clicked() {
                                    menu_action = Some(SelectionAction::Lore);
                                    ui.close();
                                }
                            });
                        }
                        if let Some(draft) = draft_shown.as_ref() {
                            ui.add_space(14.0);
                            draft_action = draft_block(
                                ui,
                                draft,
                                draft_streaming,
                                draft_error.as_deref(),
                                follow,
                                reveal,
                                font_size,
                            );
                        }
                    });
                // Follow the growing draft until the author scrolls up. A fresh
                // draft keeps asking to scroll until the request has landed, so
                // a long chapter cannot leave it cut off at the bottom.
                if draft_shown.is_some() {
                    let at_bottom = scroll_out.state.offset.y + scroll_out.inner_rect.height()
                        >= scroll_out.content_size.y - 16.0;
                    if user_scrolled {
                        self.draft_follow = at_bottom;
                    }
                    if self.draft_reveal && (at_bottom || user_scrolled) {
                        self.draft_reveal = false;
                    }
                }
                // Read the marked words before the edited body is written back.
                let menu_passage = match menu_action {
                    Some(SelectionAction::Lore) => self
                        .editor_selection
                        .as_ref()
                        .map(|range| slice_chars(&body, range).to_string()),
                    _ => None,
                };

                if title != original_title {
                    if let Some(chapter) = self
                        .library
                        .stories
                        .get_mut(self.active_story)
                        .and_then(|s| s.chapters.get_mut(self.active_chapter))
                    {
                        chapter.title = title;
                    }
                    self.mark_dirty();
                }
                if body != original_body {
                    if let Some(chapter) = self
                        .library
                        .stories
                        .get_mut(self.active_story)
                        .and_then(|s| s.chapters.get_mut(self.active_chapter))
                    {
                        chapter.body = body;
                    }
                    self.mark_dirty();
                }
                if steering != original_steering {
                    if let Some(chapter) = self
                        .library
                        .stories
                        .get_mut(self.active_story)
                        .and_then(|s| s.chapters.get_mut(self.active_chapter))
                    {
                        chapter.steering = steering;
                    }
                    self.mark_dirty();
                }
                if font_changed {
                    self.library.writing_font = font_path;
                    self.library.writing_size = font_size.clamp(10.0, 40.0);
                    let path = Path::new(&self.library.writing_font).to_path_buf();
                    let choice = if path.as_os_str().is_empty() {
                        None
                    } else {
                        Some(path)
                    };
                    theme::install_fonts(ui.ctx(), choice.as_deref());
                    self.mark_dirty();
                }

                match draft_action {
                    DraftAction::Accept => self.accept_draft(story_id.clone(), chapter_id.clone()),
                    DraftAction::Delete => self.discard_draft(story_id.clone(), chapter_id.clone()),
                    DraftAction::Retry => self.retry_draft(story_id.clone(), chapter_id.clone()),
                    DraftAction::Stop => self.stop_draft(story_id.clone(), chapter_id.clone()),
                    DraftAction::None => {}
                }
                if stop_steering {
                    if proofread_here {
                        self.stop_proofread(story_id.clone(), chapter_id.clone());
                    } else {
                        self.stop_draft(story_id.clone(), chapter_id.clone());
                    }
                }
                if let Some(action) = start_action {
                    self.run_action(action, String::new());
                }
                match menu_action {
                    Some(SelectionAction::Rewrite) => {
                        self.run_action(Action::Rewrite, String::new());
                    }
                    Some(SelectionAction::Elaborate) => {
                        self.run_action(Action::Elaborate, String::new());
                    }
                    Some(SelectionAction::Lore) => {
                        if let Some(passage) = menu_passage {
                            self.start_lore_entry(story_id.clone(), chapter_id.clone(), passage);
                        }
                    }
                    None => {}
                }
                if want_proofread {
                    self.start_proofread(story_id, chapter_id);
                }
            });
    }
}
