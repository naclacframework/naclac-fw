// ===========================================================================
// prelude.rs — Naclac unified prelude
//
// Feature routing:
//   default ("solana")       → solana_program backend (std, lifetimes) — NO borsh by default
//   "solana" + "borsh"       → solana_program + borsh serialization (Account<T>, #[component])
//   "zero-copy"              → Account<T> zero-copy branch, zero-copy events via bytemuck::Pod
//   "pinocchio"              → pinocchio backend (no_std, no lifetimes, zero-copy)
//
// The two backends are mutually exclusive. Enabling "pinocchio" hides every
// solana_program symbol and replaces it with the pinocchio equivalents.
// ===========================================================================

pub use crate::context::{Bumps, Context, LoadableAccounts, ValidationResult};
#[cfg(feature = "pinocchio")]
pub use crate::cpi::{invoke_signed_pinocchio, invoke_signed_pinocchio_unchecked};
pub use crate::cpi::{self, AccountMeta, ToAccountMetas};
pub use naclac_macros::*;

// --- Wrappers: common types (shared by both backends) ---
pub use crate::error::NaclacError;

// These traits/types exist in wrappers for BOTH Solana and Pinocchio backends
pub use crate::wrappers::{
    Account, AsRefByteSlice, AssociatedToken, CpiHandle, CpiHandleMut, Discriminator, Id, Ids,
    Interface, InterfaceAccount, NaclacAccount, NaclacZeroCopy, Owner, Program, Signer, Span,
    System, ToAccountInfo, ToAddress, ToCpiHandle, ToCpiHandleMut, Token, Token2022,
    TokenInterface, ValidateInterfaceLayout, ZcString,
};

pub type ZcVec<T> = Span<T>;

pub use crate::system_program::{CreateAccountAccounts, SystemTransferAccounts};

pub use crate::realloc::resize_with_rent;

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

/// Maximum number of accounts the pinocchio-backend CPI helpers
/// (`cpi::invoke_pinocchio`/`invoke_signed_pinocchio_handles`/
/// `invoke_signed_pinocchio` and their `_unchecked` counterparts) will
/// accept in one call — a stack-allocated array bound. Exceeding it is a
/// hard error, not a silent truncation.
pub const MAX_CPI_ACCOUNTS: usize = 32;

// `ToAccountInfos` is implemented for both branches of `Account<T>`/
// `InterfaceAccount<T>` — only absent under pinocchio, where the trait
// itself has no `to_account_infos` method to begin with.
#[cfg(not(feature = "pinocchio"))]
pub use crate::wrappers::ToAccountInfos;

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
#[cfg(any(feature = "no-std", feature = "pinocchio"))]
extern crate alloc;

#[cfg(any(feature = "no-std", feature = "pinocchio"))]
pub use alloc::{boxed::Box, format, string::String, string::ToString, vec, vec::Vec};

