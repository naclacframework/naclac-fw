// ===========================================================================
// associated_token.rs — Associated Token Program CPI helpers
// ===========================================================================

//! # Associated Token Program CPI Abstractions
//!
//! Provides dual-backend wrappers for invoking the SPL Associated Token Program.
//! Enforces borrow-checking via CPI handles.

use crate::prelude::{derive_program_address, Address, CpiHandle, CpiHandleMut, NaclacError, Result};
use crate::wrappers::{ToAddress, ToCpiHandle, ToCpiHandleMut};

/// Validates the three CPI-target program accounts for a `Create`/
/// `CreateIdempotent` invocation: `program` must genuinely be the
/// Associated Token Program, `system_program` the real System program, and
/// `token_program` either Token or Token-2022 — none of these are checked
/// anywhere else on this path (unlike `Program<Token>`/`Interface<TokenInterface>`-
/// typed instruction fields, these are raw `CpiHandle`s with no load-time
/// validation of their own), so without this a caller could pass an
/// attacker-controlled program for any of the three and this function would
/// build and invoke a CPI against it unconditionally.
fn validate_ata_cpi_programs(
    program: &CpiHandle<'_>,
    system_program: &CpiHandle<'_>,
    token_program: &CpiHandle<'_>,
) -> Result<()> {
    if program.address() != crate::prelude::ASSOCIATED_TOKEN_PROGRAM_ID {
        return Err(NaclacError::ProgramIdMismatch.into());
    }
    if system_program.address() != crate::prelude::SYSTEM_PROGRAM_ID {
        return Err(NaclacError::ProgramIdMismatch.into());
    }
    let token_program_addr = token_program.address();
    if token_program_addr != crate::prelude::TOKEN_PROGRAM_ID
        && token_program_addr != crate::prelude::TOKEN_2022_PROGRAM_ID
    {
        return Err(NaclacError::ProgramIdMismatch.into());
    }
    Ok(())
}

/// Derives the canonical Associated Token Account address for
/// `(authority, token_program, mint)` at the given `bump`. Backs the
/// `associated_token::bump =` constraint, which requires the app to supply
/// this precomputed value — naclac never runs an on-chain
/// `find_program_address` bump search (see `derive_program_address`, which
/// this delegates to).
pub fn derive_ata_address(
    authority: &Address,
    token_program: &Address,
    mint: &Address,
    bump: u8,
) -> Address {
    derive_program_address(
        &[authority.as_ref(), token_program.as_ref(), mint.as_ref()],
        bump,
        &crate::prelude::ASSOCIATED_TOKEN_PROGRAM_ID,
    )
}

/// Validates that `actual` is the canonical Associated Token Account address
/// for `(authority, token_program, mint)` at `bump`. Backs the
/// `associated_token::mint =`/`associated_token::authority =`/
/// `associated_token::bump =` constraint on existing (non-`init`) accounts.
pub fn check_associated_token_address(
    actual: &Address,
    authority: &Address,
    token_program: &Address,
    mint: &Address,
    bump: u8,
) -> core::result::Result<(), NaclacError> {
    let expected = derive_ata_address(authority, token_program, mint, bump);
    if actual != &expected {
        return Err(NaclacError::ConstraintSeeds);
    }
    Ok(())
}

/// Resolved CPI account handles for an Associated Token Program `Create`/
/// `CreateIdempotent` invocation. Bundles what would otherwise be six
/// separate parameters into one — the same fix this project's comment
/// policy calls for on a `clippy::too_many_arguments` warning (see e.g.
/// `CheckedTransferParams` in `token.rs`), applied here to account handles
/// rather than instruction data.
pub struct AtaCpiAccounts<'a> {
    pub payer: CpiHandleMut<'a>,
    pub associated_token: CpiHandleMut<'a>,
    pub authority: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    pub system_program: CpiHandle<'a>,
    pub token_program: CpiHandle<'a>,
}

pub fn create(program: CpiHandle<'_>, accounts: AtaCpiAccounts<'_>) -> Result<()> {
    create_signed(program, accounts, &[])
}

