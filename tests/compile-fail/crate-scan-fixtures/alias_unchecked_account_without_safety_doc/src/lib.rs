//! Must FAIL to build: `Raw` is an alias of `UncheckedAccount`, and a field of that
//! type needs a `/// SAFETY:` doc comment under any name. Checked by
//! `tests/diagnostics.rs`.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

type Raw = UncheckedAccount;

#[derive(Accounts)]
pub struct Touch {
    pub raw: Raw,
}
