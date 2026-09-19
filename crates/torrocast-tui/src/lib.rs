//! The terminal interface. State and keys live in [`app`], drawing in [`ui`];
//! neither fetches anything — that is the core's work.

pub mod app;
pub mod covers;
pub mod i18n;
pub mod text;
pub mod theme;
pub mod ui;
