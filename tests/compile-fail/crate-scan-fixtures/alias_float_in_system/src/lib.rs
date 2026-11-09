//! Must FAIL to build: `Price` is an alias of `f64`, and #[system] rejects a
//! float under any name. Checked by `tests/diagnostics.rs`.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

type Price = f64;

#[system]
pub fn quote(price: Price) -> Result<u64> {
    let _ = price;
    Ok(0)
}
