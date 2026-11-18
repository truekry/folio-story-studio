//! The settings view in the side panel: appearance, endpoint, model and context.

use super::*;

impl FolioApp {
    /// The settings as one view of the side panel: endpoint, model, context,
    /// appearance and the way to the prompts.
    pub(crate) fn ui_settings(&mut self, ui: &mut Ui) {
        let mut s = self.library.settings.clone();
        let mut do_test = false;
        let mut do_fetch_models = false;
        let mut do_export = false;
        let mut do_prompts = false;
        let mut do_save = false;
        let mut close = false;
        let testing = self.test_rx.is_some();
        let fetching_models = self.models_rx.is_some();
        let test_status = self.test_status.clone();
        let store_dir = self.store.as_ref().map(|store| store.dir().to_path_buf());
        let store_path = self
            .store
            .as_ref()
            .map(|store| {
                format!(
                    "{}  ·  {} books in stories\\",
                    store.dir().display(),
                    self.library.stories.len()
                )
            })
            .unwrap_or_else(|| "Preview — nothing is saved".to_string());

        // The decisions stay on a fixed strip, as in the prompt
        // editor, so "Done" is reachable no matter how short the
        // screen is or how far the settings have been scrolled.
        TopBottomPanel::bottom(Id::new("settings-footer"))
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
                        RichText::new("Settings save automatically.")
                            .size(9.5)
                            .color(C::muted()),
                    );
                });
            });
        CentralPanel::default()
                    .frame(Frame::NONE)
                    .show_inside(ui, |ui| {
                        ScrollArea::vertical()
                            .id_salt("settings-scroll")
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                            )
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.label(caps("APPEARANCE", C::accent_deep()));
                                ui.label(
                                    RichText::new(
                                        "How Folio looks. The choice is kept with this library.",
                                    )
                                    .size(11.5)
                                    .color(C::ink_soft()),
                                );
                                ui.add_space(4.0);
                                field_label(ui, "Look");
                                let mut chosen = theme::look();
                                egui::ComboBox::from_id_salt("look-pick")
                                    .selected_text(chosen.label())
                                    .width(180.0)
                                    .show_ui(ui, |ui| {
                                        for option in crate::theme::Look::ALL {
                                            ui.selectable_value(&mut chosen, option, option.label())
                                                .on_hover_text(option.note());
                                        }
                                    });
                                ui.label(RichText::new(chosen.note()).size(9.5).color(C::muted()));
                                if chosen != theme::look() {
                                    theme::apply_style(ui.ctx(), chosen);
                                    self.library.look = chosen;
                                    self.mark_dirty();
                                }
                                ui.add_space(10.0);
                                ui.separator();
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("BRING YOUR OWN AI")
                                        .size(9.5)
                                        .strong()
                                        .color(C::accent_deep()),
                                );
                                ui.label(
                                    RichText::new(
                                        "Folio talks to any service that speaks the OpenAI chat format — OpenAI, OpenRouter, LM Studio, Ollama's compatibility endpoint, or your own server. Leave the key empty for the built-in demo writer.",
                                    )
                                    .size(11.5)
                                    .color(C::ink_soft()),
                                );
                                ui.add_space(6.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new("Presets:").size(10.0).color(C::muted()));
                                    if preset(ui, "OpenAI").clicked() {
                                        s.base_url = "https://api.openai.com/v1".to_string();
                                        s.model = "gpt-4o-mini".to_string();
                                    }
                                    if preset(ui, "OpenRouter").clicked() {
                                        s.base_url = "https://openrouter.ai/api/v1".to_string();
                                        s.model = "openai/gpt-4o-mini".to_string();
                                    }
                                    if preset(ui, "Ollama").clicked() {
                                        s.base_url = "http://localhost:11434/v1".to_string();
                                        s.model = "llama3.2".to_string();
                                    }
                                    if preset(ui, "LM Studio").clicked() {
                                        s.base_url = "http://localhost:1234/v1".to_string();
                                        s.model = "local-model".to_string();
                                    }
                                });
                                ui.add_space(8.0);
                                field_label(ui, "Endpoint address");
                                ui.add(
                                    TextEdit::singleline(&mut s.base_url)
                                        .font(FontId::proportional(12.5))
                                        .desired_width(f32::INFINITY)
                                );
                                ui.label(
                                    RichText::new(endpoint_note(&s))
                                        .size(9.5)
                                        .color(C::muted()),
                                );
                                ui.add_space(6.0);
                                field_label(ui, "API key");
                                ui.add(
                                    TextEdit::singleline(&mut s.api_key)
                                        .font(FontId::monospace(12.0))
                                        .password(!self.show_key)
                                        .desired_width(f32::INFINITY)
                                );
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut self.show_key, "Show key");
                                    if !s.api_key.is_empty() && ghost(ui, "Clear key").clicked() {
                                        s.api_key.clear();
                                    }
                                });
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    field_label(ui, "Model");
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let label = if fetching_models {
                                            "Asking…"
                                        } else {
                                            "Fetch from endpoint"
                                        };
                                        if ghost(ui, label)
                                            .on_hover_text("Ask the endpoint which models it serves (GET /models)")
                                            .clicked()
                                        {
                                            do_fetch_models = true;
                                        }
                                    });
                                });
                                ui.add(
                                    TextEdit::singleline(&mut s.model)
                                        .font(FontId::proportional(12.5))
                                        .desired_width(f32::INFINITY)
                                );
                                if !self.models.is_empty() && self.models_from == s.base_url.trim() {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("From the endpoint:").size(9.5).color(C::muted()));
                                        egui::ComboBox::from_id_salt("model-pick")
                                            .selected_text(truncate(&s.model, 44))
                                            .show_ui(ui, |ui| {
                                                for name in &self.models {
                                                    ui.selectable_value(&mut s.model, name.clone(), name);
                                                }
                                            });
                                    });
                                }
                                if let Some(error) = &self.models_error {
                                    ui.add(
                                        Label::new(RichText::new(error).size(9.5).color(C::red())).wrap(),
                                    );
                                }
                                ui.label(
                                    RichText::new(context_line(s.context_limit))
                                        .size(9.5)
                                        .color(C::muted()),
                                );
                                ui.add_space(8.0);
                                ui.add(
                                    egui::Slider::new(&mut s.temperature, 0.0..=1.5)
                                        .text("Creativity")
                                        .fixed_decimals(2),
                                );
                                ui.add(
                                    egui::Slider::new(&mut s.max_tokens, 128..=2048)
                                        .text("Reply tokens")
                                        .logarithmic(true),
                                );
                                ui.add_space(8.0);
                                field_label(ui, "Chapter summaries in the AI context");
                                ui.add(
                                    egui::Slider::new(&mut s.summary_window, 0..=crate::model::MAX_SUMMARY_WINDOW)
                                        .text("recent summaries sent in full"),
                                );
                                ui.add_space(6.0);
                                field_label(ui, "Block size for long chapters");
                                ui.horizontal_wrapped(|ui| {
                                    let mut block = s.summary_block_size as u32;
                                    if word_target(
                                        ui,
                                        "summary-block-size",
                                        &mut block,
                                        ai::SUMMARY_BLOCK_MIN_WORDS as u32
                                            ..=ai::SUMMARY_BLOCK_MAX_WORDS as u32,
                                        "How many words one summary block holds. A block always ends at a sentence; if the limit falls inside one, the block runs to its end instead of cutting it.",
                                    ) {
                                        s.summary_block_size = block as usize;
                                    }
                                    ui.label(
                                        RichText::new("words per block")
                                            .size(9.5)
                                            .color(C::muted()),
                                    );
                                });
                                ui.label(
                                    RichText::new(
                                        "When the limit is reached, the block ends at the last sentence end — never in the middle of a sentence.",
                                    )
                                    .size(9.5)
                                    .color(C::muted()),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(
                                        "Everything older is folded into one story-so-far block, so a long book still fits. Pressing Summarize first writes the missing summaries for those older chapters.",
                                    )
                                    .size(9.5)
                                    .color(C::muted()),
                                );
                                ui.label(
                                    RichText::new(
                                        "Summaries are written only when you ask for one — Folio never summarizes on its own. Long chapters are summarized in blocks; the block summaries are joined into one text afterwards.",
                                    )
                                    .size(9.5)
                                    .color(C::muted()),
                                );
                                ui.add_space(8.0);
                                ui.separator();
                                ui.horizontal(|ui| {
                                    let button = Button::new(
                                        RichText::new(if testing { "Testing…" } else { "Test connection" })
                                            .size(11.5)
                                            .color(C::ink())
                                            .strong(),
                                    )
                                    .fill(C::soft())
                                    .stroke(Stroke::new(1.0_f32, C::line()))
                                    .corner_radius(theme::radius(8));
                                    if ui.add_enabled(!testing, button).clicked() {
                                        do_test = true;
                                    }
                                    if let Some((ok, message)) = &test_status {
                                        ui.add(
                                            Label::new(
                                                RichText::new(message)
                                                    .size(10.5)
                                                    .color(if *ok { C::green() } else { C::red() }),
                                            )
                                            .wrap(),
                                        );
                                    }
                                });
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(
                                        "The key is stored only on this computer, in config.json. Nothing is sent anywhere except the endpoint above.",
                                    )
                                    .size(9.5)
                                    .color(C::muted()),
                                );
                                ui.separator();
                                ui.label(RichText::new("PROMPTS").size(9.5).strong().color(C::muted()));
                                ui.label(
                                    RichText::new(
                                        "What Folio asks the AI for each function, and how many words the answer should aim for.",
                                    )
                                    .size(9.5)
                                    .color(C::muted()),
                                );
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    let button = Button::new(
                                        RichText::new("Edit prompts…")
                                            .size(11.5)
                                            .color(C::ink())
                                            .strong(),
                                    )
                                    .fill(C::soft())
                                    .stroke(Stroke::new(1.0_f32, C::line()))
                                    .corner_radius(theme::radius(8));
                                    if ui.add(button).clicked() {
                                        do_prompts = true;
                                    }
                                });
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("STORY DATA").size(9.5).strong().color(C::muted()));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ghost(ui, "Save now").clicked() {
                                            do_save = true;
                                        }
                                        if ghost(ui, "Export story…").clicked() {
                                            do_export = true;
                                        }
                                    });
                                });
                                ui.add(
                                    Label::new(
                                        RichText::new(store_path)
                                            .size(9.5)
                                            .color(C::muted())
                                            .monospace(),
                                    )
                                    .wrap(),
                                );
                                ui.horizontal(|ui| {
                                    if let Some(dir) = &store_dir {
                                        if ghost(ui, "Open data folder").clicked() {
                                            crate::folder::open_folder(dir);
                                        }
                                    }
                                });
                                ui.add(
                                    Label::new(
                                        RichText::new("Stories save automatically while you write. Ctrl+S saves immediately.")
                                            .size(9.5)
                                            .italics()
                                            .color(C::muted()),
                                    )
                                    .wrap(),
                                );
                                ui.add_space(8.0);
                            });
                    });

        if close {
            self.right_open = false;
        }
        if s != self.library.settings {
            // An answer about the old address says nothing about the new one.
            let moved = s.base_url.trim() != self.library.settings.base_url.trim()
                || s.api_key != self.library.settings.api_key;
            self.library.settings = s;
            if moved {
                self.test_status = None;
            }
            self.mark_dirty();
        }
        if do_test {
            self.start_test();
        }
        if do_fetch_models {
            self.start_model_fetch();
        }
        if do_export {
            self.open_export();
            self.open_tab(Tab::Export);
        }
        if do_prompts {
            self.open_tab(Tab::Prompts);
        }
        if do_save {
            self.save_now();
            self.notify("Saved to your library.", false);
        }
    }
}
