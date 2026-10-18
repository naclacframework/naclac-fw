//! Must FAIL to build: `Plain` is a struct with no `#[defined_type]`, so it
//! has no known maximum serialized size and `Holder::SPACE` cannot be
//! computed. Checked by `tests/diagnostics.rs`.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[derive(Clone, naclac_lang::prelude::BorshSerialize, naclac_lang::prelude::BorshDeserialize)]
#[borsh(crate = "naclac_lang::prelude::borsh")]
pub struct Plain {
    pub value: u64,
}

#[component]
pub struct Holder {
    pub plain: Plain,
}

pub const HOLDER_SPACE: usize = Holder::SPACE;