pub fn create_signed(
    program: CpiHandle<'_>,
    accounts: AtaCpiAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let AtaCpiAccounts {
        payer,
        associated_token,
        authority,
        mint,
        system_program,
        token_program,
    } = accounts;

    validate_ata_cpi_programs(&program, &system_program, &token_program)?;

    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            payer.info.key,
            authority.info.key,
            mint.info.key,
            token_program.info.key,
        );

        let accounts_array = [
            CpiHandle::from(payer),
            CpiHandle::from(associated_token),
            authority,
            mint,
            system_program,
            token_program,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts_array, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        use pinocchio_associated_token_account::instructions::Create as PinocchioCreate;

        let ix = PinocchioCreate {
            funding_account: &payer.info.view,
            account: &associated_token.info.view,
            wallet: &authority.info.view,
            mint: &mint.info.view,
            system_program: &system_program.info.view,
            token_program: &token_program.info.view,
        };

        invoke_ata_ix(ix, signer_seeds)
    }
}

/// Like [`create`], but a no-op (instead of erroring) if the associated
/// token account already exists — the SPL Associated Token Program's own
/// `CreateIdempotent` instruction, confirmed as what the real `pump_fees`
/// program uses for `claim_social_fee_pda_v2`'s `associated_recipient` (a
/// plain `Create` would fail on a recipient's second claim).
pub fn create_idempotent(program: CpiHandle<'_>, accounts: AtaCpiAccounts<'_>) -> Result<()> {
    create_idempotent_signed(program, accounts, &[])
}

pub fn create_idempotent_signed(
    program: CpiHandle<'_>,
    accounts: AtaCpiAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let AtaCpiAccounts {
        payer,
        associated_token,
        authority,
        mint,
        system_program,
        token_program,
    } = accounts;

    validate_ata_cpi_programs(&program, &system_program, &token_program)?;

    #[cfg(not(feature = "pinocchio"))]
    {
        let ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                payer.info.key,
                authority.info.key,
                mint.info.key,
                token_program.info.key,
            );

        let accounts_array = [
            CpiHandle::from(payer),
            CpiHandle::from(associated_token),
            authority,
            mint,
            system_program,
            token_program,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts_array, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        use pinocchio_associated_token_account::instructions::CreateIdempotent as PinocchioCreateIdempotent;

        let ix = PinocchioCreateIdempotent {
            funding_account: &payer.info.view,
            account: &associated_token.info.view,
            wallet: &authority.info.view,
            mint: &mint.info.view,
            system_program: &system_program.info.view,
            token_program: &token_program.info.view,
        };

        invoke_ata_ix(ix, signer_seeds)
    }
}

/// Maximum accounts `create_idempotent_with_extra_accounts_signed` can pad
/// onto the real `CreateIdempotent` CPI beyond its own six accounts.
pub const MAX_ATA_CREATE_EXTRA_ACCOUNTS: usize = 4;

