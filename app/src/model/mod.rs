//! Story library data, persistence and export.
//!
//! The model is split by topic:
//!
//! - [`library`]: all stories together with the settings and the id counter
//! - [`story`]: story, chapter and draft, plus the content fingerprint
//! - [`lore`]: lorebook entries and text matching
//! - [`settings`]: endpoint, model, context window and editable prompts
//! - [`sample`]: the sample library for the demo and screenshots
//! - [`store`]: loading and saving `library.json`
//! - [`files`]: file names, chapter text and the PDF/text export
//!
//! The types are re-exported here, so `crate::model::Story` and the rest keep
//! working as before.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

mod files;
mod library;
mod lore;
mod sample;
mod settings;
mod store;
mod story;

pub(crate) use files::*;
pub(crate) use library::*;
pub(crate) use lore::*;
pub(crate) use sample::*;
pub(crate) use settings::*;
pub(crate) use store::*;
pub(crate) use story::*;

#[cfg(test)]
mod tests;
