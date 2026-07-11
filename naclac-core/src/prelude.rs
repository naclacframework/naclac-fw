// ===========================================================================
// prelude.rs — Naclac unified prelude
//
// Feature routing:
//   default ("solana")       → solana_program backend (std, lifetimes) — NO borsh by default
//   "solana" + "borsh"       → solana_program + borsh serialization (Account<T>, #[component])
//   "zero-copy"              → AccountLoader<T> path, zero-copy events via bytemuck::Pod
//   "pinocchio"              → pinocchio backend (no_std, no lifetimes, zero-copy)
//
// The two backends are mutually exclusive. Enabling "pinocchio" hides every
// solana_program symbol and replaces it with the pinocchio equivalents.
// ===========================================================================

pub use crate::context::{Bumps, Context, LoadableAccounts, ValidationResult};
#[cfg(feature = "pinocchio")]
pub use crate::cpi::invoke_signed_pinocchio;
pub use crate::cpi::{self, AccountMeta, ToAccountMetas};
pub use naclac_macros::*;

// --- Wrappers: common types (shared by both backends) ---
pub use crate::error::NaclacError;

// These traits/types exist in wrappers for BOTH Solana and Pinocchio backends
pub use crate::wrappers::{
    // Framework account wrapper types — abstract over both backends
    AccountLoader,
    AsRefByteSlice,
    AssociatedToken,
    CpiHandle,
    CpiHandleMut,
    Discriminator,
    Id,
    Ids,
    Interface,
    KeyedRef,
    KeyedRefMut,
    NaclacAccount,
    NaclacZeroCopy,
    Owner,
    Program,
    Signer,
    Span,
    System,
    ToAccountInfo,
    ToAddress,
    ToCpiHandle,
    ToCpiHandleMut,
    Token,
    Token2022,
    TokenInterface,
    ZcString,
};

pub type ZcVec<T> = Span<T>;

pub use crate::system_program::{CreateAccountAccounts, SystemTransferAccounts};

// ---------------------------------------------------------------------------
// CPI Stack-Allocation Limits
//
// These constants control the maximum number of PDA signers and seeds that
// the generated CPI helpers will support via stack-allocated arrays.
// They are intentionally conservative to keep stack usage well under the
// Solana 4 KB limit.
//
// ⚠️  WARNING: Raising these values increases stack frame size.
//     Each additional signer costs `MAX_CPI_SEEDS_PER_SIGNER * 16` bytes.
//     Do NOT exceed values that would push total stack usage past 4 KB.
//
// To override globally in your crate, re-export your own constants from a
// prelude wrapper before including naclac_lang.
// ---------------------------------------------------------------------------

/// Maximum number of PDA signers supported per CPI call.
/// Default: 4. Each signer holds up to `MAX_CPI_SEEDS_PER_SIGNER` seeds.
pub const MAX_CPI_SIGNERS: usize = 4;

/// Maximum number of seeds per PDA signer in a CPI call.
/// Default: 16. Solana itself enforces a limit of 16 seeds per PDA.
pub const MAX_CPI_SEEDS_PER_SIGNER: usize = 16;

// Solana-only wrapper (the real `Account<T>` struct only exists in Borsh mode;
// zero-copy modes get `Account<T>` via the `AccountLoader<T>` type aliases below).
#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub use crate::wrappers::Account;

// `ToAccountInfos` is implemented for both `Account<T>` (Borsh) and
// `AccountLoader<T>` (Solana zero-copy) — only absent under pinocchio, where
// the trait itself has no `to_account_infos` method to begin with.
#[cfg(not(feature = "pinocchio"))]
pub use crate::wrappers::ToAccountInfos;

#[cfg(feature = "pinocchio")]
pub type Account<T> = AccountLoader<T>;

// Same alias for "solana zero-copy" (not pinocchio, not borsh) — otherwise
// this configuration has no `Account<T>` at all, forcing `AccountLoader<T>`
// to be spelled out explicitly unlike every other backend.
#[cfg(all(not(feature = "pinocchio"), not(feature = "borsh")))]
pub type Account<T> = AccountLoader<T>;

// Pinocchio-only: AccountView is already exported from the pinocchio block below (line ~159)
// Do NOT re-export it from wrappers here — that creates a circular private import

// --- Module Routing (core vs std) ---
#[cfg(any(feature = "pinocchio", feature = "no-std"))]
pub use core::fmt;

