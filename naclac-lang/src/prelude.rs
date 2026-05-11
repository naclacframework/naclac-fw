// ===========================================================================
// prelude.rs — Naclac unified prelude
//
// Feature routing:
//   default ("solana")  → solana_program + borsh backend (std, lifetimes)
//   "pinocchio"         → pinocchio backend (no_std, no lifetimes, zero-copy)
//
// The two backends are mutually exclusive. Enabling "pinocchio" hides every
// solana_program symbol and replaces it with the pinocchio equivalents.
// ===========================================================================

pub use crate::context::{Bumps, Context};
pub use crate::cpi::{CpiContext, AccountMeta, ToAccountMetas};
pub use naclac_macros::*;

// --- Wrappers: common types (shared by both backends) ---
pub use crate::error::NaclacError;

// These traits/types exist in wrappers for BOTH Solana and Pinocchio backends
pub use crate::wrappers::{
    Owner, Key, Discriminator, AsRefByteSlice, ToPubkey, ToAccountInfo,
    Span, ZcString, KeyedRef, KeyedRefMut, NaclacZeroCopy,
    // Framework account wrapper types — abstract over both backends
    AccountLoader, Signer, Program, System, Interface, TokenInterface
};

pub type ZcVec<'info, T> = Span<'info, T>;

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

// Solana-only wrappers (require std / borsh / spl, not available in Pinocchio)
#[cfg(not(feature = "pinocchio"))]
pub use crate::wrappers::{
    Account, ToAccountInfos, Token, Token2022,
};

// Pinocchio-only: AccountView is already exported from the pinocchio block below (line ~159)
// Do NOT re-export it from wrappers here — that creates a circular private import

// --- Module Routing (core vs std) ---
#[cfg(any(feature = "pinocchio", feature = "no-std"))]
pub use core::fmt;

#[cfg(not(any(feature = "pinocchio", feature = "no-std")))]
pub use std::fmt;

// --- Alloc Routing (Vec, String, etc.) ---
// In no_std environments (Pinocchio), alloc must be linked explicitly
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

// --- SPL Token modules ---
#[cfg(any(feature = "spl-token", feature = "pinocchio-token"))]
#[allow(deprecated)]
pub use crate::token::{
    self, Approve, Burn, CloseAccount, FreezeAccount, InitializeAccount, InitializeMint, Mint,
    MintTo, Revoke, SetAuthority, ThawAccount, TokenAccount, Transfer,
};

#[cfg(all(feature = "spl-associated-token-account", not(feature = "pinocchio")))]
pub use crate::associated_token::{self, Create as CreateAta};

// --- Bytemuck (both backends need this for zero-copy) ---
pub use crate::bytemuck;
pub use crate::bytemuck::{Pod, Zeroable};

// --- SPL extern crates ---
#[cfg(all(feature = "spl-associated-token-account", not(feature = "pinocchio")))]
pub extern crate spl_associated_token_account;
#[cfg(all(feature = "spl-token", not(feature = "pinocchio")))]
pub extern crate spl_token;
#[cfg(all(feature = "spl-token", not(feature = "pinocchio")))]
#[allow(deprecated)]
pub extern crate spl_token_2022;

// ===========================================================================
// SOLANA-PROGRAM BACKEND  (feature = "solana")
// ===========================================================================
#[cfg(not(feature = "pinocchio"))]
pub use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    log::sol_log_data,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_program::ID as SYSTEM_PROGRAM_ID,
    sysvar::rent::Rent,
    sysvar::rent::ID as RENT_ID,
    sysvar::Sysvar,
};

#[cfg(not(feature = "pinocchio"))]
pub use solana_program::msg;

#[cfg(not(feature = "pinocchio"))]
pub type NaclacResult<T = (), E = solana_program::program_error::ProgramError> =
    core::result::Result<T, E>;
#[cfg(not(feature = "pinocchio"))]
pub use NaclacResult as Result;

// --- OFFICIAL PROGRAM IDS ---
#[cfg(all(not(feature = "pinocchio"), feature = "spl-associated-token-account"))]
pub use spl_associated_token_account::ID as ASSOCIATED_TOKEN_PROGRAM_ID;
#[cfg(all(not(feature = "pinocchio"), feature = "spl-token"))]
pub use spl_token::ID as TOKEN_PROGRAM_ID;
#[cfg(all(not(feature = "pinocchio"), feature = "spl-token"))]
pub use spl_token_2022::ID as TOKEN_2022_PROGRAM_ID;

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub use crate::borsh;

