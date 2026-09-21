// ===========================================================================
// execute.rs — `ExecuteV1`: running a CPI as the asset
// ===========================================================================

//! `ExecuteV1` (discriminator `31`) lets an asset act as a signer for an
//! arbitrary inner CPI, via a derived PDA (`asset_signer`, seeds
//! `["mpl-core-execute", asset_pubkey]`, owned by the Core program) that
//! **Core itself signs for internally** — the calling program never signs
//! it. Verified directly against the real processor
//! (`processor/execute.rs`): the inner instruction's `AccountMeta`s are
//! built by copying each remaining account's actual runtime `is_signer`
//! bit through unchanged, *except* whichever one matches `asset_signer`,
//! which Core force-sets to `is_signer: true` regardless of what the
//! caller passed. `asset_signer` itself is not derived by this crate —
//! it's an ordinary PDA account the caller supplies, the same way any
//! other `#[account(seeds = [...], bump = ...)]` field is.
//!
//! **Both `payer` modes are supported**, verified directly in the real
//! processor: a normal wallet payer (`assert_signer(payer)`), or the
//! asset's own `asset_signer` PDA paying its own execute fee from its own
//! lamports (`payer_is_pda`) — in that mode `authority` becomes
//! *required*, not optional (`authority.ok_or(MissingSigner)?` then
//! `assert_signer(authority)`), which `execute_signed` enforces up front
//! rather than building a CPI that would fail on-chain. The PDA-payer mode
//! needs the *same* underlying account referenced twice in one
//! instruction — once read-only as `asset_signer`, once writable as
//! `payer` (the fee is subtracted from it) — which is safe here: the real
//! `solana_program::program::invoke_signed`'s own safety check only takes
//! sequential, transient borrows (one `AccountMeta` at a time, each
//! immediately dropped before the next), never two overlapping ones, and
//! `CpiHandle`/`CpiHandleMut`'s `info` field is `pub`, so a second handle
//! for the same account is built by cloning it directly — the exact
//! pattern the real processor itself uses (`ctx.accounts.payer.clone()`)
//! for this same "same account, two roles" case.

use crate::prelude::*;

/// One account for the inner CPI `execute_signed` runs as the asset —
/// mirrors the real SDK's `remaining_accounts: &[AccountMeta]` parameter,
/// since a plain `CpiHandle` doesn't carry `is_signer`/`is_writable`
/// intent on its own.
pub struct ExecuteRemainingAccount<'a> {
    pub handle: CpiHandle<'a>,
    pub is_signer: bool,
    pub is_writable: bool,
}

/// Who pays the real execute fee (`get_execute_fee()` in the real
/// program) — see this file's header for the "same account, two roles"
/// mechanics behind the `AssetSignerPda` variant.
pub enum ExecutePayer<'a> {
    /// A real wallet pays — must be a signer.
    Wallet(CpiHandleMut<'a>),
    /// The asset's own `asset_signer` PDA pays its own execute fee from
    /// its own lamports. Requires `accounts.authority` to be `Some` — the
    /// real processor requires a real signer in this mode since the PDA
    /// itself can't independently authorize the *action*, only the fee
    /// payment. `execute_signed` returns `NaclacError::ConstraintAccountIsNone`
    /// up front if `authority` is missing here, rather than building a CPI
    /// that would fail on-chain.
    AssetSignerPda,
}

/// Accounts consumed by `execute_signed` — mirrors the real `ExecuteV1`
/// instruction's own fixed accounts exactly (`remaining_accounts`, the
/// inner CPI's own accounts, are passed separately). `payer` is not a
/// plain field here — see `ExecutePayer`.
pub struct ExecuteAssetAccounts<'a> {
    pub asset: CpiHandleMut<'a>,
    pub collection: Option<CpiHandleMut<'a>>,
    pub asset_signer: CpiHandle<'a>,
    pub payer: ExecutePayer<'a>,
    pub authority: Option<CpiHandle<'a>>,
    pub system_program: CpiHandle<'a>,
    pub target_program: CpiHandle<'a>,
}

