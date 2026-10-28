//! A `mod.rs` file module with a file child and a `pub(crate)` file child.

use naclac_lang::prelude::*;

pub(crate) mod crate_visible;
pub mod inner;

#[constant]
pub const NESTED_SEED: &[u8] = b"nested";
