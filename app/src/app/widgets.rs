//! Small shared pieces of the interface: buttons, cards and chips.

use super::*;

/// The width a notice may use. A notice that grows with its text can become
/// wider than the window, and one whose wrapping label has no fixed width can
/// collapse into a narrow column. This keeps both ends in check.
pub(crate) fn toast_width(screen_width: f32) -> f32 {
    let room = (screen_width - 64.0).max(160.0);
    room.min(620.0).min((screen_width - 32.0).max(120.0))
}

/// The width of a confirmation dialog's text column. The frame around it adds
/// its own margins, so this stays a little narrower than the window.
pub(crate) fn confirm_width(screen_width: f32) -> f32 {
    let room = (screen_width - 96.0).max(240.0);
    room.min(460.0).min((screen_width - 48.0).max(200.0))
}

/// One card in the prompt editor: the function's name, the length it aims for,
/// and the instruction itself.
pub(crate) fn prompt_card(ui: &mut Ui, kind: ai::PromptKind, prompts: &mut PromptSettings) -> bool {
    let mut changed = false;
    Frame::NONE
        .fill(C::card())
        .stroke(Stroke::new(1.0_f32, C::line()))
        .corner_radius(10)
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let per_block = kind == ai::PromptKind::Summary;
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(kind.label())
                        .size(12.5)
                        .strong()
                        .color(C::ink()),
                );
                if kind.is_edited(prompts) {
                    ui.label(RichText::new("edited").size(9.0).color(C::accent()));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ghost(ui, "Reset").clicked() {
                        let slot = kind.slot_mut(prompts);
                        slot.text.clear();
                        slot.words = None;
                        changed = true;
                    }
                    let mut words = kind.words(prompts);
                    let range = kind.words_range();
                    if ui
                        .add(
                            egui::DragValue::new(&mut words)
                                .range(range)
                                .speed(5.0)
                                .custom_formatter(|n, _| {
                                    let n = n.round() as u32;
                                    match n {
                                        0 => "no limit".to_string(),
                                        1 => "1 word".to_string(),
                                        n => format!("{n} words"),
                                    }
                                }),
                        )
                        .on_hover_text(if per_block {
                            "How many words each block summary should aim for. The final merged summary has no separate target; it keeps all facts from the blocks."
                        } else {
                            "How many words the answer should aim for. No limit means Folio asks for no particular length and lets the reply use the room the endpoint gives it."
                        })
                        .changed()
                    {
                        kind.slot_mut(prompts).words = Some(words);
                        changed = true;
                    }
                    ui.label(
                        RichText::new(if per_block { "per block" } else { "aim for" })
                            .size(9.5)
                            .color(C::muted()),
                    );
                });
            });
            ui.add_space(3.0);
            if per_block {
                ui.label(
                    RichText::new(
                        "The chapter is summarized in blocks; this target applies to each block, not to the merged text.",
                    )
                    .size(9.0)
                    .color(C::muted()),
                );
                ui.add_space(2.0);
            }
            let default = kind.default_instruction();
            let mut shown = kind.base_text(prompts);
            let rows = if kind == ai::PromptKind::Summary { 4 } else { 3 };
            let response = ui.add(
                TextEdit::multiline(&mut shown)
                    .id(Id::new(("prompt-text", kind.label())))
                    .font(FontId::proportional(12.0))
                    .text_color(C::ink())
                    .desired_width(f32::INFINITY)
                    .desired_rows(rows),
            );
            if response.changed() {
                // Typing the default back in means "built-in", not a copy of it.
                let slot = kind.slot_mut(prompts);
                slot.text = if shown.trim() == default.trim() {
                    String::new()
                } else {
                    shown.clone()
                };
                changed = true;
            }
            ui.label(
                RichText::new(if kind.is_edited(prompts) {
                    "Your instruction is sent instead of the built-in one."
                } else {
                    "Built-in instruction."
                })
                .size(9.0)
                .italics()
                .color(C::muted()),
            );
        });
    changed
}

/// One of the two export choices, drawn as a card with a radio dot.
pub(crate) fn export_choice(ui: &mut Ui, selected: bool, title: &str, caption: &str) -> Response {
    let response = Frame::NONE
        .fill(if selected {
            C::accent_soft()
        } else {
            C::card()
        })
        .stroke(Stroke::new(
            1.0_f32,
            if selected { C::accent() } else { C::line() },
        ))
        .corner_radius(9)
        .inner_margin(Margin::symmetric(11, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                let center = rect.center();
                ui.painter().circle_stroke(
                    center,
                    6.0,
                    Stroke::new(1.4_f32, if selected { C::accent() } else { C::muted() }),
                );
                if selected {
                    ui.painter().circle_filled(center, 3.2, C::accent());
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(RichText::new(title).size(12.0).strong().color(C::ink()));
                    ui.add(Label::new(RichText::new(caption).size(9.5).color(C::muted())).wrap());
                });
            });
        })
        .response
        .interact(Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

pub(crate) fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{cut}…")
}

