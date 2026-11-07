//! Helpers shared by several macros: reading the calling crate's source
//! (`crate_source`), attribute/warning diagnostics (`diagnostics`) and
//! `#[derive(...)]` lookup (`derive_attrs`).

pub(crate) mod cfg_gates;
pub(crate) mod crate_source;
pub(crate) mod derive_attrs;
pub(crate) mod diagnostics;
