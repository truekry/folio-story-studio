//! The top bar with the logo, the endpoint lamp and the window menu.

use super::*;

impl FolioApp {
    pub(crate) fn ui_top(&mut self, ctx: &egui::Context) {
        let story_title = self
            .library
            .stories
            .get(self.active_story)
            .map(|s| s.title.clone())
            .unwrap_or_else(|| "No story".to_string());
        let chapter_line = self
            .library
            .stories
            .get(self.active_story)
            .map(|s| {
                if s.chapters.is_empty() {
                    "No chapters yet".to_string()
                } else {
                    format!(
                        "Chapter {} of {}  ·  {} words",
                        self.active_chapter + 1,
                        s.chapters.len(),
                        s.chapters
                            .get(self.active_chapter)
                            .map(Chapter::words)
                            .unwrap_or(0)
                    )
                }
            })
            .unwrap_or_else(|| "No story open".to_string());
        let save_text = if self.shot.is_some() {
            "Preview".to_string()
        } else if self.dirty {
            "Saving…".to_string()
        } else {
            "Saved".to_string()
        };
        let mut want_ai = false;
        let mut want_lore = false;
        let mut want_settings = false;
        let mut want_export = false;
        let mut want_trash = false;
        let mut want_find = false;
        let mut want_summary = false;
        let deleted_count = self.trash.len();
        let probe_base = self.library.settings.base_url.trim().to_string();
        let probe_demo = self.library.settings.is_demo();
        // A narrow window keeps the logo and the lamp, but drops the wordmark
        // and the chapter line so the story title never collides with them.
        let width = ctx.screen_rect().width();
        let roomy = width >= 1330.0;
        let with_chapter_line = width >= 1200.0;
        // The controls on the right need their own room; the title is cut
        // rather than running beneath them.
        let title_room = (width - 660.0).max(120.0);

        egui::TopBottomPanel::top("topbar")
            .exact_height(68.0)
            .frame(
                Frame::NONE
                    .fill(C::paper())
                    .inner_margin(Margin::symmetric(20, 13)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let (logo_rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                    ui.painter()
                        .rect_filled(logo_rect, CornerRadius::same(9), theme::BRAND_ORANGE);
                    ui.painter().text(
                        logo_rect.center(),
                        Align2::CENTER_CENTER,
                        "F",
                        theme::brand_id(20.0),
                        Color32::WHITE,
                    );
                    ui.add_space(3.0);
                    if roomy {
                        ui.scope(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.label(RichText::new("FOLIO").size(17.0).strong().color(C::ink()));
                            ui.label(
                                RichText::new("STORY STUDIO")
                                    .size(8.5)
                                    .color(C::muted())
                                    .strong(),
                            );
                        });
                    }
                    ui.add_space(18.0);
                    ui.separator();
                    ui.add_space(14.0);
                    ui.scope(|ui| {
                        ui.set_max_width(title_room);
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.add(
                            Label::new(
                                RichText::new(story_title)
                                    .font(serif_id(19.0))
                                    .color(C::ink()),
                            )
                            .truncate(),
                        );
                        if with_chapter_line {
                            ui.add(
                                Label::new(
                                    RichText::new(chapter_line).size(10.5).color(C::muted()),
                                )
                                .truncate(),
                            );
                        }
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if top_action(ui, "Settings", self.showing(Tab::Settings)).clicked() {
                            want_settings = true;
                        }
                        if top_action(ui, "Export", self.showing(Tab::Export)).clicked() {
                            want_export = true;
                        }
                        if top_action(ui, "Find", self.showing(Tab::Find))
                            .on_hover_text("Search the open chapter (Ctrl+F)")
                            .clicked()
                        {
                            want_find = true;
                        }
                        if top_action(ui, "Summary", self.showing(Tab::Summary))
                            .on_hover_text("Open or close the chapter summary")
                            .clicked()
                        {
                            want_summary = true;
                        }
                        if deleted_count > 0
                            && top_action(
                                ui,
                                &format!("Deleted ({deleted_count})"),
                                self.showing(Tab::Trash),
                            )
                            .on_hover_text(
                                "Put back something you deleted — kept until Folio closes",
                            )
                            .clicked()
                        {
                            want_trash = true;
                        }
                        if top_action(
                            ui,
                            "Lorebook",
                            self.right_open && self.right_tab == Tab::Lore,
                        )
                        .clicked()
                        {
                            want_lore = true;
                        }
                        if top_action(ui, "AI desk", self.right_open && self.right_tab == Tab::Ai)
                            .clicked()
                        {
                            want_ai = true;
                        }
                        ui.add_space(6.0);
                        let color = if self.dirty { C::amber() } else { C::muted() };
                        let (dot_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
                        ui.painter().circle_filled(dot_rect.center(), 3.0, color);
                        ui.label(RichText::new(save_text).size(10.5).color(C::muted()));
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(12.0);
                        let lamp = probe_chip(ui, &self.probe, &probe_base, probe_demo);
                        if lamp.clicked() {
                            if probe_demo {
                                self.open_tab(Tab::Settings);
                            } else if self.test_rx.is_none() {
                                self.start_test();
                            }
                        }
                    });
                });
            });

        if want_ai {
            self.toggle_tab(Tab::Ai);
        }
        if want_lore {
            self.toggle_tab(Tab::Lore);
        }
        if want_settings {
            self.toggle_tab(Tab::Settings);
        }
        if want_export {
            if self.showing(Tab::Export) {
                self.right_open = false;
            } else {
                self.open_export();
            }
        }
        if want_trash {
            self.toggle_tab(Tab::Trash);
        }
        if want_find {
            if self.showing(Tab::Find) {
                self.right_open = false;
            } else {
                self.open_find();
            }
        }
        if want_summary {
            self.toggle_tab(Tab::Summary);
        }
    }

    // ---------------------------------------------------------------- library
}
