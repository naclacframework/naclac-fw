// ===========================================================================
// system_program.rs — Dual-backend system program helpers
// ===========================================================================

//! # System Program CPI Abstractions
//!
//! Provides dual-backend wrappers for invoking native Solana System Program instructions.
//! Enforces borrow-checking via CPI handles.

use crate::prelude::{ToCpiHandle, ToCpiHandleMut};

// SOLANA BACKEND
#[cfg(not(feature = "pinocchio"))]
mod solana_system {
    use crate::prelude::{CpiHandle, CpiHandleMut, Result};

    /// The Solana System Program ID (all zeros)
    pub const ID: solana_address::Address = solana_address::Address::new_from_array([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ]);

    pub fn transfer(from: CpiHandleMut<'_>, to: CpiHandleMut<'_>, lamports: u64) -> Result<()> {
        transfer_signed(from, to, lamports, &[])
    }

    pub fn transfer_signed(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        lamports: u64,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        let ix =
            solana_system_interface::instruction::transfer(from.info.key, to.info.key, lamports);
        let accounts = [CpiHandle::from(from), CpiHandle::from(to)];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[repr(C)]
    struct StableAccountMeta {
        pub pubkey: solana_address::Address,
        pub is_writable: bool,
        pub is_signer: bool,
    }

    #[repr(C)]
    struct StableVec<T> {
        pub ptr: *const T,
        pub len: usize,
        pub cap: usize,
    }

    #[repr(C)]
    struct StableInstruction {
        pub accounts: StableVec<StableAccountMeta>,
        pub data: StableVec<u8>,
        pub program_id: solana_address::Address,
    }

    extern "C" {
        fn sol_invoke_signed_rust(
            instruction_addr: *const u8,
            account_infos_addr: *const u8,
            account_infos_len: u64,
            signers_seeds_addr: *const u8,
            signers_seeds_len: u64,
        ) -> u64;
    }

    pub fn create_account(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &solana_address::Address,
    ) -> Result<()> {
        create_account_signed(from, to, program, lamports, space, owner, &[])
    }

    pub fn create_account_signed(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &solana_address::Address,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        let stable_accounts = [
            StableAccountMeta {
                pubkey: from.info.address(),
                is_writable: true,
                is_signer: true,
            },
            StableAccountMeta {
                pubkey: to.info.address(),
                is_writable: true,
                is_signer: true,
            },
        ];

        let mut ix_data = [0u8; 52];
        ix_data[0..4].copy_from_slice(&0u32.to_le_bytes()); // CreateAccount discriminator = 0
        ix_data[4..12].copy_from_slice(&lamports.to_le_bytes());
        ix_data[12..20].copy_from_slice(&space.to_le_bytes());
        ix_data[20..52].copy_from_slice(owner.as_ref());

        let stable_ix = StableInstruction {
            accounts: StableVec {
                ptr: stable_accounts.as_ptr(),
                len: stable_accounts.len(),
                cap: stable_accounts.len(),
            },
            data: StableVec {
                ptr: ix_data.as_ptr(),
                len: ix_data.len(),
                cap: ix_data.len(),
            },
            program_id: ID,
        };

        let account_infos = unsafe {
            [
                from.info.to_lifetime(),
                to.info.to_lifetime(),
                program.info.to_lifetime(),
            ]
        };

        unsafe {
            let result = sol_invoke_signed_rust(
                &stable_ix as *const StableInstruction as *const u8,
                account_infos.as_ptr() as *const u8,
                account_infos.len() as u64,
                signer_seeds.as_ptr() as *const u8,
                signer_seeds.len() as u64,
            );
            if result != 0 {
                return Err(solana_program::program_error::ProgramError::Custom(
                    result as u32,
                ));
            }
        }
        Ok(())
    }
}

// PINOCCHIO BACKEND
#[cfg(feature = "pinocchio")]
mod pinocchio_system {
    use crate::prelude::{CpiHandle, CpiHandleMut, Result};

    // Re-use the system program ID directly from pinocchio-system crate
    pub use ::pinocchio_system::ID;

    pub fn transfer(from: CpiHandleMut<'_>, to: CpiHandleMut<'_>, lamports: u64) -> Result<()> {
        transfer_signed(from, to, lamports, &[])
    }

    pub fn transfer_signed(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        lamports: u64,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        let from_address = *from.info.view.address();
        let to_address = *to.info.view.address();
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable_signer(&from_address),
            ::pinocchio::instruction::InstructionAccount::writable(&to_address),
        ];
        let mut ix_data = [0u8; 12];
        ix_data[0] = 2; // system transfer discriminant
        ix_data[4..12].copy_from_slice(&lamports.to_le_bytes());

        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: &ID,
            accounts: &ix_accounts,
            data: &ix_data,
        };

