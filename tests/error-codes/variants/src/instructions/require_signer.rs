use naclac_lang::prelude::*;

// No custom logic needed — a `Signer` field's own presence-check is enough
// to naturally trigger a real NaclacError (`ConstraintSigner`, in the
// 10000+ framework codespace) when the account isn't actually signed,
// proving custom errors (6000..10000) and framework errors (10000+) genuinely
// don't collide, not just that their formulas don't overlap on paper.
#[derive(Accounts)]
pub struct RequireSigner {
    pub authority: Signer,
}

pub fn require_signer(_ctx: Context<RequireSigner>) -> Result {
    Ok(())
}
