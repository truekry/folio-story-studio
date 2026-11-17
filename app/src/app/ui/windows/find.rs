//! The find and replace window.

use super::*;

impl FolioApp {
    pub(crate) fn find_window(&mut self, ctx: &egui::Context) {
        if !self.find_open {
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.find_open = false;
            return;
        }
        let Some(story) = self.library.stories.get(self.active_story).cloned() else {
            self.find_open = false;
            return;
        };
        let Some(chapter) = story.chapters.get(self.active_chapter) else {
            self.find_open = false;
            return;
        };
        let mut open = self.find_open;
        let mut close_requested = false;
        let mut jump: Option<Range<usize>> = None;
        let mut do_replace = false;
        let mut nav_next = false;
        let mut nav_prev = false;

        Window::new(if self.find_replace {
            "Find & replace"
        } else {
            "Find"
        })
        .id(Id::new("find-window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(caps(
                    &format!("Find in “{}”", chapter.title),
                    C::accent_deep(),
                ));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if self.find_replace {
                        if ghost(ui, "Back to find").clicked() {
                            self.find_replace = false;
                        }
                    } else if ghost(ui, "Find & replace…").clicked() {
                        self.find_replace = true;
                    }
                });
            });
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 104.0).max(220.0);
                let response = ui.add_sized(
                    [width, 30.0],
                    TextEdit::singleline(&mut self.find_query)
                        .id(Id::new("find-query"))
                        .font(FontId::proportional(13.0))
                );
                if response.changed() {
                    self.find_index = None;
                }
                if self.find_focus {
                    response.request_focus();
                    self.find_focus = false;
                }
                let match_case = ui.checkbox(&mut self.find_match_case, "Match case");
                if match_case.changed() {
                    self.find_index = None;
                }
                let enter = response.has_focus()
                    && ui.input(|i| i.key_pressed(Key::Enter));
                if enter {
                    nav_next = true;
                }
            });
                let query = self.find_query.trim().to_string();
                let ranges = find_char_ranges(&chapter.body, &query, self.find_match_case);
                let total = ranges.len();
                ui.horizontal(|ui| {
                    let previous = Button::new(
                        RichText::new("Find previous").size(10.5).color(C::ink_soft()),
                    )
                    .fill(C::soft())
                    .stroke(Stroke::new(1.0_f32, C::line()))
                    .corner_radius(8)
                    .min_size(Vec2::new(0.0, 26.0));
                    let next = Button::new(
                        RichText::new("Find next").size(10.5).color(C::ink_soft()),
                    )
                    .fill(C::soft())
                    .stroke(Stroke::new(1.0_f32, C::line()))
                    .corner_radius(8)
                    .min_size(Vec2::new(0.0, 26.0));
                    let can_step = !ranges.is_empty();
                    if ui.add_enabled(can_step, previous).clicked() {
                        nav_prev = true;
                    }
                    if ui.add_enabled(can_step, next).clicked() {
                        nav_next = true;
                    }
                    ui.label(
                        RichText::new(match (self.find_index, ranges.len()) {
                            (Some(index), len) if len > 0 && index < len => {
                                format!("match {} of {}", index + 1, len)
                            }
                            (_, 0) => "No matches here".to_string(),
                            (_, len) => plural(len, "match", "matches"),
                        })
                        .size(10.0)
                        .color(C::muted()),
                    );
                });
                if nav_next || nav_prev {
                    if let Some(index) = step_find_hit(self.find_index, ranges.len(), nav_next) {
                        self.find_index = Some(index);
                        jump = Some(ranges[index].clone());
                    }
                }
                ui.add_space(4.0);
                if query.is_empty() {
                    ui.label(
                        RichText::new(
                            "Type a word or phrase. Every match in this chapter is highlighted; Find next walks through them.",
                        )
                        .size(10.5)
                        .italics()
                        .color(C::muted()),
                    );
                } else if ranges.is_empty() {
                    ui.label(
                        RichText::new(format!("No match for “{query}” in this chapter."))
                            .size(10.5)
                            .italics()
                            .color(C::muted()),
                    );
                } else {
                    ui.label(
                        RichText::new(format!(
                            "{} highlighted in this chapter.",
                            plural(ranges.len(), "match", "matches")
                        ))
                        .size(10.5)
                        .color(C::ink_soft()),
                    );
                }

                if self.find_replace {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.label(caps("Replace in this chapter", C::muted()));
                    ui.add_space(2.0);
                    ui.add(
                        TextEdit::singleline(&mut self.find_replacement)
                            .font(FontId::proportional(13.0))
                            .desired_width(f32::INFINITY)
                    );
                    ui.label(
                        RichText::new("Leave empty to delete the matches.")
                            .size(9.5)
                            .color(C::muted()),
                    );
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        let can_replace = total > 0;
                        let button = Button::new(
                            RichText::new("Replace all")
                                .size(11.5)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(C::accent())
                        .stroke(Stroke::NONE)
                        .corner_radius(8)
                        .min_size(Vec2::new(92.0, 27.0));
                        if ui
                            .add_enabled(can_replace, button)
                            .on_hover_text(
                                "Every match in this chapter changes at once; the notice offers Undo",
                            )
                            .clicked()
                        {
                            do_replace = true;
                        }
                        ui.label(
                            RichText::new(if total > 0 {
                                format!(
                                    "{} in this chapter  ·  you can take it back from the notice",
                                    plural(total, "match", "matches")
                                )
                            } else {
                                "No chapter matches to replace".to_string()
                            })
                            .size(9.5)
                            .color(C::muted()),
                        );
                    });
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ghost(ui, "Close").clicked() {
                        close_requested = true;
                    }
                    ui.label(
                        RichText::new("Escape closes this window.")
                            .size(9.5)
                            .color(C::muted()),
                    );
                });
            });

        self.find_open = open;
        if close_requested {
            self.find_open = false;
            return;
        }
        if !open {
            return;
        }
        if let Some(range) = jump {
            self.editor_selection = None;
            self.pending_selection = Some((chapter.key.clone(), range));
        } else if do_replace {
            let query = self.find_query.trim().to_string();
            let matches = find_char_ranges(&chapter.body, &query, self.find_match_case).len();
            self.confirm = Some(Confirm::Replace {
                query,
                replacement: self.find_replacement.clone(),
                match_case: self.find_match_case,
                matches,
                chapters: 1,
                story_title: story.title.clone(),
                chapter_id: Some(chapter.key.clone()),
                chapter_title: chapter.title.clone(),
            });
        }
    }
}
