//! The pure heart of Tellshot: geometry, the non-destructive document model, edits and undo.
//!
//! This crate is `no_std` (with `alloc`), so the compiler proves it cannot read files, open
//! sockets or look at the clock. Everything with side effects lives in the shell crates
//! (`tellshot-format`, `tellshot-capture`, the apps). That keeps this crate deterministic,
//! trivially testable and safe to run on untrusted input from files or AI agents.
//!
//! # Model
//!
//! * A [`Document`] is an immutable base image (stored outside this crate) plus an ordered list
//!   of annotation [`Object`]s. Nothing touches base pixels until export.
//! * Callers (UI, CLI, agents) express intent as an [`Edit`]. [`History::apply`] validates it,
//!   resolves it into an invertible [`Op`], applies it and records it for undo.
//! * All coordinates here are in *document space*: one unit is one pixel of the base image.
//!   Conversions from physical or logical screen pixels happen at the platform boundary.

#![cfg_attr(not(any(test, feature = "std")), no_std)]

extern crate alloc;

mod color;
mod document;
mod edit;
mod geometry;
mod history;
mod limits;
mod object;
mod op;

pub use color::{Color, palette};
pub use document::{BaseImage, Document, ValidationError};
pub use edit::{Arrange, Edit, EditError};
pub use geometry::{Point, Rect, Size};
pub use history::{Applied, CoalesceKey, History};
pub use limits::Limits;
pub use object::{Object, ObjectId, Shape, ShapeKind, Style};
pub use op::{Op, OpError};