/// egui's text edit treats a right-button press as the start of a click and
/// collapses the selection to the caret. Put the marked range back and keep it
/// from being overwritten this frame, so the highlight and the right-click menu
/// survive. Returns true while the right button is in play.
pub(crate) fn keep_selection_on_secondary(
    ui: &Ui,
    id: Id,
    had_focus: bool,
    selection: &mut Option<Range<usize>>,
) -> bool {
    if ui.input(|i| i.pointer.button_pressed(PointerButton::Secondary)) {
        if !had_focus {
            // The old words belong to whatever was focused before; a right
            // click into an unfocused page starts from nothing.
            *selection = None;
        } else if let Some(range) = selection
            .as_ref()
            .filter(|range| range.start < range.end)
            .cloned()
        {
            let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(range.start),
                    egui::text::CCursor::new(range.end),
                )));
            state.store(ui.ctx(), id);
            ui.memory_mut(|memory| memory.request_focus(id));
        }
    }
    ui.input(|i| {
        i.pointer.button_pressed(PointerButton::Secondary)
            || i.pointer.button_down(PointerButton::Secondary)
            || i.pointer.button_released(PointerButton::Secondary)
    })
}

/// The author's note for the next stretch of prose, pinned under the scrolling
/// page. It sends nothing by itself; Continue reads it. Rewrite, Elaborate and
/// the lorebook pass live in the right-click menu on a selection. While a draft
/// is being written, the same button becomes Stop.
pub(crate) fn steering_bar(
    ui: &mut Ui,
    steering: &mut String,
    busy: bool,
    streaming: bool,
    hint: &str,
) -> Option<SteeringAction> {
    let mut action: Option<SteeringAction> = None;
    Frame::NONE
        .fill(C::panel())
        .stroke(Stroke::new(1.0_f32, C::line()))
        .corner_radius(12)
        .inner_margin(Margin::symmetric(14, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(caps("Steering", C::muted()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new("Leave empty and Folio decides.")
                            .size(9.5)
                            .color(C::muted()),
                    );
                });
            });
            let output = TextEdit::multiline(steering)
                .id(Id::new("steering-input"))
                .font(serif_id(13.5))
                .text_color(C::ink())
                .frame(false)
                .desired_width(f32::INFINITY)
                .desired_rows(2)
                .show(ui);
            let ctrl_enter = output.response.has_focus()
                && ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(hint).size(8.5).color(C::muted()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if streaming {
                        let stop = Button::new(
                            RichText::new("Stop")
                                .size(11.5)
                                .strong()
                                .color(C::accent_deep()),
                        )
                        .fill(C::accent_soft())
                        .stroke(Stroke::new(1.0_f32, C::accent_soft()))
                        .corner_radius(8)
                        .min_size(Vec2::new(0.0, 28.0));
                        if ui.add(stop).clicked() {
                            action = Some(SteeringAction::Stop);
                        }
                    } else {
                        let go = Button::new(
                            RichText::new("Continue scene")
                                .size(11.5)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(C::accent())
                        .stroke(Stroke::NONE)
                        .corner_radius(8)
                        .min_size(Vec2::new(0.0, 28.0));
                        if ui.add_enabled(!busy, go).clicked() {
                            action = Some(SteeringAction::Continue);
                        }
                    }
                });
            });
            if ctrl_enter && !busy && !streaming {
                action = Some(SteeringAction::Continue);
            }
        });
    action
}

