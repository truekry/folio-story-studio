//! A small PDF writer, just enough to set a story on A4 with the standard
//! Helvetica faces. No dependencies, no fonts to install, no network: the
//! bytes are assembled here and written by the caller.
//!
//! Text is encoded as Windows-1252, which is what the built-in Helvetica
//! expects. Prose in Western European languages comes through untouched;
//! anything outside that range becomes "?" rather than breaking the file.
//!
//! - [`metrics`]: page geometry, faces and text width
//! - [`text`]: paragraphs and line fitting
//! - [`layout`]: the title page, contents, chapter pages and the entry point
//! - [`writer`]: encoding and the PDF objects

use crate::model::Story;

mod layout;
mod metrics;
mod text;
mod writer;

pub(crate) use layout::*;
pub(crate) use metrics::*;
pub(crate) use text::*;
pub(crate) use writer::*;

#[cfg(test)]
mod tests;
