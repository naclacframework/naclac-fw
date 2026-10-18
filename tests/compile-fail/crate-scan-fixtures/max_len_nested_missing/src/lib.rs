//! Must FAIL to build: `Outer::SPACE` is used, and `Outer` holds an `Inner`
//! whose `Vec` field has no `#[max_len]`. Checked by `tests/diagnostics.rs`.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[defined_type]
pub struct Inner {
    pub items: Vec<u8>,
}

#[component]
pub struct Outer {
    pub inner: Inner,
}

pub const OUTER_SPACE: usize = Outer::SPACE;
