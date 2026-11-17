//! The prompt editor window.

use super::*;

impl FolioApp {
    /// The prompt editor: what each AI function is asked, and how long its
    /// answer should be. Opened from the settings.
    pub(crate) fn prompts_window(&mut self, ctx: &egui::Context) {
        if !self.prompts_open {
            return;
        }
        let mut open = self.prompts_open;
        let mut changed = false;
        let mut close = false;
        Window::new("Prompts")
            .id(Id::new("prompts-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(760.0)
            .default_height(620.0)
            .min_width(560.0)
            .min_height(340.0)
            .show(ctx, |ui| {
                // The button stays at the bottom of the window, so it can be
                // reached no matter how tall the window is or how far the list
                // is scrolled. Both panels share the window's own size, which
                // keeps the height stable while it is being resized.
                TopBottomPanel::bottom(Id::new("prompts-footer"))
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
                                    "A word target also decides how much room the reply gets; no limit leaves it to the endpoint's window.",
                                )
                                .size(9.5)
                                .color(C::muted()),
                            );
                        });
                    });
                CentralPanel::default()
                    .frame(Frame::NONE)
                    .show_inside(ui, |ui| {
                        ui.label(caps("WHAT FOLIO ASKS", C::accent_deep()));
                        ui.add(
                            Label::new(
                                RichText::new(
                                    "Each function has its own instruction and a length to aim for. Changing a text replaces the built-in wording; an empty field means the built-in one is used. No limit removes the length request and lets the answer use the room the endpoint leaves. For the chapter summary, the length applies to each block; the final merge keeps all block facts and has no separate target. Settings save themselves.",
                                )
                                .size(9.5)
                                .color(C::muted()),
                            )
                            .wrap(),
                        );
                        ui.add_space(8.0);
                        ScrollArea::vertical()
                            .id_salt("prompts-scroll")
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                            )
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for kind in ai::PromptKind::ALL {
                                    if prompt_card(
                                        ui,
                                        kind,
                                        &mut self.library.settings.prompts,
                                    ) {
                                        changed = true;
                                    }
                                    ui.add_space(8.0);
                                }
                            });
                    });
            });
        if close {
            open = false;
        }
        self.prompts_open = open;
        if changed {
            self.mark_dirty();
        }
    }
}
