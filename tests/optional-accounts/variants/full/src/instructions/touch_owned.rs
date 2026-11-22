use {crate::state::Owned, naclac_lang::prelude::*};

#[derive(Accounts)]
pub struct TouchOwned {
    pub authority: Signer,

    #[account(mut, authority = authority)]
    pub owned: Option<Account<Owned>>,
}

pub fn touch_owned(ctx: Context<TouchOwned>) -> Result {
    if let Some(owned) = ctx.accounts.owned.as_mut() {
        owned.value += 1;
    }
    Ok(())
}
