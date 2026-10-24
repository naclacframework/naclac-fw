// ===========================================================================
// cpi.rs — Standardized Cross-Program Invocation (CPI) module
// ===========================================================================

//! # Cross-Program Invocation (CPI) Execution
//!
//! Provides the borrow-checked execution helpers for issuing Cross-Program Invocations.
//! This handles the compilation targets for Solana (via AccountInfo conversion)
//! and Pinocchio (via zero-overhead transmutations).
//!
//! `invoke`/`invoke_signed` (solana) and `invoke_pinocchio`/
//! `invoke_signed_pinocchio_handles`/`invoke_signed_pinocchio` (pinocchio)
//! always resolve to the *checked* path: on solana, [`sdk::invoke_signed`]
//! (the same checks as `solana_cpi::invoke_signed`), which verifies every
//! named account's `AccountInfo` `RefCell` is borrowable before invoking;
//! on pinocchio, the real `solana_instruction_view::cpi::invoke_signed_with_bounds`,
//! which verifies each account's address matches the instruction's expected
//! account at that position and that its borrow state is compatible with the
//! instruction's declared mutability. Both the checked and `_unchecked`
//! pinocchio functions reject (rather than silently truncating) an account
//! list longer than `MAX_CPI_ACCOUNTS` — that bound is a bookkeeping limit,
//! not a borrow-safety one, so skipping it isn't part of what "unchecked"
//! means here. The `_unchecked` variants skip only the borrow/address
//! validation, for callers who have already established the accounts'
//! borrow-safety themselves and want to skip that CU cost — never what
//! naclac's own CPI-caller codegen generates.

/// Unified AccountMeta for cross-backend CPI usage.
#[derive(Clone, Debug)]
pub struct AccountMeta {
    pub address: crate::prelude::Address,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    pub fn new(address: crate::prelude::Address, is_writable: bool) -> Self {
        Self {
            address,
            is_signer: false,
            is_writable,
        }
    }
    pub fn new_readonly(address: crate::prelude::Address, is_signer: bool) -> Self {
        Self {
            address,
            is_signer,
            is_writable: false,
        }
    }
}

pub trait ToAccountMetas {
    fn to_account_metas(&self) -> crate::prelude::Vec<AccountMeta>;
}

pub trait ToAccountInfos {
    fn to_account_infos(&self) -> crate::prelude::Vec<crate::prelude::AccountInfo>;
}

