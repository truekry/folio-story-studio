//! The worker side: one job at a time, its event stream, and the endpoint checks.

use super::*;

impl FolioApp {
    pub(crate) fn poll_job(&mut self, ctx: &egui::Context) {
        let Some(job) = self.job.as_mut() else {
            return;
        };
        // Events arrive faster than frames. Drain everything that is waiting,
        // so the text is complete even if the window was busy elsewhere.
        loop {
            match job.rx.try_recv() {
                Ok(event) => {
                    if job.stream.fold(event) {
                        break;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // The worker always sends Done or Failed before it exits; a
                    // bare disconnect means it died without saying why.
                    job.stream.error = Some("The AI worker stopped unexpectedly.".to_string());
                    job.stream.done = true;
                    break;
                }
            }
        }
        // The block under the chapter shows the text as it is written, so the
        // author sees the draft grow instead of waiting for the last token.
        if matches!(&job.kind, JobKind::Action(action) if action.is_prose()) {
            if let Some(live) = self.draft_live.as_mut() {
                if live.story_id == job.story_id && live.chapter_id == job.chapter_id {
                    live.draft.text.clone_from(&job.stream.text);
                }
            }
        }
        if !job.stream.done {
            ctx.request_repaint_after(Duration::from_millis(120));
            return;
        }
        let mut job = match self.job.take() {
            Some(job) => job,
            None => return,
        };
        let result = match job.stream.error.take() {
            Some(error) => Err(error),
            None => Ok(std::mem::take(&mut job.stream.text)),
        };
        match result {
            Ok(text) => match job.kind.clone() {
                JobKind::Summary { step } => {
                    // A block updates the rolling summary; the merge pass
                    // stores the final text. The run decides which comes next.
                    self.apply_summary_reply(
                        step,
                        job.story_id.clone(),
                        job.chapter_id.clone(),
                        text,
                    );
                    ctx.request_repaint();
                }
                JobKind::Action(action) => {
                    if action.is_prose() {
                        // The draft leaves the live slot only once, so a second
                        // frame cannot write it twice.
                        let belongs = self.draft_live.as_ref().is_some_and(|live| {
                            live.story_id == job.story_id && live.chapter_id == job.chapter_id
                        });
                        if belongs {
                            if let Some(mut live) = self.draft_live.take() {
                                live.draft.text = text.trim().to_string();
                                // A local model sometimes hands the passage
                                // back unchanged. That is not a rewrite: ask
                                // once more with a sharper instruction, and
                                // never store a second echo as a draft.
                                let echoed = action == Action::Rewrite
                                    && live.draft.source.as_ref().is_some_and(|source| {
                                        echo_similarity(&live.draft.text, source) >= 0.95
                                    });
                                if echoed && live.guard_attempt == 0 {
                                    let range = live.draft.range;
                                    self.draft_live = Some(live);
                                    if let Some((start, end)) = range {
                                        self.editor_selection = Some(start..end);
                                    }
                                    self.rewrite_nudge = true;
                                    self.notify(
                                        "The AI repeated the passage. Asking once more for a real rewrite.",
                                        false,
                                    );
                                    self.run_action_inner(Action::Rewrite, String::new(), true);
                                    if let Some(next) = self.draft_live.as_mut() {
                                        next.guard_attempt = 1;
                                    }
                                    ctx.request_repaint();
                                    return;
                                }
                                if echoed {
                                    // Keep the block with an empty text so
                                    // Delete and Retry stay reachable.
                                    live.draft.text.clear();
                                    live.draft.stopped = true;
                                    self.draft_live = Some(live);
                                    self.draft_error = Some(
                                        "The AI repeated the passage word for word instead of rewriting it. Retry for a new version."
                                            .to_string(),
                                    );
                                    self.notify(
                                        "Nothing was saved — the answer was the passage again.",
                                        true,
                                    );
                                    ctx.request_repaint();
                                    return;
                                }
                                let words = live.draft.text.split_whitespace().count();
                                if let Some(chapter) = self
                                    .library
                                    .story_mut(&job.story_id)
                                    .and_then(|story| story.chapter_mut(&job.chapter_id))
                                {
                                    chapter.draft = Some(live.draft);
                                }
                                self.draft_error = None;
                                self.mark_dirty();
                                if action == Action::Continue {
                                    let target =
                                        action.kind().words(&self.library.settings.prompts);
                                    if target > 0 && words > (target as usize * 3) / 2 {
                                        self.notify(
                                            format!(
                                                "The draft is longer than the {target}-word target. Retry for a shorter one."
                                            ),
                                            false,
                                        );
                                    }
                                }
                                if job.story_id == self.story_id() {
                                    self.draft_follow = true;
                                    self.draft_reveal = true;
                                }
                            }
                        }
                        ctx.request_repaint();
                    } else {
                        let answer = text.trim().to_string();
                        if !answer.is_empty() {
                            // The chat can carry an action block. The prose goes
                            // into the conversation; the action waits below it
                            // for Accept, Deny or Retry.
                            let ask = std::mem::take(&mut self.assistant_ask);
                            let parsed = self
                                .library
                                .story(&job.story_id)
                                .map(|story| ai::parse_assistant_reply(&answer, story, &ask));
                            let (prose, action) = match parsed {
                                Some(reply) => (reply.prose, reply.action),
                                None => (answer.clone(), None),
                            };
                            let action_allowed = ai::asks_for_action(&ask);
                            let prose = ai::chat_text(&prose, action.is_some(), action_allowed);
                            let propose = action.is_some() && action_allowed;
                            if !propose && !prose.is_empty() {
                                if let Some(chapter) = self
                                    .library
                                    .story_mut(&job.story_id)
                                    .and_then(|story| story.chapter_mut(&job.chapter_id))
                                {
                                    chapter.chat.push(ChatMessage {
                                        role: "assistant".to_string(),
                                        text: prose,
                                        demo: false,
                                    });
                                }
                                if job.story_id == self.story_id() {
                                    self.scroll_chat = true;
                                }
                                self.mark_dirty();
                            }
                            if propose {
                                if let Some(action) = action {
                                    self.assistant_proposal = Some(AssistantProposal {
                                        story_id: job.story_id.clone(),
                                        chapter_id: job.chapter_id.clone(),
                                        ask,
                                        action,
                                    });
                                    if job.story_id == self.story_id() {
                                        self.scroll_chat = true;
                                        self.chat_follow = true;
                                    }
                                }
                            }
                        } else {
                            self.assistant_ask.clear();
                        }
                        ctx.request_repaint();
                    }
                }
                JobKind::Lore => {
                    let story_id = job.story_id.clone();
                    let suggestion = self
                        .library
                        .story(&story_id)
                        .map(|story| ai::parse_lore_reply(&text, story));
                    if let Some(suggestion) = suggestion {
                        self.open_lore_proposal(story_id, suggestion);
                    }
                    ctx.request_repaint();
                }
                JobKind::Proofread { base_hash } => {
                    self.apply_proofread(
                        job.story_id.clone(),
                        job.chapter_id.clone(),
                        base_hash,
                        text,
                    );
                    ctx.request_repaint();
                }
            },
            Err(error) => {
                if let JobKind::Summary { step } = job.kind {
                    self.summary_failed(step, error);
                    ctx.request_repaint();
                    return;
                }
                // A draft that broke off keeps its partial text under the
                // chapter, with Delete and Retry beside it.
                let belongs = self.draft_live.as_ref().is_some_and(|live| {
                    live.story_id == job.story_id && live.chapter_id == job.chapter_id
                });
                if belongs {
                    if let Some(live) = self.draft_live.as_mut() {
                        live.draft.text = job.stream.text.trim().to_string();
                    }
                    self.draft_error = Some(error);
                    ctx.request_repaint();
                    return;
                }
                // A failed request ends the whole run; otherwise the queue would
                // sit there with nothing driving it.
                if matches!(job.kind, JobKind::Action(Action::Brainstorm)) {
                    self.assistant_ask.clear();
                }
                self.summary_queue.clear();
                self.summary_batch = None;
                self.notify(format!("AI request failed: {error}"), true);
            }
        }
    }

    pub(crate) fn poll_test(&mut self, ctx: &egui::Context) {
        let outcome = match self.test_rx.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(Ok(message))) => Some(Ok(message)),
            Some(Ok(Err(error))) => Some(Err(error)),
            Some(Err(TryRecvError::Disconnected)) => {
                Some(Err("The connection test stopped.".to_string()))
            }
            _ => None,
        };
        let Some(result) = outcome else {
            if self.test_rx.is_some() {
                ctx.request_repaint_after(Duration::from_millis(120));
            }
            return;
        };
        self.test_rx = None;
        match result {
            Ok(message) => self.test_status = Some((true, message)),
            Err(error) => self.test_status = Some((false, error)),
        }
    }

    pub(crate) fn start_test(&mut self) {
        if self.test_rx.is_some() {
            return;
        }
        let settings = self.library.settings.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(ai::test_endpoint(&settings));
        });
        self.test_rx = Some(rx);
        self.test_status = None;
        // The lamp follows the same run, so Settings and the top bar can never
        // disagree about the endpoint.
        self.probe = Probe::Checking;
        self.probe_from = self.library.settings.base_url.trim().to_string();
        // Whoever checks the connection wants the model list too.
        self.start_model_fetch();
    }

    /// Ask the endpoint which models it has, in the background.
    pub(crate) fn start_model_fetch(&mut self) {
        if self.models_rx.is_some() {
            return;
        }
        let settings = self.library.settings.clone();
        self.models_from = settings.base_url.trim().to_string();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(ai::list_models(&settings));
        });
        self.models_rx = Some(rx);
        self.models_error = None;
    }

    pub(crate) fn poll_models(&mut self, ctx: &egui::Context) {
        let outcome = match self.models_rx.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(Ok(names))) => Some(Ok(names)),
            Some(Ok(Err(error))) => Some(Err(error)),
            Some(Err(TryRecvError::Disconnected)) => {
                Some(Err("The model request stopped.".to_string()))
            }
            _ => None,
        };
        let Some(result) = outcome else {
            if self.models_rx.is_some() {
                ctx.request_repaint_after(Duration::from_millis(150));
            }
            return;
        };
        self.models_rx = None;
        // The address changed while the request was out: the answer belongs to
        // a server the author is no longer pointing at.
        if self.models_from != self.library.settings.base_url.trim() {
            return;
        }
        match result {
            Ok(list) => {
                // A model that is not in the list (or no model at all) is almost
                // always a placeholder; the endpoint just told us better.
                let current = self.library.settings.model.trim().to_string();
                if current.is_empty() || !list.names.iter().any(|name| name == &current) {
                    if let Some(first) = list.names.first() {
                        self.library.settings.model = first.clone();
                        self.mark_dirty();
                    }
                }
                // What the endpoint says about its window wins over what we
                // remembered from an earlier address.
                let limit = list.context_limit.unwrap_or(0);
                if self.library.settings.context_limit != limit {
                    self.library.settings.context_limit = limit;
                    self.mark_dirty();
                }
                self.models = list.names;
            }
            Err(error) => {
                self.models_error = Some(error);
            }
        }
    }

    /// Turn the finished test and model answer into one lamp state. Called
    /// after both pollers, so the picture is complete before it is judged.
    pub(crate) fn poll_probe(&mut self, ctx: &egui::Context) {
        if !self.probe.is_checking() {
            return;
        }
        if self.test_rx.is_some() || self.models_rx.is_some() {
            ctx.request_repaint_after(Duration::from_millis(120));
            return;
        }
        let base = self.library.settings.base_url.trim().to_string();
        // A test about an address that is no longer in use says nothing about
        // the endpoint the author points at now.
        if self.probe_from != base {
            self.probe = Probe::Idle;
            return;
        }
        // A model list from another address would be a green light for a
        // server we are not talking to.
        if self.models_from != base {
            self.probe = Probe::Idle;
            return;
        }
        let model = self.library.settings.model.trim().to_string();
        let context = self.library.settings.context_limit;
        let Some(test) = self.test_status.clone() else {
            // No run to judge (a screenshot with a fixed lamp). Stay as we are
            // rather than inventing an answer.
            return;
        };
        self.probe = judge_probe(
            test,
            &self.models,
            self.models_error.as_deref(),
            &model,
            context,
        );
    }

    /// A lamp must never stay green for an address that is no longer in use.
    pub(crate) fn sync_probe(&mut self) {
        if matches!(self.probe, Probe::Idle) {
            return;
        }
        if self.probe_from != self.library.settings.base_url.trim() {
            self.probe = Probe::Idle;
        }
    }
}

