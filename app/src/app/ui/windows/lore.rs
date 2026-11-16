//! The lore entry editor and the AI lore proposal window.

use super::*;

impl FolioApp {
    pub(crate) fn lore_editor_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.lore_editor.take() else {
            return;
        };
        let mut open = true;
        let mut save = false;
        let mut cancel = false;
        let title = if draft.entry_id.is_some() {
            "Edit lore entry"
        } else {
            "New lore entry"
        };
        Window::new(title)
            .id(Id::new("lore-editor-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(500.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Entries are looked up by name, alias and trigger word, and relevant ones are sent to the AI with every request.",
                    )
                    .size(10.5)
                    .color(C::muted()),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        field_label(ui, "Name");
                        ui.add(
                            TextEdit::singleline(&mut draft.name)
                                .font(serif_id(17.0))
                                .desired_width(230.0)
                        );
                    });
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        field_label(ui, "Category");
                        egui::ComboBox::from_id_salt("lore-category")
                            .selected_text(&draft.category)
                            .width(150.0)
                            .show_ui(ui, |ui| {
                                for category in
                                    ["Character", "Place", "Faction", "Item", "Lore"]
                                {
                                    ui.selectable_value(
                                        &mut draft.category,
                                        category.to_string(),
                                        category,
                                    );
                                }
                            });
                    });
                });
                ui.add_space(6.0);
                field_label(ui, "Also known as (comma-separated)");
                ui.add(
                    TextEdit::singleline(&mut draft.aliases)
                        .font(FontId::proportional(12.0))
                        .desired_width(f32::INFINITY)
                );
                ui.add_space(6.0);
                field_label(ui, "Trigger words (comma-separated)");
                ui.add(
                    TextEdit::singleline(&mut draft.triggers)
                        .font(FontId::proportional(12.0))
                        .desired_width(f32::INFINITY)
                );
                ui.label(
                    RichText::new(
                        "Write any of these in a chapter and this entry joins the AI request — even if its name never appears.",
                    )
                    .size(9.5)
                    .italics()
                    .color(C::muted()),
                );
                ui.add_space(6.0);
                field_label(ui, "Description");
                ui.add(
                    TextEdit::multiline(&mut draft.description)
                        .font(FontId::proportional(12.5))
                        .desired_width(f32::INFINITY)
                        .desired_rows(5)
                );
                ui.add_space(5.0);
                ui.checkbox(&mut draft.pinned, "Always include in the AI context");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary(ui, "Save entry").clicked() {
                        save = true;
                    }
                    if ghost(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                });
                if draft.entry_id.is_some() {
                    ui.label(
                        RichText::new("Delete is in the entry's right-click menu.")
                            .size(9.5)
                            .italics()
                            .color(C::muted()),
                    );
                }
            });
        if cancel {
            open = false;
        }

        if save {
            let name = draft.name.trim().to_string();
            if name.is_empty() {
                self.notify("Give the lore entry a name first.", true);
                self.lore_editor = Some(draft);
                return;
            }
            let new_id = if draft.entry_id.is_none() {
                Some(self.library.next_id())
            } else {
                None
            };
            if let Some(story) = self.library.story_mut(draft.story_id) {
                if let Some(id) = draft.entry_id {
                    if let Some(entry) = story.lore.iter_mut().find(|e| e.id == id) {
                        entry.name = name;
                        entry.category = draft.category;
                        entry.aliases = draft.aliases;
                        entry.triggers = draft.triggers;
                        entry.description = draft.description;
                        entry.pinned = draft.pinned;
                    }
                } else if let Some(id) = new_id {
                    story.lore.push(LoreEntry {
                        id,
                        name,
                        category: draft.category,
                        aliases: draft.aliases,
                        triggers: draft.triggers,
                        description: draft.description,
                        pinned: draft.pinned,
                    });
                }
            }
            self.mark_dirty();
            self.notify("Lore entry saved.", false);
        } else if open {
            self.lore_editor = Some(draft);
        }
    }

    /// The AI's lorebook suggestion in a review window. Nothing is written to
    /// the lorebook until the author saves it; the target is a starting point
    /// the author can change.
    pub(crate) fn lore_proposal_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.lore_proposal.take() else {
            return;
        };
        let Some(story) = self.library.story(draft.story_id) else {
            self.notify("That story is no longer open.", true);
            return;
        };
        let entries: Vec<(u64, String, String)> = story
            .lore
            .iter()
            .map(|entry| (entry.id, entry.name.clone(), entry.category.clone()))
            .collect();
        let current = draft
            .target
            .and_then(|id| story.lore.iter().find(|entry| entry.id == id).cloned());
        let add_to = current.as_ref().map(|entry| entry.name.clone());

        let mut open = true;
        let mut save = false;
        let mut cancel = false;
        let title = match &add_to {
            Some(name) => format!("Add to “{name}”"),
            None => "New lore entry".to_string(),
        };
        Window::new(title)
            .id(Id::new("lore-proposal-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "The AI read the selected passage and compared it with the lorebook. Check the result before saving.",
                    )
                    .size(10.5)
                    .color(C::muted()),
                );
                if !draft.well_formed {
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new("The reply did not follow the expected shape.")
                            .size(9.5)
                            .color(C::amber()),
                    );
                }
                ui.add_space(6.0);
                field_label(ui, "Entry");
                egui::ComboBox::from_id_salt("lore-proposal-target")
                    .selected_text(match (&add_to, &current) {
                        (Some(name), Some(entry)) => format!("{name} [{}]", entry.category),
                        _ => "New entry".to_string(),
                    })
                    .width(400.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.target, None, "New entry");
                        for (id, name, category) in &entries {
                            ui.selectable_value(
                                &mut draft.target,
                                Some(*id),
                                format!("{name} [{category}]"),
                            );
                        }
                    });
                ui.add_space(6.0);
                if let Some(entry) = &current {
                    Frame::NONE
                        .fill(C::summary_bg())
                        .corner_radius(10)
                        .inner_margin(Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(caps("In the lorebook", C::muted()));
                            ui.add(
                                Label::new(
                                    RichText::new(truncate(entry.description.trim(), 600))
                                        .font(serif_id(13.0))
                                        .color(C::ink_soft()),
                                )
                                .wrap(),
                            );
                        });
                    ui.add_space(6.0);
                    field_label(ui, "Facts to add");
                    ui.add(
                        TextEdit::multiline(&mut draft.text)
                            .font(FontId::proportional(12.5))
                            .desired_width(f32::INFINITY)
                            .desired_rows(5),
                    );
                } else {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            field_label(ui, "Name");
                            ui.add(
                                TextEdit::singleline(&mut draft.name)
                                    .font(serif_id(17.0))
                                    .desired_width(260.0),
                            );
                        });
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            field_label(ui, "Category");
                            egui::ComboBox::from_id_salt("lore-proposal-category")
                                .selected_text(&draft.category)
                                .width(160.0)
                                .show_ui(ui, |ui| {
                                    for category in
                                        ["Character", "Place", "Faction", "Item", "Lore"]
                                    {
                                        ui.selectable_value(
                                            &mut draft.category,
                                            category.to_string(),
                                            category,
                                        );
                                    }
                                });
                        });
                    });
                    ui.add_space(6.0);
                    field_label(ui, "Also known as (comma-separated)");
                    ui.add(
                        TextEdit::singleline(&mut draft.aliases)
                            .font(FontId::proportional(12.0))
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(6.0);
                    field_label(ui, "Trigger words (comma-separated)");
                    ui.add(
                        TextEdit::singleline(&mut draft.triggers)
                            .font(FontId::proportional(12.0))
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(6.0);
                    field_label(ui, "Description");
                    ui.add(
                        TextEdit::multiline(&mut draft.text)
                            .font(FontId::proportional(12.5))
                            .desired_width(f32::INFINITY)
                            .desired_rows(5),
                    );
                    ui.add_space(5.0);
                    ui.checkbox(&mut draft.pinned, "Always include in the AI context");
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary(
                        ui,
                        if add_to.is_some() {
                            "Add to entry"
                        } else {
                            "Create entry"
                        },
                    )
                    .clicked()
                    {
                        save = true;
                    }
                    if ghost(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel {
            open = false;
        }

        if save {
            let text = draft.text.trim().to_string();
            if text.is_empty() {
                self.notify("Nothing to save — the text is empty.", true);
                self.lore_proposal = Some(draft);
                return;
            }
            if let Some(id) = draft.target {
                let mut saved = None;
                if let Some(story) = self.library.story_mut(draft.story_id) {
                    if let Some(entry) = story.lore.iter_mut().find(|entry| entry.id == id) {
                        entry.description = append_lore(&entry.description, &text);
                        saved = Some(entry.name.clone());
                    }
                }
                if let Some(name) = saved {
                    self.mark_dirty();
                    self.notify(format!("Added the facts to “{name}”."), false);
                } else {
                    self.notify("That entry is gone. Pick another target.", true);
                    self.lore_proposal = Some(draft);
                }
            } else {
                let name = draft.name.trim().to_string();
                if name.is_empty() {
                    self.notify("Give the new entry a name first.", true);
                    self.lore_proposal = Some(draft);
                    return;
                }
                let id = self.library.next_id();
                if let Some(story) = self.library.story_mut(draft.story_id) {
                    story.lore.push(LoreEntry {
                        id,
                        name: name.clone(),
                        category: draft.category.clone(),
                        aliases: draft.aliases.trim().to_string(),
                        triggers: draft.triggers.trim().to_string(),
                        description: text,
                        pinned: draft.pinned,
                    });
                }
                self.mark_dirty();
                self.notify(format!("Created the lore entry “{name}”."), false);
            }
        } else if open {
            self.lore_proposal = Some(draft);
        }
    }
}