#[cfg(not(any(feature = "pinocchio", feature = "no-std")))]
pub use std::fmt;

// --- Alloc Routing (Vec, String, etc.) ---
// In no_std environments (Pinocchio), alloc must be linked explicitly.
// The full entrypoint path calls pinocchio::default_allocator!() which registers one.
#[cfg(feature = "pinocchio")]
extern crate alloc;

#[cfg(feature = "no-std")]
pub use alloc::{boxed::Box, format, string::String, string::ToString, vec, vec::Vec};

#[cfg(not(any(feature = "no-std", feature = "pinocchio")))]
pub use std::{boxed::Box, format, string::String, string::ToString, vec, vec::Vec};

#[cfg(feature = "pinocchio")]
pub use alloc::{boxed::Box, format, string::String, string::ToString, vec, vec::Vec};

// --- IO Routing ---
#[cfg(all(feature = "no-std", not(feature = "pinocchio")))]
pub use borsh::io;

#[cfg(not(any(feature = "pinocchio", feature = "no-std")))]
pub use std::io;

// --- Bytemuck (both backends need this for zero-copy) ---
pub use crate::bytemuck;
pub use crate::bytemuck::{Pod, Zeroable};

// ===========================================================================
// SOLANA-PROGRAM BACKEND  (feature = "solana")
// ===========================================================================
#[cfg(not(feature = "pinocchio"))]
pub use solana_program::{
    clock::Clock,
    entrypoint::ProgramResult,
    log::sol_log_data,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    sysvar::rent::Rent,
    sysvar::rent::ID as RENT_ID,
    sysvar::Sysvar,
};

#[cfg(not(feature = "pinocchio"))]
pub use solana_system_interface::program::ID as SYSTEM_PROGRAM_ID;

#[cfg(not(feature = "pinocchio"))]
pub use solana_program::msg;

#[cfg(not(feature = "pinocchio"))]
pub type NaclacResult<T = (), E = solana_program::program_error::ProgramError> =
    core::result::Result<T, E>;
#[cfg(not(feature = "pinocchio"))]
pub use NaclacResult as Result;

#[cfg(not(feature = "pinocchio"))]
pub use solana_address::Address;

// --- OFFICIAL PROGRAM IDS ---
#[cfg(not(feature = "pinocchio"))]
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Address =
    solana_address::address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
#[cfg(not(feature = "pinocchio"))]
pub const TOKEN_PROGRAM_ID: Address =
    solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
#[cfg(not(feature = "pinocchio"))]
pub const TOKEN_2022_PROGRAM_ID: Address =
    solana_address::address!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub use crate::borsh;

#[cfg(not(feature = "pinocchio"))]
pub use crate::system_program;
#[cfg(not(feature = "pinocchio"))]
#[cfg(not(feature = "pinocchio"))]
pub use solana_system_interface;

#[cfg(not(feature = "pinocchio"))]
#[derive(Clone)]
#[repr(C)]
pub struct AccountInfo {
    pub key: &'static Address,
    pub lamports: std::rc::Rc<core::cell::RefCell<&'static mut u64>>,
    pub data: std::rc::Rc<core::cell::RefCell<&'static mut [u8]>>,
    pub owner: &'static Address,
    pub _unused: u64,
    pub is_signer: bool,
    pub is_writable: bool,
    pub executable: bool,
}

#[cfg(not(feature = "pinocchio"))]
impl AccountInfo {
    pub fn data_is_empty(&self) -> bool {
        self.data.borrow().is_empty()
    }

