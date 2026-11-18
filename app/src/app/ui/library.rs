//! The left rail: stories, chapters and their menus.

use super::*;

impl FolioApp {
    pub(crate) fn ui_left(&mut self, ctx: &egui::Context) {
        let stories: Vec<(usize, String, String, usize, usize)> = self
            .library
            .stories
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    i,
                    s.key.clone(),
                    s.title.clone(),
                    s.chapters.len(),
                    s.words(),
                )
            })
            .collect();
        let chapters: Vec<(usize, String, String, usize, &'static str)> = self
            .library
            .stories
            .get(self.active_story)
            .map(|s| {
                s.chapters
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        (
                            i,
                            c.key.clone(),
                            c.title.clone(),
                            c.words(),
                            c.summary_state(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let premise = self
            .library
            .stories
            .get(self.active_story)
            .map(|s| s.premise.clone())
            .unwrap_or_default();
        let lore_count = self
            .library
            .stories
            .get(self.active_story)
            .map(|s| s.lore.len())
            .unwrap_or(0);
        let active_story = self.active_story;
        let active_chapter = self.active_chapter;
        let total_words = self.library.total_words();
        let mut pick_story = None;
        let mut pick_chapter = None;
        let mut want_new_chapter = false;
        let mut want_new_story = false;
        let mut story_menu: Option<(String, StoryMenu)> = None;
        let mut chapter_menu: Option<(String, ChapterMenu)> = None;

        egui::SidePanel::left("library")
            .exact_width(LIBRARY_WIDTH)
            .resizable(false)
            .frame(
                Frame::NONE
                    .fill(C::sidebar())
                    .inner_margin(Margin::symmetric(13, 15)),
            )
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(C::sidebar_text());
                // The version keeps to the bottom-left corner, below the lists,
                // also when there are more stories or chapters than fit.
                TopBottomPanel::bottom(Id::new("library-version"))
                    .frame(Frame::NONE)
                    .show_inside(ui, |ui| {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .size(9.5)
                                .color(C::sidebar_muted()),
                        );
                    });
                ui.horizontal(|ui| {
                    ui.label(caps("Stories", C::sidebar_muted()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if sidebar_action(ui, "+ New").clicked() {
                            want_new_story = true;
                        }
                        ui.label(
                            RichText::new(format!("{} total", total_words))
                                .size(9.5)
                                .color(C::sidebar_muted()),
                        );
                    });
                });
                ui.add_space(4.0);
                for (index, story_id, title, chapters, words) in &stories {
                    let meta = format!(
                        "{}  ·  {} words",
                        plural(*chapters, "chapter", "chapters"),
                        words
                    );
                    let response = story_card(ui, title, &meta, *index == active_story);
                    if response.clicked() {
                        pick_story = Some(*index);
                    }
                    let mut picked: Option<StoryMenu> = None;
                    response.context_menu(|ui| {
                        ui.set_min_width(176.0);
                        ui.label(caps("Story", C::muted()));
                        ui.add_space(2.0);
                        if menu_item(ui, "Edit", false).clicked() {
                            picked = Some(StoryMenu::Edit);
                            ui.close();
                        }
                        ui.separator();
                        if menu_item(ui, "Delete", true).clicked() {
                            picked = Some(StoryMenu::Delete);
                            ui.close();
                        }
                    });
                    if picked.is_some() {
                        story_menu = picked.map(|action| (story_id.clone(), action));
                    }
                    ui.add_space(1.0);
                }
                if stories.is_empty() {
                    ui.add_space(8.0);
                    Frame::NONE
                        .fill(C::sidebar_card())
                        .corner_radius(theme::radius(11))
                        .inner_margin(Margin::symmetric(11, 10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(
                                    "No stories yet. Start one and give it a premise — the chapters and lorebook follow.",
                                )
                                .size(10.5)
                                .color(C::sidebar_text())
                                .line_height(Some(15.0)),
                            );
                        });
                } else {
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new("Right-click a story to edit or delete it.")
                            .size(9.5)
                            .color(C::sidebar_muted()),
                    );
                    ui.add_space(11.0);
                    ui.horizontal(|ui| {
                        ui.label(caps("Chapters", C::sidebar_muted()));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{}", chapters.len()))
                                    .size(10.0)
                                    .color(C::sidebar_muted()),
                            );
                        });
                    });
                    ui.add_space(4.0);
                    for (index, chapter_id, title, words, state) in &chapters {
                        let response =
                            chapter_row(ui, *index, title, *words, state, *index == active_chapter);
                        if response.clicked() {
                            pick_chapter = Some(*index);
                        }
                        let mut picked: Option<ChapterMenu> = None;
                        response.context_menu(|ui| {
                            ui.set_min_width(176.0);
                            ui.label(caps("Chapter", C::muted()));
                            ui.add_space(2.0);
                            if menu_item(ui, "Edit", false).clicked() {
                                picked = Some(ChapterMenu::Edit);
                                ui.close();
                            }
                            ui.separator();
                            if menu_item(ui, "Delete", true).clicked() {
                                picked = Some(ChapterMenu::Delete);
                                ui.close();
                            }
                        });
                        if picked.is_some() {
                            chapter_menu = picked.map(|action| (chapter_id.clone(), action));
                        }
                        ui.add_space(1.0);
                    }
                    if chapters.is_empty() {
                        ui.add_space(5.0);
                        ui.label(
                            RichText::new("No chapters yet.")
                                .size(9.5)
                                .italics()
                                .color(C::sidebar_muted()),
                        );
                    } else {
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new("Right-click a chapter to edit or delete it.")
                                .size(9.5)
                                .color(C::sidebar_muted()),
                        );
                    }
                    ui.add_space(4.0);
                    if sidebar_action(ui, "+  New chapter").clicked() {
                        want_new_chapter = true;
                    }

                    ui.add_space(20.0);
                    ui.horizontal(|ui| {
                        ui.label(caps("Story bible", C::sidebar_muted()));
                    });
                    ui.add_space(5.0);
                    Frame::NONE
                        .fill(C::sidebar_card())
                        .corner_radius(theme::radius(11))
                        .inner_margin(Margin::symmetric(11, 9))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(if premise.trim().is_empty() {
                                    "No premise yet. Add one in the story settings."
                                } else {
                                    premise.trim()
                                })
                                .size(11.0)
                                .color(C::sidebar_text())
                                .line_height(Some(16.0)),
                            );
                            ui.add_space(5.0);
                            ui.label(
                                RichText::new(format!(
                                    "{}  ·  {}",
                                    plural(lore_count, "lore entry", "lore entries"),
                                    plural(chapters.len(), "chapter", "chapters")
                                ))
                                .size(9.5)
                                .color(C::sidebar_muted()),
                            );
                        });
                }
            });

        if let Some(index) = pick_story {
            self.switch_story(index);
        }
        if let Some(index) = pick_chapter {
            self.switch_chapter(index);
        }
        if want_new_chapter {
            if let Some(story) = self.library.stories.get(self.active_story) {
                self.new_chapter_open = true;
                self.new_chapter_title = format!("Chapter {}", story.chapters.len() + 1);
            }
        }
        if want_new_story {
            self.new_story_title.clear();
            self.new_story_premise.clear();
            self.new_story_open = true;
        }
        if let Some((story_id, action)) = story_menu {
            match action {
                StoryMenu::Edit => self.open_story_editor_for(story_id),
                StoryMenu::Delete => self.ask_delete_story(story_id),
            }
        }
        if let Some((chapter_id, action)) = chapter_menu {
            let story_id = self.story_id();
            match action {
                ChapterMenu::Edit => self.edit_chapter(story_id, chapter_id),
                ChapterMenu::Delete => self.ask_delete_chapter(story_id, chapter_id),
            }
        }
    }

    // ---------------------------------------------------------------- editor
}