// ===========================================================================
// SOLANA-PROGRAM BACKEND (invoke / invoke_signed)
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
pub fn invoke(
    instruction: &solana_instruction::Instruction,
    account_handles: &[crate::prelude::CpiHandle<'_>],
) -> crate::prelude::Result<()> {
    invoke_signed(instruction, account_handles, &[])
}

#[cfg(not(feature = "pinocchio"))]
pub fn invoke_signed(
    instruction: &solana_instruction::Instruction,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    let mut infos = crate::prelude::Vec::with_capacity(account_handles.len());
    for handle in account_handles {
        unsafe {
            infos.push(handle.info.to_lifetime());
        }
    }
    sdk::invoke_signed(instruction, &infos, signer_seeds)
}

/// Raw, no-borrow-check CPI — bypasses the `RefCell` borrow checks
/// [`sdk::invoke_signed`] performs.
/// See this module's own doc comment for when to reach for this instead.
#[cfg(not(feature = "pinocchio"))]
pub fn invoke_unchecked(
    instruction: &solana_instruction::Instruction,
    account_handles: &[crate::prelude::CpiHandle<'_>],
) -> crate::prelude::Result<()> {
    invoke_signed_unchecked(instruction, account_handles, &[])
}

#[cfg(not(feature = "pinocchio"))]
pub fn invoke_signed_unchecked(
    instruction: &solana_instruction::Instruction,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    let mut infos = crate::prelude::Vec::with_capacity(account_handles.len());
    for handle in account_handles {
        unsafe {
            infos.push(handle.info.to_lifetime());
        }
    }
    sdk::invoke_signed_unchecked(instruction, &infos, signer_seeds)
}

/// The runtime-facing CPI ABI of the `solana_program` backend, built from
/// borrowed memory instead of the owning `Vec`s
/// `solana_instruction::Instruction` requires (which `invoke_signed`
/// then clones a second time into this very layout). The runtime reads these
/// structs by offset, so their layouts are checked byte for byte against the
/// SDK's `StableInstruction`/`AccountMeta` in this module's tests.
#[cfg(not(feature = "pinocchio"))]
pub(crate) mod stable {
    use solana_account_info::AccountInfo;

    /// Field order and sizes match `solana_stable_layout::StableVec`.
    #[repr(C)]
    pub(crate) struct StableVec<T> {
        addr: u64,
        cap: u64,
        len: u64,
        _marker: core::marker::PhantomData<T>,
    }

    impl<T> StableVec<T> {
        #[inline(always)]
        fn borrowing(slice: &[T]) -> Self {
            Self {
                addr: slice.as_ptr() as u64,
                cap: slice.len() as u64,
                len: slice.len() as u64,
                _marker: core::marker::PhantomData,
            }
        }
    }

    /// Matches `solana_instruction::AccountMeta`'s `repr(C)` layout.
    #[repr(C)]
    pub(crate) struct StableAccountMeta {
        pub(crate) pubkey: solana_address::Address,
        pub(crate) is_signer: bool,
        pub(crate) is_writable: bool,
    }

    /// Matches `solana_stable_layout::StableInstruction`. Borrows `accounts`
    /// and `data` for `'a`, so it cannot outlive the memory it points into.
    #[repr(C)]
    pub(crate) struct StableInstruction<'a> {
        accounts: StableVec<StableAccountMeta>,
        data: StableVec<u8>,
        program_id: solana_address::Address,
        _borrows: core::marker::PhantomData<&'a ()>,
    }

    impl<'a> StableInstruction<'a> {
        #[inline(always)]
        pub(crate) fn new(
            program_id: solana_address::Address,
            accounts: &'a [StableAccountMeta],
            data: &'a [u8],
        ) -> Self {
            Self {
                accounts: StableVec::borrowing(accounts),
                data: StableVec::borrowing(data),
                program_id,
                _borrows: core::marker::PhantomData,
            }
        }

        /// Points at `instruction`'s own account metas and data. Sound because
        /// `StableAccountMeta` has `solana_instruction::AccountMeta`'s exact
        /// layout (checked in this module's tests), and the runtime reads only
        /// each vector's address and length.
        #[inline(always)]
        pub(crate) fn from_instruction(instruction: &'a solana_instruction::Instruction) -> Self {
            Self {
                accounts: StableVec {
                    addr: instruction.accounts.as_ptr() as u64,
                    cap: instruction.accounts.len() as u64,
                    len: instruction.accounts.len() as u64,
                    _marker: core::marker::PhantomData,
                },
                data: StableVec::borrowing(&instruction.data),
                program_id: instruction.program_id,
                _borrows: core::marker::PhantomData,
            }
        }
    }

    /// Invokes `instruction` through the `sol_invoke_signed_rust` syscall with
    /// no `RefCell` borrow checks: the caller must not pass a writable account
    /// whose data or lamports are borrowed elsewhere. Goes through
    /// `solana_define_syscall`, since on SBPF v3 a plain `extern "C"`
    /// declaration is not a syscall call.
    #[inline(always)]
    pub(crate) fn invoke_signed_raw(
        instruction: &StableInstruction<'_>,
        account_infos: &[AccountInfo<'_>],
        signers_seeds: &[&[&[u8]]],
    ) -> crate::prelude::Result<()> {
        #[cfg(target_os = "solana")]
        {
            let result = unsafe {
                solana_define_syscall::definitions::sol_invoke_signed_rust(
                    instruction as *const StableInstruction<'_> as *const u8,
                    account_infos.as_ptr() as *const u8,
                    account_infos.len() as u64,
                    signers_seeds.as_ptr() as *const u8,
                    signers_seeds.len() as u64,
                )
            };
            if result != 0 {
                return Err(result.into());
            }
        }
        #[cfg(not(target_os = "solana"))]
        let _ = (instruction, account_infos, signers_seeds);
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use {
            super::*,
            alloc::vec,
            core::mem::{align_of, offset_of, size_of},
            solana_instruction::{AccountMeta, Instruction},
            solana_stable_layout::stable_instruction::StableInstruction as Official,
        };

        fn bytes_of<T>(value: &T) -> &[u8] {
            unsafe { core::slice::from_raw_parts(value as *const T as *const u8, size_of::<T>()) }
        }

        #[test]
        fn layouts_match_the_sdk() {
            assert_eq!(size_of::<StableInstruction<'_>>(), size_of::<Official>());
            assert_eq!(align_of::<StableInstruction<'_>>(), align_of::<Official>());
            assert_eq!(
                offset_of!(StableInstruction<'_>, accounts),
                offset_of!(Official, accounts)
            );
            assert_eq!(
                offset_of!(StableInstruction<'_>, data),
                offset_of!(Official, data)
            );
            assert_eq!(
                offset_of!(StableInstruction<'_>, program_id),
                offset_of!(Official, program_id)
            );

            assert_eq!(size_of::<StableAccountMeta>(), size_of::<AccountMeta>());
            assert_eq!(align_of::<StableAccountMeta>(), align_of::<AccountMeta>());
            assert_eq!(
                offset_of!(StableAccountMeta, pubkey),
                offset_of!(AccountMeta, pubkey)
            );
            assert_eq!(
                offset_of!(StableAccountMeta, is_signer),
                offset_of!(AccountMeta, is_signer)
            );
            assert_eq!(
                offset_of!(StableAccountMeta, is_writable),
                offset_of!(AccountMeta, is_writable)
            );
        }

        #[test]
        fn encoding_matches_the_sdk_for_the_same_instruction() {
            let program_id = solana_address::Address::new_from_array([7; 32]);
            let first = solana_address::Address::new_from_array([9; 32]);
            let second = solana_address::Address::new_from_array([5; 32]);
            let data = [1u8, 2, 3, 4, 5];

            let official = Official::from(Instruction {
                program_id,
                accounts: vec![
                    AccountMeta {
                        pubkey: first,
                        is_signer: true,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: second,
                        is_signer: false,
                        is_writable: true,
                    },
                ],
                data: data.to_vec(),
            });

            let accounts = [
                StableAccountMeta {
                    pubkey: first,
                    is_signer: true,
                    is_writable: false,
                },
                StableAccountMeta {
                    pubkey: second,
                    is_signer: false,
                    is_writable: true,
                },
            ];
            let ours = StableInstruction::new(program_id, &accounts, &data);

            assert_eq!(ours.accounts.len, official.accounts.len);
            assert_eq!(ours.data.len, official.data.len);
            assert_eq!(bytes_of(&ours.program_id), bytes_of(&official.program_id));

            let official_metas = unsafe {
                core::slice::from_raw_parts(
                    official.accounts.addr as *const AccountMeta,
                    official.accounts.len as usize,
                )
            };
            for (mine, theirs) in accounts.iter().zip(official_metas) {
                assert_eq!(bytes_of(mine), bytes_of(theirs));
            }
            let official_data = unsafe {
                core::slice::from_raw_parts(
                    official.data.addr as *const u8,
                    official.data.len as usize,
                )
            };
            assert_eq!(official_data, data);
        }

        #[test]
        fn from_instruction_encodes_like_the_sdk() {
            let instruction = Instruction {
                program_id: solana_address::Address::new_from_array([3; 32]),
                accounts: vec![
                    AccountMeta {
                        pubkey: solana_address::Address::new_from_array([4; 32]),
                        is_signer: true,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: solana_address::Address::new_from_array([6; 32]),
                        is_signer: false,
                        is_writable: false,
                    },
                ],
                data: vec![9, 8, 7],
            };
            let official = Official::from(instruction.clone());
            let ours = StableInstruction::from_instruction(&instruction);

            assert_eq!(ours.accounts.len, official.accounts.len);
            assert_eq!(ours.data.len, official.data.len);
            assert_eq!(bytes_of(&ours.program_id), bytes_of(&official.program_id));

            let meta_bytes = official.accounts.len as usize * size_of::<AccountMeta>();
            let our_metas =
                unsafe { core::slice::from_raw_parts(ours.accounts.addr as *const u8, meta_bytes) };
            let official_metas = unsafe {
                core::slice::from_raw_parts(official.accounts.addr as *const u8, meta_bytes)
            };
            assert_eq!(our_metas, official_metas);

            let our_data = unsafe {
                core::slice::from_raw_parts(ours.data.addr as *const u8, ours.data.len as usize)
            };
            let official_data = unsafe {
                core::slice::from_raw_parts(
                    official.data.addr as *const u8,
                    official.data.len as usize,
                )
            };
            assert_eq!(our_data, official_data);
        }
    }
}

/// The `solana_cpi` 3.1.0 API over the SDK's `AccountInfo`, with the same
/// signatures, the same pre-invoke borrow checks and the same syscalls, so a
/// program calling these behaves as it would with that crate.
#[cfg(not(feature = "pinocchio"))]
pub mod sdk {
    use {
        super::stable::{invoke_signed_raw, StableInstruction},
        crate::prelude::Vec,
        solana_account_info::AccountInfo,
        solana_address::Address,
        solana_instruction::Instruction,
        solana_program_error::ProgramResult,
    };

    /// [`invoke_signed`] with no signer seeds.
    pub fn invoke(instruction: &Instruction, account_infos: &[AccountInfo]) -> ProgramResult {
        invoke_signed(instruction, account_infos, &[])
    }

    /// [`invoke_signed_unchecked`] with no signer seeds.
    pub fn invoke_unchecked(
        instruction: &Instruction,
        account_infos: &[AccountInfo],
    ) -> ProgramResult {
        invoke_signed_unchecked(instruction, account_infos, &[])
    }

    /// Invokes `instruction` after checking, for each of its account metas,
    /// the first entry of `account_infos` with that key: a writable meta's
    /// lamports and data must be mutably borrowable, a read-only meta's
    /// borrowable, otherwise this returns `AccountBorrowFailed` without
    /// invoking. The check borrows are released before the call.
    pub fn invoke_signed(
        instruction: &Instruction,
        account_infos: &[AccountInfo],
        signers_seeds: &[&[&[u8]]],
    ) -> ProgramResult {
        for account_meta in instruction.accounts.iter() {
            for account_info in account_infos.iter() {
                if account_meta.pubkey == *account_info.key {
                    if account_meta.is_writable {
                        let _ = account_info.try_borrow_mut_lamports()?;
                        let _ = account_info.try_borrow_mut_data()?;
                    } else {
                        let _ = account_info.try_borrow_lamports()?;
                        let _ = account_info.try_borrow_data()?;
                    }
                    break;
                }
            }
        }

        invoke_signed_unchecked(instruction, account_infos, signers_seeds)
    }

    /// Invokes `instruction` with no borrow checks: the caller must not pass a
    /// writable account whose data or lamports are borrowed elsewhere.
    /// Off-chain this does nothing and returns `Ok(())`.
    pub fn invoke_signed_unchecked(
        instruction: &Instruction,
        account_infos: &[AccountInfo],
        signers_seeds: &[&[&[u8]]],
    ) -> ProgramResult {
        invoke_signed_raw(
            &StableInstruction::from_instruction(instruction),
            account_infos,
            signers_seeds,
        )
    }

    /// The largest return data the runtime accepts.
    pub const MAX_RETURN_DATA: usize = 1024;

    /// Sets the current instruction's return data. Off-chain this does nothing.
    pub fn set_return_data(data: &[u8]) {
        #[cfg(target_os = "solana")]
        unsafe {
            solana_define_syscall::definitions::sol_set_return_data(
                data.as_ptr(),
                data.len() as u64,
            )
        };
        #[cfg(not(target_os = "solana"))]
        let _ = data;
    }

    /// The return data most recently set during this transaction and the id
    /// of the program that set it, at most [`MAX_RETURN_DATA`] bytes; `None`
    /// when it is empty. Off-chain always `None`.
    pub fn get_return_data() -> Option<(Address, Vec<u8>)> {
        #[cfg(target_os = "solana")]
        {
            let mut buf = [0u8; MAX_RETURN_DATA];
            let mut program_id = Address::default();

            let size = unsafe {
                solana_define_syscall::definitions::sol_get_return_data(
                    buf.as_mut_ptr(),
                    buf.len() as u64,
                    &mut program_id as *mut _ as *mut u8,
                )
            };

            if size == 0 {
                None
            } else {
                let size = core::cmp::min(size as usize, MAX_RETURN_DATA);
                Some((program_id, buf[..size].to_vec()))
            }
        }

        #[cfg(not(target_os = "solana"))]
        None
    }

    #[cfg(test)]
    mod tests {
        use {
            super::*, alloc::vec, solana_instruction::AccountMeta,
            solana_program_error::ProgramError,
        };

        fn key(byte: u8) -> Address {
            Address::new_from_array([byte; 32])
        }

        fn instruction(metas: &[(u8, bool)]) -> Instruction {
            Instruction {
                program_id: key(0xEE),
                accounts: metas
                    .iter()
                    .map(|&(byte, is_writable)| AccountMeta {
                        pubkey: key(byte),
                        is_signer: false,
                        is_writable,
                    })
                    .collect(),
                data: vec![1, 2, 3],
            }
        }

        fn both(instruction: &Instruction, infos: &[AccountInfo]) -> ProgramResult {
            let ours = invoke_signed(instruction, infos, &[]);
            let stock = solana_cpi::invoke_signed(instruction, infos, &[]);
            assert!(ours == stock);
            ours
        }

        #[test]
        fn borrow_checks_match_solana_cpi() {
            let (k1, k2, owner) = (key(1), key(2), key(0xAA));
            let (mut lamports1, mut lamports2) = (10u64, 20u64);
            let (mut data1, mut data2) = ([0u8; 4], [0u8; 4]);
            let first =
                AccountInfo::new(&k1, false, true, &mut lamports1, &mut data1, &owner, false);
            let second =
                AccountInfo::new(&k2, false, false, &mut lamports2, &mut data2, &owner, false);
            let infos = [first.clone(), second.clone()];

            assert!(both(&instruction(&[(1, true), (2, false)]), &infos) == Ok(()));
            assert!(both(&instruction(&[(9, true)]), &infos) == Ok(()));

            {
                let _shared = first.data.borrow();
                assert!(
                    both(&instruction(&[(1, true)]), &infos)
                        == Err(ProgramError::AccountBorrowFailed)
                );
                assert!(both(&instruction(&[(1, false)]), &infos) == Ok(()));
            }
            {
                let _exclusive = first.lamports.borrow_mut();
                assert!(
                    both(&instruction(&[(1, false)]), &infos)
                        == Err(ProgramError::AccountBorrowFailed)
                );
            }
            {
                let _exclusive = second.data.borrow_mut();
                assert!(both(&instruction(&[(1, true)]), &infos) == Ok(()));
                assert!(
                    both(&instruction(&[(1, true), (2, false)]), &infos)
                        == Err(ProgramError::AccountBorrowFailed)
                );
            }
        }

        #[test]
        fn only_the_first_matching_account_info_is_checked() {
            let (k1, owner) = (key(1), key(0xAA));
            let (mut lamports_a, mut lamports_b) = (1u64, 2u64);
            let (mut data_a, mut data_b) = ([0u8; 2], [0u8; 2]);
            let free = AccountInfo::new(
                &k1,
                false,
                true,
                &mut lamports_a,
                &mut data_a,
                &owner,
                false,
            );
            let busy = AccountInfo::new(
                &k1,
                false,
                true,
                &mut lamports_b,
                &mut data_b,
                &owner,
                false,
            );
            let _held = busy.data.borrow_mut();

            assert!(both(&instruction(&[(1, true)]), &[free.clone(), busy.clone()]) == Ok(()));
            assert!(
                both(&instruction(&[(1, true)]), &[busy.clone(), free.clone()])
                    == Err(ProgramError::AccountBorrowFailed)
            );
        }
    }
}

// ===========================================================================
// PINOCCHIO BACKEND
// ===========================================================================

/// Bridges naclac's own `CpiHandle` to the real
/// `solana_instruction_view::cpi` (re-exported as `pinocchio::cpi`) checked
/// invoke helpers, which are generic over `AsRef<AccountView>`.
#[cfg(feature = "pinocchio")]
impl<'a> AsRef<::pinocchio::AccountView> for crate::prelude::CpiHandle<'a> {
    #[inline(always)]
    fn as_ref(&self) -> &::pinocchio::AccountView {
        &self.info.view
    }
}

/// Converts naclac's backend-uniform `&[&[&[u8]]]` signer seeds into
/// pinocchio's own `Signer`/`Seed` types without a heap allocation, binding
/// the result to `$signers_var`. A macro rather than a function: each
/// `Signer` built here borrows from the seed buffer built in the same scope,
/// and threading that self-reference across a function boundary risks a
/// subtly wrong lifetime signature (same reasoning as
/// `system_program.rs`'s own `pinocchio_signers_from_seeds!`). Must be
/// invoked directly inside a function returning `crate::prelude::Result<()>`
/// — it returns early on overflow.
///
/// `#[macro_export]`'d so `naclac-token` (and any other downstream crate)
/// can reuse this exact, single implementation instead of hand-rolling the
/// same unsafe signer/seed-array construction independently at each CPI call
/// site — which is what `token/src/token.rs` used to do, 9 times
/// over, via raw `get_unchecked`/`as_mut_ptr().add(j)` writes into
/// `MaybeUninit` arrays (functionally the same job, more unsafe surface,
/// nine separate places to keep in sync). Every `crate::` reference inside
/// is `$crate::` instead — required for a macro invoked from a different
/// crate to still resolve against naclac-core's own `prelude`, not the
/// caller's.
#[cfg(feature = "pinocchio")]
#[macro_export]
macro_rules! cpi_signers_from_seeds {
    ($seeds:expr, $signers_var:ident) => {
        if $seeds.len() > $crate::prelude::MAX_CPI_SIGNERS {
            return Err($crate::prelude::NaclacError::TooManyCpiSigners.into());
        }
        let mut __signers: [::pinocchio::cpi::Signer; $crate::prelude::MAX_CPI_SIGNERS] =
            unsafe { core::mem::zeroed() };
        let mut __seeds_buffer: [::pinocchio::cpi::Seed;
            $crate::prelude::MAX_CPI_SEEDS_PER_SIGNER * $crate::prelude::MAX_CPI_SIGNERS] =
            unsafe { core::mem::zeroed() };

        let mut __remaining: &mut [::pinocchio::cpi::Seed] = &mut __seeds_buffer;
        for (__slot, seed_parts) in __signers.iter_mut().zip($seeds.iter()) {
            if seed_parts.len() > $crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
                return Err($crate::prelude::NaclacError::TooManyCpiSeeds.into());
            }
            let Some((__mine, __tail)) =
                core::mem::take(&mut __remaining).split_at_mut_checked(seed_parts.len())
            else {
                return Err($crate::prelude::NaclacError::TooManyCpiSeeds.into());
            };
            for (__dst, part) in __mine.iter_mut().zip(seed_parts.iter()) {
                *__dst = ::pinocchio::cpi::Seed::from(*part);
            }
            *__slot = ::pinocchio::cpi::Signer::from(&*__mine);
            __remaining = __tail;
        }

        let $signers_var: &[::pinocchio::cpi::Signer] = match __signers.get(..$seeds.len()) {
            Some(signers) => signers,
            None => return Err($crate::prelude::NaclacError::TooManyCpiSigners.into()),
        };
    };
}

#[cfg(feature = "pinocchio")]
pub fn invoke_pinocchio(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
) -> crate::prelude::Result<()> {
    invoke_signed_pinocchio_handles(instruction, account_handles, &[])
}

#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio_handles(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    if signer_seeds.is_empty() {
        return ::pinocchio::cpi::invoke_signed_with_bounds::<
            { crate::prelude::MAX_CPI_ACCOUNTS },
            _,
        >(instruction, account_handles, &[]);
    }
    cpi_signers_from_seeds!(signer_seeds, signers);
    ::pinocchio::cpi::invoke_signed_with_bounds::<{ crate::prelude::MAX_CPI_ACCOUNTS }, _>(
        instruction,
        account_handles,
        signers,
    )
}

/// Raw, no-borrow-check CPI — the original hand-rolled implementation this
/// function has always used. See this module's own doc comment for when to
/// reach for this instead of the checked `invoke_pinocchio`/
/// `invoke_signed_pinocchio_handles`.
#[cfg(feature = "pinocchio")]
pub fn invoke_pinocchio_unchecked(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
) -> crate::prelude::Result<()> {
    invoke_signed_pinocchio_handles_unchecked(instruction, account_handles, &[])
}

#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio_handles_unchecked(
    instruction: &::pinocchio::instruction::InstructionView,
    account_handles: &[crate::prelude::CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    let account_infos: &[crate::prelude::AccountInfo] =
        unsafe { core::mem::transmute(account_handles) };

    if account_infos.len() > crate::prelude::MAX_CPI_ACCOUNTS {
        return Err(crate::prelude::NaclacError::TooManyCpiAccounts.into());
    }

    let mut cpi_accounts_arr =
        [const { core::mem::MaybeUninit::<::pinocchio::cpi::CpiAccount>::uninit() };
            crate::prelude::MAX_CPI_ACCOUNTS];
    let cpi_accounts_len = account_infos.len();
    for (uninit, info) in cpi_accounts_arr
        .iter_mut()
        .take(cpi_accounts_len)
        .zip(account_infos.iter())
    {
        uninit.write(::pinocchio::cpi::CpiAccount::from(&info.view));
    }
    let cpi_accounts = unsafe {
        core::slice::from_raw_parts(
            cpi_accounts_arr.as_ptr() as *const ::pinocchio::cpi::CpiAccount,
            cpi_accounts_len,
        )
    };

    if signer_seeds.is_empty() {
        unsafe {
            ::pinocchio::cpi::invoke_unchecked(instruction, cpi_accounts);
        }
    } else {
        cpi_signers_from_seeds!(signer_seeds, signers);
        unsafe {
            ::pinocchio::cpi::invoke_signed_unchecked(instruction, cpi_accounts, signers);
        }
    }
    Ok(())
}

/// Checked CPI from a raw slice of `AccountView`s (rather than naclac's own
/// `CpiHandle`) — for hand-authored instruction bodies constructing an
/// `InstructionView` directly.
#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio(
    instruction: &::pinocchio::instruction::InstructionView,
    account_infos: &[::pinocchio::AccountView],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    if signer_seeds.is_empty() {
        return ::pinocchio::cpi::invoke_signed_with_bounds::<
            { crate::prelude::MAX_CPI_ACCOUNTS },
            _,
        >(instruction, account_infos, &[]);
    }
    cpi_signers_from_seeds!(signer_seeds, signers);
    ::pinocchio::cpi::invoke_signed_with_bounds::<{ crate::prelude::MAX_CPI_ACCOUNTS }, _>(
        instruction,
        account_infos,
        signers,
    )
}

/// Raw, no-borrow-check CPI — the original hand-rolled implementation this
/// function has always used. See this module's own doc comment for when to
/// reach for this instead of the checked `invoke_signed_pinocchio`.
#[cfg(feature = "pinocchio")]
pub fn invoke_signed_pinocchio_unchecked(
    instruction: &::pinocchio::instruction::InstructionView,
    account_infos: &[::pinocchio::AccountView],
    signer_seeds: &[&[&[u8]]],
) -> crate::prelude::Result<()> {
    if account_infos.len() > crate::prelude::MAX_CPI_ACCOUNTS {
        return Err(crate::prelude::NaclacError::TooManyCpiAccounts.into());
    }

    let mut cpi_accounts_arr =
        [const { core::mem::MaybeUninit::<::pinocchio::cpi::CpiAccount>::uninit() };
            crate::prelude::MAX_CPI_ACCOUNTS];
    let cpi_accounts_len = account_infos.len();
    for (uninit, view) in cpi_accounts_arr
        .iter_mut()
        .take(cpi_accounts_len)
        .zip(account_infos.iter())
    {
        uninit.write(::pinocchio::cpi::CpiAccount::from(view));
    }
    let cpi_accounts = unsafe {
        core::slice::from_raw_parts(
            cpi_accounts_arr.as_ptr() as *const ::pinocchio::cpi::CpiAccount,
            cpi_accounts_len,
        )
    };

    if signer_seeds.is_empty() {
        unsafe {
            ::pinocchio::cpi::invoke_unchecked(instruction, cpi_accounts);
        }
    } else {
        cpi_signers_from_seeds!(signer_seeds, signers);
        unsafe {
            ::pinocchio::cpi::invoke_signed_unchecked(instruction, cpi_accounts, signers);
        }
    }
    Ok(())
}

#[cfg(all(kani, feature = "pinocchio"))]
mod kani_proofs {
    use crate::prelude::{MAX_CPI_SEEDS_PER_SIGNER, MAX_CPI_SIGNERS};

    /// Proves `cpi_signers_from_seeds!` rejects `> MAX_CPI_SIGNERS` signer
    /// groups with a clean `TooManyCpiSigners` error rather than truncating
    /// silently — this exact class of bug (silent truncation instead of
    /// erroring) was found and fixed once already in
    /// `system_program.rs`'s sibling macro (see docs/plan/kani-audit.md);
    /// this proves the fix actually holds for the shared macro every CPI
    /// call site in the workspace now goes through.
    #[kani::proof]
    fn prove_rejects_too_many_signers() -> crate::prelude::Result<()> {
        let empty: &[&[u8]] = &[];
        // One more than the real limit — every real limit is small (4), so
        // a fixed array one longer than that is cheap to build directly.
        let too_many: [&[&[u8]]; MAX_CPI_SIGNERS + 1] = [empty; MAX_CPI_SIGNERS + 1];
        let signer_seeds: &[&[&[u8]]] = &too_many;
        let result: crate::prelude::Result<()> = (|| {
            cpi_signers_from_seeds!(signer_seeds, _signers);
            Ok(())
        })();
        assert!(
            result.is_err(),
            "more than MAX_CPI_SIGNERS signer groups must be rejected, not silently truncated"
        );
        Ok(())
    }

    /// Same proof for the per-signer seed-count limit.
    #[kani::proof]
    fn prove_rejects_too_many_seeds_per_signer() -> crate::prelude::Result<()> {
        let one_seed: &[u8] = &[0u8];
        let too_many_seeds: [&[u8]; MAX_CPI_SEEDS_PER_SIGNER + 1] =
            [one_seed; MAX_CPI_SEEDS_PER_SIGNER + 1];
        let signer_seeds: &[&[&[u8]]] = &[&too_many_seeds];
        let result: crate::prelude::Result<()> = (|| {
            cpi_signers_from_seeds!(signer_seeds, _signers);
            Ok(())
        })();
        assert!(
            result.is_err(),
            "more than MAX_CPI_SEEDS_PER_SIGNER seeds in one group must be rejected, not silently truncated"
        );
        Ok(())
    }

    /// Proves the macro never panics and produces a correctly-shaped
    /// `signers` slice (one `Signer` per signer group, in order) for any
    /// symbolic seed *bytes* within a small, bounded, realistic shape (2
    /// signer groups, 2 seed parts each, 4 bytes per part) — deliberately
    /// modest, per the same "keep buffers/unwind bounds small" lesson from
    /// `external_plugin_registry`'s earlier resource blowup, since this
    /// exercises the same macro body's loops regardless of exact
    /// signer/seed count.
    #[kani::proof]
    fn prove_within_bounds_is_correct() -> crate::prelude::Result<()> {
        let seed_a0: [u8; 4] = kani::any();
        let seed_a1: [u8; 4] = kani::any();
        let seed_b0: [u8; 4] = kani::any();
        let seed_b1: [u8; 4] = kani::any();
        let signer_a: [&[u8]; 2] = [&seed_a0, &seed_a1];
        let signer_b: [&[u8]; 2] = [&seed_b0, &seed_b1];
        let signer_seeds: &[&[&[u8]]] = &[&signer_a, &signer_b];

        cpi_signers_from_seeds!(signer_seeds, signers);
        assert_eq!(
            signers.len(),
            signer_seeds.len(),
            "must produce exactly one Signer per input signer group"
        );
        Ok(())
    }
}

#[cfg(all(test, feature = "pinocchio"))]
mod signer_seed_tests {
    use crate::prelude::{MAX_CPI_SEEDS_PER_SIGNER, MAX_CPI_SIGNERS};

    fn signer_count(signer_seeds: &[&[&[u8]]]) -> crate::prelude::Result<usize> {
        cpi_signers_from_seeds!(signer_seeds, signers);
        Ok(signers.len())
    }

    #[test]
    fn accepts_no_signers() {
        assert_eq!(signer_count(&[]).ok(), Some(0));
    }

    #[test]
    fn accepts_the_maximum_shape() {
        let seed: &[u8] = &[7u8];
        let group: [&[u8]; MAX_CPI_SEEDS_PER_SIGNER] = [seed; MAX_CPI_SEEDS_PER_SIGNER];
        let group_ref: &[&[u8]] = &group;
        let all: [&[&[u8]]; MAX_CPI_SIGNERS] = [group_ref; MAX_CPI_SIGNERS];
        assert_eq!(signer_count(&all).ok(), Some(MAX_CPI_SIGNERS));
    }

    #[test]
    fn accepts_groups_of_different_lengths() {
        let seed: &[u8] = &[1u8];
        let short: [&[u8]; 1] = [seed];
        let long: [&[u8]; 3] = [seed; 3];
        let empty: [&[u8]; 0] = [];
        let all: [&[&[u8]]; 3] = [&short, &empty, &long];
        assert_eq!(signer_count(&all).ok(), Some(3));
    }

    #[test]
    fn rejects_too_many_signers() {
        let empty: &[&[u8]] = &[];
        let all: [&[&[u8]]; MAX_CPI_SIGNERS + 1] = [empty; MAX_CPI_SIGNERS + 1];
        assert!(signer_count(&all).is_err());
    }

    #[test]
    fn rejects_too_many_seeds_in_one_group() {
        let seed: &[u8] = &[0u8];
        let group: [&[u8]; MAX_CPI_SEEDS_PER_SIGNER + 1] = [seed; MAX_CPI_SEEDS_PER_SIGNER + 1];
        let group_ref: &[&[u8]] = &group;
        assert!(signer_count(&[group_ref]).is_err());
    }
}
