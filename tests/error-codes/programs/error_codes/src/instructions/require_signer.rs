use naclac_lang::prelude::*;

// No custom logic needed — a `Signer` field's own presence-check is enough
// to naturally trigger a real NaclacError (`ConstraintSigner`, in the
// 3000-range framework codespace) when the account isn't actually signed,
// proving custom errors (6000+) and framework errors (3000s) genuinely
// don't collide, not just that their formulas don't overlap on paper.
#[derive(Accounts)]
pub struct RequireSigner {
    pub authority: Signer,
}

#[instruction]
pub fn require_signer(_ctx: Context<RequireSigner>) -> Result {
    Ok(())
}
