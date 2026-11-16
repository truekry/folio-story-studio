//! Everything that talks to the AI partner: prompts, endpoint, streaming.
//!
//! The module is split by task:
//!
//! - [`kind`]: the actions and the editable prompts behind them
//! - [`prompt`]: what the partner is told about story, chapter and passage
//! - [`proofread`]: the paragraph-by-paragraph spelling and grammar pass
//! - [`lore`]: lore suggestions read out of a reply
//! - [`endpoint`]: addresses, model lists and context windows
//! - [`budget`]: how the context window is divided
//! - [`stream`]: a reply as it arrives, including the cancel flag
//! - [`reply`]: the finished reply, its ending and content extraction
//! - [`demo`]: the offline partner
//!
//! The type names are re-exported here, so `crate::ai::Action` and
//! `crate::ai::stream_with_target` keep working as before.

use crate::model::{
    AiSettings, Chapter, ChatMessage, LoreEntry, PromptSetting, PromptSettings, Story,
};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

mod budget;
mod demo;
mod endpoint;
mod kind;
mod lore;
mod prompt;
mod proofread;
mod reply;
mod stream;

pub use budget::*;
pub use demo::*;
pub use endpoint::*;
pub use kind::*;
pub use lore::*;
pub use prompt::*;
pub use proofread::*;
pub use reply::*;
pub use stream::*;

#[cfg(test)]
mod tests;
