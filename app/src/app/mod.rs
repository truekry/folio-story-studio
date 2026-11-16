//! The Folio window: library, chapter desk, AI partner, lorebook and dialogs.
//!
//! The window used to be one large file. It is split by task:
//!
//! - [`setup`]: startup, saving, switching stories and chapters, shortcuts
//! - [`jobs`]: the one running AI job, its stream and the endpoint checks
//! - [`drafts`]: prose drafts, proofreading, brainstorm chat and lore proposals
//! - [`summaries`]: chapter summaries and the summary window
//! - [`stories`]: creating, editing and deleting stories, chapters, lore; trash
//! - [`export`]: PDF and per-chapter text export
//! - [`replace`]: find and replace across a chapter or the whole book
//! - [`text`]: pure text helpers (ranges, paragraphs, draft application)
//! - [`widgets`]: shared buttons, cards and chips
//! - [`copy`]: the sentences that explain what the AI sees
//! - [`ui`]: top bar, library, editor, right desk and the dialogs
//!
//! The shared state (`FolioApp`, the small state enums, the job plumbing types)
//! stays in this file; every submodule adds an `impl FolioApp` block with the
//! methods it owns and pulls the shared names in with `use super::*;`.

use crate::ai::{self, Action, StreamEvent};
use crate::model::{
    AiSettings, Chapter, ChatMessage, Draft, Library, LoreEntry, PromptSettings, Story,
};
use crate::theme::{self, serif_id, C};
use eframe::egui::{
    self, Align, Align2, Button, CentralPanel, Color32, CornerRadius, CursorIcon, FontId, Frame,
    Id, Key, Label, Layout, Margin, Modal, PointerButton, Pos2, Response, RichText, ScrollArea,
    Sense, Stroke, TextEdit, TextFormat, TopBottomPanel, Ui, Vec2, ViewportCommand, Window,
};
use std::collections::{HashMap, VecDeque};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod copy;
mod drafts;
mod export;
mod jobs;
mod replace;
mod setup;
mod stories;
mod summaries;
mod text;
mod ui;
mod widgets;

#[cfg(test)]
mod tests;

// The free functions each module owns are re-exported here, so every other
// module can call them without repeating a long import list.
pub(crate) use copy::*;
pub(crate) use export::*;
pub(crate) use jobs::*;
pub(crate) use replace::*;
pub(crate) use text::*;
pub(crate) use widgets::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Ai,
    Lore,
}

/// What the steering bar's button asks for: start a continuation, or stop the
/// draft that is being written right now.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SteeringAction {
    Continue,
    Stop,
}

/// What the author chose inside a draft block.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DraftAction {
    None,
    Accept,
    Delete,
    Retry,
    Stop,
}

#[derive(Clone)]
enum JobKind {
    Action(Action),
    Summary {
        story_id: u64,
        chapter_id: u64,
        /// The chapter text the summary was written from. It is recorded with
        /// the answer even when the author edits the chapter while it runs, so
        /// a changed body leaves the summary marked as needing a refresh.
        base_len: usize,
        base_hash: u64,
    },
    Lore,
    /// A paragraph-by-paragraph spelling and grammar pass.
    Proofread {
        base_hash: u64,
    },
}

/// What the author picked from the right-click menu on a selection.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SelectionAction {
    Rewrite,
    Elaborate,
    Lore,
}

/// One AI suggestion for the lorebook, waiting for the author's yes. For an
/// update, `text` holds only the new facts; `target` names the entry.
struct LoreProposal {
    story_id: u64,
    target: Option<u64>,
    name: String,
    category: String,
    aliases: String,
    triggers: String,
    text: String,
    pinned: bool,
    well_formed: bool,
}

/// The text of a running job, folded out of the event stream. Keeping it here
/// lets `poll_job` drain every pending delta in one frame instead of waking the
/// window for each token.
#[derive(Default)]
struct StreamFold {
    text: String,
    error: Option<String>,
    done: bool,
    /// How far a multi-step job (the spelling check) has got.
    progress: Option<(usize, usize)>,
}

