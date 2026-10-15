use crate::prelude::{AccountType, Address};

/// Unified trait for the generated Bumps struct
pub trait Bumps {
    type BumpsStruct: Clone + Copy;
}

/// Implemented by every `#[derive(Accounts)]` struct. `NEEDS_EVENT_CPI` is
/// `true` only for a struct annotated with `#[event_cpi]`; `#[program]` ORs
/// this across its handlers' account structs in a const context to decide
/// whether the reserved event-CPI dispatch arm is live.
pub trait EventCpiAccounts {
    const NEEDS_EVENT_CPI: bool;
}

/// Implemented by `#[program]` for every accounts struct one of its handlers
/// uses: the program's reserved self-CPI dispatch tag, at the program's own
/// instruction-discriminator width. `emit_cpi!` reads it through the handler's
/// `Context`, so it needs no path into the caller's crate.
pub trait EventCpiTag {
    const EVENT_DISPATCH_TAG: &'static [u8];
}

/// Implemented by `#[event_cpi]` for the accounts struct it annotates: access
/// to the `event_authority` account and bump it injected, so `emit_cpi!` never
/// names those fields itself. A struct without `#[event_cpi]` has no impl,
/// which is what produces `emit_cpi!`'s "needs `#[event_cpi]`" error.
#[diagnostic::on_unimplemented(
    message = "`emit_cpi!` needs the handler's accounts struct to carry `#[event_cpi]`",
    label = "`{Self}` is not annotated with `#[event_cpi]`",
    note = "put `#[event_cpi]` directly above `#[derive(Accounts)]` on `{Self}`; it injects the `event_authority` and `program` accounts the self-CPI needs"
)]
pub trait EventCpiAuthority: Bumps + 'static {
    fn event_authority(&self) -> &crate::prelude::AccountInfo;
    fn event_authority_bump(bumps: &Self::BumpsStruct) -> u8;
}

/// Type alias to simplify load_and_validate return type and satisfy type complexity rules.
pub type ValidationResult<'a, P, B> =
    core::result::Result<(P, &'a [AccountType], B), crate::prelude::ProgramError>;

/// Trait implemented by instruction Accounts structs to safely load, validate,
/// dereference, and teardown account payloads (conditional boxed vs stack).
pub trait LoadableAccounts<'a>: Bumps {
    type Payload;

    /// `__NACLAC_IX_DISC_LEN` is the byte width of the program's instruction
    /// discriminator: instruction arguments start at that offset in
    /// `instruction_data`.
    fn load_and_validate<const __NACLAC_IX_DISC_LEN: usize>(
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