    pub fn try_borrow_data(
        &self,
    ) -> Result<core::cell::Ref<'_, [u8]>, solana_program::program_error::ProgramError> {
        self.data
            .try_borrow()
            .map(|r| core::cell::Ref::map(r, |d| &**d))
            .map_err(|_| solana_program::program_error::ProgramError::AccountBorrowFailed)
    }

    pub fn try_borrow_mut_data(
        &self,
    ) -> Result<core::cell::RefMut<'_, [u8]>, solana_program::program_error::ProgramError> {
        self.data
            .try_borrow_mut()
            .map(|r| core::cell::RefMut::map(r, |d| &mut **d))
            .map_err(|_| solana_program::program_error::ProgramError::AccountBorrowFailed)
    }

    /// # Safety
    ///
    /// This function transmutes an `AccountInfo` with a specific lifetime into a lifetime-erased representation.
    /// The caller must ensure that the source lifetime is valid for the duration of the erased type's usage.
    #[inline(always)]
    pub unsafe fn from_lifetime<'info>(
        info: solana_program::account_info::AccountInfo<'info>,
    ) -> Self {
        core::mem::transmute(info)
    }

    /// # Safety
    ///
    /// This function transmutes the lifetime-erased `AccountInfo` back to a specific lifetime `'info`.
    /// The caller must ensure that the lifetime `'info` is valid for the lifetime-erased representation.
    #[inline(always)]
    pub unsafe fn to_lifetime<'info>(&self) -> solana_program::account_info::AccountInfo<'info> {
        core::mem::transmute(self.clone())
    }

    pub fn sub_lamports(&self, amount: u64) -> Result<()> {
        let mut lamports = self
            .lamports
            .try_borrow_mut()
            .map_err(|_| solana_program::program_error::ProgramError::AccountBorrowFailed)?;
        **lamports = lamports
            .checked_sub(amount)
            .ok_or(solana_program::program_error::ProgramError::InsufficientFunds)?;
        Ok(())
    }

    pub fn add_lamports(&self, amount: u64) -> Result<()> {
        let mut lamports = self
            .lamports
            .try_borrow_mut()
            .map_err(|_| solana_program::program_error::ProgramError::AccountBorrowFailed)?;
        **lamports = lamports
            .checked_add(amount)
            .ok_or(solana_program::program_error::ProgramError::InvalidArgument)?;
        Ok(())
    }

    pub fn lamports(&self) -> u64 {
        **self.lamports.borrow()
    }

    pub fn assign(&self, new_owner: &Address) {
        unsafe {
            let solana_info = self.to_lifetime();
            solana_info.assign(core::mem::transmute::<
                &Address,
                &solana_program::pubkey::Pubkey,
            >(new_owner));
        }
    }

    pub fn address(&self) -> Address {
        *self.key
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub use crate::borsh::{BorshDeserialize, BorshSerialize};

/// Helper: get unix timestamp from the Clock sysvar (solana_program only)
#[inline(always)]
#[cfg(not(feature = "pinocchio"))]
pub fn unix_timestamp() -> Result<i64> {
    Clock::get().map(|c| c.unix_timestamp)
}

/// Helper: get unix timestamp from the Clock sysvar (pinocchio only)
#[inline(always)]
#[cfg(feature = "pinocchio")]
pub fn unix_timestamp() -> Result<i64> {
    Ok(Clock::get()?.unix_timestamp)
}

// ===========================================================================
// PINOCCHIO BACKEND  (feature = "pinocchio")
// ===========================================================================
#[cfg(feature = "pinocchio")]
pub extern crate pinocchio;

#[cfg(feature = "pinocchio")]
pub use pinocchio::{
    error::ProgramError,
    instruction,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    AccountView, Address as PinocchioAddress, ProgramResult,
};

pub use crate::cursor::{mut_mask_set_bit, AccountBitvec};
#[cfg(feature = "pinocchio")]
pub use crate::cursor::{parse_ix_context, AccountCursor};

#[cfg(feature = "pinocchio")]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Address(pub [u8; 32]);

#[cfg(feature = "pinocchio")]
impl From<PinocchioAddress> for Address {
    #[inline(always)]
    fn from(address: PinocchioAddress) -> Self {
        // SAFETY: PinocchioAddress is [u8; 32], same as Address([u8; 32]).
        // ptr::copy_nonoverlapping of exactly 32 bytes lets LLVM inline
        // 4 × u64 register stores instead of emitting a sol_memcpy_ call.
        unsafe { core::mem::transmute(address) }
    }
}

#[cfg(feature = "pinocchio")]
unsafe impl bytemuck::Zeroable for Address {}
#[cfg(feature = "pinocchio")]
unsafe impl bytemuck::Pod for Address {}
#[cfg(feature = "pinocchio")]
unsafe impl bytemuck::ZeroableInOption for Address {}
#[cfg(feature = "pinocchio")]
unsafe impl bytemuck::PodInOption for Address {}

/// Trait for Naclac-compatible POD types (including Option<T> support)
pub trait NaclacPod: Sized {
    fn naclac_from_bytes(data: &[u8]) -> Self;
    fn naclac_size() -> usize;
}

#[macro_export]
macro_rules! impl_naclac_pod {
    ($($t:ty),*) => {
        $(
            impl $crate::prelude::NaclacPod for $t {
                #[inline(always)]
                fn naclac_from_bytes(data: &[u8]) -> Self {
                    // SAFETY: We expect the data to be the correct size and valid for the type.
                    // We use read_unaligned to avoid alignment issues in zero-copy buffers.
                    unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) }
                }
                #[inline(always)]
                fn naclac_size() -> usize {
                    core::mem::size_of::<$t>()
                }
            }
        )*
    };
}
pub use impl_naclac_pod;

