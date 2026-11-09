//! Must FAIL to build: `Raw` is an alias of `AccountInfo`, and a field of that
//! type needs a `/// SAFETY:` doc comment under any name. Checked by
//! `tests/diagnostics.rs`.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

type Raw = AccountInfo;

#[derive(Accounts)]
pub struct Touch {
    pub raw: Raw,
}