#[cfg(not(any(feature = "no-std", feature = "pinocchio")))]
pub use std::{boxed::Box, format, string::String, string::ToString, vec, vec::Vec};

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
pub use solana_program;
#[cfg(not(feature = "pinocchio"))]
pub use solana_program::{
    clock::Clock,
    entrypoint::ProgramResult,
    log::sol_log_data,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    sysvar::rent::Rent,
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
pub use solana_address;
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
#[cfg(not(feature = "pinocchio"))]
pub const RENT_SYSVAR_ID: Address =
    solana_address::address!("SysvarRent111111111111111111111111111111111");

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub use crate::borsh;

#[cfg(not(feature = "pinocchio"))]
pub use crate::base58;
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
            .map_err(|_| crate::error::NaclacError::AccountBorrowFailed.err(0))
    }

    pub fn try_borrow_mut_data(
        &self,
    ) -> Result<core::cell::RefMut<'_, [u8]>, solana_program::program_error::ProgramError> {
        self.data
            .try_borrow_mut()
            .map(|r| core::cell::RefMut::map(r, |d| &mut **d))
            .map_err(|_| crate::error::NaclacError::AccountBorrowFailed.err(0))
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
            .map_err(|_| crate::error::NaclacError::AccountBorrowFailed.err(0))?;
        **lamports = lamports
            .checked_sub(amount)
            .ok_or(crate::error::NaclacError::InsufficientFunds.err(0))?;
        Ok(())
    }

    pub fn add_lamports(&self, amount: u64) -> Result<()> {
        let mut lamports = self
            .lamports
            .try_borrow_mut()
            .map_err(|_| crate::error::NaclacError::AccountBorrowFailed.err(0))?;
        **lamports = lamports
            .checked_add(amount)
            .ok_or(crate::error::NaclacError::ArithmeticOverflow.err(0))?;
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

    /// Resize the account's data, delegating to the real
    /// `solana_program::account_info::AccountInfo::resize` (an inherent
    /// `&self` method backed by unsafe raw-pointer manipulation into
    /// runtime memory) via the same `to_lifetime()` cast already used by
    /// `assign()` above.
    pub fn resize(&self, new_len: usize) -> Result<()> {
        unsafe { self.to_lifetime().resize(new_len) }
            .map_err(|_| crate::error::NaclacError::InvalidRealloc.err(0))
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

/// Real Solana protocol limit on the number of seeds a PDA derivation may
/// use (`solana_program::pubkey::MAX_SEEDS`) — mirrored here, not invented,
/// since `derive_program_address` below needs a fixed-size scratch buffer
/// (no heap allocation, so it works under pinocchio's `no_std`).
pub const MAX_PDA_SEEDS: usize = 16;

/// Derives a PDA address for arbitrary `seeds` under `program_id` at the
/// given `bump`, via a single hash-and-compare (`create_program_address`-
/// equivalent) rather than an on-chain `find_program_address` bump search —
/// naclac never runs the search loop on-chain, so `bump` must already be
/// known by the caller. The one shared implementation every PDA-verifying
/// code path in naclac uses: the `#[account(seeds = [...], bump = ...)]`
/// constraint naclac-macros generates and `associated_token::derive_ata_address`
/// both call this internally rather than each hashing independently. It's
/// also the function hand-written instruction code needs to verify an
/// arbitrary `AccountInfo` — e.g. one of `ctx.remaining_accounts` — against
/// an expected PDA, something no declarative `#[account(...)]` constraint
/// can reach (those only attach to a named struct field).
///
/// Panics if `seeds.len() > MAX_PDA_SEEDS` — the same real protocol limit
/// `find_program_address`/`create_program_address` themselves enforce, not
/// an invented restriction.
pub fn derive_program_address(seeds: &[&[u8]], bump: u8, program_id: &Address) -> Address {
    assert!(
        seeds.len() <= MAX_PDA_SEEDS,
        "derive_program_address: too many seeds (max {MAX_PDA_SEEDS})"
    );

    let bump_arr = [bump];
    let mut scratch: [&[u8]; MAX_PDA_SEEDS + 3] = [&[]; MAX_PDA_SEEDS + 3];
    let mut n = 0;
    for seed in seeds {
        scratch[n] = seed;
        n += 1;
    }
    scratch[n] = &bump_arr[..];
    n += 1;
    scratch[n] = program_id.as_ref();
    n += 1;
    scratch[n] = b"ProgramDerivedAddress";
    n += 1;
    let inputs = &scratch[..n];

    #[cfg(not(feature = "pinocchio"))]
    {
        let hash_result = solana_program::hash::hashv(inputs);
        Address::new_from_array(hash_result.to_bytes())
    }

    #[cfg(feature = "pinocchio")]
    {
        extern "C" {
            fn sol_sha256(vals: *const u8, val_len: u64, hash_result: *mut u8) -> u32;
        }
        let mut hash_result = [0u8; 32];
        // SAFETY: `sol_sha256` is a native Solana SBF syscall; `inputs`
        // outlives the call and each slice element is a valid (ptr, len)
        // pair, matching the syscall's expected array-of-`SolBytes` layout.
        unsafe {
            sol_sha256(inputs.as_ptr() as *const u8, inputs.len() as u64, hash_result.as_mut_ptr());
        }
        Address::new_from_array(hash_result)
    }
}

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

impl_naclac_pod!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, Bool, Address);

// Generic over `N` rather than hand-listed sizes (the macro above only
// covers exact types) — any `[u8; N]` fixed-size instruction arg or event
// field works, not just the specific lengths someone happened to enumerate.
impl<const N: usize> NaclacPod for [u8; N] {
    #[inline(always)]
    fn naclac_from_bytes(data: &[u8]) -> Self {
        // SAFETY: same contract as `impl_naclac_pod!`'s generated impls —
        // `data` is expected to be at least `naclac_size()` bytes, valid for
        // reading `Self`. `read_unaligned` avoids alignment requirements in
        // zero-copy instruction-data buffers.
        unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) }
    }
    #[inline(always)]
    fn naclac_size() -> usize {
        N
    }
}