/// Duplicates an `AccountInfo` for the "same account, two roles" case (see
/// this file's header). `AccountInfo` is `Clone`-only on `solana` but also
/// `Copy` on `pinocchio` — a plain `.clone()` call is correct on both but
/// triggers `clippy::clone_on_copy` on the branch where it's redundant, so
/// each backend gets its own real (not suppressed) implementation.
#[cfg(not(feature = "pinocchio"))]
fn clone_account_info(info: &AccountInfo) -> AccountInfo {
    info.clone()
}
#[cfg(feature = "pinocchio")]
fn clone_account_info(info: &AccountInfo) -> AccountInfo {
    *info
}

/// Runs `instruction_data` as a CPI to `accounts.target_program`, signed by
/// the asset's own derived PDA, via a real `ExecuteV1` CPI.
pub fn execute_signed(
    program: CpiHandle<'_>,
    accounts: ExecuteAssetAccounts<'_>,
    instruction_data: &[u8],
    remaining_accounts: &[ExecuteRemainingAccount<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    if matches!(accounts.payer, ExecutePayer::AssetSignerPda) && accounts.authority.is_none() {
        return Err(NaclacError::ConstraintAccountIsNone.into());
    }

    // `payer_handle_mut`/`payer_is_signer` resolve `ExecutePayer` into the
    // concrete account + signer flag the rest of this function needs,
    // regardless of which mode was chosen.
    let (payer_handle_mut, payer_is_signer): (CpiHandleMut<'_>, bool) = match accounts.payer {
        ExecutePayer::Wallet(handle) => (handle, true),
        ExecutePayer::AssetSignerPda => (
            CpiHandleMut {
                info: clone_account_info(&accounts.asset_signer.info),
                _phantom: core::marker::PhantomData,
            },
            false,
        ),
    };

    #[cfg(not(feature = "pinocchio"))]
    {
        // ExecuteV1 wire format verified against the real `mpl-core` crate's
        // own generated `instructions/execute_v1.rs`: discriminator(1) +
        // `instruction_data: Vec<u8>` (4-byte len + bytes). Account order
        // (asset, collection, asset_signer, payer, authority,
        // system_program, target_program, ...remaining) matches the
        // `pinocchio` branch below exactly, including `payer`'s
        // signer-or-not flag depending on `ExecutePayer` mode.
        let mut data = crate::prelude::Vec::with_capacity(5 + instruction_data.len());
        data.push(31u8); // ExecuteV1 discriminator
        data.extend_from_slice(&(instruction_data.len() as u32).to_le_bytes());
        data.extend_from_slice(instruction_data);

        let mut accounts_meta = vec![
            solana_program::instruction::AccountMeta::new(accounts.asset.address(), false),
            match &accounts.collection {
                Some(c) => solana_program::instruction::AccountMeta::new(c.address(), false),
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            solana_program::instruction::AccountMeta::new_readonly(
                accounts.asset_signer.address(),
                false,
            ),
            solana_program::instruction::AccountMeta::new(
                payer_handle_mut.address(),
                payer_is_signer,
            ),
            match &accounts.authority {
                Some(a) => {
                    solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            solana_program::instruction::AccountMeta::new_readonly(
                accounts.system_program.address(),
                false,
            ),
            solana_program::instruction::AccountMeta::new_readonly(
                accounts.target_program.address(),
                false,
            ),
        ];
        for a in remaining_accounts {
            accounts_meta.push(solana_program::instruction::AccountMeta {
                pubkey: a.handle.address(),
                is_signer: a.is_signer,
                is_writable: a.is_writable,
            });
        }

        let ix = solana_program::instruction::Instruction {
            program_id: crate::ID,
            accounts: accounts_meta,
            data,
        };

        let mut cpi_accounts: crate::prelude::Vec<CpiHandle<'_>> =
            crate::prelude::Vec::with_capacity(8 + remaining_accounts.len());
        cpi_accounts.push(CpiHandle::from(accounts.asset));
        cpi_accounts.push(
            accounts
                .collection
                .map(CpiHandle::from)
                .unwrap_or_else(|| program.clone()),
        );
        cpi_accounts.push(accounts.asset_signer);
        cpi_accounts.push(CpiHandle::from(payer_handle_mut));
        cpi_accounts.push(accounts.authority.unwrap_or_else(|| program.clone()));
        cpi_accounts.push(accounts.system_program);
        cpi_accounts.push(accounts.target_program);
        for a in remaining_accounts {
            cpi_accounts.push(a.handle.clone());
        }
        cpi_accounts.push(program);
        crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::prelude::Vec::with_capacity(6 + instruction_data.len());
        data.push(31u8); // ExecuteV1 discriminator
        data.extend_from_slice(&(instruction_data.len() as u32).to_le_bytes());
        data.extend_from_slice(instruction_data);

        let asset_handle: CpiHandle<'_> = CpiHandle::from(accounts.asset);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(payer_handle_mut);
        let collection_is_some = accounts.collection.is_some();
        let authority_is_some = accounts.authority.is_some();
        let collection_handle = accounts.collection.map(CpiHandle::from).unwrap_or(program);
        let authority_handle = accounts.authority.unwrap_or(program);

        const FIXED_ACCOUNTS: usize = 7;
        if !cpi_account_count_fits(FIXED_ACCOUNTS, remaining_accounts.len(), MAX_CPI_ACCOUNTS) {
            return Err(NaclacError::TooManyCpiAccounts.into());
        }

        // Fixed-capacity, zero-heap account arrays: every slot starts as a
        // harmless placeholder (`program`'s own handle/address — the same
        // "unused optional slot" convention used everywhere else in this
        // crate), then the first `FIXED_ACCOUNTS + remaining_accounts.len()`
        // slots are overwritten with real values before the array is sliced
        // down to exactly that length, so no placeholder is ever read.
        let mut ix_accounts: [::pinocchio::instruction::InstructionAccount<'_>; MAX_CPI_ACCOUNTS] =
            core::array::from_fn(|_| {
                ::pinocchio::instruction::InstructionAccount::readonly(program.info.view.address())
            });
        ix_accounts[0] =
            ::pinocchio::instruction::InstructionAccount::writable(asset_handle.info.view.address());
        ix_accounts[1] = if collection_is_some {
            ::pinocchio::instruction::InstructionAccount::writable(
                collection_handle.info.view.address(),
            )
        } else {
            ::pinocchio::instruction::InstructionAccount::readonly(
                collection_handle.info.view.address(),
            )
        };
        ix_accounts[2] = ::pinocchio::instruction::InstructionAccount::readonly(
            accounts.asset_signer.info.view.address(),
        );
        ix_accounts[3] = if payer_is_signer {
            ::pinocchio::instruction::InstructionAccount::writable_signer(
                payer_handle.info.view.address(),
            )
        } else {
            ::pinocchio::instruction::InstructionAccount::writable(
                payer_handle.info.view.address(),
            )
        };
        ix_accounts[4] = if authority_is_some {
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                authority_handle.info.view.address(),
            )
        } else {
            ::pinocchio::instruction::InstructionAccount::readonly(
                authority_handle.info.view.address(),
            )
        };
        ix_accounts[5] = ::pinocchio::instruction::InstructionAccount::readonly(
            accounts.system_program.info.view.address(),
        );
        ix_accounts[6] = ::pinocchio::instruction::InstructionAccount::readonly(
            accounts.target_program.info.view.address(),
        );
        for (i, a) in remaining_accounts.iter().enumerate() {
            ix_accounts[FIXED_ACCOUNTS + i] = match (a.is_writable, a.is_signer) {
                (true, true) => ::pinocchio::instruction::InstructionAccount::writable_signer(
                    a.handle.info.view.address(),
                ),
                (true, false) => ::pinocchio::instruction::InstructionAccount::writable(
                    a.handle.info.view.address(),
                ),
                (false, true) => ::pinocchio::instruction::InstructionAccount::readonly_signer(
                    a.handle.info.view.address(),
                ),
                (false, false) => ::pinocchio::instruction::InstructionAccount::readonly(
                    a.handle.info.view.address(),
                ),
            };
        }
        let total = FIXED_ACCOUNTS + remaining_accounts.len();

        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts[..total],
            data: &data,
        };

        let mut handles = [program; MAX_CPI_ACCOUNTS];
        handles[0] = asset_handle;
        handles[1] = collection_handle;
        handles[2] = accounts.asset_signer;
        handles[3] = payer_handle;
        handles[4] = authority_handle;
        handles[5] = accounts.system_program;
        handles[6] = accounts.target_program;
        for (i, a) in remaining_accounts.iter().enumerate() {
            handles[FIXED_ACCOUNTS + i] = a.handle;
        }
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles[..total], signer_seeds)
    }
}
