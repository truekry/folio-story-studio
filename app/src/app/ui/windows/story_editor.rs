//! The story editor and the new-story window.

use super::*;

impl FolioApp {
    pub(crate) fn story_editor_window(&mut self, ctx: &egui::Context) {
        if !self.story_editor_open {
            return;
        }
        let mut open = self.story_editor_open;
        let mut save = false;
        let mut cancel = false;
        Window::new("Story details")
            .id(Id::new("story-editor-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(500.0)
            .show(ctx, |ui| {
                field_label(ui, "Title");
                ui.add(
                    TextEdit::singleline(&mut self.story_title_draft)
                        .font(serif_id(20.0))
                        .desired_width(f32::INFINITY)
                );
                ui.add_space(6.0);
                field_label(ui, "Premise");
                ui.add(
                    TextEdit::multiline(&mut self.story_premise_draft)
                        .font(FontId::proportional(12.5))
                        .desired_width(f32::INFINITY)
                        .desired_rows(4)
                );
                ui.label(
                    RichText::new(
                        "The title, premise and relevant lore go into every AI request. Keep the premise short.",
                    )
                    .size(9.5)
                    .color(C::muted()),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary(ui, "Save changes").clicked() {
                        save = true;
                    }
                    if ghost(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                });
                ui.label(
                    RichText::new("Delete is in the story's right-click menu.")
                        .size(9.5)
                        .italics()
                        .color(C::muted()),
                );
            });
        if cancel {
            open = false;
        }
        self.story_editor_open = open;
        if !open {
            self.story_editor_story_id = None;
        }
        if save {
            let title = self.story_title_draft.trim().to_string();
            if title.is_empty() {
                self.notify("A story needs a title.", true);
                return;
            }
            let story_id = self
                .story_editor_story_id
                .clone()
                .unwrap_or_else(|| self.story_id());
            if let Some(story) = self.library.story_mut(&story_id) {
                story.title = title;
                story.premise = self.story_premise_draft.trim().to_string();
            }
            self.mark_dirty();
            self.story_editor_open = false;
            self.story_editor_story_id = None;
            self.notify("Story details saved.", false);
        }
    }

    pub(crate) fn new_story_window(&mut self, ctx: &egui::Context) {
        if !self.new_story_open {
            return;
        }
        let mut open = self.new_story_open;
        let mut create = false;
        let mut cancel = false;
        Window::new("New story")
            .id(Id::new("new-story-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("A story, a premise, and a first blank chapter.")
                        .size(11.5)
                        .color(C::muted()),
                );
                ui.add_space(6.0);
                field_label(ui, "Title");
                ui.add(
                    TextEdit::singleline(&mut self.new_story_title)
                        .font(serif_id(18.0))
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);
                field_label(ui, "Premise");
                ui.add(
                    TextEdit::multiline(&mut self.new_story_premise)
                        .font(FontId::proportional(12.5))
                        .desired_width(f32::INFINITY)
                        .desired_rows(3),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary(ui, "Create story").clicked() {
                        create = true;
                    }
                    if ghost(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel {
            open = false;
        }
        self.new_story_open = open;
        if create {
            self.create_story();
        }
    }
}