/// Same as `create_idempotent_signed`, but appends `extra_accounts` to the
/// CPI's account list as harmless, functionally-unused pass-through
/// entries.
///
/// Needed whenever a preceding raw (`sub_lamports`/`add_lamports`) lamport
/// mutation touched `accounts.payer` (or an account whose balance must be
/// reconciled alongside it, such as the source `payer`'s lamports were
/// credited from) — see `naclac_token::token::sync_native_with_extra_accounts`
/// for the full explanation of why: Solana's runtime only reconciles a raw
/// lamport write into its own tracked bookkeeping for accounts present in
/// whatever CPI is entered next, so a raw credit to `payer` right before it
/// pays this CPI's rent is invisible unless the debit side is passed here
/// too, as `extra_accounts` — the standard Solana "remaining accounts"
/// idiom, threaded through an inner CPI instead of the top-level
/// instruction.
pub fn create_idempotent_with_extra_accounts_signed(
    program: CpiHandle<'_>,
    accounts: AtaCpiAccounts<'_>,
    extra_accounts: &[CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    if extra_accounts.len() > MAX_ATA_CREATE_EXTRA_ACCOUNTS {
        return Err(NaclacError::TooManyExtraAccounts.into());
    }

    let AtaCpiAccounts {
        payer,
        associated_token,
        authority,
        mint,
        system_program,
        token_program,
    } = accounts;

    validate_ata_cpi_programs(&program, &system_program, &token_program)?;

    #[cfg(not(feature = "pinocchio"))]
    {
        let mut ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                payer.info.key,
                authority.info.key,
                mint.info.key,
                token_program.info.key,
            );
        ix.accounts.extend(
            extra_accounts
                .iter()
                .map(|extra| solana_program::instruction::AccountMeta::new(extra.address(), false)),
        );

        let extra_len = extra_accounts.len();
        let payer_handle: CpiHandle<'_> = CpiHandle::from(payer);
        let mut accounts_array: [CpiHandle<'_>; MAX_ATA_CREATE_EXTRA_ACCOUNTS + 7] =
            core::array::from_fn(|_| payer_handle.clone());
        accounts_array[1] = CpiHandle::from(associated_token);
        accounts_array[2] = authority;
        accounts_array[3] = mint;
        accounts_array[4] = system_program;
        accounts_array[5] = token_program;
        accounts_array[6] = program;
        accounts_array[7..7 + extra_len].clone_from_slice(extra_accounts);

        crate::cpi::invoke_signed(&ix, &accounts_array[..7 + extra_len], signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let extra_len = extra_accounts.len();
        let mut addresses = [*payer.info.view.address(); MAX_ATA_CREATE_EXTRA_ACCOUNTS + 6];
        addresses[1] = *associated_token.info.view.address();
        addresses[2] = *authority.info.view.address();
        addresses[3] = *mint.info.view.address();
        addresses[4] = *system_program.info.view.address();
        addresses[5] = *token_program.info.view.address();
        for (slot, extra) in addresses[6..].iter_mut().zip(extra_accounts.iter()) {
            *slot = *extra.info.view.address();
        }

        let ix_accounts_all: [::pinocchio::instruction::InstructionAccount;
            MAX_ATA_CREATE_EXTRA_ACCOUNTS + 6] = core::array::from_fn(|i| match i {
            0 => ::pinocchio::instruction::InstructionAccount::writable_signer(&addresses[0]),
            1 => ::pinocchio::instruction::InstructionAccount::writable(&addresses[1]),
            2..=5 => ::pinocchio::instruction::InstructionAccount::readonly(&addresses[i]),
            _ => ::pinocchio::instruction::InstructionAccount::writable(&addresses[i]),
        });
        let ix_accounts = &ix_accounts_all[..extra_len + 6];

        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: &::pinocchio_associated_token_account::ID,
            accounts: ix_accounts,
            data: &[1u8],
        };

        let payer_handle: CpiHandle<'_> = CpiHandle::from(payer);
        let mut cpi_handles: [CpiHandle<'_>; MAX_ATA_CREATE_EXTRA_ACCOUNTS + 6] =
            core::array::from_fn(|_| payer_handle.clone());
        cpi_handles[1] = CpiHandle::from(associated_token);
        cpi_handles[2] = authority;
        cpi_handles[3] = mint;
        cpi_handles[4] = system_program;
        cpi_handles[5] = token_program;
        cpi_handles[6..6 + extra_len].clone_from_slice(extra_accounts);

        crate::cpi::invoke_signed_pinocchio_handles(
            &instruction,
            &cpi_handles[..extra_len + 6],
            signer_seeds,
        )
    }
}

#[cfg(feature = "pinocchio")]
fn invoke_ata_ix<I: PinocchioAtaInstruction>(ix: I, signer_seeds: &[&[&[u8]]]) -> Result<()> {
    if signer_seeds.is_empty() {
        return ix.invoke();
    }

    if signer_seeds.len() > crate::prelude::MAX_CPI_SIGNERS {
        return Err(crate::prelude::NaclacError::TooManyCpiSigners.into());
    }
    for parts in signer_seeds.iter() {
        if parts.len() > crate::prelude::MAX_CPI_SEEDS_PER_SIGNER {
            return Err(crate::prelude::NaclacError::TooManyCpiSeeds.into());
        }
    }

    let mut signers = [const { core::mem::MaybeUninit::<::pinocchio::cpi::Signer>::uninit() };
        crate::prelude::MAX_CPI_SIGNERS];
    let mut all_seeds = [const {
        [const { core::mem::MaybeUninit::<::pinocchio::cpi::Seed>::uninit() };
            crate::prelude::MAX_CPI_SEEDS_PER_SIGNER]
    }; crate::prelude::MAX_CPI_SIGNERS];

    let signer_len = signer_seeds.len();
    for i in 0..signer_len {
        unsafe {
            let src_signer = *signer_seeds.get_unchecked(i);
            let seed_len = src_signer.len();
            for j in 0..seed_len {
                let src_seed = *src_signer.get_unchecked(j);
                let seed_cell = &mut *all_seeds.get_unchecked_mut(i).as_mut_ptr().add(j);
                seed_cell.write(::pinocchio::cpi::Seed::from(src_seed));
            }
            let seeds_slice = core::slice::from_raw_parts(
                all_seeds.get_unchecked(i).as_ptr() as *const ::pinocchio::cpi::Seed,
                seed_len,
            );
            let signer_cell = &mut *signers.as_mut_ptr().add(i);
            signer_cell.write(::pinocchio::cpi::Signer::from(seeds_slice));
        }
    }
    let signers = unsafe {
        core::slice::from_raw_parts(
            signers.as_ptr() as *const ::pinocchio::cpi::Signer,
            signer_len,
        )
    };

    ix.invoke_signed(signers)
}

#[cfg(feature = "pinocchio")]
trait PinocchioAtaInstruction {
    fn invoke(&self) -> Result<()>;
    fn invoke_signed(&self, signers: &[::pinocchio::cpi::Signer]) -> Result<()>;
}

#[cfg(feature = "pinocchio")]
impl PinocchioAtaInstruction for pinocchio_associated_token_account::instructions::Create<'_> {
    fn invoke(&self) -> Result<()> {
        pinocchio_associated_token_account::instructions::Create::invoke(self)
    }
    fn invoke_signed(&self, signers: &[::pinocchio::cpi::Signer]) -> Result<()> {
        pinocchio_associated_token_account::instructions::Create::invoke_signed(self, signers)
    }
}

#[cfg(feature = "pinocchio")]
impl PinocchioAtaInstruction
    for pinocchio_associated_token_account::instructions::CreateIdempotent<'_>
{
    fn invoke(&self) -> Result<()> {
        pinocchio_associated_token_account::instructions::CreateIdempotent::invoke(self)
    }
    fn invoke_signed(&self, signers: &[::pinocchio::cpi::Signer]) -> Result<()> {
        pinocchio_associated_token_account::instructions::CreateIdempotent::invoke_signed(
            self, signers,
        )
    }
}

#[derive(Debug)]
pub struct CreateAtaAccounts<'a, A, B, C, D, E, F> {
    pub payer: &'a mut A,
    pub associated_token: &'a mut B,
    pub authority: &'a C,
    pub mint: &'a D,
    pub system_program: &'a E,
    pub token_program: &'a F,
}

