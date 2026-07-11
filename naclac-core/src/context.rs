use crate::prelude::{AccountType, Address};

/// Unified trait for the generated Bumps struct
pub trait Bumps {
    type BumpsStruct: Clone + Copy;
}

/// Type alias to simplify load_and_validate return type and satisfy type complexity rules.
pub type ValidationResult<'a, P, B> =
    core::result::Result<(P, &'a [AccountType], B), crate::prelude::ProgramError>;

/// Trait implemented by instruction Accounts structs to safely load, validate,
/// dereference, and teardown account payloads (conditional boxed vs stack).
pub trait LoadableAccounts<'a>: Bumps {
    type Payload;

    fn load_and_validate(
        program_id: &Address,
        accounts: &'a [AccountType],
        instruction_data: &[u8],
        __duplicates: Option<&crate::prelude::AccountBitvec>,
    ) -> ValidationResult<'a, Self::Payload, Self::BumpsStruct>;

    fn as_mut_payload(payload: &mut Self::Payload) -> &mut Self;

    fn teardown_payload(
        payload: &mut Self::Payload,
        program_id: &Address,
        bumps: &Self::BumpsStruct,
    ) -> Result<(), crate::prelude::ProgramError>;
}

/// The Context object passed to instruction handlers.
///
/// Lifetime-free Context using static references, transmutting internal
/// program lifetimes to keep user code completely clean of lifetimes.
pub struct Context<T: Bumps + 'static> {
    /// The currently executing program ID
    pub program_id: &'static Address,
    /// The validated and hydrated accounts struct
    pub accounts: &'static mut T,
    /// Any remaining accounts not specified in the struct
    pub remaining_accounts: &'static [AccountType],
    /// The bumps for any PDA accounts
    pub bumps: T::BumpsStruct,
}

impl<T: Bumps + 'static> Context<T> {
    /// Constructs a lifetime-free `Context` by transmuting temporary reference lifetimes to `'static`.
    ///
    /// # Safety
    /// This is safe because instruction execution is synchronous and the referenced accounts
    /// and program ID live for the entire duration of the transaction call.
    #[inline(always)]
    pub unsafe fn new(
        program_id: &Address,
        accounts: &mut T,
        remaining_accounts: &[AccountType],
        bumps: T::BumpsStruct,
    ) -> Self {
        Self {
            program_id: core::mem::transmute::<&Address, &'static Address>(program_id),
            accounts: core::mem::transmute::<&mut T, &'static mut T>(accounts),
            remaining_accounts: core::mem::transmute::<&[AccountType], &'static [AccountType]>(
                remaining_accounts,
            ),
            bumps,
        }
    }
}
