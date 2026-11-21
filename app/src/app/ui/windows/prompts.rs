//! The prompt editor, a second view in the side panel.

use super::*;

impl FolioApp {
    /// The prompt editor as a view of the side panel, reached from Settings.
    pub(crate) fn ui_prompts(&mut self, ui: &mut Ui) {
        let mut changed = false;
        let mut close = false;
        let mut back = false;
        // What the two layouts would send right now. Built before the cards,
        // because they borrow the settings mutably.
        let (writing_size, chat_size) = self.prompt_sizes();
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
                                    writing_size,
                                ) {
                                    changed = true;
                                }
                                ui.add_space(8.0);
                                if prompt_layout_card(
                                    ui,
                                    LayoutKind::Chat,
                                    &mut self.library.settings.prompts.chat_order,
                                    &mut self.library.settings.prompts.chat_disabled,
                                    chat_size,
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

    /// What the two layouts would send right now, in the same estimate the
    /// budget uses: the system text, the assembled prompt and, for the
    /// assistant, its conversation and its memory. `None` when no chapter is
    /// open to build from.
    fn prompt_sizes(&self) -> (Option<PromptSize>, Option<PromptSize>) {
        let Some(story) = self.library.stories.get(self.active_story) else {
            return (None, None);
        };
        let Some(chapter) = story.chapters.get(self.active_chapter) else {
            return (None, None);
        };
        let settings = &self.library.settings;
        let window = settings.context_limit;
        let system = ai::estimated_tokens(ai::SYSTEM_PROMPT);
        let body_chars = chapter.body.chars().count();

        let writing = ai::chat_prompt_with_neighbours(
            story,
            chapter,
            Action::Continue,
            "",
            "",
            None,
            None,
            settings,
        );
        let writing_tokens = ai::prompt_tokens(&writing, &[], "");
        let writing_detail = format!(
            "System text {system} · layout {} · chapter excerpt {} characters. The assistant reads more of the chapter and adds its conversation, so its request is larger.",
            ai::estimated_tokens(&writing),
            ai::chapter_excerpt_chars(Action::Continue, settings).min(body_chars)
        );

        let chat = ai::chat_prompt_with_neighbours(
            story,
            chapter,
            Action::Brainstorm,
            &self.ai_input,
            "",
            None,
            None,
            settings,
        );
        let chat_tokens = ai::prompt_tokens(&chat, &chapter.chat, &chapter.chat_summary);
        let conversation =
            ai::history_tokens(&chapter.chat) + ai::estimated_tokens(&chapter.chat_summary);
        let mut parts: Vec<String> = Vec::new();
        if conversation > 0 {
            parts.push(format!("chat so far ≈ {conversation} tokens"));
        }
        if self.ai_input.trim().is_empty() {
            parts.push("plus your question".to_string());
        }
        let chat_note = (!parts.is_empty()).then(|| parts.join(" · "));
        let chat_detail = format!(
            "System text {system} · layout {} · chapter excerpt {} characters · chat so far {conversation} tokens.",
            ai::estimated_tokens(&chat),
            ai::chapter_excerpt_chars(Action::Brainstorm, settings).min(body_chars)
        );

        (
            Some(PromptSize {
                line: ai::size_line("Continue request", writing_tokens, window),
                tight: ai::size_is_tight(writing_tokens, window),
                detail: writing_detail,
                note: None,
            }),
            Some(PromptSize {
                line: ai::size_line("Assistant request", chat_tokens, window),
                tight: ai::size_is_tight(chat_tokens, window),
                detail: chat_detail,
                note: chat_note,
            }),
        )
    }
}