/// Deserializes an instruction argument from `data` starting at `*offset`,
/// advancing `*offset` past the consumed bytes. Unlike `NaclacPod` (fixed
/// size, known without reading the buffer), this also covers argument types
/// whose encoded length depends on their own content — namely an
/// `#[instruction_args]`-grouped struct containing a `ZcString`/`ZcVec`
/// field, whose macro-generated impl parses its fields one at a time instead
/// of implementing `NaclacPod` (a length-prefixed dynamic field can never be
/// read via a single raw `size_of`-based byte cast). `program.rs`'s
/// instruction dispatch calls this uniformly for every non-collection
/// instruction argument, fixed- or dynamic-size alike.
pub trait NaclacArgs: Sized {
    fn naclac_deserialize(data: &[u8], offset: &mut usize) -> NaclacResult<Self>;
}

impl<T: NaclacPod> NaclacArgs for T {
    #[inline(always)]
    fn naclac_deserialize(data: &[u8], offset: &mut usize) -> NaclacResult<Self> {
        let sz = T::naclac_size();
        if data.len() < *offset + sz {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let val = T::naclac_from_bytes(&data[*offset..*offset + sz]);
        *offset += sz;
        Ok(val)
    }
}

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
            .ok_or(crate::error::NaclacError::InsufficientFunds.err(0))?;
        view.set_lamports(new_lamports);
        Ok(())
    }

    pub fn add_lamports(&self, amount: u64) -> Result<()> {
        let mut view = self.view;
        let new_lamports = view
            .lamports()
            .checked_add(amount)
            .ok_or(crate::error::NaclacError::ArithmeticOverflow.err(0))?;
        view.set_lamports(new_lamports);
        Ok(())
    }

    /// Resize the account's data. `AccountView` is `Copy` (it's just a raw
    /// pointer into runtime memory — see `sub_lamports`/`add_lamports`
    /// above for the same copy-then-mutate pattern), so calling the
    /// `&mut self` `Resize::resize` on a local copy still mutates the real
    /// underlying account.
    pub fn resize(&self, new_len: usize) -> Result<()> {
        let mut view = self.view;
        pinocchio::Resize::resize(&mut view, new_len)
            .map_err(|_| crate::error::NaclacError::InvalidRealloc.err(0))
    }

    pub fn address(&self) -> Address {
        Address::from_address(self.view.address())
    }
}

#[cfg(feature = "pinocchio")]
pub type NaclacResult<T = (), E = pinocchio::error::ProgramError> = core::result::Result<T, E>;
#[cfg(feature = "pinocchio")]
pub use NaclacResult as Result;

