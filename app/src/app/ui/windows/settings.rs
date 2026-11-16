//! The settings window: endpoint, model and context.

use super::*;

impl FolioApp {
    pub(crate) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = self.settings_open;
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
        let show_summary_before = self.library.show_summary;
        let store_path = self
            .store_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Preview — nothing is saved".to_string());

        Window::new("AI endpoint & settings")
            .id(Id::new("settings-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(600.0)
            .show(ctx, |ui| {
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
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut s.temperature, 0.0..=1.5)
                            .text("Creativity")
                            .fixed_decimals(2),
                    );
                    ui.add_space(10.0);
                    ui.add(
                        egui::Slider::new(&mut s.max_tokens, 128..=2048)
                            .text("Reply tokens")
                            .logarithmic(true),
                    );
                });
                ui.add_space(8.0);
                field_label(ui, "Chapter summaries in the AI context");
                ui.add(
                    egui::Slider::new(&mut s.summary_window, 0..=crate::model::MAX_SUMMARY_WINDOW)
                        .text("recent summaries sent in full"),
                );
                ui.label(
                    RichText::new(
                        "Everything older is folded into one story-so-far block, so a long book still fits. Pressing Summarize first writes the missing summaries for those older chapters.",
                    )
                    .size(9.5)
                    .color(C::muted()),
                );
                ui.label(
                    RichText::new(
                        "Summaries are written only when you ask for one — Folio never summarizes on its own.",
                    )
                    .size(9.5)
                    .color(C::muted()),
                );
                ui.add_space(4.0);
                ui.checkbox(
                    &mut self.library.show_summary,
                    "Show a Summary… button under the chapter title",
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
                    .corner_radius(8);
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
                        "The key is stored only on this computer, in the story library file. Nothing is sent anywhere except the endpoint above.",
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
                    .corner_radius(8);
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
                ui.label(
                    RichText::new("Stories save automatically while you write. Ctrl+S saves immediately.")
                        .size(9.5)
                        .italics()
                        .color(C::muted()),
                );
                ui.add_space(8.0);
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
                ui.add_space(2.0);
            });

        if close {
            open = false;
        }
        self.settings_open = open;
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
        if self.library.show_summary != show_summary_before {
            self.mark_dirty();
        }
        if do_test {
            self.start_test();
        }
        if do_fetch_models {
            self.start_model_fetch();
        }
        if do_export {
            self.settings_open = false;
            self.open_export();
        }
        if do_prompts {
            self.prompts_open = true;
        }
        if do_save {
            self.save_now();
            self.notify("Saved to your library.", false);
        }
    }
}