impl<T: NaclacPod> NaclacPod for Option<T> {
    #[inline(always)]
    fn naclac_from_bytes(data: &[u8]) -> Self {
        if data[0] == 0 {
            None
        } else {
            Some(T::naclac_from_bytes(&data[1..1 + T::naclac_size()]))
        }
    }
    #[inline(always)]
    fn naclac_size() -> usize {
        1 + T::naclac_size()
    }
}

impl_naclac_pod!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, Address, [u8; 32], [u8; 64]);

#[cfg(feature = "pinocchio")]
impl core::ops::Deref for Address {
    type Target = [u8; 32];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "pinocchio")]
impl core::fmt::Debug for Address {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Address(")?;
        for byte in self.0.iter() {
            write!(f, "{:02x}", byte)?;
        }
        write!(f, ")")
    }
}

#[cfg(feature = "pinocchio")]
impl Address {
    pub const fn new_from_array(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// For compatibility with Solana-style code expecting .to_bytes()
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    /// Convert from a pinocchio `Address`.
    /// SAFETY: `Address` is `#[repr(transparent)]` over `[u8; 32]`.
    #[inline(always)]
    pub fn from_address(addr: &PinocchioAddress) -> Self {
        // SAFETY: Address is #[repr(transparent)] over [u8; 32].
        // We use a pointer read here to ensure no alignment issues during the cast.
        Self(unsafe { *(addr as *const PinocchioAddress as *const [u8; 32]) })
    }

    /// Borrow as a pinocchio `Address` reference.
    /// SAFETY: `Address` is `#[repr(transparent)]` over `[u8; 32]`.
    #[inline(always)]
    pub fn as_address(&self) -> &PinocchioAddress {
        // SAFETY: Address is #[repr(transparent)] over [u8; 32], representing a 32-byte account address.
        unsafe { &*(self.0.as_ptr() as *const PinocchioAddress) }
    }
}

#[cfg(feature = "pinocchio")]
impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

// SYSTEM_PROGRAM_ID is all zeros — system program address
#[cfg(feature = "pinocchio")]
// SAFETY: pinocchio_system::ID is an `Address` (a transparent [u8; 32] wrapper) which is memory-identical to our `Address`.
pub const SYSTEM_PROGRAM_ID: Address = unsafe { core::mem::transmute(pinocchio_system::ID) };

#[cfg(feature = "pinocchio")]
pub use pinocchio_system;

#[cfg(feature = "pinocchio")]
pub struct RefMut<'a> {
    slice: &'a mut [u8],
}

#[cfg(feature = "pinocchio")]
impl<'a> core::ops::Deref for RefMut<'a> {
    type Target = [u8];
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

#[cfg(feature = "pinocchio")]
impl<'a> core::ops::DerefMut for RefMut<'a> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.slice
    }
}

#[cfg(feature = "pinocchio")]
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct AccountInfo {
    pub view: AccountView,
}

#[cfg(feature = "pinocchio")]
impl core::ops::Deref for AccountInfo {
    type Target = AccountView;
    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

#[cfg(feature = "pinocchio")]
impl AccountInfo {
    pub fn data_is_empty(&self) -> bool {
        self.view.data_len() == 0
    }

    pub fn data(&self) -> &[u8] {
        // SAFETY: The AccountView provides a valid data pointer and length from the Solana runtime.
        unsafe { core::slice::from_raw_parts(self.view.data_ptr(), self.view.data_len()) }
    }

    pub fn owner(&self) -> Address {
        Address::from_address(self.view.owner())
    }

    pub fn lamports(&self) -> u64 {
        self.view.lamports()
    }

    pub fn is_signer(&self) -> bool {
        self.view.is_signer()
    }

