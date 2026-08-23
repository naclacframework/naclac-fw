use naclac_lang::prelude::*;

/// Field-for-field mirror of `pump-bonding-curve`'s own real `Global`
/// account — `pump_fees` reads this account (owned by that program) via a
/// raw byte cast, so this layout depends on matching it exactly.
#[component]
pub struct Global {
    pub initialized: Bool,
    pub authority: Address,
    pub fee_recipient: Address,
    pub initial_virtual_token_reserves: u64,
    pub initial_virtual_sol_reserves: u64,
    pub initial_real_token_reserves: u64,
    pub token_total_supply: u64,
    pub fee_basis_points: u64,
    pub withdraw_authority: Address,
    pub enable_migrate: Bool,
    pub pool_migration_fee: u64,
    pub creator_fee_basis_points: u64,
    pub fee_recipients: [Address; 7],
    pub set_creator_authority: Address,
    pub admin_set_creator_authority: Address,
    pub create_v2_enabled: Bool,
    pub whitelist_pda: Address,
    pub reserved_fee_recipient: Address,
    pub mayhem_mode_enabled: Bool,
    pub reserved_fee_recipients: [Address; 7],
    pub is_cashback_enabled: Bool,
    pub buyback_fee_recipients: [Address; 8],
    pub buyback_basis_points: u64,
    pub initial_virtual_quote_reserves: u64,
    pub whitelisted_quote_mints: [Address; 2],
}