/// Generic instruction-return-data serialization for `#[instruction]` handlers
/// declared `-> Result<T>`. `#[program]`'s dispatcher calls `to_return_data()`
/// on `Ok(value)` and passes the bytes to `set_return_data` automatically —
/// handlers never call `set_return_data` themselves. An `Option<T>` outer
/// wrapper (e.g. a handler that may or may not have anything to return) is
/// detected at macro-expansion time in `naclac-macros`, not via a blanket
/// trait impl here, since a blanket `impl<T: Pod> for T` and a blanket
/// `impl<T: NaclacReturnData> for Option<T>` are rejected by Rust's coherence
/// checker as potentially overlapping.
#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
pub trait NaclacReturnData {
    fn to_return_data(&self) -> Vec<u8>;
}

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
impl<T: crate::bytemuck::Pod> NaclacReturnData for T {
    fn to_return_data(&self) -> Vec<u8> {
        crate::bytemuck::bytes_of(self).to_vec()
    }
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
pub trait NaclacReturnData {
    fn to_return_data(&self) -> Vec<u8>;
}

#[cfg(all(feature = "borsh", not(feature = "pinocchio")))]
impl<T: crate::borsh::BorshSerialize> NaclacReturnData for T {
    fn to_return_data(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        T::serialize(self, &mut buf).expect("return data serialization failed");
        buf
    }
}

#[cfg(feature = "pinocchio")]
pub fn next_account_info<'a, I: Iterator<Item = &'a AccountInfo>>(
    iter: &mut I,
) -> core::result::Result<&'a AccountInfo, pinocchio::error::ProgramError> {
    iter.next()
        .ok_or(crate::error::NaclacError::NotEnoughAccountKeys.err(0))
}

#[cfg(not(feature = "pinocchio"))]
pub fn next_account_info<'a, I: Iterator<Item = &'a AccountInfo>>(
    iter: &mut I,
) -> core::result::Result<&'a AccountInfo, solana_program::program_error::ProgramError> {
    iter.next()
        .ok_or(crate::error::NaclacError::NotEnoughAccountKeys.err(0))
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
pub const TOKEN_PROGRAM_ID: Address = unsafe {
    core::mem::transmute(pinocchio::address::address!(
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    ))
};
#[cfg(feature = "pinocchio")]
pub const TOKEN_2022_PROGRAM_ID: Address = unsafe {
    core::mem::transmute(pinocchio::address::address!(
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
    ))
};
#[cfg(feature = "pinocchio")]
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Address = unsafe {
    core::mem::transmute(pinocchio::address::address!(
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
    ))
};
#[cfg(feature = "pinocchio")]
pub const RENT_SYSVAR_ID: Address = unsafe {
    core::mem::transmute(pinocchio::address::address!(
        "SysvarRent111111111111111111111111111111111"
    ))
};

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
#[cfg(feature = "pinocchio")]
#[macro_export]
macro_rules! msg {
    ($msg:expr) => {{
        let mut logger = $crate::pinocchio_log::logger::Logger::<128>::default();
        logger.append($msg);
        logger.log();
    }};
}

/// No-op msg! macro for non-pinocchio, non-debug builds to save space
#[cfg(all(not(feature = "pinocchio"), not(feature = "debug-mode")))]
#[macro_export]
macro_rules! msg {
    ($($arg:tt)*) => {{}};
}

#[cfg(feature = "pinocchio")]
pub use crate::system_program;

#[cfg(feature = "pinocchio")]
pub use crate::base58;

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
        __event
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
    ($event:expr) => {{
        let __event = $event;
        __event.emit();
        __event
    }};
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

pub use crate::{address, declare_id, emit, require};
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

/// Parses a base58 address literal into a `$crate::prelude::Address` constant,
/// usable anywhere (not just a crate's own program ID, unlike `declare_id!`).
/// Resolves the same Pinocchio-vs-Solana backend split `declare_id!` does
/// internally, so callers never need to know `pinocchio::address::address!`
/// returns a different (structurally identical) `Address` type that needs an
/// explicit conversion.
#[macro_export]
macro_rules! address {
    ($id:expr) => {{
        #[cfg(not(feature = "pinocchio"))]
        {
            $crate::solana_address::address!($id)
        }
        #[cfg(feature = "pinocchio")]
        {
            let __addr: $crate::prelude::Address =
                unsafe { core::mem::transmute($crate::pinocchio::address::address!($id)) };
            __addr
        }
    }};
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
