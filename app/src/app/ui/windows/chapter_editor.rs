//! The chapter editor and the new-chapter window.

use super::*;

impl FolioApp {
    pub(crate) fn new_chapter_window(&mut self, ctx: &egui::Context) {
        if !self.new_chapter_open {
            return;
        }
        let mut open = self.new_chapter_open;
        let mut create = false;
        let mut cancel = false;
        Window::new("New chapter")
            .id(Id::new("new-chapter-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Add a chapter to the open story.")
                        .size(11.5)
                        .color(C::muted()),
                );
                ui.add_space(6.0);
                field_label(ui, "Chapter title");
                ui.add(
                    TextEdit::singleline(&mut self.new_chapter_title)
                        .font(serif_id(18.0))
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary(ui, "Add chapter").clicked() {
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
        self.new_chapter_open = open;
        if create {
            self.create_chapter();
        }
    }

    pub(crate) fn chapter_editor_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.chapter_editor.take() else {
            return;
        };
        let story_title = self
            .library
            .story(draft.story_id)
            .map(|story| story.title.clone())
            .unwrap_or_default();
        let mut open = true;
        let mut save = false;
        let mut cancel = false;
        Window::new("Chapter details")
            .id(Id::new("chapter-editor-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(500.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!("In “{story_title}”"))
                        .size(10.5)
                        .color(C::muted()),
                );
                ui.add_space(6.0);
                field_label(ui, "Chapter title");
                ui.add(
                    TextEdit::singleline(&mut draft.title)
                        .font(serif_id(20.0))
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);
                field_label(ui, "Chapter summary");
                ui.add(
                    TextEdit::multiline(&mut draft.summary)
                        .font(serif_id(13.5))
                        .desired_width(f32::INFINITY)
                        .desired_rows(5),
                );
                ui.label(
                    RichText::new("This summary is sent to the AI with later chapters.")
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
                    RichText::new("Delete is in the chapter's right-click menu.")
                        .size(9.5)
                        .italics()
                        .color(C::muted()),
                );
            });
        if cancel {
            open = false;
        }
        if save {
            let title = draft.title.trim().to_string();
            if title.is_empty() {
                self.notify("A chapter needs a title.", true);
                self.chapter_editor = Some(draft);
                return;
            }
            if let Some(chapter) = self
                .library
                .story_mut(draft.story_id)
                .and_then(|story| story.chapter_mut(draft.chapter_id))
            {
                chapter.title = title;
                let summary = draft.summary.trim().to_string();
                if chapter.summary != summary {
                    chapter.summary = summary;
                    chapter.summary_source = "manual".to_string();
                    chapter.summary_body_len = chapter.body.chars().count();
                    chapter.summary_body_hash = crate::model::content_hash(&chapter.body);
                }
            }
            self.mark_dirty();
            self.chapter_editor = None;
            self.notify("Chapter details saved.", false);
        } else if open {
            self.chapter_editor = Some(draft);
        }
    }
}
