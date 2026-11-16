//! PDF and per-chapter text export.

use super::*;

impl FolioApp {
    pub(crate) fn open_export(&mut self) {
        self.export_status = None;
        self.export_written = None;
        self.export_existing = None;
        if self.export_dir.trim().is_empty() {
            self.export_dir = crate::model::default_export_dir().display().to_string();
        }
        // Start from the name used last time for this story, else its title.
        let story_id = self
            .library
            .stories
            .get(self.active_story)
            .map(|story| story.id);
        self.export_name = story_id
            .and_then(|id| self.export_names.get(&id).cloned())
            .unwrap_or_else(|| {
                self.library
                    .stories
                    .get(self.active_story)
                    .map(|story| story.title.clone())
                    .unwrap_or_default()
            });
        self.export_open = true;
    }

    /// The name the files are written under: what the author typed, or the
    /// story's title when the field is empty.
    pub(crate) fn export_file_stem(&self) -> String {
        let typed = self.export_name.trim();
        if typed.is_empty() {
            self.library
                .stories
                .get(self.active_story)
                .map(|story| story.title.trim().to_string())
                .unwrap_or_default()
        } else {
            typed.to_string()
        }
    }

    /// Press Export: write at once, or ask first when files would be replaced.
    pub(crate) fn start_export(&mut self) {
        if self.shot.is_some() {
            self.notify("Export is off in preview mode.", false);
            return;
        }
        let Some(story) = self.library.stories.get(self.active_story) else {
            return;
        };
        let dir = PathBuf::from(self.export_dir.trim());
        if dir.as_os_str().is_empty() {
            self.export_status = Some((false, "Choose a folder first.".to_string()));
            return;
        }
        let name = self.export_file_stem();
        let targets = export_targets(story, &name, &dir, self.export_kind);
        let existing = existing_names(&targets);
        if existing.is_empty() {
            self.export_story();
        } else {
            self.export_existing = Some(existing);
        }
    }

    /// Write the story, either as one PDF or as one text file per chapter.
    pub(crate) fn export_story(&mut self) {
        self.export_existing = None;
        if self.shot.is_some() {
            self.notify("Export is off in preview mode.", false);
            return;
        }
        let Some(story) = self.library.stories.get(self.active_story).cloned() else {
            return;
        };
        let dir = PathBuf::from(self.export_dir.trim());
        if dir.as_os_str().is_empty() {
            self.export_status = Some((false, "Choose a folder first.".to_string()));
            return;
        }
        let name = self.export_file_stem();
        let kind = self.export_kind;
        let result = match kind {
            ExportKind::Pdf => crate::model::export_pdf(&story, &name, &dir).map(|path| vec![path]),
            ExportKind::Chapters => crate::model::export_chapters(&story, &name, &dir),
        };
        match result {
            Ok(files) => {
                self.export_written = Some(dir.clone());
                self.export_names.insert(story.id, name.clone());
                let message = match kind {
                    ExportKind::Pdf => format!(
                        "Wrote {} to {}",
                        files
                            .first()
                            .and_then(|path| path.file_name())
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        dir.display()
                    ),
                    ExportKind::Chapters => format!(
                        "Wrote {} to {}",
                        plural(files.len(), "chapter file", "chapter files"),
                        dir.display()
                    ),
                };
                self.export_status = Some((true, message));
            }
            Err(error) => self.export_status = Some((false, error)),
        }
        // Remember the choice, so the next export starts where this one ended.
        let dir_text = dir.display().to_string();
        let wants_pdf = kind == ExportKind::Pdf;
        if self.library.export_dir != dir_text || self.library.export_pdf != wants_pdf {
            self.library.export_dir = dir_text;
            self.library.export_pdf = wants_pdf;
            self.mark_dirty();
        }
    }
}

/// The files an export would write for the chosen format.
pub(crate) fn export_targets(
    story: &Story,
    name: &str,
    dir: &Path,
    kind: ExportKind,
) -> Vec<PathBuf> {
    match kind {
        ExportKind::Pdf => vec![crate::model::pdf_target(name, dir)],
        ExportKind::Chapters => crate::model::chapter_targets(name, story, dir),
    }
}

/// The names of those targets that are already on disk.
pub(crate) fn existing_names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| path.is_file())
        .map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .collect()
}