impl StreamFold {
    /// Folds one event into the buffer. Returns true once the job is over.
    fn fold(&mut self, event: StreamEvent) -> bool {
        match event {
            StreamEvent::Delta(delta) => self.text.push_str(&delta),
            StreamEvent::Progress { done, total } => self.progress = Some((done, total)),
            StreamEvent::Done { .. } => self.done = true,
            StreamEvent::Failed(error) => {
                self.error = Some(error);
                self.done = true;
            }
        }
        self.done
    }
}

struct Job {
    kind: JobKind,
    story_id: u64,
    chapter_id: u64,
    stream: StreamFold,
    rx: Receiver<StreamEvent>,
    cancel: Arc<AtomicBool>,
    _started: Instant,
}

impl Drop for Job {
    fn drop(&mut self) {
        // Dropping the receiver alone would leave the worker writing into a
        // closed channel until the reply is done. The flag stops it at the next
        // frame, so deleting a chapter does not leave a request running.
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// A draft that is still growing under the open chapter. Once the job is done,
/// it moves into `chapter.draft`; until then it is kept here so the chapter
/// text never changes while the model writes.
struct LiveDraft {
    story_id: u64,
    chapter_id: u64,
    draft: Draft,
}

struct Toast {
    text: String,
    error: bool,
    born: Instant,
    /// The change this notice can take back, while the notice is up.
    undo: Option<UndoAction>,
}

struct Shot {
    path: PathBuf,
    frames: u32,
    requested: bool,
}

/// One press on Summarize: the open chapter plus every older chapter that was
/// reaching the AI without a usable summary.
struct SummaryBatch {
    total: usize,
    asked_title: String,
}

/// The little lamp in the top bar: can Folio reach its writing partner?
///
/// It mirrors the Settings window's "Test connection" run, so there is only
/// one answer in the app. Green needs an answer that actually names a model;
/// a server that is merely listening is not enough.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Probe {
    /// Nothing checked yet — or the endpoint address changed since the answer.
    Idle,
    /// A check is on its way.
    Checking,
    /// The endpoint answered and listed a model.
    Online {
        model: String,
        context: Option<usize>,
        /// The sentence the Settings window shows, e.g. "Connected (HTTP 200)."
        note: String,
    },
    /// No answer, or an answer without a model.
    Offline(String),
}

impl Probe {
    fn is_checking(&self) -> bool {
        matches!(self, Probe::Checking)
    }

    fn label(&self) -> &'static str {
        match self {
            Probe::Idle => "Not checked",
            Probe::Checking => "Checking",
            Probe::Online { .. } => "Online",
            Probe::Offline(_) => "Offline",
        }
    }

    fn color(&self) -> Color32 {
        match self {
            Probe::Idle => C::muted(),
            Probe::Checking => C::amber(),
            Probe::Online { .. } => C::green(),
            Probe::Offline(_) => C::red(),
        }
    }

    /// What the dot means, in words, on hover.
    fn hint(&self, base: &str, demo: bool) -> String {
        match self {
            Probe::Idle if demo => {
                "The demo writer is active, so there is no endpoint to check.\nClick to open settings."
                    .to_string()
            }
            Probe::Idle => format!("{base}\nNot checked yet — click to test the endpoint."),
            Probe::Checking => "Asking the endpoint for its models…".to_string(),
            Probe::Online {
                model,
                context,
                note,
            } => {
                let window = match context {
                    Some(tokens) => format!("{tokens} tokens of context"),
                    None => "no context limit reported".to_string(),
                };
                let name = if model.is_empty() {
                    "unnamed model".to_string()
                } else {
                    model.clone()
                };
                format!("Endpoint online\n{base}\n{name} — {window}\n{note}\nClick to check again.")
            }
            Probe::Offline(error) => {
                format!("Endpoint offline\n{base}\n{error}\nClick to try again.")
            }
        }
    }
}

/// Chapter bodies from before a book-wide replacement, so it can be undone.
#[derive(Clone)]
pub(crate) struct ReplaceUndo {
    story_id: u64,
    chapters: Vec<(u64, String)>,
}

