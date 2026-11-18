//! The right side panel: AI chips, brainstorm chat, the lorebook and the
//! full-page views (settings, summary, find, export, prompts, deleted).

use super::*;

/// The heading of a full view in the side panel. Returns true when the close
/// cross was pressed.
fn panel_head(ui: &mut Ui, title: &str) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(13.0).strong().color(C::ink()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ghost(ui, "×").on_hover_text("Close this panel").clicked() {
                close = true;
            }
        });
    });
    ui.separator();
    close
}

impl FolioApp {
    pub(crate) fn ui_right(&mut self, ctx: &egui::Context) {
        let mut close = false;
        let screen = ctx.screen_rect();
        let (min_width, max_width) = desk_width_range(screen.width());
        let wanted = self.desk_width.clamp(min_width, max_width);
        let response = egui::SidePanel::right("desk")
            .resizable(true)
            .default_width(wanted)
            .width_range(min_width..=max_width)
            .frame(
                Frame::NONE
                    .fill(C::panel())
                    .stroke(Stroke::new(1.0_f32, C::line()))
                    .inner_margin(Margin {
                        left: 15,
                        right: 15,
                        top: 14,
                        // Matches the editor's bottom margin, so the chat bar
                        // and the steering bar sit on the same line.
                        bottom: 16,
                    }),
            )
            .show(ctx, |ui| {
                let tab = self.right_tab;
                if panel_head(ui, tab.title()) {
                    close = true;
                }
                ui.add_space(6.0);
                match tab {
                    Tab::Ai => self.ui_ai_tab(ui),
                    Tab::Lore => self.ui_lore_tab(ui),
                    Tab::Settings => self.ui_settings(ui),
                    Tab::Prompts => self.ui_prompts(ui),
                    Tab::Summary => self.ui_summary(ui),
                    Tab::Find => self.ui_find(ui),
                    Tab::Export => self.ui_export(ui),
                    Tab::Trash => self.ui_trash(ui),
                }
            });
        let shown = response.response.rect.width();
        let at_edge = ctx.input(|input| {
            input
                .pointer
                .interact_pos()
                .is_some_and(|pos| (pos.x - response.response.rect.left()).abs() <= 14.0)
        });
        if at_edge && ctx.input(|input| input.pointer.primary_down()) {
            // The author is dragging the edge: remember the width they picked,
            // not the width this window happened to allow.
            self.desk_width = shown.clamp(DESK_MIN_WIDTH, DESK_MAX_WIDTH);
        } else if self.desk_width > shown + 0.5 && max_width < DESK_MAX_WIDTH {
            // The window is too narrow for the chosen width. Keep it for later
            // instead of letting egui overwrite its memory with the clamped one.
            let rect = egui::Rect::from_min_max(
                egui::pos2(
                    response.response.rect.right() - self.desk_width,
                    response.response.rect.top(),
                ),
                response.response.rect.max,
            );
            ctx.data_mut(|data| {
                data.insert_persisted(
                    Id::new("desk"),
                    egui::containers::panel::PanelState { rect },
                )
            });
        }
        if close {
            self.right_open = false;
        }
    }