#[cfg(not(feature = "pinocchio"))]
pub use crate::system_program;
#[cfg(not(feature = "pinocchio"))]
pub use solana_system_interface;

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
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    AccountView,
    Address,
    ProgramResult,
    cpi,
    instruction,
};

#[cfg(feature = "pinocchio")]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Pubkey(pub [u8; 32]);

#[cfg(feature = "pinocchio")]
impl Default for Pubkey {
    fn default() -> Self {
        Self([0u8; 32])
    }
}

#[cfg(feature = "pinocchio")]
impl From<Address> for Pubkey {
    fn from(address: Address) -> Self {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(address.as_ref());
        Self(bytes)
    }
}

#[cfg(feature = "pinocchio")]
unsafe impl bytemuck::Zeroable for Pubkey {}
#[cfg(feature = "pinocchio")]
unsafe impl bytemuck::Pod for Pubkey {}
#[cfg(feature = "pinocchio")]
unsafe impl bytemuck::ZeroableInOption for Pubkey {}
#[cfg(feature = "pinocchio")]
unsafe impl bytemuck::PodInOption for Pubkey {}

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

impl_naclac_pod!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, Pubkey, [u8; 32], [u8; 64]);

#[cfg(feature = "pinocchio")]
impl core::ops::Deref for Pubkey {
    type Target = [u8; 32];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "pinocchio")]
impl core::fmt::Debug for Pubkey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Pubkey(")?;
        for byte in self.0.iter() {
            write!(f, "{:02x}", byte)?;
        }
        write!(f, ")")
    }
}

#[cfg(feature = "pinocchio")]
impl Pubkey {
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
    pub fn from_address(addr: &Address) -> Self {
        // SAFETY: Address is #[repr(transparent)] over [u8; 32].
        // We use a pointer read here to ensure no alignment issues during the cast.
        Self(unsafe { *(addr as *const Address as *const [u8; 32]) })
    }

    /// Borrow as a pinocchio `Address` reference.
    /// SAFETY: `Address` is `#[repr(transparent)]` over `[u8; 32]`.
    #[inline(always)]
    pub fn as_address(&self) -> &Address {
        // SAFETY: Address is #[repr(transparent)] over [u8; 32], same as Pubkey.
        unsafe { &*(self.0.as_ptr() as *const Address) }
    }

    pub fn find_program_address(seeds: &[&[u8]], program_id: &Pubkey) -> (Self, u8) {
        #[cfg(any(target_os = "solana", target_arch = "bpf"))]
        {
            let (addr, bump) = Address::find_program_address(seeds, program_id.as_address());
            (Self::from_address(&addr), bump)
        }
        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unreachable!("PDA derivation requires the Solana SBF runtime")
    }

    pub fn create_program_address(seeds: &[&[u8]], program_id: &Pubkey) -> Result<Self> {
        #[cfg(any(target_os = "solana", target_arch = "bpf"))]
        {
            Address::create_program_address(seeds, program_id.as_address())
                .map(|addr| Self::from_address(&addr))
                .map_err(|e| e.into())
        }
        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unreachable!("PDA derivation requires the Solana SBF runtime")
    }
}

#[cfg(feature = "pinocchio")]
impl AsRef<[u8]> for Pubkey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

// SYSTEM_PROGRAM_ID is all zeros — system program address
#[cfg(feature = "pinocchio")]
// SAFETY: pinocchio_system::ID is an `Address` (a transparent [u8; 32] wrapper) which is memory-identical to our `Pubkey`.
pub const SYSTEM_PROGRAM_ID: Pubkey = unsafe { core::mem::transmute(pinocchio_system::ID) };

#[cfg(all(feature = "pinocchio", feature = "pinocchio-ata"))]
pub use pinocchio_associated_token_account;
#[cfg(feature = "pinocchio")]
pub use pinocchio_system;
#[cfg(all(feature = "pinocchio", feature = "pinocchio-token"))]
pub use pinocchio_token;
#[cfg(all(feature = "pinocchio", feature = "pinocchio-token"))]
pub use pinocchio_token_2022;

