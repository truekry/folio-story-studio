//! Recently deleted, in the side panel.

use super::*;

impl FolioApp {
    /// The session's deleted items as a view of the side panel.
    pub(crate) fn ui_trash(&mut self, ui: &mut Ui) {
        let mut close = false;
        let mut restore: Option<u64> = None;
        ui.label(
                    RichText::new(
                        "Everything you delete stays here until you close Folio. After a restart it is really gone.",
                    )
                    .size(11.5)
                    .color(C::ink_soft()),
                );
        ui.add_space(7.0);
        if self.trash.is_empty() {
            Frame::NONE
                .fill(C::card())
                .stroke(Stroke::new(1.0_f32, C::line()))
                .corner_radius(theme::radius(11))
                .inner_margin(Margin::symmetric(13, 15))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("Nothing deleted since Folio started.")
                            .size(12.0)
                            .italics()
                            .color(C::muted()),
                    );
                });
        } else {
            ScrollArea::vertical()
                .id_salt("trash-list")
                .max_height(400.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for item in self.trash.iter().rev() {
                        let id = item.id();
                        let kind = item.kind();
                        let title = item.title().to_string();
                        let context = item.context();
                        let story_gone = item
                            .story_id()
                            .map(|story_id| self.library.story(&story_id).is_none())
                            .unwrap_or(false);
                        let seconds = item.deleted_at().elapsed().as_secs();
                        let age = if seconds < 5 {
                            "just now".to_string()
                        } else if seconds < 60 {
                            format!("{seconds}s ago")
                        } else {
                            format!("{} min ago", seconds / 60)
                        };
                        Frame::NONE
                            .fill(C::card())
                            .stroke(Stroke::new(1.0_f32, C::line()))
                            .corner_radius(theme::radius(11))
                            .inner_margin(Margin::symmetric(12, 9))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            pill(ui, kind, C::ink_soft(), C::soft());
                                            ui.label(
                                                RichText::new(&title)
                                                    .font(serif_id(14.5))
                                                    .color(C::ink()),
                                            );
                                            ui.label(
                                                RichText::new(&age).size(9.0).color(C::muted()),
                                            );
                                        });
                                        ui.label(
                                            RichText::new(if story_gone {
                                                format!("{context}  ·  its story is still deleted")
                                            } else {
                                                context.clone()
                                            })
                                            .size(10.0)
                                            .color(if story_gone { C::red() } else { C::muted() }),
                                        );
                                    });
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let button = Button::new(
                                            RichText::new("Put back").size(10.5).strong().color(
                                                if story_gone { C::muted() } else { C::ink() },
                                            ),
                                        )
                                        .fill(C::soft())
                                        .stroke(Stroke::new(1.0_f32, C::line()))
                                        .corner_radius(theme::radius(7));
                                        if ui
                                            .add_enabled(!story_gone, button)
                                            .on_hover_text(if story_gone {
                                                "Put the story back first"
                                            } else {
                                                "Return it to where it was"
                                            })
                                            .clicked()
                                        {
                                            restore = Some(id);
                                        }
                                    });
                                });
                            });
                        ui.add_space(4.0);
                    }
                });
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if primary(ui, "Done").clicked() {
                close = true;
            }
            ui.label(
                RichText::new(format!(
                    "{} kept for this session",
                    plural(self.trash.len(), "item", "items")
                ))
                .size(9.5)
                .color(C::muted()),
            );
        });
        if close {
            self.right_open = false;
        }
        if let Some(id) = restore {
            self.restore_trash(id);
        }
    }
}
