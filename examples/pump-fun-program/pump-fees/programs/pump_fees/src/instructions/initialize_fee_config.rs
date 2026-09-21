use naclac_lang::prelude::*;
use crate::components::FeeConfig;
use crate::constants::{ADMIN_PUBKEY, FEE_CONFIG_SEED};
use crate::events::InitializeFeeConfigEvent;

// `config_program_id` must be declared before `fee_config`: its seeds
// reference `config_program_id.address()`, and sibling-field seed
// references must come after the field they reference.
//
// `fee_config_bump` is an arg with no equivalent in the real IDL (which
// takes zero args here) — naclac bans on-chain `find_program_address`, so
// any PDA seeded by a non-constant value (here, `config_program_id`'s
// address) needs its bump computed off-chain and passed in for a cheap
// on-chain hash-and-compare instead.
#[derive(Accounts)]
#[instruction(fee_config_bump: u8)]
pub struct InitializeFeeConfig {
    // fees-05: only the hardcoded bootstrap admin may create a FeeConfig.
    #[account(mut, address = ADMIN_PUBKEY)]
    pub admin: Signer,
    /// SAFETY: only used as PDA seed material for `fee_config`; not
    /// deserialized or invoked.
    pub config_program_id: AccountInfo,
    #[account(init, payer = admin, seeds = [FEE_CONFIG_SEED, config_program_id.address().as_ref()], bump = fee_config_bump)]
    pub fee_config: Account<FeeConfig>,
    pub system_program: Program<System>,
}

/// Initialize FeeConfig admin
pub fn initialize_fee_config(ctx: Context<InitializeFeeConfig>, fee_config_bump: u8) -> Result {
    let timestamp = unix_timestamp()?;

    ctx.accounts.fee_config.bump = fee_config_bump;
    ctx.accounts.fee_config.admin = ctx.accounts.admin.address();

    emit!(InitializeFeeConfigEvent {
        timestamp,
        admin: ctx.accounts.admin.address(),
        fee_config: ctx.accounts.fee_config.address(),
    });

    Ok(())
}