    pub fn is_writable(&self) -> bool {
        self.view.is_writable()
    }

    pub fn is_executable(&self) -> bool {
        self.view.executable()
    }

    pub fn try_borrow_data(&self) -> Result<&[u8]> {
        Ok(self.data())
    }

    pub fn try_borrow_mut_data(&self) -> Result<RefMut<'_>> {
        let view = self.view;
        // SAFETY: The AccountView provides a valid mutable data pointer and length from the Solana runtime.
        let slice =
            unsafe { core::slice::from_raw_parts_mut(view.data_ptr() as *mut u8, view.data_len()) };
        Ok(RefMut { slice })
    }

    pub fn sub_lamports(&self, amount: u64) -> Result<()> {
        let mut view = self.view;
        let new_lamports = view
            .lamports()
            .checked_sub(amount)
            .ok_or(pinocchio::error::ProgramError::InsufficientFunds)?;
        view.set_lamports(new_lamports);
        Ok(())
    }

    pub fn add_lamports(&self, amount: u64) -> Result<()> {
        let mut view = self.view;
        let new_lamports = view
            .lamports()
            .checked_add(amount)
            .ok_or(pinocchio::error::ProgramError::InvalidArgument)?;
        view.set_lamports(new_lamports);
        Ok(())
    }

    pub fn address(&self) -> Address {
        Address::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
pub type NaclacResult<T = (), E = pinocchio::error::ProgramError> = core::result::Result<T, E>;
#[cfg(feature = "pinocchio")]
pub use NaclacResult as Result;

#[cfg(feature = "pinocchio")]
pub fn next_account_info<'a, I: Iterator<Item = &'a AccountInfo>>(
    iter: &mut I,
) -> core::result::Result<&'a AccountInfo, pinocchio::error::ProgramError> {
    iter.next()
        .ok_or(pinocchio::error::ProgramError::NotEnoughAccountKeys)
}

#[cfg(not(feature = "pinocchio"))]
pub fn next_account_info<'a, I: Iterator<Item = &'a AccountInfo>>(
    iter: &mut I,
) -> core::result::Result<&'a AccountInfo, solana_program::program_error::ProgramError> {
    iter.next()
        .ok_or(solana_program::program_error::ProgramError::NotEnoughAccountKeys)
}

#[cfg(feature = "pinocchio")]
#[allow(unused_variables)]
pub fn sol_log_data(data: &[&[u8]]) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    // SAFETY: sol_log_data is a native Solana SBF syscall. The runtime guarantees it handles pointer boundaries securely.
    unsafe {
        extern "C" {
            fn sol_log_data(data: *const u8, data_len: u64);
        }
        sol_log_data(data.as_ptr() as *const u8, data.len() as u64);
    }
}

#[cfg(feature = "pinocchio")]
pub fn sol_log_compute_units() {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    unsafe {
        extern "C" {
            fn sol_log_compute_units_();
        }
        sol_log_compute_units_();
    }
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        // No-op on non-Solana targets
    }
}

// --- OFFICIAL PROGRAM IDS (PINOCCHIO) ---
#[cfg(feature = "pinocchio")]
pub const TOKEN_PROGRAM_ID: Address = Address([
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
]);
#[cfg(feature = "pinocchio")]
pub const TOKEN_2022_PROGRAM_ID: Address = Address([
    6, 221, 246, 225, 238, 117, 143, 222, 24, 66, 93, 188, 228, 108, 205, 218, 182, 26, 252, 77,
    131, 185, 13, 39, 254, 189, 249, 40, 216, 161, 139, 252,
]);
#[cfg(feature = "pinocchio")]
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Address = Address([
    140, 151, 37, 143, 78, 36, 137, 241, 187, 61, 16, 41, 20, 142, 13, 131, 11, 90, 19, 153, 218,
    255, 16, 132, 4, 142, 123, 216, 219, 233, 248, 89,
]);

#[cfg(not(feature = "pinocchio"))]
pub type AccountType = AccountInfo;

#[cfg(feature = "pinocchio")]
pub type AccountType = AccountInfo;

/// Re-export the pinocchio msg! macro into the prelude namespace
/// so `use naclac_lang::prelude::*` makes `msg!` available.
#[cfg(feature = "pinocchio")]
pub use crate::msg;

