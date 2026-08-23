use naclac_lang::prelude::*;
use crate::components::ConfigEntry;
use crate::constants::SEED_CONFIG_ENTRY;

#[derive(Accounts)]
pub struct TouchConfigEntryBareBumpWithArgs {
    /// SAFETY: only used as PDA seed material for `config_entry` below; never
    /// deserialized, invoked, or otherwise trusted for its own contents.
    pub config_program_id: AccountInfo,

    #[account(
        mut,
        seeds = [SEED_CONFIG_ENTRY, config_program_id.address().as_ref()],
        bump
    )]
    pub config_entry: Account<ConfigEntry>,
}

#[instruction]
pub fn touch_config_entry_bare_bump_with_args(
    ctx: Context<TouchConfigEntryBareBumpWithArgs>,
    is_pump_pool: Bool,
    market_cap_lamports: u128,
    _trade_size_lamports: u64,
    is_new_quote_mint: Bool,
) -> Result {
    let _ = (is_pump_pool, market_cap_lamports, is_new_quote_mint);
    let config_entry = &mut ctx.accounts.config_entry;
    config_entry.value += 1;
    Ok(())
}
