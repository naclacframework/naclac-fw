//! Must FAIL to build: `Ledger::SPACE` is used, but its `Vec` field has no
//! `#[max_len]`, so its size is unknown. Checked by `tests/diagnostics.rs`.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[component]
pub struct Ledger {
    pub entries: Vec<u64>,
}

pub const LEDGER_SPACE: usize = Ledger::SPACE;
