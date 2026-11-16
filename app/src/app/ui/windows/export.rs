//! The export window.

use super::*;

impl FolioApp {
    pub(crate) fn export_window(&mut self, ctx: &egui::Context) {
        if !self.export_open {
            return;
        }
        let Some(story) = self.library.stories.get(self.active_story).cloned() else {
            self.export_open = false;
            return;
        };
        let title = story.title.clone();
        let chapters = story.chapters.len();
        let words = story.words();

        let mut open = self.export_open;
        let mut kind = self.export_kind;
        let mut dir = self.export_dir.clone();
        let mut name = self.export_name.clone();
        let mut choose = false;
        let mut run = false;
        let mut replace = false;
        let mut cancel_overwrite = false;
        let mut close = false;
        let mut reveal = false;
        let status = self.export_status.clone();
        let written = self.export_written.clone();
        let existing = self.export_existing.clone();

        Window::new("Export story")
            .id(Id::new("export-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(540.0)
            .min_width(520.0)
            .max_width(560.0)
            .show(ctx, |ui| {
                ui.label(caps(
                    &format!("EXPORT · {} · {words} WORDS", plural(chapters, "CHAPTER", "CHAPTERS")),
                    C::muted(),
                ));
                ui.add_space(2.0);
                ui.label(RichText::new(&title).font(serif_id(21.0)).color(C::ink()));
                ui.add_space(10.0);

                field_label(ui, "What to write");
                if export_choice(
                    ui,
                    kind == ExportKind::Pdf,
                    "One PDF for the whole book",
                    "A title page, a contents page, then every chapter — with a bookmark for each one.",
                )
                .clicked()
                {
                    kind = ExportKind::Pdf;
                }
                ui.add_space(4.0);
                if export_choice(
                    ui,
                    kind == ExportKind::Chapters,
                    "One text file per chapter",
                    "Plain .txt, one file per chapter: name_01_chapter.txt",
                )
                .clicked()
                {
                    kind = ExportKind::Chapters;
                }
                ui.add_space(10.0);

                field_label(ui, "File name");
                ui.add(
                    TextEdit::singleline(&mut name)
                        .font(FontId::proportional(12.0))
                        .desired_width(ui.available_width())
                );
                let stem = if name.trim().is_empty() {
                    story.title.trim().to_string()
                } else {
                    name.trim().to_string()
                };
                let preview = match kind {
                    ExportKind::Pdf => crate::model::pdf_file_name(&stem),
                    ExportKind::Chapters if chapters > 0 => {
                        crate::model::chapter_file_name(&stem, &story, 0)
                    }
                    ExportKind::Chapters => String::new(),
                };
                ui.label(
                    RichText::new(match kind {
                        ExportKind::Pdf => format!("Will write: {preview}"),
                        ExportKind::Chapters => {
                            format!("Will write {}: {preview} …", plural(chapters, "file", "files"))
                        }
                    })
                    .size(9.5)
                    .color(C::muted()),
                );
                ui.add_space(10.0);

                field_label(ui, "Where");
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 112.0).max(180.0);
                    ui.add(
                        TextEdit::singleline(&mut dir)
                            .font(FontId::proportional(12.0))
                            .desired_width(width)
                    );
                    if ghost(ui, "Choose folder…").clicked() {
                        choose = true;
                    }
                });
                ui.label(
                    RichText::new("If a file is already there, Folio asks before replacing it.")
                        .size(9.5)
                        .color(C::muted()),
                );
                if let Some(existing) = &existing {
                    ui.add_space(6.0);
                    Frame::NONE
                        .fill(C::amber_soft())
                        .stroke(Stroke::new(1.0_f32, C::amber()))
                        .corner_radius(8)
                        .inner_margin(Margin::symmetric(11, 9))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(if existing.len() == 1 {
                                    "This file is already in the folder:".to_string()
                                } else {
                                    format!("{} of these files are already in the folder:", existing.len())
                                })
                                .size(10.5)
                                .strong()
                                .color(C::ink()),
                            );
                            for file in existing.iter().take(4) {
                                ui.label(RichText::new(file).size(9.5).color(C::ink_soft()));
                            }
                            if existing.len() > 4 {
                                ui.label(
                                    RichText::new(format!("… and {} more", existing.len() - 4))
                                        .size(9.5)
                                        .color(C::muted()),
                                );
                            }
                            ui.label(
                                RichText::new("Replacing overwrites what is there now.")
                                    .size(9.5)
                                    .color(C::muted()),
                            );
                        });
                }
                if let Some((ok, message)) = &status {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.add(
                            Label::new(
                                RichText::new(message)
                                    .size(10.0)
                                    .color(if *ok { C::green() } else { C::red() }),
                            )
                            .wrap(),
                        );
                        if *ok && written.is_some() && ghost(ui, "Open folder").clicked() {
                            reveal = true;
                        }
                    });
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if existing.is_some() {
                        if primary(ui, "Replace files").clicked() {
                            replace = true;
                        }
                        if ghost(ui, "Cancel").clicked() {
                            cancel_overwrite = true;
                        }
                        ui.label(
                            RichText::new("Nothing has been written yet.")
                                .size(9.0)
                                .italics()
                                .color(C::muted()),
                        );
                    } else {
                        if primary(ui, if status.is_some() { "Export again" } else { "Export" }).clicked() {
                            run = true;
                        }
                        if ghost(ui, "Close").clicked() {
                            close = true;
                        }
                        ui.label(
                            RichText::new("Written on this computer — nothing is uploaded.")
                                .size(9.0)
                                .italics()
                                .color(C::muted()),
                        );
                    }
                });
                ui.add_space(2.0);
            });

        if close {
            open = false;
        }
        // A changed name, folder or format makes an earlier question stale.
        let changed =
            self.export_kind != kind || self.export_dir != dir || self.export_name != name;
        self.export_kind = kind;
        self.export_dir = dir;
        self.export_name = name;
        self.export_open = open;
        if changed || cancel_overwrite {
            self.export_existing = None;
        }
        if choose {
            let start = PathBuf::from(self.export_dir.trim());
            let start = if start.is_dir() { Some(start) } else { None };
            if let Some(picked) =
                crate::folder::pick_folder("Where should the story go?", start.as_deref())
            {
                self.export_dir = picked.display().to_string();
                self.export_existing = None;
            }
        }
        if run {
            self.start_export();
        }
        if replace {
            self.export_story();
        }
        if reveal {
            if let Some(dir) = &self.export_written {
                crate::folder::open_folder(dir);
            }
        }
    }
}