#[cfg(feature = "pinocchio")]
#[derive(Clone)]
#[repr(transparent)]
pub struct AccountInfo<'info> {
    pub view: AccountView,
    pub _phantom: core::marker::PhantomData<&'info ()>,
}

#[cfg(feature = "pinocchio")]
impl<'info> core::ops::Deref for AccountInfo<'info> {
    type Target = AccountView;
    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

#[cfg(feature = "pinocchio")]
impl<'info> AccountInfo<'info> {
    pub fn data_is_empty(&self) -> bool {
        self.view.data_len() == 0
    }

    pub fn data(&self) -> &[u8] {
        // SAFETY: The AccountView provides a valid data pointer and length from the Solana runtime.
        unsafe { core::slice::from_raw_parts(self.view.data_ptr(), self.view.data_len()) }
    }

    pub fn data_mut(&self) -> &mut [u8] {
        let view = self.view.clone();
        // SAFETY: The AccountView provides a valid mutable data pointer and length from the Solana runtime.
        unsafe { core::slice::from_raw_parts_mut(view.data_ptr() as *mut u8, view.data_len()) }
    }

    pub fn key(&self) -> Pubkey {
        Pubkey::from_address(self.view.address())
    }

    pub fn owner(&self) -> Pubkey {
        // SAFETY: The AccountView owner call is a direct sysvar/SBF call provided by Pinocchio.
        Pubkey::from_address(unsafe { self.view.owner() })
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

    pub fn try_borrow_mut_data(&self) -> Result<&mut [u8]> {
        Ok(self.data_mut())
    }
}

#[cfg(feature = "pinocchio")]
pub type NaclacResult<T = (), E = pinocchio::error::ProgramError> = core::result::Result<T, E>;
#[cfg(feature = "pinocchio")]
pub use NaclacResult as Result;

#[cfg(feature = "pinocchio")]
pub fn next_account_info<'a, 'b: 'a, I: Iterator<Item = &'a AccountInfo<'b>>>(
    iter: &mut I,
) -> core::result::Result<&'a AccountInfo<'b>, pinocchio::error::ProgramError> {
    iter.next()
        .ok_or(pinocchio::error::ProgramError::NotEnoughAccountKeys)
}

#[cfg(feature = "pinocchio")]
pub fn sol_log_data(data: &[&[u8]]) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    // SAFETY: sol_log_data is a native Solana SBF syscall. The runtime guarantees it handles pointer boundaries securely.
    unsafe {
        extern "C" {
            fn sol_log_data(data: *const u8, data_len: u64);
        }
        sol_log_data(data.as_ptr() as *const u8, data.len() as u64);
    }
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        // No-op on non-Solana targets
    }
}

// --- OFFICIAL PROGRAM IDS (PINOCCHIO) ---
// SAFETY: All pinocchio program IDs are `Address` types (`#[repr(transparent)]` over `[u8; 32]`), which is memory-identical to our `Pubkey`.
#[cfg(all(feature = "pinocchio", feature = "pinocchio-token"))]
pub const TOKEN_PROGRAM_ID: Pubkey = unsafe { core::mem::transmute(pinocchio_token::ID) };
#[cfg(all(feature = "pinocchio", feature = "pinocchio-token"))]
pub const TOKEN_2022_PROGRAM_ID: Pubkey = unsafe { core::mem::transmute(pinocchio_token_2022::ID) };
#[cfg(all(feature = "pinocchio", feature = "pinocchio-ata"))]
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    unsafe { core::mem::transmute(pinocchio_associated_token_account::ID) };

#[cfg(not(feature = "pinocchio"))]
pub type AccountType<'info> = AccountInfo<'info>;

#[cfg(feature = "pinocchio")]
pub type AccountType<'info> = AccountInfo<'info>;

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
        let mut logger = ::naclac_lang::pinocchio_log::logger::Logger::<128>::default();
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
        pub const ID: $crate::prelude::Pubkey =
            unsafe { core::mem::transmute($crate::pinocchio::address::address!($id)) };
        #[cfg(feature = "pinocchio")]
        pub fn id() -> $crate::prelude::Pubkey {
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
