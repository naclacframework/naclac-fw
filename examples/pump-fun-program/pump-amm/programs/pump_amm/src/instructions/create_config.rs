use naclac_lang::prelude::*;
use crate::components::GlobalConfig;
use crate::constants::{ADMIN_PUBKEY, GLOBAL_CONFIG_SEED};

#[derive(Accounts)]
pub struct CreateConfig {
    #[account(mut, address = ADMIN_PUBKEY)]
    pub admin: Signer,

    #[account(init, payer = admin, seeds = [GLOBAL_CONFIG_SEED], bump)]
    pub global_config: Account<GlobalConfig>,

    pub system_program: Program<System>,
}

/// Creates the singleton `GlobalConfig`, gated by the project's own
/// `ADMIN_PUBKEY` (see its doc comment for why this diverges from real
/// `pump_amm`'s hardcoded admin). Leaves `boost_enabled`/`boost_authority`
/// at their zero-init defaults -- `toggle_boost`/`set_boost_authority`
/// (both gated on `global_config.admin`, set here) enable them separately,
/// matching real `create_config`'s own args, which don't include boost
/// fields at all.
#[instruction]
pub fn create_config(ctx: Context<CreateConfig>) -> Result {
    let global_config = &mut ctx.accounts.global_config;
    global_config.admin = ctx.accounts.admin.address();
    global_config.disable_flags = 0;
    global_config.boost_enabled = Bool::from(false);
    global_config.boost_authority = Address::default();

    Ok(())
}