/// The highlighted block under the chapter text. It shows the draft as it is
/// written and carries its own controls; the chapter itself never changes
/// until Accept.
pub(crate) fn draft_block(
    ui: &mut Ui,
    draft: &Draft,
    streaming: bool,
    error: Option<&str>,
    follow: bool,
    reveal: bool,
    font_size: f32,
) -> DraftAction {
    let kind = match draft.action.as_str() {
        "rewrite" => "rewrite — replaces the selection",
        "elaborate" => "elaborate — replaces the selection",
        _ => "continue",
    };
    let words = draft.text.split_whitespace().count();
    let mut action = DraftAction::None;
    let frame = Frame::NONE
        .fill(C::draft_bg())
        .stroke(Stroke::new(1.0_f32, C::draft_line()))
        .corner_radius(14)
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(caps("AI draft", C::accent_deep()));
                ui.label(RichText::new("·").size(9.5).color(C::muted()));
                ui.label(RichText::new(kind).size(9.5).color(C::muted()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if streaming {
                        ui.spinner();
                    }
                    ui.label(
                        RichText::new(format!("{words} words"))
                            .size(10.0)
                            .color(C::muted()),
                    );
                });
            });
            ui.add_space(4.0);
            if draft.text.trim().is_empty() {
                ui.label(
                    RichText::new(if streaming {
                        "Writing…"
                    } else {
                        "Nothing came back."
                    })
                    .font(serif_id(font_size))
                    .color(C::muted()),
                );
            } else {
                ui.label(
                    RichText::new(&draft.text)
                        .font(serif_id(font_size))
                        .color(C::ink()),
                );
            }
            if let Some(error) = error {
                ui.add_space(6.0);
                ui.label(RichText::new(error).size(10.5).color(C::red()));
            }
            if draft.stopped && !streaming {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Stopped early — you can keep what is here.")
                        .size(10.5)
                        .color(C::amber()),
                );
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if streaming {
                    if ghost(ui, "Stop").clicked() {
                        action = DraftAction::Stop;
                    }
                } else {
                    if error.is_none() && !draft.text.trim().is_empty() {
                        if primary(ui, "Accept").clicked() {
                            action = DraftAction::Accept;
                        }
                    }
                    if ghost(ui, "Delete").clicked() {
                        action = DraftAction::Delete;
                    }
                    if ghost(ui, "Retry").clicked() {
                        action = DraftAction::Retry;
                    }
                }
            });
        });
    if follow && (streaming || reveal) {
        ui.scroll_to_rect(frame.response.rect, Some(Align::BOTTOM));
    }
    action
}

/// The summary state as a button: the old traffic-light dot plus "Summary…".
/// Clicking opens the window; the tooltip names the state in words.
pub(crate) fn summary_button(ui: &mut Ui, dot: Color32, state: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(
        "Summary…".to_string(),
        FontId::proportional(10.5),
        C::ink_soft(),
    );
    let pad = 8.0;
    let size = Vec2::new(pad * 2.0 + 12.0 + galley.size().x, 20.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let background = if response.hovered() {
        C::line()
    } else {
        C::soft()
    };
    ui.painter().rect_filled(rect, 6, background);
    ui.painter().circle_filled(
        Pos2::new(rect.left() + pad + 3.0, rect.center().y),
        2.6,
        dot,
    );
    ui.painter().galley(
        Pos2::new(
            rect.left() + pad + 12.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        C::ink_soft(),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.on_hover_text(format!("Open the chapter summary — {state}"))
}

pub(crate) fn caps(text: &str, color: Color32) -> RichText {
    RichText::new(text.to_uppercase())
        .size(9.5)
        .strong()
        .color(color)
}

/// Decide the lamp from the two answers that came back. A model named by the
/// endpoint is what turns it green; a merely reachable server that stays quiet
/// about its models is not enough.
pub(crate) fn judge_probe(
    test: (bool, String),
    listed: &[String],
    models_error: Option<&str>,
    model: &str,
    context: usize,
) -> Probe {
    match test {
        (false, error) => Probe::Offline(error),
        (true, note) => {
            let wanted = model.trim();
            let named = if listed.iter().any(|name| name == wanted) {
                wanted.to_string()
            } else {
                listed.first().cloned().unwrap_or_default()
            };
            if named.is_empty() || models_error.is_some() {
                Probe::Offline(models_error.map(str::to_string).unwrap_or_else(|| {
                    "The endpoint answered, but named no model. Check the model name in Settings."
                        .to_string()
                }))
            } else {
                Probe::Online {
                    model: named,
                    context: (context > 0).then_some(context),
                    note,
                }
            }
        }
    }
}

/// The endpoint lamp, shaped like the "Saved" dot: one dot and one word, with
/// the whole story on hover.
pub(crate) fn probe_chip(ui: &mut Ui, probe: &Probe, base: &str, demo: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(
        probe.label().to_string(),
        FontId::proportional(10.5),
        C::muted(),
    );
    let dot = 7.0;
    let gap = 5.0;
    let height = galley.size().y.max(16.0);
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(dot + gap + galley.size().x, height),
        Sense::click(),
    );
    let center_y = rect.center().y;
    ui.painter().circle_filled(
        Pos2::new(rect.left() + dot / 2.0, center_y),
        3.0,
        probe.color(),
    );
    ui.painter().galley(
        Pos2::new(rect.left() + dot + gap, center_y - galley.size().y / 2.0),
        galley,
        C::muted(),
    );
    response
        .on_hover_text(probe.hint(base, demo))
        .on_hover_cursor(CursorIcon::PointingHand)
}

pub(crate) fn top_action(ui: &mut Ui, label: &str, active: bool) -> Response {
    ui.add(
        Button::new(RichText::new(label).size(11.5).color(if active {
            C::accent_deep()
        } else {
            C::ink_soft()
        }))
        .fill(if active {
            C::accent_soft()
        } else {
            Color32::TRANSPARENT
        })
        .stroke(if active {
            Stroke::new(1.0_f32, C::accent_soft())
        } else {
            Stroke::new(1.0_f32, C::line())
        })
        .corner_radius(8)
        .min_size(Vec2::new(0.0, 28.0)),
    )
}

pub(crate) fn sidebar_action(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        Button::new(RichText::new(label).size(11.5).color(C::sidebar_muted()))
            .frame(false)
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE),
    )
}

