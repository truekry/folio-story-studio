//! Which window is open, and the notices at the bottom.

use super::*;

mod chapter_editor;
mod confirm;
mod export;
mod find;
mod lore;
mod prompts;
mod settings;
mod story_editor;
mod trash;

impl FolioApp {
    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
        self.settings_window(ctx);
        self.prompts_window(ctx);
        self.summary_window(ctx);
        self.export_window(ctx);
        self.story_editor_window(ctx);
        self.new_story_window(ctx);
        self.new_chapter_window(ctx);
        self.chapter_editor_window(ctx);
        self.lore_editor_window(ctx);
        self.lore_proposal_window(ctx);
        self.trash_window(ctx);
        self.find_window(ctx);
        self.confirm_window(ctx);
    }

    pub(crate) fn ui_toast(&mut self, ctx: &egui::Context) {
        let Some(toast) = &self.toast else {
            return;
        };
        let undo = toast.undo.clone();
        let lifetime = if undo.is_some() { 14 } else { 5 };
        if toast.born.elapsed() > Duration::from_secs(lifetime) {
            self.toast = None;
            return;
        }
        let text = toast.text.clone();
        let error = toast.error;
        let undo_label = match &undo {
            Some(UndoAction::Replace(_)) => "Undo replace",
            Some(UndoAction::Delete(_)) => "Undo delete",
            None => "",
        };
        let max_width = toast_width(ctx.screen_rect().width());
        let mut undo_clicked = false;
        egui::Area::new(Id::new("toast"))
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -18.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                Frame::NONE
                    .fill(if error { C::red_deep() } else { C::toast() })
                    .corner_radius(10)
                    .inner_margin(Margin::symmetric(14, 9))
                    .show(ui, |ui| {
                        // The text gets a real column to wrap in. The action
                        // sits below it instead of stealing width from the
                        // sentence, which is what made long notices unreadable.
                        ui.set_max_width(max_width);
                        ui.add(
                            Label::new(RichText::new(text).size(12.0).color(C::on_toast()))
                                .wrap(),
                        );
                        if undo.is_some() {
                            ui.add_space(6.0);
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new(undo_label)
                                            .size(11.5)
                                            .strong()
                                            .color(C::ink()),
                                    )
                                    .fill(C::card())
                                    .corner_radius(6),
                                )
                                .on_hover_text(match &undo {
                                    Some(UndoAction::Replace(_)) => {
                                        "Put the previous chapter text back"
                                    }
                                    _ => {
                                        "Put it back — it stays available in Deleted until Folio closes"
                                    }
                                })
                                .clicked()
                            {
                                undo_clicked = true;
                            }
                        }
                    });
            });
        if undo_clicked {
            match undo {
                Some(UndoAction::Delete(id)) => self.restore_trash(id),
                Some(UndoAction::Replace(snapshot)) => self.restore_replacement(snapshot),
                None => {}
            }
        }
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}