pub trait AssociatedTokenCpi {
    fn create<'a, A, B, C, D, E, F>(
        &self,
        accounts: CreateAtaAccounts<'a, A, B, C, D, E, F>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>,
        D: ToCpiHandle<'a>,
        E: ToCpiHandle<'a>,
        F: ToCpiHandle<'a>;

    fn create_signed<'a, A, B, C, D, E, F>(
        &self,
        accounts: CreateAtaAccounts<'a, A, B, C, D, E, F>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>,
        D: ToCpiHandle<'a>,
        E: ToCpiHandle<'a>,
        F: ToCpiHandle<'a>;

    fn create_idempotent<'a, A, B, C, D, E, F>(
        &self,
        accounts: CreateAtaAccounts<'a, A, B, C, D, E, F>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>,
        D: ToCpiHandle<'a>,
        E: ToCpiHandle<'a>,
        F: ToCpiHandle<'a>;

    fn create_idempotent_signed<'a, A, B, C, D, E, F>(
        &self,
        accounts: CreateAtaAccounts<'a, A, B, C, D, E, F>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>,
        D: ToCpiHandle<'a>,
        E: ToCpiHandle<'a>,
        F: ToCpiHandle<'a>;
}

impl AssociatedTokenCpi for crate::wrappers::Program<crate::wrappers::AssociatedToken> {
    fn create<'a, A, B, C, D, E, F>(
        &self,
        accounts: CreateAtaAccounts<'a, A, B, C, D, E, F>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>,
        D: ToCpiHandle<'a>,
        E: ToCpiHandle<'a>,
        F: ToCpiHandle<'a>,
    {
        let program_h = self.to_cpi_handle();
        let payer_h = accounts.payer.to_cpi_handle_mut();
        let ata_h = accounts.associated_token.to_cpi_handle_mut();
        let authority_h = accounts.authority.to_cpi_handle();
        let mint_h = accounts.mint.to_cpi_handle();
        let sys_h = accounts.system_program.to_cpi_handle();
        let tok_h = accounts.token_program.to_cpi_handle();
        create(
            program_h,
            AtaCpiAccounts {
                payer: payer_h,
                associated_token: ata_h,
                authority: authority_h,
                mint: mint_h,
                system_program: sys_h,
                token_program: tok_h,
            },
        )
    }