pub(crate) fn ghost(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        Button::new(RichText::new(label).size(10.5).color(C::muted()))
            .frame(false)
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE),
    )
}

pub(crate) fn menu_item(ui: &mut Ui, label: &str, danger: bool) -> Response {
    let width = ui.available_width().max(140.0);
    let response = ui.add_sized(
        [width, 26.0],
        Button::new(RichText::new(label).size(11.5).color(if danger {
            C::red()
        } else {
            C::ink()
        }))
        .frame(false)
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .corner_radius(6),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

pub(crate) fn primary(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        Button::new(
            RichText::new(label)
                .size(12.0)
                .strong()
                .color(Color32::WHITE),
        )
        .fill(C::accent())
        .stroke(Stroke::NONE)
        .corner_radius(8)
        .min_size(Vec2::new(0.0, 32.0)),
    )
}

pub(crate) fn pill(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) -> Response {
    Frame::NONE
        .fill(bg)
        .corner_radius(6)
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(8.5).strong().color(fg));
        })
        .response
}

pub(crate) fn stat(ui: &mut Ui, value: usize, label: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(
            RichText::new(format!("{value}"))
                .font(serif_id(19.0))
                .color(C::ink()),
        );
        ui.label(RichText::new(label).size(8.5).color(C::muted()));
    });
    ui.add_space(8.0);
}

pub(crate) fn filter_chip(ui: &mut Ui, label: &str, active: bool) -> Response {
    ui.add(
        Button::new(RichText::new(label).size(10.0).color(if active {
            Color32::WHITE
        } else {
            C::ink_soft()
        }))
        .fill(if active { C::accent() } else { C::card() })
        .stroke(if active {
            Stroke::NONE
        } else {
            Stroke::new(1.0_f32, C::line())
        })
        .corner_radius(6)
        .min_size(Vec2::new(0.0, 24.0)),
    )
}

pub(crate) fn preset(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        Button::new(RichText::new(label).size(10.0).color(C::ink_soft()))
            .fill(C::soft())
            .stroke(Stroke::new(1.0_f32, C::line()))
            .corner_radius(6)
            .min_size(Vec2::new(0.0, 22.0)),
    )
}

pub(crate) fn field_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(9.5).strong().color(C::muted()));
    ui.add_space(1.0);
}

pub(crate) fn story_card(ui: &mut Ui, title: &str, meta: &str, selected: bool) -> Response {
    let response = Frame::NONE
        .fill(if selected {
            C::sidebar_card()
        } else {
            Color32::TRANSPARENT
        })
        .corner_radius(10)
        .inner_margin(Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(title)
                    .font(serif_id(15.5))
                    .color(C::sidebar_text()),
            );
            ui.label(RichText::new(meta).size(9.5).color(C::sidebar_muted()));
        })
        .response
        .interact(Sense::click());
    if selected {
        let rect = egui::Rect::from_min_size(
            response.rect.left_top() + Vec2::new(0.0, 4.0),
            Vec2::new(3.0, response.rect.height() - 8.0),
        );
        ui.painter()
            .rect_filled(rect, CornerRadius::same(2), C::accent());
    }
    response
}

