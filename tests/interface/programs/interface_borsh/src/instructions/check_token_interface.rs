use naclac_lang::prelude::*;

// Same shape/comments as `programs/interface`'s copy — see that file for the
// full rationale. Kept identical deliberately: `interface.rs`'s only branch
// is `#[cfg(feature = "pinocchio")]` vs. not, so solana-borsh and
// solana-zerocopy hit the exact same non-pinocchio code path here. This
// borsh variant is the one representative of that shared path (cheaper to
// iterate on than zero-copy); a dedicated `interface_zc` variant was judged
// redundant for this specific wrapper and skipped — see TEST_PLAN.md.
#[derive(Accounts)]
pub struct CheckTokenInterface {
    pub token_program: Interface<TokenInterface>,
}

#[instruction]
pub fn check_token_interface(_ctx: Context<CheckTokenInterface>) -> Result {
    Ok(())
}