    fn create_signed<'a, A, B, C, D, E, F>(
        &self,
        accounts: CreateAtaAccounts<'a, A, B, C, D, E, F>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>,
        D: ToCpiHandle<'a>,
        E: ToCpiHandle<'a>,
        F: ToCpiHandle<'a>,
    {
        let program_h = self.to_cpi_handle();
        let payer_h = accounts.payer.to_cpi_handle_mut();
        let ata_h = accounts.associated_token.to_cpi_handle_mut();
        let authority_h = accounts.authority.to_cpi_handle();
        let mint_h = accounts.mint.to_cpi_handle();
        let sys_h = accounts.system_program.to_cpi_handle();
        let tok_h = accounts.token_program.to_cpi_handle();
        create_signed(
            program_h,
            AtaCpiAccounts {
                payer: payer_h,
                associated_token: ata_h,
                authority: authority_h,
                mint: mint_h,
                system_program: sys_h,
                token_program: tok_h,
            },
            signer_seeds,
        )
    }

    fn create_idempotent<'a, A, B, C, D, E, F>(
        &self,
        accounts: CreateAtaAccounts<'a, A, B, C, D, E, F>,
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>,
        D: ToCpiHandle<'a>,
        E: ToCpiHandle<'a>,
        F: ToCpiHandle<'a>,
    {
        let program_h = self.to_cpi_handle();
        let payer_h = accounts.payer.to_cpi_handle_mut();
        let ata_h = accounts.associated_token.to_cpi_handle_mut();
        let authority_h = accounts.authority.to_cpi_handle();
        let mint_h = accounts.mint.to_cpi_handle();
        let sys_h = accounts.system_program.to_cpi_handle();
        let tok_h = accounts.token_program.to_cpi_handle();
        create_idempotent(
            program_h,
            AtaCpiAccounts {
                payer: payer_h,
                associated_token: ata_h,
                authority: authority_h,
                mint: mint_h,
                system_program: sys_h,
                token_program: tok_h,
            },
        )
    }

    fn create_idempotent_signed<'a, A, B, C, D, E, F>(
        &self,
        accounts: CreateAtaAccounts<'a, A, B, C, D, E, F>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()>
    where
        A: ToCpiHandleMut<'a>,
        B: ToCpiHandleMut<'a>,
        C: ToCpiHandle<'a>,
        D: ToCpiHandle<'a>,
        E: ToCpiHandle<'a>,
        F: ToCpiHandle<'a>,
    {
        let program_h = self.to_cpi_handle();
        let payer_h = accounts.payer.to_cpi_handle_mut();
        let ata_h = accounts.associated_token.to_cpi_handle_mut();
        let authority_h = accounts.authority.to_cpi_handle();
        let mint_h = accounts.mint.to_cpi_handle();
        let sys_h = accounts.system_program.to_cpi_handle();
        let tok_h = accounts.token_program.to_cpi_handle();
        create_idempotent_signed(
            program_h,
            AtaCpiAccounts {
                payer: payer_h,
                associated_token: ata_h,
                authority: authority_h,
                mint: mint_h,
                system_program: sys_h,
                token_program: tok_h,
            },
            signer_seeds,
        )
    }
}
