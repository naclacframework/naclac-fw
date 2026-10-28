//! Loaded through `#[path]` from inside an inline module (`crate::inline::deeper::pathed`); the
//! path is relative to the inline module's own directory, `src/inline/deeper/`.

use naclac_lang::prelude::*;

#[event]
pub struct TargetEvent {
    pub y: u32,
}
