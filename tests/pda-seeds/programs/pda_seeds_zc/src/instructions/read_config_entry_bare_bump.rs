use naclac_lang::prelude::*;
use crate::components::ConfigEntry;
use crate::constants::SEED_CONFIG_ENTRY;

#[derive(Accounts)]
pub struct ReadConfigEntryBareBump {
    /// SAFETY: only used as PDA seed material for `config_entry` below; never
    /// deserialized, invoked, or otherwise trusted for its own contents.
    pub config_program_id: AccountInfo,

    #[account(seeds = [SEED_CONFIG_ENTRY, config_program_id.address().as_ref()], bump)]
    pub config_entry: Account<ConfigEntry>,
}

#[instruction]
pub fn read_config_entry_bare_bump(
    ctx: Context<ReadConfigEntryBareBump>,
    is_pump_pool: Bool,
    market_cap_lamports: u128,
    _trade_size_lamports: u64,
    is_new_quote_mint: Bool,
) -> Result<u64> {
    let _ = (is_pump_pool, market_cap_lamports, is_new_quote_mint);
    Ok(ctx.accounts.config_entry.value)
}