#[derive(Clone)]
enum UndoAction {
    Delete(u64),
    Replace(ReplaceUndo),
}

struct LoreDraft {
    story_id: u64,
    entry_id: Option<u64>,
    name: String,
    category: String,
    aliases: String,
    triggers: String,
    description: String,
    pinned: bool,
}

struct ChapterDraft {
    story_id: u64,
    chapter_id: u64,
    title: String,
    summary: String,
}

/// A destructive action waiting for the person's confirmation.
#[derive(Clone)]
enum Confirm {
    Story {
        story_id: u64,
        title: String,
        chapters: usize,
        lore: usize,
    },
    Chapter {
        story_id: u64,
        chapter_id: u64,
        story_title: String,
        title: String,
        number: usize,
        words: usize,
        summarized: bool,
    },
    Lore {
        story_id: u64,
        entry_id: u64,
        story_title: String,
        name: String,
        category: String,
        description: String,
        pinned: bool,
    },
    Chat {
        story_id: u64,
        chapter_id: u64,
    },
    Replace {
        query: String,
        replacement: String,
        match_case: bool,
        matches: usize,
        chapters: usize,
        story_title: String,
        /// Some when only the open chapter is replaced; empty for the whole book.
        chapter_id: Option<u64>,
        chapter_title: String,
    },
}

/// Something removed while this window is open. It lives only in memory, so
/// closing Folio really forgets it; restoring puts it back where it was.
enum TrashItem {
    Story {
        id: u64,
        index: usize,
        story: Story,
        deleted_at: Instant,
    },
    Chapter {
        id: u64,
        story_id: u64,
        story_title: String,
        index: usize,
        chapter: Chapter,
        deleted_at: Instant,
    },
    Lore {
        id: u64,
        story_id: u64,
        story_title: String,
        index: usize,
        entry: LoreEntry,
        deleted_at: Instant,
    },
}

impl TrashItem {
    fn id(&self) -> u64 {
        match self {
            TrashItem::Story { id, .. }
            | TrashItem::Chapter { id, .. }
            | TrashItem::Lore { id, .. } => *id,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            TrashItem::Story { .. } => "Story",
            TrashItem::Chapter { .. } => "Chapter",
            TrashItem::Lore { .. } => "Lore entry",
        }
    }

    fn title(&self) -> &str {
        match self {
            TrashItem::Story { story, .. } => &story.title,
            TrashItem::Chapter { chapter, .. } => &chapter.title,
            TrashItem::Lore { entry, .. } => &entry.name,
        }
    }

    fn context(&self) -> String {
        match self {
            TrashItem::Story { story, .. } => format!(
                "{}  ·  {}",
                plural(story.chapters.len(), "chapter", "chapters"),
                plural(story.lore.len(), "lore entry", "lore entries")
            ),
            TrashItem::Chapter { story_title, .. } => format!("from “{story_title}”"),
            TrashItem::Lore {
                entry, story_title, ..
            } => format!("{}  ·  from “{story_title}”", entry.category),
        }
    }

    fn story_id(&self) -> Option<u64> {
        match self {
            TrashItem::Story { .. } => None,
            TrashItem::Chapter { story_id, .. } | TrashItem::Lore { story_id, .. } => {
                Some(*story_id)
            }
        }
    }

    fn deleted_at(&self) -> Instant {
        match self {
            TrashItem::Story { deleted_at, .. }
            | TrashItem::Chapter { deleted_at, .. }
            | TrashItem::Lore { deleted_at, .. } => *deleted_at,
        }
    }
}

#[derive(Clone, Copy)]
enum StoryMenu {
    Edit,
    Delete,
}

#[derive(Clone, Copy)]
enum ChapterMenu {
    Edit,
    Delete,
}

#[derive(Clone, Copy)]
enum LoreMenu {
    Edit,
    TogglePin,
    Delete,
}

/// What an export writes: the whole story as one PDF, or one text file per
/// chapter.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ExportKind {
    Pdf,
    Chapters,
}

