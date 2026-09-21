use naclac_lang::prelude::*;
use crate::components::Vault;

// No test program in this repo boxes any account field before this. The
// point isn't the `Box` keyword itself — it's the blanket impls added for
// it in `naclac-core/src/wrappers/mod.rs` (`NaclacAccount`, `Owner`,
// `ToAddress`, `AsRefByteSlice`, `ToAccountInfo`/`ToAccountInfos`,
// `ToCpiHandle`/`ToCpiHandleMut`, confirmed by reading that file directly):
// a `Box<Account<T>>` field should be a fully transparent stand-in for a
// bare `Account<T>` field everywhere naclac's own codegen or a handler body
// touches it — same `#[account(mut)]` constraint parsing, same
// `.address()`/`Deref`/mutation behavior, same eligibility as a CPI handle
// source. This instruction exercises all of that on a real, already-existing
// `Vault` component rather than a purpose-built dummy type, so a regression
// here would be directly comparable to `touch_pair_no_alias`'s un-boxed
// behavior.
#[derive(Accounts)]
pub struct TouchBoxedVault {
    #[account(mut)]
    pub target: Box<Account<Vault>>,
}

pub fn touch_boxed_vault(ctx: Context<TouchBoxedVault>) -> Result {
    // Deref/DerefMut through the Box, exactly like an unboxed `Account<T>`.
    ctx.accounts.target.balance += 1;
    // `.address()` (ToAddress) must also work transparently through the Box.
    let _addr = ctx.accounts.target.address();
    Ok(())
}