/// Pinocchio logging — bridges Naclac's `msg!` API to pinocchio_log's Logger.
/// Uses a 128-byte stack-allocated buffer. For longer messages use Logger directly.
#[cfg(all(feature = "pinocchio", feature = "debug-mode"))]
#[macro_export]
macro_rules! msg {
    ($msg:expr) => {{
        let mut logger = $crate::pinocchio_log::logger::Logger::<128>::default();
        logger.append($msg);
        logger.log();
    }};
}

/// No-op msg! macro for non-debug builds to save space
#[cfg(not(feature = "debug-mode"))]
#[macro_export]
macro_rules! msg {
    ($($arg:tt)*) => {{}};
}

#[cfg(feature = "pinocchio")]
pub use crate::system_program;

// ===========================================================================
// Shared macros (both backends)
// ===========================================================================

/// Anchor-style event emission macro.
/// Handles both `field: value` and shorthand `field` syntax.
/// Uses mutation pattern so internal padding fields are invisible to users.
#[macro_export]
macro_rules! emit {
    // Main arm: handles mixed shorthand + explicit fields
    ($ty:path { $($field:ident $(: $val:expr)?),* $(,)? }) => {{
        let mut __event = <$ty as core::default::Default>::default();
        $(
            emit!(@__set __event, $field $(, $val)?);
        )*
        __event.emit();
    }};
    // Internal helper: explicit field: value
    (@__set $ev:ident, $field:ident, $val:expr) => {
        $ev.$field = ($val).into();
    };
    // Internal helper: shorthand field (uses local variable of same name)
    (@__set $ev:ident, $field:ident) => {
        $ev.$field = ($field).into();
    };
    // Pass-through: emit!(my_event_instance)
    ($event:expr) => {
        ($event).emit();
    };
}

/// Concise condition checking that returns a ProgramError.
/// Usage: `require!(condition, MyError::SomeVariant)`
#[macro_export]
macro_rules! require {
    ($cond:expr, $err:expr $(,)?) => {
        if !($cond) {
            return Err(($err).into());
        }
    };
}

pub use crate::{declare_id, emit, require};
pub use naclac_macros::{NaclacDeserialize, NaclacSerialize};

/// Declares the program's static ID constant.
///
/// - Solana backend: delegates to `solana_program::declare_id!`
/// - Pinocchio backend: creates a `pub const ID: Address` using pinocchio's
///   compile-time address parsing.
#[macro_export]
macro_rules! declare_id {
    ($id:expr) => {
        #[cfg(not(feature = "pinocchio"))]
        $crate::solana_program::declare_id!($id);

        #[cfg(feature = "pinocchio")]
        pub const ID: $crate::prelude::Address =
            unsafe { core::mem::transmute($crate::pinocchio::address::address!($id)) };
        #[cfg(feature = "pinocchio")]
        pub fn id() -> $crate::prelude::Address {
            ID
        }
    };
}

// --- Zero-Copy Helper Types ---

/// A zero-copy compatible boolean wrapper.
#[repr(transparent)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    bytemuck::Pod,
    bytemuck::Zeroable,
)]
pub struct Bool(pub u8);

impl From<u8> for Bool {
    fn from(b: u8) -> Self {
        Self(b)
    }
}

impl From<bool> for Bool {
    fn from(b: bool) -> Self {
        Self(if b { 1 } else { 0 })
    }
}

impl From<Bool> for bool {
    fn from(b: Bool) -> Self {
        b.0 != 0
    }
}

/// A zero-copy compatible Option wrapper.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Opt<T: bytemuck::Pod> {
    pub value: T,
    pub has_value: u8,
    pub _padding: [u8; 7], // Ensure alignment for most types (u64, etc)
}

unsafe impl<T: bytemuck::Pod> bytemuck::Zeroable for Opt<T> {}
unsafe impl<T: bytemuck::Pod> bytemuck::Pod for Opt<T> {}

impl<T: bytemuck::Pod> Opt<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            has_value: 1,
            _padding: [0; 7],
        }
    }
    pub fn none(default: T) -> Self {
        Self {
            value: default,
            has_value: 0,
            _padding: [0; 7],
        }
    }
}

impl<T: bytemuck::Pod> From<Option<T>> for Opt<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => Self::new(v),
            None => Self {
                value: unsafe { core::mem::zeroed() },
                has_value: 0,
                _padding: [0; 7],
            },
        }
    }
}