        let cpi_accounts = [CpiHandle::from(from), CpiHandle::from(to)];

        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &cpi_accounts, signer_seeds)
    }

    #[inline(always)]
    pub fn create_account_unchecked(
        from: &crate::prelude::AccountView,
        to: &crate::prelude::AccountView,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
        signer_seeds: &[::pinocchio::cpi::Signer],
    ) -> Result<()> {
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable_signer(from.address()),
            ::pinocchio::instruction::InstructionAccount::writable_signer(to.address()),
        ];
        let mut ix_data = [0u8; 52];
        ix_data[0..4].copy_from_slice(&0u32.to_le_bytes()); // system create_account discriminant
        ix_data[4..12].copy_from_slice(&lamports.to_le_bytes());
        ix_data[12..20].copy_from_slice(&space.to_le_bytes());
        ix_data[20..52].copy_from_slice(owner.as_ref());

        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: &ID,
            accounts: &ix_accounts,
            data: &ix_data,
        };

        let cpi_accounts = [
            ::pinocchio::cpi::CpiAccount::from(from),
            ::pinocchio::cpi::CpiAccount::from(to),
        ];

        // SAFETY: Bypasses Pinocchio's runtime check, safe as caller validates borrows
        unsafe {
            ::pinocchio::cpi::invoke_signed_unchecked(&instruction, &cpi_accounts, signer_seeds);
        }

        Ok(())
    }

    pub fn create_account(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
    ) -> Result<()> {
        create_account_signed(from, to, program, lamports, space, owner, &[])
    }

    pub fn create_account_signed(
        from: CpiHandleMut<'_>,
        to: CpiHandleMut<'_>,
        _program: CpiHandle<'_>,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        let mut signers: [::pinocchio::cpi::Signer; 4] = unsafe { core::mem::zeroed() };
        let mut seeds_buffer: [::pinocchio::cpi::Seed; 32] = unsafe { core::mem::zeroed() };
        let mut seed_ranges = [(0usize, 0usize); 4];

        let mut seed_idx = 0;
        let mut signer_idx = 0;

        for (i, seed_parts) in signer_seeds.iter().enumerate() {
            if i >= signers.len() {
                break;
            }
            let start_seed = seed_idx;
            for part in seed_parts.iter() {
                if seed_idx >= seeds_buffer.len() {
                    break;
                }
                seeds_buffer[seed_idx] = ::pinocchio::cpi::Seed::from(*part);
                seed_idx += 1;
            }
            seed_ranges[i] = (start_seed, seed_idx);
            signer_idx += 1;
        }

        for i in 0..signer_idx {
            let (start, end) = seed_ranges[i];
            signers[i] = ::pinocchio::cpi::Signer::from(&seeds_buffer[start..end]);
        }

        create_account_unchecked(
            &from.info.view,
            &to.info.view,
            lamports,
            space,
            owner,
            &signers[..signer_idx],
        )
    }
}

// ===========================================================================
// Re-exports
// ===========================================================================
#[cfg(not(feature = "pinocchio"))]
pub use solana_system::{create_account, create_account_signed, transfer, transfer_signed, ID};

#[cfg(feature = "pinocchio")]
pub use pinocchio_system::{
    create_account, create_account_signed, create_account_unchecked, transfer, transfer_signed, ID,
};

#[derive(Debug)]
pub struct SystemTransferAccounts<'a, A, B> {
    pub from: &'a mut A,
    pub to: &'a mut B,
}

#[derive(Debug)]
pub struct CreateAccountAccounts<'a, A, B> {
    pub from: &'a mut A,
    pub to: &'a mut B,
}

impl crate::wrappers::Program<crate::wrappers::System> {
    pub fn transfer<'a, A, B>(
        &self,
        accounts: SystemTransferAccounts<'a, A, B>,
        amount: u64,
    ) -> crate::prelude::Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
    {
        let from_h = accounts.from.to_cpi_handle_mut();
        let to_h = accounts.to.to_cpi_handle_mut();
        transfer(from_h, to_h, amount)
    }

    pub fn transfer_signed<'a, A, B>(
        &self,
        accounts: SystemTransferAccounts<'a, A, B>,
        amount: u64,
        signer_seeds: &[&[&[u8]]],
    ) -> crate::prelude::Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
    {
        let from_h = accounts.from.to_cpi_handle_mut();
        let to_h = accounts.to.to_cpi_handle_mut();
        transfer_signed(from_h, to_h, amount, signer_seeds)
    }

    pub fn create_account<'a, A, B>(
        &self,
        accounts: CreateAccountAccounts<'a, A, B>,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
    ) -> crate::prelude::Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
    {
        let from_h = accounts.from.to_cpi_handle_mut();
        let to_h = accounts.to.to_cpi_handle_mut();
        let program_h = self.to_cpi_handle();
        create_account(from_h, to_h, program_h, lamports, space, owner)
    }

    pub fn create_account_signed<'a, A, B>(
        &self,
        accounts: CreateAccountAccounts<'a, A, B>,
        lamports: u64,
        space: u64,
        owner: &crate::prelude::Address,
        signer_seeds: &[&[&[u8]]],
    ) -> crate::prelude::Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
    {
        let from_h = accounts.from.to_cpi_handle_mut();
        let to_h = accounts.to.to_cpi_handle_mut();
        let program_h = self.to_cpi_handle();
        create_account_signed(
            from_h,
            to_h,
            program_h,
            lamports,
            space,
            owner,
            signer_seeds,
        )
    }
}
