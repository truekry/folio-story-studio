//! The confirmation window for destructive actions.

use super::*;

impl FolioApp {
    pub(crate) fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.confirm = None;
            return;
        }

        let mut confirmed = false;
        let mut cancelled = false;
        let modal = Modal::new(Id::new("delete-confirm"))
            .frame(
                Frame::NONE
                    .fill(C::paper())
                    .stroke(Stroke::new(1.0_f32, C::line()))
                    .corner_radius(16)
                    .inner_margin(Margin::symmetric(24, 20)),
            )
            .backdrop_color(Color32::from_black_alpha(110))
            .show(ctx, |ui| {
                ui.set_width(430.0);
                let (eyebrow, heading, detail, keep_label, delete_label, danger) = match &confirm {
                    Confirm::Story {
                        title,
                        chapters,
                        lore,
                        ..
                    } => (
                        "DELETE STORY",
                        format!("Delete “{title}”?"),
                        format!(
                            "This removes the story, its {} and all {}. Its notes and history go too.",
                            plural(*chapters, "chapter", "chapters"),
                            plural(*lore, "lorebook entry", "lorebook entries")
                        ),
                        "Keep story",
                        "Delete story",
                        true,
                    ),
                    Confirm::Chapter {
                        story_title,
                        title,
                        number,
                        words,
                        summarized,
                        ..
                    } => {
                        let extra = if *summarized {
                            format!("{} and its saved summary", plural(*words, "word", "words"))
                        } else {
                            plural(*words, "word", "words")
                        };
                        (
                            "DELETE CHAPTER",
                            format!("Delete “{title}”?"),
                            format!(
                                "Chapter {number} of “{story_title}” holds {extra}. The other chapters stay exactly as they are.",
                            ),
                            "Keep chapter",
                            "Delete chapter",
                            true,
                        )
                    }
                    Confirm::Lore {
                        story_title,
                        name,
                        category,
                        description,
                        pinned,
                        ..
                    } => {
                        let mut detail = format!(
                            "This removes the {category} entry from the lorebook of “{story_title}”. The other entries stay."
                        );
                        if !description.trim().is_empty() {
                            detail.push_str(&format!(
                                " “{}”",
                                truncate(description.trim(), 120)
                            ));
                        }
                        if *pinned {
                            detail.push_str(" It is pinned, so the AI sees it in every request.");
                        }
                        (
                            "DELETE LORE ENTRY",
                            format!("Delete “{name}”?"),
                            detail,
                            "Keep entry",
                            "Delete entry",
                            true,
                        )
                    }
                    Confirm::Chat { .. } => (
                        "CLEAR CHAT",
                        "Clear this conversation?".to_string(),
                        "This removes this chapter's brainstorm — every message here and the condensed memory of older turns. The chapter text, its summary and the lorebook stay exactly as they are.".to_string(),
                        "Keep chat",
                        "Clear chat",
                        false,
                    ),
                    Confirm::Replace {
                        query,
                        replacement,
                        matches,
                        chapters,
                        story_title,
                        chapter_title,
                        ..
                    } => {
                        let replacement = if replacement.is_empty() {
                            "nothing (the matches are removed)".to_string()
                        } else {
                            format!("“{}”", truncate(replacement, 60))
                        };
                        let in_chapter = !chapter_title.is_empty();
                        let detail = if in_chapter {
                            format!(
                                "Every “{}” in this chapter becomes {replacement}. The rest of the book, the lorebook and the chapter titles stay as they are.",
                                truncate(query, 60)
                            )
                        } else {
                            format!(
                                "Every “{}” in {} becomes {replacement}. The lorebook and chapter titles are left alone.",
                                truncate(query, 60),
                                plural(*chapters, "chapter", "chapters")
                            )
                        };
                        let heading = if in_chapter {
                            format!(
                                "Replace {} in “{chapter_title}”?",
                                plural(*matches, "match", "matches")
                            )
                        } else {
                            format!(
                                "Replace {} in “{story_title}”?",
                                plural(*matches, "match", "matches")
                            )
                        };
                        (
                            if in_chapter {
                                "REPLACE IN CHAPTER"
                            } else {
                                "REPLACE IN BOOK"
                            },
                            heading,
                            detail,
                            "Keep the text",
                            "Replace all",
                            false,
                        )
                    }
                };
                let note = if matches!(&confirm, Confirm::Chat { .. }) {
                    "This cannot be undone. The chapter and its summary stay exactly as they are."
                } else if danger {
                    "You can put it back from Deleted until you close Folio."
                } else {
                    "The notice right afterwards offers Undo, and every chapter summary asks for a refresh."
                };

                ui.label(
                    RichText::new(eyebrow)
                        .size(9.5)
                        .strong()
                        .color(if danger { C::red() } else { C::accent_deep() }),
                );
                ui.add_space(4.0);
                ui.add(
                    Label::new(
                        RichText::new(heading)
                            .font(serif_id(24.0))
                            .color(C::ink()),
                    )
                    .wrap(),
                );
                ui.add_space(7.0);
                ui.add(
                    Label::new(
                        RichText::new(detail)
                            .size(12.0)
                            .color(C::ink_soft())
                            .line_height(Some(18.0)),
                    )
                    .wrap(),
                );
                ui.add_space(5.0);
                ui.label(
                    RichText::new(note)
                        .size(10.0)
                        .italics()
                        .color(C::muted()),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let keep = Button::new(
                        RichText::new(keep_label).size(12.0).strong().color(C::ink()),
                    )
                    .fill(C::soft())
                    .stroke(Stroke::new(1.0_f32, C::line()))
                    .corner_radius(8)
                    .min_size(Vec2::new(0.0, 32.0));
                    if ui.add(keep).clicked() {
                        cancelled = true;
                    }
                    let remove = Button::new(
                        RichText::new(delete_label)
                            .size(12.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(if danger { C::red_deep() } else { C::accent() })
                    .stroke(Stroke::NONE)
                    .corner_radius(8)
                    .min_size(Vec2::new(0.0, 32.0));
                    if ui.add(remove).clicked() {
                        confirmed = true;
                    }
                });
                ui.add_space(2.0);
                ui.label(
                    RichText::new("Escape or a click outside keeps everything.")
                        .size(9.5)
                        .color(C::muted()),
                );
            });

        if modal.backdrop_response.clicked() {
            cancelled = true;
        }
        if confirmed {
            match confirm {
                Confirm::Story { story_id, .. } => self.delete_story_now(story_id),
                Confirm::Chapter {
                    story_id,
                    chapter_id,
                    ..
                } => self.delete_chapter_now(story_id, chapter_id),
                Confirm::Lore {
                    story_id, entry_id, ..
                } => self.delete_lore_now(story_id, entry_id),
                Confirm::Chat {
                    story_id,
                    chapter_id,
                    ..
                } => self.clear_chat_now(story_id, chapter_id),
                Confirm::Replace {
                    query,
                    replacement,
                    match_case,
                    chapter_id,
                    ..
                } => self.replace_in_book(&query, &replacement, match_case, chapter_id),
            }
        } else if cancelled {
            self.confirm = None;
        }
    }
}