pub(crate) fn chapter_row(
    ui: &mut Ui,
    index: usize,
    title: &str,
    words: usize,
    state: &'static str,
    selected: bool,
) -> Response {
    let response = Frame::NONE
        .fill(if selected {
            C::sidebar_card()
        } else {
            Color32::TRANSPARENT
        })
        .corner_radius(9)
        .inner_margin(Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (badge, _) = ui.allocate_exact_size(Vec2::splat(21.0), Sense::hover());
                ui.painter().circle_filled(
                    badge.center(),
                    10.0,
                    if selected {
                        C::accent()
                    } else {
                        C::sidebar_hover()
                    },
                );
                ui.painter().text(
                    badge.center(),
                    Align2::CENTER_CENTER,
                    format!("{}", index + 1),
                    FontId::proportional(9.5),
                    if selected {
                        Color32::WHITE
                    } else {
                        C::sidebar_muted()
                    },
                );
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(RichText::new(title).size(12.5).color(C::sidebar_text()));
                    ui.label(
                        RichText::new(format!("{words} words"))
                            .size(8.5)
                            .color(C::sidebar_muted()),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let color = if !state.starts_with("summary") && state != "not summarized" {
                        C::green()
                    } else if state == "not summarized" {
                        C::sidebar_muted()
                    } else if state.contains("needs") {
                        C::amber()
                    } else {
                        C::green()
                    };
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
                    ui.painter().circle_filled(dot.center(), 3.2, color);
                });
            });
        })
        .response
        .interact(Sense::click());
    response.on_hover_text(state)
}

/// One lorebook entry in the right-hand desk. Like the story and chapter rows,
/// the card senses clicks so that a right-click can open its menu. `matched` is
/// the word that pulled the entry into the current chapter, if any.
pub(crate) fn lore_card(ui: &mut Ui, entry: &LoreEntry, matched: Option<&str>) -> Response {
    Frame::NONE
        .fill(C::card())
        .stroke(Stroke::new(1.0_f32, C::line()))
        .corner_radius(11)
        .inner_margin(Margin::symmetric(11, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&entry.name)
                        .font(serif_id(15.5))
                        .color(C::ink()),
                );
                pill(ui, &entry.category, C::ink_soft(), C::soft());
                if entry.pinned {
                    pill(ui, "pinned", C::green(), C::green_soft());
                }
                if let Some(term) = matched {
                    pill(ui, "in chapter", C::green(), C::green_soft())
                        .on_hover_text(format!("“{term}” appears in this chapter"));
                }
            });
            if !entry.aliases.trim().is_empty() {
                ui.label(
                    RichText::new(format!("also {}", entry.aliases.trim()))
                        .size(9.5)
                        .italics()
                        .color(C::muted()),
                );
            }
            if !entry.triggers.trim().is_empty() {
                ui.label(
                    RichText::new(format!("triggers: {}", entry.triggers.trim()))
                        .size(9.5)
                        .color(C::hint()),
                );
            }
            ui.label(
                RichText::new(&entry.description)
                    .size(11.5)
                    .color(C::ink_soft())
                    .line_height(Some(16.0)),
            );
        })
        .response
        // A bare Frame senses hovering only, and egui reports a right-click just
        // for widgets that sense clicks. Without this the menu could never open.
        .interact(Sense::click())
}

pub(crate) fn avatar(ui: &mut Ui, letter: &str, user: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
    ui.painter().circle_filled(
        rect.center(),
        9.0,
        if user { C::accent() } else { C::green_deep() },
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        letter,
        FontId::proportional(8.5),
        Color32::WHITE,
    );
}

pub(crate) fn intro_bubble(ui: &mut Ui) {
    Frame::NONE
        .fill(C::green_soft())
        .corner_radius(11)
        .inner_margin(Margin::symmetric(11, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                avatar(ui, "F", false);
                ui.label(RichText::new("FOLIO").size(8.5).strong().color(C::green()));
            });
            ui.add_space(1.0);
            ui.add(
                Label::new(
                    RichText::new(
                        "I can see this story's premise, its lorebook, a condensed recap of everything before and the summaries of the last few chapters. Ask me what could happen next, what a character wants, or which thread to pull. Your prose draft stays out of this conversation.",
                    )
                    .font(serif_id(13.5))
                    .color(C::ink_soft())
                    .line_height(Some(19.0)),
                )
                .wrap(),
            );
        });
}

pub(crate) fn plural(count: usize, singular: &'static str, plural: &'static str) -> String {
    format!("{count} {}", if count == 1 { singular } else { plural })
}