    pub(crate) fn ui_ai_tab(&mut self, ui: &mut Ui) {
        let Some(story) = self.library.stories.get(self.active_story) else {
            return;
        };
        let Some(chapter) = story.chapters.get(self.active_chapter) else {
            return;
        };
        let messages = chapter.chat.clone();
        let memory = chapter.chat_summary.clone();
        let story_id = story.key.clone();
        let chapter_id = chapter.key.clone();
        let chapter_title = chapter.title.clone();
        let proposal = self.assistant_proposal.clone();
        let proposal_here = proposal
            .as_ref()
            .is_some_and(|offer| offer.story_id == story_id && offer.chapter_id == chapter_id);
        let has_chat = !messages.is_empty() || !memory.trim().is_empty();
        let window = self.library.settings.recent_window();
        let position = self.active_chapter;
        let recent_summaries = story.recent_summaries(position, window).len();
        // Only older chapters with a summary actually reach the recap block.
        let recap_start = story
            .recent_start(position, window)
            .min(story.chapters.len());
        let recap_chapters = story.chapters[..recap_start]
            .iter()
            .filter(|chapter| chapter.is_summarized())
            .count();
        let lore_count = story.lore_for_text(&chapter.body).len();
        let words = chapter.words();
        let demo = self.library.settings.is_demo();
        let model_label = self.library.settings.label();
        let busy = self.job.is_some();
        // A brainstorm answer for this chapter, while it is still being written.
        let live_reply = self.job.as_ref().and_then(|job| match job.kind {
            JobKind::Action(Action::Brainstorm)
                if job.story_id == story_id && job.chapter_id == chapter_id =>
            {
                Some(job.stream.text.clone())
            }
            _ => None,
        });
        let streaming_chat = live_reply.is_some();
        let proofread_here = self.job.as_ref().is_some_and(|job| {
            job.story_id == story_id
                && job.chapter_id == chapter_id
                && matches!(job.kind, JobKind::Proofread { .. })
        });
        let chat_stop = streaming_chat || proofread_here;
        let job_label = match self.job.as_ref().map(|j| &j.kind) {
            Some(JobKind::Summary { .. }) => {
                if self.summary_merging() {
                    "Merging summary…".to_string()
                } else if let Some((part, total)) = self.summary_block_progress() {
                    format!("Summarizing {part}/{total}…")
                } else {
                    match self.summary_progress() {
                        Some((done, total)) if done < total => {
                            format!("Summarizing {}/{total}…", done + 1)
                        }
                        _ => "Summarizing…".to_string(),
                    }
                }
            }
            Some(JobKind::Action(_)) => "Writing…".to_string(),
            Some(JobKind::Lore) => "Reading the selection…".to_string(),
            Some(JobKind::Proofread { .. }) => {
                match self.job.as_ref().and_then(|job| job.stream.progress) {
                    Some((done, total)) => format!("Proofreading {done}/{total}…"),
                    None => "Proofreading…".to_string(),
                }
            }
            None => String::new(),
        };
        let mut do_copy: Option<String> = None;
        let mut want_clear = false;

        ui.horizontal(|ui| {
            ui.label(caps("Assistant", C::muted()));
            let rest = ui.available_width();
            ui.allocate_ui_with_layout(
                Vec2::new(rest, 16.0),
                Layout::right_to_left(Align::Center),
                |ui| {
                    if demo {
                        pill(ui, "Demo writer", C::amber(), C::amber_soft());
                    } else {
                        pill(ui, &truncate(&model_label, 22), C::green(), C::green_soft());
                    }
                    if has_chat && !busy {
                        if ghost(ui, "Clear chat")
                            .on_hover_text(
                                "Remove every message from this chapter's conversation and start fresh",
                            )
                            .clicked()
                        {
                            want_clear = true;
                        }
                    }
                },
            );
        });
        ui.label(
            RichText::new(
                "A working partner with this chapter open. It can read the page, refresh the chapter summary, or prepare lore facts for your approval — its draft stays out of this conversation.",
            )
            .size(9.5)
            .color(C::muted()),
        );
        ui.add_space(6.0);

        Frame::NONE
            .fill(C::card())
            .stroke(Stroke::new(1.0_f32, C::line()))
            .corner_radius(theme::radius(11))
            .inner_margin(Margin::symmetric(12, 9))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    stat(
                        ui,
                        recent_summaries,
                        if recent_summaries == 1 {
                            "recent summary"
                        } else {
                            "recent summaries"
                        },
                    );
                    stat(
                        ui,
                        lore_count,
                        if lore_count == 1 {
                            "lore entry"
                        } else {
                            "lore entries"
                        },
                    );
                    stat(ui, words, "chapter words");
                });
                ui.label(
                    RichText::new(context_caption(recap_chapters, recent_summaries))
                        .size(9.0)
                        .color(C::muted()),
                );
                if self.library.settings.context_limit > 0 {
                    ui.label(
                        RichText::new(conversation_line(
                            chapter.conversation_tokens(),
                            self.library.settings.context_limit,
                        ))
                        .size(9.0)
                        .color(C::muted()),
                    );
                }
            });
        ui.add_space(7.0);

        // The question field sits at the foot of the panel, built like the
        // steering bar under the chapter: same header row, same two-line field,
        // same button row, so both bars line up across the window.
        let mut send = false;
        let mut stop_chat = false;
        let mut proposal_choice: Option<AssistantChoice> = None;
        TopBottomPanel::bottom(Id::new("brainstorm-input"))
            .frame(Frame::NONE)
            .show_inside(ui, |ui| {
                ui.add_space(6.0);
                Frame::NONE
                    .fill(C::card())
                    .stroke(Stroke::new(1.0_f32, C::line()))
                    .corner_radius(theme::radius(12))
                    .inner_margin(Margin::symmetric(14, 9))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(caps("Your question", C::muted()));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new("Ctrl+Enter to send")
                                        .size(9.5)
                                        .color(C::muted()),
                                );
                            });
                        });
                        let output = TextEdit::multiline(&mut self.ai_input)
                            .id(Id::new("ai-input"))
                            .font(serif_id(13.5))
                            .text_color(C::ink())
                            .frame(false)
                            .desired_width(f32::INFINITY)
                            .desired_rows(2)
                            .show(ui);
                        if output.response.has_focus()
                            && !busy
                            && ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter))
                        {
                            send = true;
                        }
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if demo {
                                ui.label(RichText::new("offline demo").size(8.5).color(C::amber()));
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if chat_stop {
                                    let stop = Button::new(
                                        RichText::new("Stop")
                                            .size(11.5)
                                            .strong()
                                            .color(C::accent_deep()),
                                    )
                                    .fill(C::accent_soft())
                                    .stroke(Stroke::new(1.0_f32, C::accent_soft()))
                                    .corner_radius(theme::radius(8))
                                    .min_size(Vec2::new(62.0, 28.0));
                                    if ui.add(stop).clicked() {
                                        stop_chat = true;
                                    }
                                } else {
                                    let can_send = !busy && !self.ai_input.trim().is_empty();
                                    let button = Button::new(
                                        RichText::new("Send")
                                            .size(11.5)
                                            .color(Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(C::accent())
                                    .stroke(Stroke::NONE)
                                    .corner_radius(theme::radius(8))
                                    .min_size(Vec2::new(62.0, 28.0));
                                    if ui.add_enabled(can_send, button).clicked() {
                                        send = true;
                                    }
                                }
                            });
                        });
                    });
            });

        if stop_chat {
            self.stop_chat(story_id.clone(), chapter_id.clone());
        }
        let user_scrolled =
            ui.input(|i| i.raw_scroll_delta.y.abs() > 0.5 || i.smooth_scroll_delta.y.abs() > 0.5);
        let follow_chat = self.chat_follow && !user_scrolled;
        let stick_chat = self.scroll_chat || ((streaming_chat || proposal_here) && follow_chat);
        self.scroll_chat = false;
        let chat_out = ScrollArea::vertical()
            .id_salt("ai-chat")
            .stick_to_bottom(stick_chat)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                intro_bubble(ui);
                ui.add_space(2.0);
                if !memory.trim().is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "Earlier turns, condensed to fit the context: {}…",
                                truncate(memory.trim(), 52)
                            ))
                            .size(9.5)
                            .italics()
                            .color(C::muted()),
                        )
                        .on_hover_text(memory.trim());
                    });
                    ui.add_space(6.0);
                }
                for message in &messages {
                    if message.role == "note" {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(&message.text)
                                    .size(9.5)
                                    .italics()
                                    .color(C::muted()),
                            );
                        });
                        ui.add_space(2.0);
                        continue;
                    }
                    let is_user = message.role == "user";
                    Frame::NONE
                        .fill(if is_user { C::accent_soft() } else { C::card() })
                        .stroke(if is_user {
                            Stroke::NONE
                        } else {
                            Stroke::new(1.0_f32, C::line())
                        })
                        .corner_radius(theme::radius(11))
                        .inner_margin(Margin::symmetric(11, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                avatar(ui, if is_user { "Y" } else { "F" }, is_user);
                                ui.label(
                                    RichText::new(if is_user { "YOU" } else { "FOLIO" })
                                        .size(8.5)
                                        .strong()
                                        .color(C::muted()),
                                );
                                if message.demo {
                                    pill(ui, "demo answer", C::amber(), C::amber_soft());
                                }
                            });
                            ui.add_space(2.0);
                            ui.add(
                                Label::new(
                                    RichText::new(&message.text)
                                        .font(serif_id(if is_user { 13.5 } else { 14.0 }))
                                        .color(C::ink_soft())
                                        .line_height(Some(19.0)),
                                )
                                .wrap(),
                            );
                            if !is_user {
                                ui.add_space(2.0);
                                ui.horizontal(|ui| {
                                    if ghost(ui, "Copy").clicked() {
                                        do_copy = Some(message.text.clone());
                                    }
                                });
                            }
                        });
                    ui.add_space(4.0);
                }
                if let Some(live) = &live_reply {
                    Frame::NONE
                        .fill(C::card())
                        .stroke(Stroke::new(1.0_f32, C::line()))
                        .corner_radius(theme::radius(11))
                        .inner_margin(Margin::symmetric(11, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                avatar(ui, "F", false);
                                ui.label(
                                    RichText::new("FOLIO").size(8.5).strong().color(C::muted()),
                                );
                                ui.spinner();
                            });
                            ui.add_space(2.0);
                            if live.trim().is_empty() {
                                ui.label(
                                    RichText::new("Writing…")
                                        .font(serif_id(14.0))
                                        .color(C::muted()),
                                );
                            } else {
                                ui.add(
                                    Label::new(
                                        RichText::new(live)
                                            .font(serif_id(14.0))
                                            .color(C::ink_soft())
                                            .line_height(Some(19.0)),
                                    )
                                    .wrap(),
                                );
                            }
                        });
                    ui.add_space(4.0);
                } else if busy {
                    Frame::NONE
                        .fill(C::card())
                        .stroke(Stroke::new(1.0_f32, C::line()))
                        .corner_radius(theme::radius(11))
                        .inner_margin(Margin::symmetric(11, 9))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new(job_label)
                                        .size(11.5)
                                        .italics()
                                        .color(C::muted()),
                                );
                            });
                        });
                }
                if proposal_here {
                    if let Some(proposal) = &proposal {
                        if let Some(choice) = assistant_proposal_card(ui, proposal, &chapter_title)
                        {
                            proposal_choice = Some(choice);
                        }
                    }
                }
            });
        if streaming_chat || proposal_here {
            let at_bottom = chat_out.state.offset.y + chat_out.inner_rect.height()
                >= chat_out.content_size.y - 16.0;
            if user_scrolled || at_bottom {
                self.chat_follow = at_bottom;
            }
        }

        match proposal_choice {
            Some(AssistantChoice::Accept) => self.accept_assistant_proposal(),
            Some(AssistantChoice::Deny) => self.assistant_proposal = None,
            Some(AssistantChoice::Retry) => self.retry_assistant_proposal(),
            None => {}
        }
        if send {
            self.send_brainstorm();
        }
        if want_clear {
            self.confirm = Some(Confirm::Chat {
                story_id,
                chapter_id,
            });
        }
        if let Some(text) = do_copy {
            ui.ctx().copy_text(text);
            self.notify("Copied to the clipboard.", false);
        }
    }

    pub(crate) fn ui_lore_tab(&mut self, ui: &mut Ui) {
        let Some(story) = self.library.stories.get(self.active_story) else {
            return;
        };
        let story_id = story.key.clone();
        let entries = story.lore.clone();
        let body = story
            .chapters
            .get(self.active_chapter)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let mentioned = story.mentioned_lore(&body).len();
        let mut lore_menu: Option<(String, LoreMenu)> = None;
        let mut edit_entry: Option<Option<String>> = None;
        let filter = self.lore_filter.clone();

        ui.horizontal(|ui| {
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(caps("Lorebook", C::muted()));
                ui.label(
                    RichText::new(format!(
                        "{} entries  ·  {} mentioned in this chapter",
                        entries.len(),
                        mentioned
                    ))
                    .size(9.5)
                    .color(C::muted()),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if top_action(ui, "+ Entry", false).clicked() {
                    edit_entry = Some(None);
                }
            });
        });
        ui.add_space(6.0);
        ui.add(
            TextEdit::singleline(&mut self.lore_search)
                .id(Id::new("lore-search"))
                .font(FontId::proportional(12.5))
                .frame(true)
                .desired_width(f32::INFINITY),
        );
        ui.add_space(5.0);
        ui.horizontal_wrapped(|ui| {
            for category in ["All", "Character", "Place", "Faction", "Item", "Lore"] {
                let active = filter == category;
                if filter_chip(ui, category, active).clicked() {
                    self.lore_filter = category.to_string();
                }
            }
        });
        ui.add_space(2.0);
        ui.label(
            RichText::new("Right-click an entry to edit, pin or delete it.")
                .size(9.5)
                .color(C::hint()),
        );
        ui.add_space(4.0);

        let search = self.lore_search.trim().to_lowercase();
        let visible: Vec<&LoreEntry> = entries
            .iter()
            .filter(|entry| {
                (filter == "All" || entry.category == filter)
                    && (search.is_empty()
                        || entry.name.to_lowercase().contains(&search)
                        || entry.aliases.to_lowercase().contains(&search)
                        || entry.triggers.to_lowercase().contains(&search)
                        || entry.description.to_lowercase().contains(&search))
            })
            .collect();

        if visible.is_empty() {
            Frame::NONE
                .fill(C::card())
                .stroke(Stroke::new(1.0_f32, C::line()))
                .corner_radius(theme::radius(11))
                .inner_margin(Margin::symmetric(13, 15))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(if entries.is_empty() {
                            "No lore yet. Add the characters, places and rules the AI should remember."
                        } else {
                            "Nothing matches that search."
                        })
                        .size(12.0)
                        .italics()
                        .color(C::muted()),
                    );
                });
        }

        let lower_body = body.to_lowercase();
        ScrollArea::vertical()
            .id_salt("lore-list")
            .max_height((ui.available_height() - 8.0).max(120.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                for entry in visible {
                    let matched = entry.matched_term(&lower_body);
                    let response = lore_card(ui, entry, matched.as_deref());
                    let mut picked: Option<LoreMenu> = None;
                    response.context_menu(|ui| {
                        ui.set_min_width(176.0);
                        ui.label(caps("Lore entry", C::muted()));
                        ui.add_space(2.0);
                        if menu_item(ui, "Edit", false).clicked() {
                            picked = Some(LoreMenu::Edit);
                            ui.close();
                        }
                        let pin_label = if entry.pinned { "Unpin" } else { "Pin" };
                        if menu_item(ui, pin_label, false).clicked() {
                            picked = Some(LoreMenu::TogglePin);
                            ui.close();
                        }
                        ui.separator();
                        if menu_item(ui, "Delete", true).clicked() {
                            picked = Some(LoreMenu::Delete);
                            ui.close();
                        }
                    });
                    if picked.is_some() {
                        lore_menu = picked.map(|action| (entry.key.clone(), action));
                    }
                    ui.add_space(4.0);
                }
            });

        if let Some((entry_id, action)) = lore_menu {
            match action {
                LoreMenu::Edit => self.edit_lore(Some(entry_id)),
                LoreMenu::TogglePin => {
                    if let Some(entry) = self
                        .library
                        .story_mut(&story_id)
                        .and_then(|story| story.lore.iter_mut().find(|e| e.key == entry_id))
                    {
                        entry.pinned = !entry.pinned;
                    }
                    self.mark_dirty();
                }
                LoreMenu::Delete => self.ask_delete_lore(story_id, entry_id),
            }
        }
        if let Some(entry) = edit_entry {
            self.edit_lore(entry);
        }
    }

    // ---------------------------------------------------------------- dialogs
}