/// Runs one request on a worker thread and turns its deltas into events. The
/// returned flag stops the worker between two frames.
pub(crate) fn start_stream(
    settings: AiSettings,
    prompt: String,
    history: Vec<ChatMessage>,
    memory: String,
    words: u32,
    allow_continuation: bool,
) -> (Receiver<StreamEvent>, Arc<AtomicBool>) {
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = {
            let mut on_delta = |delta: &str| {
                let _ = tx.send(StreamEvent::Delta(delta.to_string()));
            };
            ai::stream_with_target(
                &settings,
                &prompt,
                &history,
                &memory,
                None,
                words,
                allow_continuation,
                &worker_cancel,
                &mut on_delta,
            )
        };
        let event = match result {
            Ok(reply) => StreamEvent::Done {
                truncated: reply.truncated,
            },
            Err(error) => StreamEvent::Failed(error),
        };
        let _ = tx.send(event);
    });
    (rx, cancel)
}

/// The offline demo writer, fed through the same event path as a real one so
/// the draft block grows in small pieces instead of appearing all at once.
pub(crate) fn start_demo_stream(text: String) -> (Receiver<StreamEvent>, Arc<AtomicBool>) {
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut piece = String::new();
        let mut chars = 0;
        for ch in text.chars() {
            if worker_cancel.load(Ordering::Relaxed) {
                break;
            }
            piece.push(ch);
            chars += 1;
            let boundary = ch.is_whitespace() || matches!(ch, '.' | ',' | '!' | '?');
            if chars >= 12 && boundary {
                if tx
                    .send(StreamEvent::Delta(std::mem::take(&mut piece)))
                    .is_err()
                {
                    return;
                }
                chars = 0;
                std::thread::sleep(Duration::from_millis(40));
            }
        }
        if !piece.is_empty() {
            let _ = tx.send(StreamEvent::Delta(piece));
        }
        let _ = tx.send(StreamEvent::Done { truncated: false });
    });
    (rx, cancel)
}
