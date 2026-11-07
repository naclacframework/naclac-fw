//! Checks on the mints the AMM's instructions receive that the framework's
//! declarative constraints do not cover.

use {crate::errors::AmmError, naclac_lang::prelude::*};

/// Whether `a` sorts strictly before `b` as raw bytes.
pub fn is_ascending(a: &Address, b: &Address) -> bool {
    let a: &[u8] = a.as_ref();
    let b: &[u8] = b.as_ref();
    a < b
}

/// Fails unless every extension on `mint` is one of
/// [`INFORMATIONAL_MINT_EXTENSIONS`]. A mint owned by the legacy Token program
/// has none.
pub fn require_supported_mint(mint: &InterfaceAccount<Mint>) -> Result {
    let outside = mint.first_extension_outside(INFORMATIONAL_MINT_EXTENSIONS)?;
    require!(outside.is_none(), AmmError::UnsupportedMintExtension);
    Ok(())
}
