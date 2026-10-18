//! Must FAIL to build: `#[max_len]` on a `u64` field, which has no `Vec` or
//! `String` for the limit to apply to. Checked by `tests/diagnostics.rs`.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[component]
pub struct Counter {
    #[max_len(4)]
    pub count: u64,
}
