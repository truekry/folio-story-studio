//! Unit tests for the AI side.
//!
//! Split by topic, so a change to the stream does not mean scrolling through
//! the prompt tests:
//!
//! - [`lan`]: the ignored tests against a real endpoint on the home network
//! - [`endpoint`]: addresses, model lists, context windows
//! - [`budget`]: how the context window is divided
//! - [`prompt`]: what the partner is told
//! - [`reply`]: endings and continuation
//! - [`stream`]: the SSE reader against a small local server

use super::*;
use crate::model::Library;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

mod budget;
mod endpoint;
mod lan;
mod prompt;
mod reply;
mod stream;
mod summarize;
mod tuning;

/// The non-streaming shape used by most tests: collect the whole reply and
/// ignore the deltas.
fn request_with_target(
    settings: &AiSettings,
    prompt: &str,
    history: &[ChatMessage],
    memory: &str,
    target_words: u32,
) -> Result<String, String> {
    let cancel = AtomicBool::new(false);
    stream_with_target(
        settings,
        prompt,
        history,
        memory,
        None,
        target_words,
        true,
        &cancel,
        &mut |_: &str| {},
    )
    .map(|reply| reply.text)
}
