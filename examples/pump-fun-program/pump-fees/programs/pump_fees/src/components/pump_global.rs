use naclac_lang::prelude::*;

/// Field-for-field mirror of `pump-bonding-curve`'s own real `Global`
/// account, as far as field *order* goes — but this specific `#[component]`
/// (zero-copy, `#[repr(C)]`) instance is itself already deployed on devnet,
/// so its own layout is what must stay stable now, not the real (Borsh,
/// hence unpaddded) program's byte-for-byte layout, which a `#[repr(C)]`
/// struct could never replicate exactly regardless of field order anyway.
/// The three `_padding*` fields below make internal alignment gaps
/// bytemuck would otherwise silently insert explicit instead — needed
/// because reordering fields (which would remove them) isn't safe while
/// this exact layout is already live. Revisit alongside a redeploy: at that
/// point the fields can be reordered largest-alignment-first and these
/// padding fields removed instead.
#[component]
pub struct Global {
    pub initialized: Bool,
    pub authority: Address,
    pub fee_recipient: Address,
    _padding_a: [u8; 7],
    pub initial_virtual_token_reserves: u64,
    pub initial_virtual_sol_reserves: u64,
    pub initial_real_token_reserves: u64,
    pub token_total_supply: u64,
    pub fee_basis_points: u64,
    pub withdraw_authority: Address,
    pub enable_migrate: Bool,
    _padding_b: [u8; 7],
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
    _padding_c: [u8; 5],
    pub buyback_basis_points: u64,
    pub initial_virtual_quote_reserves: u64,
    pub whitelisted_quote_mints: [Address; 2],
}
