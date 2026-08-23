use naclac_lang::prelude::*;
use crate::components::Shareholder;

#[event(alloc)]
pub struct CreateEvent {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub mint: Address,
    pub bonding_curve: Address,
    pub user: Address,
    pub creator: Address,
    pub timestamp: i64,
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub token_total_supply: u64,
    pub token_program: Address,
    pub is_mayhem_mode: Bool,
    pub is_cashback_enabled: Bool,
    pub quote_mint: Address,
    pub virtual_quote_reserves: u64,
}

#[event]
pub struct MigrateBondingCurveCreatorEvent {
    pub timestamp: i64,
    pub mint: Address,
    pub bonding_curve: Address,
    pub sharing_config: Address,
    pub old_creator: Address,
    pub new_creator: Address,
}

#[event]
pub struct SetParamsEvent {
    pub initial_virtual_token_reserves: u64,
    pub initial_virtual_sol_reserves: u64,
    pub initial_real_token_reserves: u64,
    pub final_real_sol_reserves: u64,
    pub token_total_supply: u64,
    pub fee_basis_points: u64,
    pub withdraw_authority: Address,
    pub pool_migration_fee: u64,
    pub creator_fee_basis_points: u64,
    pub fee_recipients: [Address; 8],
    pub timestamp: i64,
    pub set_creator_authority: Address,
    pub admin_set_creator_authority: Address,
    pub enable_migrate: Bool,
}

#[event(alloc)]
pub struct DistributeCreatorFeesEvent {
    pub timestamp: i64,
    pub mint: Address,
    pub bonding_curve: Address,
    pub sharing_config: Address,
    pub admin: Address,
    pub shareholders: Vec<Shareholder>,
    pub distributed: u64,
    pub quote_mint: Address,
}

#[event]
pub struct CompleteEvent {
    pub user: Address,
    pub mint: Address,
    pub bonding_curve: Address,
    pub timestamp: i64,
    pub quote_mint: Address,
}

/// Field-for-field mirror of the real `pump::CompletePumpAmmMigrationEvent`
/// (confirmed against `reference/pump-rust-client/idls/pump.json`) — this is
/// the event the real `migrate` instruction emits; there is no separate
/// "MigrateEvent" in the real IDL.
#[event]
pub struct CompletePumpAmmMigrationEvent {
    pub timestamp: i64,
    pub mint_amount: u64,
    pub sol_amount: u64,
    pub pool_migration_fee: u64,
    pub user: Address,
    pub mint: Address,
    pub bonding_curve: Address,
    pub pool: Address,
    pub quote_mint: Address,
}

#[event]
pub struct ExtendAccountEvent {
    pub account: Address,
    pub user: Address,
    pub current_size: u64,
    pub new_size: u64,
    pub timestamp: i64,
}

#[event(alloc)]
pub struct TradeEvent {
    pub mint: Address,
    pub sol_amount: u64,
    pub token_amount: u64,
    pub is_buy: Bool,
    pub user: Address,
    pub timestamp: i64,
    pub virtual_sol_reserves: u64,
    pub virtual_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub fee_recipient: Address,
    pub fee_basis_points: u64,
    pub fee: u64,
    pub creator: Address,
    pub creator_fee_basis_points: u64,
    pub creator_fee: u64,
    pub track_volume: Bool,
    pub total_unclaimed_tokens: u64,
    pub total_claimed_tokens: u64,
    pub current_sol_volume: u64,
    pub last_update_timestamp: i64,
    pub ix_name: String,
    pub mayhem_mode: Bool,
    pub cashback_fee_basis_points: u64,
    pub cashback: u64,
    pub buyback_fee_basis_points: u64,
    pub buyback_fee: u64,
    pub shareholders: Vec<Shareholder>,
    pub quote_mint: Address,
    pub quote_amount: u64,
    pub virtual_quote_reserves: u64,
    pub real_quote_reserves: u64,
}

#[event]
pub struct UpdateGlobalAuthorityEvent {
    pub global: Address,
    pub authority: Address,
    pub new_authority: Address,
    pub timestamp: i64,
}

#[event]
pub struct ReservedFeeRecipientsEvent {
    pub timestamp: i64,
    pub reserved_fee_recipient: Address,
    pub reserved_fee_recipients: [Address; 7],
}

#[event]
pub struct SetCreatorEvent {
    pub timestamp: i64,
    pub mint: Address,
    pub bonding_curve: Address,
    pub creator: Address,
}

#[event]
pub struct SetMetaplexCreatorEvent {
    pub timestamp: i64,
    pub mint: Address,
    pub bonding_curve: Address,
    pub metadata: Address,
    pub creator: Address,
}

#[event]
pub struct AdminSetCreatorEvent {
    pub timestamp: i64,
    pub admin_set_creator_authority: Address,
    pub mint: Address,
    pub bonding_curve: Address,
    pub old_creator: Address,
    pub new_creator: Address,
}
