//! Helpers shared by several macros:
//! attribute/warning diagnostics (`diagnostics`) and
//! `#[derive(...)]` lookup (`derive_attrs`).

pub(crate) mod cfg_gates;
pub(crate) mod derive_attrs;
pub(crate) mod diagnostics;
