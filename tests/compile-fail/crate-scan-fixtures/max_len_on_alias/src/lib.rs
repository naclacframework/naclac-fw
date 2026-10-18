//! Must BUILD: a crate-local alias of `String` takes the `#[max_len]` limit
//! like `String` itself, so `Named::SPACE` is the 8-byte discriminator plus a
//! 4-byte length prefix plus the 8-byte limit. Checked by `tests/diagnostics.rs`.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

type Label = String;

#[component]
pub struct Named {
    #[max_len(8)]
    pub label: Label,
}

const _: () = assert!(Named::SPACE == 8 + 4 + 8);
