use crate::prelude::{Pubkey, AccountType};

/// Unified trait for the generated Bumps struct
pub trait Bumps {
    type BumpsStruct: Clone + Copy;
}

/// The Context object passed to instruction handlers.
/// 
/// We use a single 'info lifetime to keep signatures clean for developers.
/// To support account teardown (saving/closing) after instruction logic,
/// we provide a teardown() method on the Context itself.
pub struct Context<'info, T: Bumps> {
    /// The currently executing program ID
    pub program_id: &'info Pubkey,
    /// The validated and hydrated accounts struct
    pub accounts: &'info mut T,
    /// Any remaining accounts not specified in the struct
    pub remaining_accounts: &'info [AccountType<'info>],
    /// The bumps for any PDA accounts
    pub bumps: T::BumpsStruct,
}

impl<'info, T: Bumps> Context<'info, T> {
    pub fn new(
        program_id: &'info Pubkey,
        accounts: &'info mut T,
        remaining_accounts: &'info [AccountType<'info>],
        bumps: T::BumpsStruct,
    ) -> Self {
        Self {
            program_id,
            accounts,
            remaining_accounts,
            bumps,
        }
    }
}