pub struct FolioApp {
    library: Library,
    store_path: Option<PathBuf>,
    shot: Option<Shot>,
    active_story: usize,
    active_chapter: usize,
    right_open: bool,
    right_tab: Tab,
    settings_open: bool,
    show_key: bool,
    new_story_open: bool,
    new_story_title: String,
    new_story_premise: String,
    new_chapter_open: bool,
    new_chapter_title: String,
    story_editor_open: bool,
    story_editor_story_id: Option<u64>,
    story_title_draft: String,
    story_premise_draft: String,
    chapter_editor: Option<ChapterDraft>,
    confirm: Option<Confirm>,
    trash: Vec<TrashItem>,
    trash_next_id: u64,
    trash_open: bool,
    lore_editor: Option<LoreDraft>,
    /// An AI lorebook suggestion waiting in its review window.
    lore_proposal: Option<LoreProposal>,
    lore_search: String,
    /// Installed fonts for the type bar, read once at startup.
    system_fonts: Vec<crate::fonts::SystemFont>,
    lore_filter: String,
    ai_input: String,
    job: Option<Job>,
    /// Chapters still waiting for a summary inside the current run, oldest first.
    summary_queue: VecDeque<(u64, u64)>,
    summary_batch: Option<SummaryBatch>,
    /// Model names the endpoint listed the last time it was asked, and the
    /// address they were listed for.
    models: Vec<String>,
    models_from: String,
    models_rx: Option<Receiver<Result<ai::EndpointModels, String>>>,
    models_error: Option<String>,
    test_rx: Option<Receiver<Result<String, String>>>,
    test_status: Option<(bool, String)>,
    /// The prompt editor, opened from the settings.
    prompts_open: bool,
    /// The endpoint lamp, kept in step with `test_status` and the model list.
    probe: Probe,
    /// The address `probe` speaks about, so a changed endpoint cannot leave a
    /// stale green light behind.
    probe_from: String,
    toast: Option<Toast>,
    dirty: bool,
    last_edit: Instant,
    summary_open: bool,
    find_open: bool,
    find_query: String,
    find_replacement: String,
    find_match_case: bool,
    /// True shows the replacement field and "Replace all"; false is plain find.
    find_replace: bool,
    /// Which match "Find next" last jumped to, if any.
    find_index: Option<usize>,
    find_focus: bool,
    pending_selection: Option<(u64, Range<usize>)>,
    editor_selection: Option<Range<usize>>,
    /// The prose draft currently being written, if any.
    draft_live: Option<LiveDraft>,
    /// Why the last live draft failed; shown inside its block.
    draft_error: Option<String>,
    /// Whether the draft block should keep scrolling into view as it grows.
    draft_follow: bool,
    /// Scroll to the draft once, when it first appears.
    draft_reveal: bool,
    scroll_chat: bool,
    /// Follow the running brainstorm answer unless the author scrolls up.
    chat_follow: bool,
    export_open: bool,
    export_kind: ExportKind,
    export_dir: String,
    /// The file name as typed in the dialog.
    export_name: String,
    /// Names already used per story, so reopening the dialog keeps them.
    export_names: HashMap<u64, String>,
    /// Files that would be replaced if the export ran now. While this is set,
    /// the dialog asks before writing anything.
    export_existing: Option<Vec<String>>,
    /// The last export's result: true for success, with the message shown
    /// under the dialog's buttons.
    export_status: Option<(bool, String)>,
    /// The folder that was written to, so it can be opened in Explorer.
    export_written: Option<PathBuf>,
}

impl eframe::App for FolioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Before anything is painted: light or dark, as the system wants it.
        theme::follow_system(ctx);
        self.clamp();
        self.handle_screenshot(ctx);
        self.poll_job(ctx);
        self.poll_test(ctx);
        self.poll_models(ctx);
        self.poll_probe(ctx);
        self.sync_probe();
        self.shortcuts(ctx);
        self.ui_top(ctx);
        self.ui_left(ctx);
        if self.right_open {
            self.ui_right(ctx);
        }
        self.ui_center(ctx);
        self.dialogs(ctx);
        self.ui_toast(ctx);
        self.autosave(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.dirty {
            self.save_now();
        }
    }
}
