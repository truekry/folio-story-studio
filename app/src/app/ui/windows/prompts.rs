//! The prompt editor, a second view in the side panel.

use super::*;

impl FolioApp {
    /// The prompt editor as a view of the side panel, reached from Settings.
    pub(crate) fn ui_prompts(&mut self, ui: &mut Ui) {
        let mut changed = false;
        let mut close = false;
        let mut back = false;
        // The button stays at the bottom of the panel, so it can be
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
                            if ghost(ui, "Back to settings").clicked() {
                                back = true;
                            }
                        });
                        ui.add(
                            Label::new(
                                RichText::new(
                                    "A word target also decides how much room the reply gets; no limit leaves it to the endpoint's window.",
                                )
                                .size(9.5)
                                .color(C::muted()),
                            )
                            .wrap(),
                        );
                    });
        CentralPanel::default()
                    .frame(Frame::NONE)
                    .show_inside(ui, |ui| {
                        ui.label(caps("WHAT FOLIO ASKS", C::accent_deep()));
                        ui.add(
                            Label::new(
                                RichText::new(
                                    "Each function has its own instruction and a length to aim for. Changing a text replaces the built-in wording; an empty field means the built-in one is used. No limit removes the length request and lets the answer use the room the endpoint leaves. For the chapter summary, the first target applies to each block and the second to the merged text. Numbers are typed, not dragged. Settings save themselves.",
                                )
                                .size(9.5)
                                .color(C::muted()),
                            )
                            .wrap(),
                        );
                        ui.add_space(8.0);
                        // Screenshot aid: open the layout at the end of the list.
                        let at_end =
                            std::env::var("FOLIO_VIEW").ok().as_deref() == Some("prompt-layout");
                        let scroll = ScrollArea::vertical()
                            .id_salt("prompts-scroll")
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                            )
                            .auto_shrink([false, false]);
                        scroll.show(ui, |ui| {
                                if !at_end {
                                    for kind in ai::PromptKind::ALL {
                                        if prompt_card(
                                            ui,
                                            kind,
                                            &mut self.library.settings.prompts,
                                        ) {
                                            changed = true;
                                        }
                                        if kind == ai::PromptKind::Summary
                                            && summary_merge_card(
                                                ui,
                                                &mut self.library.settings.prompts,
                                            )
                                        {
                                            changed = true;
                                        }
                                        ui.add_space(8.0);
                                    }
                                    ui.add_space(2.0);
                                }
                                // The screenshot of the layout shows it alone,
                                // so a prompt card above it cannot be cut off.
                                // Two layouts: Continue and the assistant.
                                // Rewrite and Elaborate do not use one.
                                if prompt_layout_card(
                                    ui,
                                    LayoutKind::Writing,
                                    &mut self.library.settings.prompts.order,
                                    &mut self.library.settings.prompts.disabled,
                                ) {
                                    changed = true;
                                }
                                ui.add_space(8.0);
                                if prompt_layout_card(
                                    ui,
                                    LayoutKind::Chat,
                                    &mut self.library.settings.prompts.chat_order,
                                    &mut self.library.settings.prompts.chat_disabled,
                                ) {
                                    changed = true;
                                }
                            });
                    });
        if back {
            self.open_tab(Tab::Settings);
        }
        if close {
            self.right_open = false;
        }
        if changed {
            self.mark_dirty();
        }
    }
}
