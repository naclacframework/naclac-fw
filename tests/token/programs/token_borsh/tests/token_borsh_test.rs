use naclac_client::*;
use token_borsh_client::{
    fetch_mint_authority, get_mint_authority_pda,
    instructions::{
        build_approve_vault_delegate2022,
        build_check_ata_constraints, build_check_default_account_state,
        build_check_cpi_guard,
        build_check_group_member_pointer, build_check_group_pointer,
        build_check_immutable_owner,
        build_check_interest_bearing_mint,
        build_check_memo_transfer,
        build_check_metadata_pointer,
        build_check_mint_close_authority,
        build_check_mint_freeze_authority,
        build_check_pausable_config,
        build_check_permanent_delegate,
        build_check_permissioned_burn,
        build_check_scaled_ui_amount_config,
        build_check_transfer_fee_config, build_check_transfer_hook, build_check_vault_constraints,
        build_check_vault_program, build_create_ata, build_create_ata_idempotent, build_create_mint,
        build_create_mint2022,
        build_create_mint2022_init_if_needed,
        build_close_mint2022,
        build_create_mint2022_with_mint_close_authority,
        build_create_mint2022_with_pausable,
        build_create_mint2022_with_permissioned_burn,
        build_create_mint2022_with_scaled_ui_amount,
        build_create_mint2022_with_transfer_hook,
        build_check_token_group, build_check_token_group_member,
        build_create_mint2022_with_group_member_pointer_and_member,
        build_create_mint2022_with_group_pointer_and_group,
        build_create_mint2022_with_metadata_pointer_and_metadata,
        build_exercise_token_metadata_lifecycle,
        build_exercise_token_metadata_remove_key_and_authority,
        build_exercise_update_token_group,
        build_create_mint_with_freeze, build_create_token_account_with_memo_transfer_required,
        build_exercise_memo_transfer_disable, build_exercise_pause_mint,
        build_exercise_permissioned_burn,
        build_exercise_resume_mint, build_exercise_transfer_checked_with_hook,
        build_exercise_update_scaled_ui_amount_multiplier,
        build_init_mint_authority, build_mint_to_vault, build_mint_to_vault2022,
        build_transfer_tokens, build_transfer_tokens2022, build_transfer_tokens2022_with_memo,
        build_burn_vault_tokens2022,
        ApproveVaultDelegate2022Accounts,
        BurnVaultTokens2022Accounts,
        CheckAtaConstraintsAccounts, CheckDefaultAccountStateAccounts,
        CheckCpiGuardAccounts,
        CheckGroupMemberPointerAccounts, CheckGroupPointerAccounts,
        CheckImmutableOwnerAccounts,
        CheckInterestBearingMintAccounts,
        CheckMemoTransferAccounts,
        CheckMetadataPointerAccounts,
        CheckMintCloseAuthorityAccounts,
        CheckMintFreezeAuthorityAccounts,
        CheckPausableConfigAccounts,
        CheckPermanentDelegateAccounts,
        CheckPermissionedBurnAccounts,
        CheckScaledUiAmountConfigAccounts,
        CheckTokenGroupAccounts, CheckTokenGroupMemberAccounts,
        CheckTransferFeeConfigAccounts, CheckTransferHookAccounts, CheckVaultConstraintsAccounts,
        CheckVaultProgramAccounts, CreateAtaAccounts,
        CreateAtaIdempotentAccounts, CreateMint2022WithTransferHookAccounts, CreateMintAccounts,
        CreateMint2022Accounts,
        CreateMint2022InitIfNeededAccounts,
        CreateMint2022WithGroupMemberPointerAndMemberAccounts,
        CreateMint2022WithGroupPointerAndGroupAccounts,
        CreateMint2022WithMetadataPointerAndMetadataAccounts,
        CloseMint2022Accounts,
        CreateMint2022WithMintCloseAuthorityAccounts,
        CreateMint2022WithPausableAccounts,
        CreateMint2022WithPermissionedBurnAccounts,
        CreateMint2022WithScaledUiAmountAccounts,
        CreateMintWithFreezeAccounts,
        CreateTokenAccountWithMemoTransferRequiredAccounts,
        ExerciseMemoTransferDisableAccounts,
        ExercisePauseMintAccounts,
        ExercisePermissionedBurnAccounts,
        ExerciseResumeMintAccounts,
        ExerciseTokenMetadataLifecycleAccounts,
        ExerciseTokenMetadataRemoveKeyAndAuthorityAccounts,
        ExerciseTransferCheckedWithHookAccounts,
        ExerciseUpdateScaledUiAmountMultiplierAccounts,
        ExerciseUpdateTokenGroupAccounts,
        InitMintAuthorityAccounts, MintToVault2022Accounts, MintToVaultAccounts,
        TransferTokens2022Accounts, TransferTokens2022WithMemoAccounts, TransferTokensAccounts,
    },
    types::{
        CheckInterestBearingMintArgs, CheckTransferFeeConfigArgs,
        CreateMint2022WithGroupMemberPointerAndMemberArgs,
        CreateMint2022WithGroupPointerAndGroupArgs,
        CreateMint2022WithMetadataPointerAndMetadataArgs,
        CreateMint2022WithTransferHookArgs, PROGRAM_ID,
    },
};

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

/// Asserts a transaction failed with exactly the given `Custom` error code
/// â€” not just "any error", the specific numeric `NaclacError` (framework
/// errors, 3000s) the failure actually produces. Mirrors
/// `tests/error-codes/programs/error_codes/tests/error_codes_test.rs`'s
/// helper of the same name and shape.
fn assert_custom_code(result: Result<NaclacTransactionMetadata, NaclacClientError>, expected: u32) {
    match result {
        Err(NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            ..
        }) => {
            assert_eq!(code, expected, "wrong custom error code");
        }
        other => panic!("expected a Custom({expected}) transaction failure, got {other:?}"),
    }
}

fn read_token_account_amount(provider: &NaclacProvider, address: &Address) -> u64 {
    let data = provider.get_account_data(address).expect("token account should exist");
    u64::from_le_bytes(data[64..72].try_into().unwrap())
}

/// `mint::decimals` + `mint::authority` combined with `init` (real CPI mint
/// creation via `init_cpi.rs`, mirroring `examples/launchpad`), then a real
/// `mint_to`/`transfer` CPI signed by the `mint_authority` PDA â€” the whole
/// happy path exercising actual SPL Token program logic via litesvm, not a
/// mocked CPI (per `TEST_PLAN.md`'s explicit requirement for this case).
#[test]
fn mint_creation_and_token_cpis_work_end_to_end() {
    let provider = setup();
    let (mint_authority_pda, mint_authority_bump) = get_mint_authority_pda(&PROGRAM_ID);

    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let fetched_authority = fetch_mint_authority(&provider, &mint_authority_pda)
        .expect("mint_authority should be readable");
    assert_eq!(fetched_authority.bump, mint_authority_bump);

    let id: u64 = 0;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMintAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint should succeed (real CPI mint creation via mint::decimals/mint::authority + init)");

    let mint_account = provider.get_account(&mint_pda).expect("mint account should exist");
    assert_eq!(
        mint_account.owner, TOKEN_PROGRAM_ID,
        "the mint account created via init + mint::* must actually be owned by the token program"
    );
    assert_eq!(mint_account.data[44], 6, "mint::decimals must be reflected in the real SPL mint layout");

    let vault_a = Keypair::new();
    create_token_account(&provider, &vault_a, &mint_pda, &mint_authority_pda)
        .expect("client-side create_token_account for vault_a should succeed");
    let vault_b = Keypair::new();
    create_token_account(&provider, &vault_b, &mint_pda, &mint_authority_pda)
        .expect("client-side create_token_account for vault_b should succeed");

    build_mint_to_vault(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVaultAccounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault_a.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault should succeed (real mint_to CPI signed by the PDA authority)");

    assert_eq!(read_token_account_amount(&provider, &vault_a.address()), 1_000_000);

    build_transfer_tokens(
        &provider,
        PROGRAM_ID,
        400_000,
        TransferTokensAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            from: vault_a.address(),
            to: vault_b.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("transfer_tokens should succeed (real transfer CPI signed by the PDA authority)");

    assert_eq!(read_token_account_amount(&provider, &vault_a.address()), 600_000);
    assert_eq!(read_token_account_amount(&provider, &vault_b.address()), 400_000);
}

/// `token::mint`/`token::authority` on an existing (non-`init`) account:
/// a vault backed by the wrong mint is rejected (`ConstraintAccountIsNone`),
/// a vault owned by the wrong authority is rejected (`ConstraintAddress`) â€”
/// deliberately distinct error variants, confirmed from `security.rs`
/// directly rather than assumed. A correctly-matching vault is accepted.
#[test]
fn token_constraint_rejects_wrong_mint_and_wrong_authority() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);

    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id_a: u64 = 10;
    let (mint_a, mint_a_bump) =
        Address::find_program_address(&[b"mint", &id_a.to_le_bytes()], &PROGRAM_ID);
    build_create_mint(
        &provider,
        PROGRAM_ID,
        id_a,
        mint_a_bump,
        6,
        CreateMintAccounts {
            payer: provider.payer.address(),
            mint: mint_a,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint (mint_a) should succeed");

    let id_b: u64 = 11;
    let (mint_b, mint_b_bump) =
        Address::find_program_address(&[b"mint", &id_b.to_le_bytes()], &PROGRAM_ID);
    build_create_mint(
        &provider,
        PROGRAM_ID,
        id_b,
        mint_b_bump,
        6,
        CreateMintAccounts {
            payer: provider.payer.address(),
            mint: mint_b,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint (mint_b) should succeed");

    let correct_vault = Keypair::new();
    create_token_account(&provider, &correct_vault, &mint_a, &mint_authority_pda)
        .expect("correct_vault creation should succeed");

    let wrong_mint_vault = Keypair::new();
    create_token_account(&provider, &wrong_mint_vault, &mint_b, &mint_authority_pda)
        .expect("wrong_mint_vault creation should succeed");

    let wrong_authority_vault = Keypair::new();
    create_token_account(&provider, &wrong_authority_vault, &mint_a, &provider.payer.address())
        .expect("wrong_authority_vault creation should succeed");

    build_check_vault_constraints(
        &provider,
        PROGRAM_ID,
        CheckVaultConstraintsAccounts {
            mint: mint_a,
            mint_authority: mint_authority_pda,
            vault: correct_vault.address(),
        },
    )
    .send_and_confirm()
    .expect("check_vault_constraints must accept a vault whose mint and authority both match");

    let wrong_mint_result = build_check_vault_constraints(
        &provider,
        PROGRAM_ID,
        CheckVaultConstraintsAccounts {
            mint: mint_a,
            mint_authority: mint_authority_pda,
            vault: wrong_mint_vault.address(),
        },
    )
    .send_and_confirm();
    // `CheckVaultConstraints { mint, mint_authority, vault }` â€” `vault` is
    // field index 2; `token::mint` mismatch emits `ConstraintAccountIsNone`
    // (8), not `ConstraintAddress` (confirmed in `security.rs`'s
    // `token::mint` codegen branch) -> 3000 + 2*100 + 8 = 3208.
    assert_custom_code(wrong_mint_result, 3208);

    let wrong_authority_result = build_check_vault_constraints(
        &provider,
        PROGRAM_ID,
        CheckVaultConstraintsAccounts {
            mint: mint_a,
            mint_authority: mint_authority_pda,
            vault: wrong_authority_vault.address(),
        },
    )
    .send_and_confirm();
    // `CheckVaultConstraints { mint, mint_authority, vault }` â€” `vault` is
    // field index 2; `token::authority` mismatch emits `ConstraintAddress`
    // (3) -> 3000 + 2*100 + 3 = 3203.
    assert_custom_code(wrong_authority_result, 3203);

    // `TokenAccount`'s own baked-in owner check (`Discriminator::validate_account`,
    // `naclac-token/src/token.rs`) â€” distinct from the `token::mint`/
    // `token::authority` byte-comparison checks above, and runs first: it
    // rejects an account not owned by the Token program at all, regardless
    // of what its bytes happen to contain. `provider.payer.address()` is
    // owned by the System program, not the Token program, and â€” unlike
    // `mint_authority_pda` â€” isn't already referenced by another field in
    // this same call, so it doesn't also trip
    // `ConstraintDuplicateMutableAccount`.
    let wrong_owner_result = build_check_vault_constraints(
        &provider,
        PROGRAM_ID,
        CheckVaultConstraintsAccounts {
            mint: mint_a,
            mint_authority: mint_authority_pda,
            vault: provider.payer.address(),
        },
    )
    .send_and_confirm();
    // `vault` is field index 2; `ConstraintOwner` (4) -> 3000 + 2*100 + 4 = 3204.
    assert_custom_code(wrong_owner_result, 3204);

    // `TokenAccount`'s raw-layout validation (`validate_token_account_layout`,
    // `naclac-token/src/token.rs`) â€” distinct from both the owner check above
    // and the `token::mint`/`token::authority` byte-comparison checks: a
    // 165-byte, all-zero buffer genuinely owned by the Token program passes
    // the owner check (owner is correct) and has canonical `COption` tags
    // (all-zero == `None`, which is canonical), but its `state` byte (108)
    // is `0` (`Uninitialized`) â€” a shape the real Token program's
    // `InitializeAccount` would never actually leave on-chain.
    // `provider.set_account` writes ledger state directly (litesvm-only),
    // which is the only way to construct this fixture without a real
    // Token-program instruction to do it for us.
    let uninitialized_vault = Keypair::new();
    provider
        .set_account(&uninitialized_vault.address(), vec![0u8; 165], &TOKEN_PROGRAM_ID, 1_000_000)
        .expect("set_account for uninitialized_vault should succeed");
    let uninitialized_result = build_check_vault_constraints(
        &provider,
        PROGRAM_ID,
        CheckVaultConstraintsAccounts {
            mint: mint_a,
            mint_authority: mint_authority_pda,
            vault: uninitialized_vault.address(),
        },
    )
    .send_and_confirm();
    // `vault` is field index 2; `AccountNotInitialized` (15) -> 3000 + 2*100 + 15 = 3215.
    assert_custom_code(uninitialized_result, 3215);
}

/// `token::program`: validates an existing account's owning program against
/// an expected `Program<Token>` field
/// (`generate_relational_checks`/`security.rs`, post account-load). A real
/// SPL token account genuinely owned by the Token program is accepted; an
/// account owned by our own program (the `mint_authority` PDA) is rejected
/// with `NaclacError::ProgramIdMismatch` â€” a distinct error variant from
/// `token::mint`'s `ConstraintAccountIsNone` and `token::authority`'s
/// `ConstraintAddress`, confirmed by reading the codegen directly.
#[test]
fn token_program_constraint_rejects_wrong_owning_program() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);

    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 50;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMintAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint should succeed");

    let real_vault = Keypair::new();
    create_token_account(&provider, &real_vault, &mint_pda, &mint_authority_pda)
        .expect("real_vault creation should succeed");

    build_check_vault_program(
        &provider,
        PROGRAM_ID,
        CheckVaultProgramAccounts {
            token_program: TOKEN_PROGRAM_ID,
            vault: real_vault.address(),
        },
    )
    .send_and_confirm()
    .expect("check_vault_program must accept a vault genuinely owned by the Token program");

    let wrong_program_result = build_check_vault_program(
        &provider,
        PROGRAM_ID,
        CheckVaultProgramAccounts {
            token_program: TOKEN_PROGRAM_ID,
            // Owned by our own program, not the Token program.
            vault: mint_authority_pda,
        },
    )
    .send_and_confirm();
    // `CheckVaultProgram { token_program, vault }` â€” `vault` is field index
    // 1; `token::program` mismatch emits `ProgramIdMismatch` (9), checked
    // post-load in `generate_relational_checks` -> 3000 + 1*100 + 9 = 3109.
    assert_custom_code(wrong_program_result, 3109);
}

/// `mint::freeze_authority`, both code paths: (a) combined with `init`
/// (`create_mint_with_freeze`, a real CPI-based mint creation setting the
/// freeze authority â€” `init_cpi.rs`'s freeze-authority branch, previously
/// unexercised since `create_mint` only ever set
/// `mint::decimals`/`mint::authority`), and (b) on an existing account
/// (`check_mint_freeze_authority`, `security.rs`'s raw SPL `Mint` byte-layout
/// read at `[46..50]`/`[50..82]`). A matching freeze authority is accepted;
/// a mismatched one is rejected with `NaclacError::ConstraintAddress`; a
/// mint that never had a freeze authority set at all (the original
/// `create_mint`'s mint) is rejected with `NaclacError::Unauthorized` â€”
/// deliberately distinct error variants, confirmed by reading the codegen
/// directly rather than assumed.
///
/// Unlike `init_mint_authority`/`init_counter`'s bare-`bump` auto-write-back
/// gotcha noted elsewhere in this test suite, `create_mint_with_freeze`'s
/// `mint` field is a bare `AccountInfo` with an *explicit* `bump = mint_bump`
/// (not a `#[component]` with a `.bump` field), so no manual bump write-back
/// is needed here in solana-borsh mode.
#[test]
fn mint_freeze_authority_distinguishes_missing_and_wrong_authority() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);

    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    // Mint with no freeze authority at all (the original `create_mint`).
    let no_freeze_id: u64 = 60;
    let (no_freeze_mint, no_freeze_bump) =
        Address::find_program_address(&[b"mint", &no_freeze_id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint(
        &provider,
        PROGRAM_ID,
        no_freeze_id,
        no_freeze_bump,
        6,
        CreateMintAccounts {
            payer: provider.payer.address(),
            mint: no_freeze_mint,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint (no freeze authority) should succeed");

    // Mint with a real freeze authority set at creation time.
    let freeze_id: u64 = 61;
    let (freeze_mint, freeze_bump) =
        Address::find_program_address(&[b"mint", &freeze_id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint_with_freeze(
        &provider,
        PROGRAM_ID,
        freeze_id,
        freeze_bump,
        6,
        CreateMintWithFreezeAccounts {
            payer: provider.payer.address(),
            mint: freeze_mint,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect(
        "create_mint_with_freeze should succeed (real CPI mint creation via \
         mint::decimals/mint::authority/mint::freeze_authority + init)",
    );

    build_check_mint_freeze_authority(
        &provider,
        PROGRAM_ID,
        CheckMintFreezeAuthorityAccounts {
            mint: freeze_mint,
            freeze_authority: mint_authority_pda,
        },
    )
    .send_and_confirm()
    .expect("check_mint_freeze_authority must accept the matching freeze authority");

    let stranger = Keypair::new();
    let wrong_authority_result = build_check_mint_freeze_authority(
        &provider,
        PROGRAM_ID,
        CheckMintFreezeAuthorityAccounts {
            mint: freeze_mint,
            freeze_authority: stranger.address(),
        },
    )
    .send_and_confirm();
    // `CheckMintFreezeAuthority { freeze_authority, mint }` â€” `mint` is
    // field index 1; a mismatched (but present) freeze authority emits
    // `ConstraintAddress` (3) -> 3000 + 1*100 + 3 = 3103.
    assert_custom_code(wrong_authority_result, 3103);

    let missing_authority_result = build_check_mint_freeze_authority(
        &provider,
        PROGRAM_ID,
        CheckMintFreezeAuthorityAccounts {
            mint: no_freeze_mint,
            freeze_authority: mint_authority_pda,
        },
    )
    .send_and_confirm();
    // `CheckMintFreezeAuthority { freeze_authority, mint }` â€” `mint` is
    // field index 1; no freeze authority set at all (COption discriminant
    // absent) emits `Unauthorized` (21) -> 3000 + 1*100 + 21 = 3121.
    assert_custom_code(missing_authority_result, 3121);
}

/// `AssociatedTokenCpi::create` â€” a real CPI to the Associated Token
/// Program (already preloaded by `LiteSVM::new()` by default, no
/// `.add_program()` needed) via `create_ata`'s manual CPI call in its
/// instruction body (same convention as `mint_to_vault`/`transfer_tokens`,
/// not a macro-level `#[account(...)]` constraint). Derives the real ATA
/// address off-chain (mirroring `naclac_client::create_ata_with_program`'s
/// own `Address::find_program_address(&[owner, token_program, mint],
/// ASSOCIATED_TOKEN_PROGRAM_ID)` derivation), calls `create_ata`, then
/// fetches the resulting account's raw bytes and confirms it deserializes as
/// a valid SPL token account with `mint` (bytes `[0..32]`) and `owner`
/// (bytes `[32..64]`) set correctly â€” proving the CPI genuinely created a
/// real, correctly-owned token account, not just that the instruction
/// didn't error.
#[test]
fn create_ata_creates_a_real_associated_token_account() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);

    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 70;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMintAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint should succeed");

    // A distinct keypair, not `provider.payer` itself: Solana collapses a
    // pubkey repeated across multiple account slots in one transaction into
    // a single writable meta if any occurrence requests `mut` â€” reusing the
    // payer's own address as `owner` here would silently alias `owner` onto
    // the same writable slot as `payer` (`#[account(mut)]`), which naclac's
    // `ConstraintDuplicateMutableAccount` check correctly rejects (a
    // runtime-only concern â€” which accounts share a pubkey depends on what's
    // actually passed into a given invocation, not on the struct's shape, so
    // this can never be caught at compile time).
    let owner = Keypair::new().address();
    let (ata_address, _ata_bump) = Address::find_program_address(
        &[owner.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint_pda.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    build_create_ata(
        &provider,
        PROGRAM_ID,
        CreateAtaAccounts {
            payer: provider.payer.address(),
            owner,
            mint: mint_pda,
            associated_token: ata_address,
            system_program: Address::default(),
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("create_ata should succeed (real CPI to the Associated Token Program)");

    let ata_account = provider
        .get_account(&ata_address)
        .expect("the associated token account should now exist");
    assert_eq!(
        ata_account.owner, TOKEN_PROGRAM_ID,
        "the ATA created via the Associated Token Program CPI must be owned by the Token program"
    );

    let data = &ata_account.data;
    assert_eq!(
        &data[0..32],
        mint_pda.as_ref(),
        "the ATA's raw `mint` field must match the mint it was created for"
    );
    assert_eq!(
        &data[32..64],
        owner.as_ref(),
        "the ATA's raw `owner` field must match the wallet it was created for"
    );
}

/// `create_ata_idempotent` (`init_if_needed` + `associated_token::mint`/
/// `::authority`) â€” first call creates the ATA exactly like `create_ata`;
/// the second call against the same already-existing ATA must succeed as a
/// no-op (`CreateIdempotent`), unlike `create_ata`'s strict `init`, which
/// reverts with `AccountAlreadyInitialized` on a repeat call.
#[test]
fn create_ata_idempotent_is_a_noop_on_second_call() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);

    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 71;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMintAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint should succeed");

    let owner = Keypair::new().address();
    let (ata_address, _ata_bump) = Address::find_program_address(
        &[owner.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint_pda.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    let ata_accounts = || CreateAtaIdempotentAccounts {
        payer: provider.payer.address(),
        owner,
        mint: mint_pda,
        associated_token: ata_address,
        system_program: Address::default(),
        token_program: TOKEN_PROGRAM_ID,
        associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
    };

    build_create_ata_idempotent(&provider, PROGRAM_ID, ata_accounts())
        .send_and_confirm()
        .expect("create_ata_idempotent should succeed on first call (creates the ATA)");

    let data_after_first = provider
        .get_account(&ata_address)
        .expect("the associated token account should now exist")
        .data;

    // A harmless extra readonly account, distinct per call, so the two
    // transactions aren't byte-identical (an identical tx sent twice in the
    // same blockhash window is rejected as `AlreadyProcessed` regardless of
    // program logic â€” this isn't a program-level concern).
    build_create_ata_idempotent(&provider, PROGRAM_ID, ata_accounts())
        .remaining_accounts(vec![AccountMeta::new_readonly(
            Keypair::new().address(),
            false,
        )])
        .send_and_confirm()
        .expect("create_ata_idempotent should succeed on second call (no-op, ATA already exists)");

    let data_after_second = provider
        .get_account(&ata_address)
        .expect("the associated token account should still exist")
        .data;

    assert_eq!(
        data_after_first, data_after_second,
        "a repeat idempotent call must not change the ATA's data"
    );
}

/// `associated_token::mint`/`::authority`/`::bump` on an *existing*
/// (non-`init`) account â€” the fix for the gap documented in
/// `naclac-token/docs/04-associated-token-existing-account-gap.md`, where
/// this constraint used to compile to nothing at all. Covers: the real ATA
/// with its correct bump succeeds; the real ATA with a wrong bump is
/// rejected (`ConstraintSeeds`, the hash-and-compare address recomputes to
/// something else); and a plain keypair-owned token account carrying the
/// *same* mint/owner data as the real ATA â€” but not actually the canonical
/// PDA â€” is also rejected (`ConstraintSeeds`), which is the exact spoofing
/// class the missing check used to let through silently.
#[test]
fn check_ata_constraints_validates_mint_authority_and_pda_bump() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);

    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 90;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMintAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint should succeed");

    let owner = Keypair::new().address();
    let (ata_address, ata_bump) = Address::find_program_address(
        &[owner.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint_pda.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    build_create_ata(
        &provider,
        PROGRAM_ID,
        CreateAtaAccounts {
            payer: provider.payer.address(),
            owner,
            mint: mint_pda,
            associated_token: ata_address,
            system_program: Address::default(),
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("create_ata should succeed");

    build_check_ata_constraints(
        &provider,
        PROGRAM_ID,
        ata_bump,
        CheckAtaConstraintsAccounts {
            mint: mint_pda,
            owner,
            associated_token: ata_address,
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("check_ata_constraints must accept the real ATA at its correct bump");

    let wrong_bump_result = build_check_ata_constraints(
        &provider,
        PROGRAM_ID,
        ata_bump.wrapping_sub(1),
        CheckAtaConstraintsAccounts {
            mint: mint_pda,
            owner,
            associated_token: ata_address,
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    // `CheckAtaConstraints { mint, owner, associated_token, token_program }`
    // â€” `associated_token` is field index 2; a bump mismatch fails the PDA
    // hash-and-compare with `ConstraintSeeds` (6) -> 3000 + 200 + 6 = 3206.
    assert_custom_code(wrong_bump_result, 3206);

    let spoofed_vault = Keypair::new();
    create_token_account(&provider, &spoofed_vault, &mint_pda, &owner)
        .expect("client-side create_token_account for spoofed_vault should succeed");

    let spoofed_result = build_check_ata_constraints(
        &provider,
        PROGRAM_ID,
        ata_bump,
        CheckAtaConstraintsAccounts {
            mint: mint_pda,
            owner,
            associated_token: spoofed_vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    // Same mint/owner data as the real ATA, but not the canonical PDA for
    // (owner, token_program, mint) â€” must still be rejected on the address
    // recomputation, same `ConstraintSeeds` (6) -> 3206.
    assert_custom_code(spoofed_result, 3206);
}

/// Hand-assembles a real Token-2022 `Mint` (82 bytes) plus a
/// `TransferFeeConfig` extension (108 bytes) in the exact TLV wire format â€”
/// same fixture as `tests/token/programs/token/tests/token_test.rs`'s
/// version, run here specifically to prove `get_extension` also works
/// through `Account<T>`'s Borsh branch (a genuinely different struct from
/// the zero-copy branch the pinocchio test exercises â€” see
/// `naclac-token/src/extensions.rs`'s `TokenInterfaceAccountExtensions`
/// impls).
fn build_transfer_fee_mint_bytes(
    withheld_amount: u64,
    older_epoch: u64,
    older_max_fee: u64,
    older_basis_points: u16,
    newer_epoch: u64,
    newer_max_fee: u64,
    newer_basis_points: u16,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(278);

    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.push(6);
    data.push(1);
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    assert_eq!(data.len(), 82);

    data.extend_from_slice(&[0u8; 83]);
    assert_eq!(data.len(), 165);

    data.push(1); // AccountType::Mint
    data.extend_from_slice(&1u16.to_le_bytes()); // ExtensionType::TransferFeeConfig
    data.extend_from_slice(&108u16.to_le_bytes());

    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&withheld_amount.to_le_bytes());
    data.extend_from_slice(&older_epoch.to_le_bytes());
    data.extend_from_slice(&older_max_fee.to_le_bytes());
    data.extend_from_slice(&older_basis_points.to_le_bytes());
    data.extend_from_slice(&newer_epoch.to_le_bytes());
    data.extend_from_slice(&newer_max_fee.to_le_bytes());
    data.extend_from_slice(&newer_basis_points.to_le_bytes());

    assert_eq!(data.len(), 278, "165 + 1 + 4 + 108 = 278");
    data
}

#[test]
fn check_transfer_fee_config_reads_extension_and_calculates_fee_correctly() {
    let provider = setup();

    let mint_bytes = build_transfer_fee_mint_bytes(500, 0, 1_000, 100, 0, 2_000, 200);
    let mint = Keypair::new();
    provider
        .set_account(&mint.address(), mint_bytes, &TOKEN_2022_PROGRAM_ID, 10_000_000)
        .expect("set_account for the transfer-fee mint fixture should succeed");

    let args = CheckTransferFeeConfigArgs {
        expected_withheld_amount: 500,
        expected_newer_basis_points: 200,
        expected_newer_maximum_fee: 2_000,
        current_epoch: 5,
        transfer_amount: 200_000,
        expected_fee: 2_000,
        expected_post_fee_amount: 198_000,
    };
    build_check_transfer_fee_config(
        &provider,
        PROGRAM_ID,
        args,
        CheckTransferFeeConfigAccounts { mint: mint.address() },
    )
    .send_and_confirm()
    .expect("check_transfer_fee_config must accept correct expected values");

    let wrong_args = CheckTransferFeeConfigArgs {
        expected_withheld_amount: 500,
        expected_newer_basis_points: 200,
        expected_newer_maximum_fee: 2_000,
        current_epoch: 5,
        transfer_amount: 200_000,
        expected_fee: 999_999, // deliberately wrong
        expected_post_fee_amount: 198_000,
    };
    let wrong_result = build_check_transfer_fee_config(
        &provider,
        PROGRAM_ID,
        wrong_args,
        CheckTransferFeeConfigAccounts { mint: mint.address() },
    )
    .send_and_confirm();
    assert_custom_code(wrong_result, 3003);
}

fn build_transfer_hook_mint_bytes(authority: [u8; 32], program_id: [u8; 32]) -> Vec<u8> {
    let mut data = Vec::with_capacity(234);

    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.push(6);
    data.push(1);
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    assert_eq!(data.len(), 82);

    data.extend_from_slice(&[0u8; 83]);
    assert_eq!(data.len(), 165);

    data.push(1); // AccountType::Mint
    data.extend_from_slice(&14u16.to_le_bytes()); // ExtensionType::TransferHook
    data.extend_from_slice(&64u16.to_le_bytes());

    data.extend_from_slice(&authority);
    data.extend_from_slice(&program_id);

    assert_eq!(data.len(), 234, "165 + 1 + 4 + 64 = 234");
    data
}

fn build_transfer_hook_token_account_bytes(transferring: bool) -> Vec<u8> {
    let mut data = Vec::with_capacity(171);

    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(1); // state: Initialized
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    assert_eq!(data.len(), 165);

    data.push(2); // AccountType::Account
    data.extend_from_slice(&15u16.to_le_bytes()); // ExtensionType::TransferHookAccount
    data.extend_from_slice(&1u16.to_le_bytes());
    data.push(if transferring { 1 } else { 0 });

    assert_eq!(data.len(), 171, "165 + 1 + 4 + 1 = 171");
    data
}

/// Same shape as `token_test.rs`'s `check_transfer_hook_reads_extensions_correctly`,
/// run here to prove `get_extension` works through the Borsh `Account<T>`
/// wrapper specifically.
#[test]
fn check_transfer_hook_reads_extensions_correctly() {
    let provider = setup();

    let authority = Keypair::new().address();
    let hook_program = Keypair::new().address();

    let mint_bytes = build_transfer_hook_mint_bytes(authority.to_bytes(), hook_program.to_bytes());
    let mint = Keypair::new();
    provider
        .set_account(&mint.address(), mint_bytes, &TOKEN_2022_PROGRAM_ID, 10_000_000)
        .expect("set_account for the transfer-hook mint fixture should succeed");

    let token_account_bytes = build_transfer_hook_token_account_bytes(true);
    let token_account = Keypair::new();
    provider
        .set_account(&token_account.address(), token_account_bytes, &TOKEN_2022_PROGRAM_ID, 10_000_000)
        .expect("set_account for the transfer-hook token account fixture should succeed");

    build_check_transfer_hook(
        &provider,
        PROGRAM_ID,
        Some(authority),
        Some(hook_program),
        1,
        CheckTransferHookAccounts { mint: mint.address(), token_account: token_account.address() },
    )
    .send_and_confirm()
    .expect("check_transfer_hook must accept correct expected values");

    let wrong_result = build_check_transfer_hook(
        &provider,
        PROGRAM_ID,
        Some(authority),
        Some(hook_program),
        0, // deliberately wrong: fixture has transferring = true
        CheckTransferHookAccounts { mint: mint.address(), token_account: token_account.address() },
    )
    .send_and_confirm();
    assert_custom_code(wrong_result, 3003);
}

fn build_permanent_delegate_mint_bytes(delegate: [u8; 32]) -> Vec<u8> {
    let mut data = Vec::with_capacity(202);

    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.push(6);
    data.push(1);
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    assert_eq!(data.len(), 82);

    data.extend_from_slice(&[0u8; 83]);
    assert_eq!(data.len(), 165);

    data.push(1); // AccountType::Mint
    data.extend_from_slice(&12u16.to_le_bytes()); // ExtensionType::PermanentDelegate
    data.extend_from_slice(&32u16.to_le_bytes());

    data.extend_from_slice(&delegate);

    assert_eq!(data.len(), 202, "165 + 1 + 4 + 32 = 202");
    data
}

/// Proves `InterfaceAccount<Mint>::get_extension::<PermanentDelegate>()`
/// (`naclac-token/src/extensions.rs`) reads the delegate field at the
/// correct byte offset under the Borsh backend too â€” `check_transfer_hook`'s
/// own hybrid gap (`11-tier2-owner-check-progress.md`) means this must be
/// checked per-backend, not assumed from the pinocchio test alone.
#[test]
fn check_permanent_delegate_reads_extension_correctly() {
    let provider = setup();

    let delegate = Keypair::new().address();

    let mint_bytes = build_permanent_delegate_mint_bytes(delegate.to_bytes());
    let mint = Keypair::new();
    provider
        .set_account(&mint.address(), mint_bytes, &TOKEN_2022_PROGRAM_ID, 10_000_000)
        .expect("set_account for the permanent-delegate mint fixture should succeed");

    build_check_permanent_delegate(
        &provider,
        PROGRAM_ID,
        Some(delegate),
        CheckPermanentDelegateAccounts { mint: mint.address() },
    )
    .send_and_confirm()
    .expect("check_permanent_delegate must accept the correct expected delegate");

    let wrong_result = build_check_permanent_delegate(
        &provider,
        PROGRAM_ID,
        None, // deliberately wrong: fixture has a real delegate set
        CheckPermanentDelegateAccounts { mint: mint.address() },
    )
    .send_and_confirm();
    assert_custom_code(wrong_result, 3003);
}

fn build_default_account_state_mint_bytes(state: u8) -> Vec<u8> {
    let mut data = Vec::with_capacity(171);

    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.push(6);
    data.push(1);
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    assert_eq!(data.len(), 82);

    data.extend_from_slice(&[0u8; 83]);
    assert_eq!(data.len(), 165);

    data.push(1); // AccountType::Mint
    data.extend_from_slice(&6u16.to_le_bytes()); // ExtensionType::DefaultAccountState
    data.extend_from_slice(&1u16.to_le_bytes());

    data.push(state);

    assert_eq!(data.len(), 171, "165 + 1 + 4 + 1 = 171");
    data
}

/// Proves `InterfaceAccount<Mint>::get_extension::<DefaultAccountState>()`
/// (`naclac-token/src/extensions/default_account_state.rs`) reads the state
/// field at the correct byte offset under the Borsh backend too.
#[test]
fn check_default_account_state_reads_extension_correctly() {
    let provider = setup();

    let mint_bytes = build_default_account_state_mint_bytes(2); // AccountState::Frozen
    let mint = Keypair::new();
    provider
        .set_account(&mint.address(), mint_bytes, &TOKEN_2022_PROGRAM_ID, 10_000_000)
        .expect("set_account for the default-account-state mint fixture should succeed");

    build_check_default_account_state(
        &provider,
        PROGRAM_ID,
        2,
        CheckDefaultAccountStateAccounts { mint: mint.address() },
    )
    .send_and_confirm()
    .expect("check_default_account_state must accept the correct expected state");

    let wrong_result = build_check_default_account_state(
        &provider,
        PROGRAM_ID,
        1, // deliberately wrong: fixture has state = Frozen (2)
        CheckDefaultAccountStateAccounts { mint: mint.address() },
    )
    .send_and_confirm();
    assert_custom_code(wrong_result, 3003);
}

fn build_immutable_owner_vault_bytes() -> Vec<u8> {
    let mut data = Vec::with_capacity(170);

    data.extend_from_slice(&[0u8; 32]); // mint
    data.extend_from_slice(&[0u8; 32]); // owner
    data.extend_from_slice(&0u64.to_le_bytes()); // amount
    data.extend_from_slice(&[0u8; 4]); // delegate COption tag
    data.extend_from_slice(&[0u8; 32]); // delegate pubkey
    data.push(1); // state: AccountState::Initialized
    data.extend_from_slice(&[0u8; 4]); // is_native COption tag
    data.extend_from_slice(&[0u8; 8]); // is_native value
    data.extend_from_slice(&0u64.to_le_bytes()); // delegated_amount
    data.extend_from_slice(&[0u8; 4]); // close_authority COption tag
    data.extend_from_slice(&[0u8; 32]); // close_authority pubkey
    assert_eq!(data.len(), 165);

    data.push(2); // AccountType::Account
    data.extend_from_slice(&7u16.to_le_bytes()); // ExtensionType::ImmutableOwner
    data.extend_from_slice(&0u16.to_le_bytes()); // zero-byte value

    assert_eq!(data.len(), 170, "165 + 1 + 4 + 0 = 170");
    data
}

/// Proves `InterfaceAccount<TokenAccount>::get_extension::<ImmutableOwner>()`
/// (`naclac-token/src/extensions/immutable_owner.rs`) reads the extension's
/// presence at the correct byte offset under the Borsh backend too.
#[test]
fn check_immutable_owner_reads_extension_correctly() {
    let provider = setup();

    let vault_bytes = build_immutable_owner_vault_bytes();
    let vault = Keypair::new();
    provider
        .set_account(&vault.address(), vault_bytes, &TOKEN_2022_PROGRAM_ID, 10_000_000)
        .expect("set_account for the immutable-owner vault fixture should succeed");

    build_check_immutable_owner(
        &provider,
        PROGRAM_ID,
        1,
        CheckImmutableOwnerAccounts { vault: vault.address() },
    )
    .send_and_confirm()
    .expect("check_immutable_owner must confirm the extension is present");

    let wrong_result = build_check_immutable_owner(
        &provider,
        PROGRAM_ID,
        0, // deliberately wrong: fixture has the extension present
        CheckImmutableOwnerAccounts { vault: vault.address() },
    )
    .send_and_confirm();
    assert_custom_code(wrong_result, 3003);
}

fn build_memo_transfer_vault_bytes(require_incoming_transfer_memos: u8) -> Vec<u8> {
    let mut data = Vec::with_capacity(171);

    data.extend_from_slice(&[0u8; 32]); // mint
    data.extend_from_slice(&[0u8; 32]); // owner
    data.extend_from_slice(&0u64.to_le_bytes()); // amount
    data.extend_from_slice(&[0u8; 4]); // delegate COption tag
    data.extend_from_slice(&[0u8; 32]); // delegate pubkey
    data.push(1); // state: AccountState::Initialized
    data.extend_from_slice(&[0u8; 4]); // is_native COption tag
    data.extend_from_slice(&[0u8; 8]); // is_native value
    data.extend_from_slice(&0u64.to_le_bytes()); // delegated_amount
    data.extend_from_slice(&[0u8; 4]); // close_authority COption tag
    data.extend_from_slice(&[0u8; 32]); // close_authority pubkey
    assert_eq!(data.len(), 165);

    data.push(2); // AccountType::Account
    data.extend_from_slice(&8u16.to_le_bytes()); // ExtensionType::MemoTransfer
    data.extend_from_slice(&1u16.to_le_bytes()); // 1-byte value
    data.push(require_incoming_transfer_memos);

    assert_eq!(data.len(), 171, "165 + 1 + 4 + 1 = 171");
    data
}

/// Proves `InterfaceAccount<TokenAccount>::get_extension::<MemoTransfer>()`
/// (`naclac-token/src/extensions/memo_transfer.rs`) reads both the
/// extension's presence and its `require_incoming_transfer_memos` flag at
/// the correct byte offset under the Borsh backend too.
#[test]
fn check_memo_transfer_reads_extension_correctly() {
    let provider = setup();

    let vault_bytes = build_memo_transfer_vault_bytes(1);
    let vault = Keypair::new();
    provider
        .set_account(&vault.address(), vault_bytes, &TOKEN_2022_PROGRAM_ID, 10_000_000)
        .expect("set_account for the memo-transfer vault fixture should succeed");

    build_check_memo_transfer(
        &provider,
        PROGRAM_ID,
        1,
        1,
        CheckMemoTransferAccounts { vault: vault.address() },
    )
    .send_and_confirm()
    .expect("check_memo_transfer must confirm the extension is present and required");

    let wrong_result = build_check_memo_transfer(
        &provider,
        PROGRAM_ID,
        1,
        0, // deliberately wrong: fixture has require_incoming_transfer_memos = 1
        CheckMemoTransferAccounts { vault: vault.address() },
    )
    .send_and_confirm();
    assert_custom_code(wrong_result, 3003);
}

fn build_interest_bearing_mint_mint_bytes(rate_authority: [u8; 32], current_rate: i16) -> Vec<u8> {
    let mut data = Vec::with_capacity(222);

    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.push(6);
    data.push(1);
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    assert_eq!(data.len(), 82);

    data.extend_from_slice(&[0u8; 83]);
    assert_eq!(data.len(), 165);

    data.push(1); // AccountType::Mint
    data.extend_from_slice(&10u16.to_le_bytes()); // ExtensionType::InterestBearingConfig
    data.extend_from_slice(&52u16.to_le_bytes());

    data.extend_from_slice(&rate_authority);
    data.extend_from_slice(&0i64.to_le_bytes()); // initialization_timestamp
    data.extend_from_slice(&0i16.to_le_bytes()); // pre_update_average_rate
    data.extend_from_slice(&0i64.to_le_bytes()); // last_update_timestamp
    data.extend_from_slice(&current_rate.to_le_bytes());

    assert_eq!(data.len(), 222, "165 + 1 + 4 + 52 = 222");
    data
}

/// Proves `InterfaceAccount<Mint>::get_extension::<InterestBearingConfig>()`
/// (`naclac-token/src/extensions/interest_bearing_mint.rs`) reads the rate
/// authority and rate fields at the correct byte offsets under the Borsh
/// backend too.
#[test]
fn check_interest_bearing_mint_reads_extension_correctly() {
    let provider = setup();

    let rate_authority = Keypair::new().address();
    let mint_bytes = build_interest_bearing_mint_mint_bytes(rate_authority.to_bytes(), 300);
    let mint = Keypair::new();
    provider
        .set_account(&mint.address(), mint_bytes, &TOKEN_2022_PROGRAM_ID, 10_000_000)
        .expect("set_account for the interest-bearing mint fixture should succeed");

    build_check_interest_bearing_mint(
        &provider,
        PROGRAM_ID,
        CheckInterestBearingMintArgs {
            expected_rate_authority: Some(rate_authority),
            expected_current_rate: 300,
        },
        CheckInterestBearingMintAccounts { mint: mint.address() },
    )
    .send_and_confirm()
    .expect("check_interest_bearing_mint must accept the correct expected values");

    let wrong_result = build_check_interest_bearing_mint(
        &provider,
        PROGRAM_ID,
        CheckInterestBearingMintArgs {
            expected_rate_authority: Some(rate_authority),
            expected_current_rate: 999, // deliberately wrong: fixture has current_rate = 300
        },
        CheckInterestBearingMintAccounts { mint: mint.address() },
    )
    .send_and_confirm();
    assert_custom_code(wrong_result, 3003);
}

fn build_pointer_mint_bytes(extension_type: u16, authority: [u8; 32], target: [u8; 32]) -> Vec<u8> {
    let mut data = Vec::with_capacity(234);

    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.push(6);
    data.push(1);
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 32]);
    assert_eq!(data.len(), 82);

    data.extend_from_slice(&[0u8; 83]);
    assert_eq!(data.len(), 165);

    data.push(1); // AccountType::Mint
    data.extend_from_slice(&extension_type.to_le_bytes());
    data.extend_from_slice(&64u16.to_le_bytes());

    data.extend_from_slice(&authority);
    data.extend_from_slice(&target);

    assert_eq!(data.len(), 234, "165 + 1 + 4 + 64 = 234");
    data
}

/// Proves `InterfaceAccount<Mint>::get_extension::<MetadataPointer>()`
/// (`naclac-token/src/extensions/metadata_pointer.rs`) reads the authority
/// and metadata address fields at the correct byte offsets under the Borsh
/// backend too.
#[test]
fn check_metadata_pointer_reads_extension_correctly() {
    let provider = setup();

    let authority = Keypair::new().address();
    let metadata_address = Keypair::new().address();
    let mint_bytes = build_pointer_mint_bytes(18, authority.to_bytes(), metadata_address.to_bytes());
    let mint = Keypair::new();
    provider
        .set_account(&mint.address(), mint_bytes, &TOKEN_2022_PROGRAM_ID, 10_000_000)
        .expect("set_account for the metadata-pointer mint fixture should succeed");

    build_check_metadata_pointer(
        &provider,
        PROGRAM_ID,
        Some(authority),
        Some(metadata_address),
        CheckMetadataPointerAccounts { mint: mint.address() },
    )
    .send_and_confirm()
    .expect("check_metadata_pointer must accept the correct expected values");

    let wrong_result = build_check_metadata_pointer(
        &provider,
        PROGRAM_ID,
        Some(authority),
        None, // deliberately wrong: fixture has a real metadata address set
        CheckMetadataPointerAccounts { mint: mint.address() },
    )
    .send_and_confirm();
    assert_custom_code(wrong_result, 3003);
}

/// Proves `InterfaceAccount<Mint>::get_extension::<GroupPointer>()`
/// (`naclac-token/src/extensions/group_pointer.rs`) reads the authority and
/// group address fields at the correct byte offsets under the Borsh
/// backend too.
#[test]
fn check_group_pointer_reads_extension_correctly() {
    let provider = setup();

    let authority = Keypair::new().address();
    let group_address = Keypair::new().address();
    let mint_bytes = build_pointer_mint_bytes(20, authority.to_bytes(), group_address.to_bytes());
    let mint = Keypair::new();
    provider
        .set_account(&mint.address(), mint_bytes, &TOKEN_2022_PROGRAM_ID, 10_000_000)
        .expect("set_account for the group-pointer mint fixture should succeed");

    build_check_group_pointer(
        &provider,
        PROGRAM_ID,
        Some(authority),
        Some(group_address),
        CheckGroupPointerAccounts { mint: mint.address() },
    )
    .send_and_confirm()
    .expect("check_group_pointer must accept the correct expected values");

    let wrong_result = build_check_group_pointer(
        &provider,
        PROGRAM_ID,
        Some(authority),
        None, // deliberately wrong: fixture has a real group address set
        CheckGroupPointerAccounts { mint: mint.address() },
    )
    .send_and_confirm();
    assert_custom_code(wrong_result, 3003);
}

/// Proves `InterfaceAccount<Mint>::get_extension::<GroupMemberPointer>()`
/// (`naclac-token/src/extensions/group_member_pointer.rs`) reads the
/// authority and member address fields at the correct byte offsets under
/// the Borsh backend too.
#[test]
fn check_group_member_pointer_reads_extension_correctly() {
    let provider = setup();

    let authority = Keypair::new().address();
    let member_address = Keypair::new().address();
    let mint_bytes = build_pointer_mint_bytes(22, authority.to_bytes(), member_address.to_bytes());
    let mint = Keypair::new();
    provider
        .set_account(&mint.address(), mint_bytes, &TOKEN_2022_PROGRAM_ID, 10_000_000)
        .expect("set_account for the group-member-pointer mint fixture should succeed");

    build_check_group_member_pointer(
        &provider,
        PROGRAM_ID,
        Some(authority),
        Some(member_address),
        CheckGroupMemberPointerAccounts { mint: mint.address() },
    )
    .send_and_confirm()
    .expect("check_group_member_pointer must accept the correct expected values");

    let wrong_result = build_check_group_member_pointer(
        &provider,
        PROGRAM_ID,
        Some(authority),
        None, // deliberately wrong: fixture has a real member address set
        CheckGroupMemberPointerAccounts { mint: mint.address() },
    )
    .send_and_confirm();
    assert_custom_code(wrong_result, 3003);
}

fn load_hook_program(provider: &NaclacProvider, program_id: &Address) {
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // token workspace root
    so_path.push("target/deploy/test_transfer_hook.so");

    provider
        .add_program(program_id, so_path.to_str().unwrap())
        .expect("Failed to load test_transfer_hook program binary");
}

/// Real end-to-end proof of `transfer_checked_with_hook`
/// (`naclac-token/src/extensions/transfer_hook.rs`) against a real, deployed
/// `spl-transfer-hook-interface` program (`tests/token/programs/test_transfer_hook`)
/// â€” not just that `initialize_transfer_hook`/`transfer_hook_update` are
/// readable, but that a real Token-2022 transfer against a hook-gated mint
/// actually resolves the hook's extra accounts and CPIs into the hook
/// program, proven by asserting the transaction's own logs contain the
/// hook program's log line (independent proof its code actually ran, not
/// just that the outer instruction returned success).
#[test]
fn transfer_checked_with_hook_invokes_the_real_hook_program() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);
    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let hook_program_id = Keypair::new().address();
    load_hook_program(&provider, &hook_program_id);

    let id: u64 = 400;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint2022_with_transfer_hook(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithTransferHookArgs {
            id,
            mint_bump,
            decimals: 6,
            hook_program_id,
        },
        CreateMint2022WithTransferHookAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_transfer_hook should succeed");

    let validate_state_pda =
        spl_transfer_hook_interface::get_extra_account_metas_address(&mint_pda, &hook_program_id);

    // Initialize the hook's `ExtraAccountMetaList` directly against the raw
    // hook program (not through token_borsh) â€” empty metas list, sufficient
    // to prove the CPI chain reaches the hook. Signed by a throwaway
    // keypair as the hook's own "authority": this test's minimal hook
    // program only checks that account is a signer, decoupled from the
    // mint's own `TransferHook.authority` field.
    let hook_init_authority = Keypair::new();
    transfer_sol(&provider, &hook_init_authority.address(), 10_000_000)
        .expect("funding hook_init_authority should succeed");
    let mut init_metas_ix = spl_transfer_hook_interface::instruction::initialize_extra_account_meta_list(
        &hook_program_id,
        &validate_state_pda,
        &mint_pda,
        &hook_init_authority.address(),
        &[],
    );
    // The interface spec only requires this account as `[s]` (a signer),
    // but this test's hook program also uses it as the `CreateAccount` fee
    // payer for the `ExtraAccountMetaList` PDA, which requires it `[w]` too
    // â€” the library builder doesn't know that implementation choice, so
    // the writable flag is added here.
    init_metas_ix.accounts[2].is_writable = true;
    let payer_pubkey = provider.payer.pubkey();
    let recent_blockhash = provider.get_latest_blockhash().expect("blockhash");
    let v0_msg = solana_message::v0::Message::try_compile(
        &payer_pubkey,
        &[init_metas_ix],
        &[],
        recent_blockhash,
    )
    .expect("failed to compile v0 message for InitializeExtraAccountMetaList");
    let versioned_message = solana_message::VersionedMessage::V0(v0_msg);
    let tx = VersionedTransaction::try_new(
        versioned_message,
        &[&*provider.payer, &hook_init_authority],
    )
    .expect("failed to sign InitializeExtraAccountMetaList transaction");
    provider
        .send_transaction(&tx, None)
        .expect("InitializeExtraAccountMetaList should succeed against the real hook program");

    // `TransferHookAccount` (1-byte value) is auto-attached to every token
    // account of a `TransferHook` mint: 165 + 1 (AccountType) + 4 (TLV
    // header) + 1 = 171.
    const VAULT_WITH_TRANSFER_HOOK_ACCOUNT_SPACE: usize = 165 + 1 + 4 + 1;
    let source = Keypair::new();
    utils::create_token_account_with_program_and_space(
        &provider,
        &source,
        &mint_pda,
        &mint_authority_pda,
        &TOKEN_2022_PROGRAM_ID,
        VAULT_WITH_TRANSFER_HOOK_ACCOUNT_SPACE,
    )
    .expect("source vault creation should succeed");
    let destination = Keypair::new();
    utils::create_token_account_with_program_and_space(
        &provider,
        &destination,
        &mint_pda,
        &mint_authority_pda,
        &TOKEN_2022_PROGRAM_ID,
        VAULT_WITH_TRANSFER_HOOK_ACCOUNT_SPACE,
    )
    .expect("destination vault creation should succeed");

    build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVault2022Accounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: source.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault2022 should succeed");

    let result = build_exercise_transfer_checked_with_hook(
        &provider,
        PROGRAM_ID,
        400_000,
        6,
        ExerciseTransferCheckedWithHookAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            source: source.address(),
            destination: destination.address(),
            hook_program: hook_program_id,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .remaining_accounts(vec![AccountMeta::new(hook_program_id, false), AccountMeta::new(validate_state_pda, false)])
    .send_and_confirm()
    .expect(
        "exercise_transfer_checked_with_hook should succeed end-to-end against real \
         Token-2022 and the real hook program",
    );

    assert!(
        result.logs.iter().any(|line| line.contains("test_transfer_hook: Execute amount=400000")),
        "the real hook program's own log line must appear in the transaction logs, proving \
         Token-2022 actually CPI'd into it during the transfer â€” logs were: {:?}",
        result.logs
    );
}

/// Real end-to-end proof of `initialize_token_metadata`, `update_token_metadata_field`,
/// `remove_token_metadata_key`, `update_token_metadata_authority`, and
/// `emit_token_metadata` (`naclac-token/src/extensions/token_metadata.rs`).
/// `TokenMetadata` has no fixed byte layout naclac can read via
/// `get_extension` (unlike every other extension in this crate) â€” instead,
/// each mutating step ends with `emit_token_metadata`, and this test decodes
/// the transaction's real on-chain return data via
/// `spl_token_metadata_interface::state::TokenMetadata`'s own
/// `VariableLenPack::unpack_from_slice`, proving each CPI actually changed
/// on-chain state.
#[test]
fn token_metadata_lifecycle_mutates_real_on_chain_state() {
    use spl_type_length_value::variable_len_pack::VariableLenPack;

    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);
    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 500;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint2022_with_metadata_pointer_and_metadata(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithMetadataPointerAndMetadataArgs {
            id,
            mint_bump,
            decimals: 6,
            name: "Original Name".to_string(),
            symbol: "ORIG".to_string(),
            uri: "https://example.invalid/original.json".to_string(),
        },
        CreateMint2022WithMetadataPointerAndMetadataAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_metadata_pointer_and_metadata should succeed");

    let result1 = build_exercise_token_metadata_lifecycle(
        &provider,
        PROGRAM_ID,
        "Updated Name".to_string(),
        "extra_key".to_string(),
        "extra_value".to_string(),
        ExerciseTokenMetadataLifecycleAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("exercise_token_metadata_lifecycle should succeed");

    let metadata1 = spl_token_metadata_interface::state::TokenMetadata::unpack_from_slice(
        &result1.return_data,
    )
    .expect("return data should decode as real TokenMetadata bytes");
    assert_eq!(metadata1.name, "Updated Name");
    assert_eq!(
        metadata1.additional_metadata,
        vec![("extra_key".to_string(), "extra_value".to_string())]
    );

    let new_authority = Keypair::new().address();
    let result2 = build_exercise_token_metadata_remove_key_and_authority(
        &provider,
        PROGRAM_ID,
        "extra_key".to_string(),
        new_authority,
        ExerciseTokenMetadataRemoveKeyAndAuthorityAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("exercise_token_metadata_remove_key_and_authority should succeed");

    let metadata2 = spl_token_metadata_interface::state::TokenMetadata::unpack_from_slice(
        &result2.return_data,
    )
    .expect("return data should decode as real TokenMetadata bytes");
    assert!(
        metadata2.additional_metadata.is_empty(),
        "remove_token_metadata_key should have removed the only additional_metadata entry"
    );
    assert_eq!(
        Option::<Address>::from(metadata2.update_authority),
        Some(new_authority),
        "update_token_metadata_authority should have changed the on-chain update authority"
    );
}

/// Real end-to-end proof of `initialize_token_group`,
/// `update_token_group_max_size`, and `update_token_group_authority`
/// (`naclac-token/src/extensions/token_group.rs`). Unlike `TokenMetadata`,
/// `TokenGroup` is a fixed-size `Pod` struct naclac reads directly via
/// `get_extension` (`check_token_group`'s own on-chain assertions), so no
/// return-data decoding is needed here.
#[test]
fn token_group_lifecycle_mutates_real_on_chain_state() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);
    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 501;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint2022_with_group_pointer_and_group(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithGroupPointerAndGroupArgs {
            id,
            mint_bump,
            decimals: 6,
            max_size: 10,
        },
        CreateMint2022WithGroupPointerAndGroupAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_group_pointer_and_group should succeed");

    build_check_token_group(
        &provider,
        PROGRAM_ID,
        0,
        10,
        CheckTokenGroupAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_token_group should confirm size=0, max_size=10 right after initialization");

    // The member must join while `mint_authority_pda` is still the real
    // `update_authority` â€” `exercise_update_token_group` (below) changes it
    // to an unrelated throwaway keypair, which would make
    // `initialize_token_group_member`'s own authority check reject a
    // member-join signed by the now-stale `mint_authority_pda`.
    let member_seed: u64 = 502;
    let (member_mint_pda, member_mint_bump) =
        Address::find_program_address(&[b"member_mint", &member_seed.to_le_bytes()], &PROGRAM_ID);

    build_create_mint2022_with_group_member_pointer_and_member(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithGroupMemberPointerAndMemberArgs {
            member_seed,
            member_mint_bump,
            decimals: 6,
        },
        CreateMint2022WithGroupMemberPointerAndMemberAccounts {
            payer: provider.payer.address(),
            member_mint: member_mint_pda,
            group_mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect(
        "create_mint2022_with_group_member_pointer_and_member should succeed, joining the \
         real on-chain TokenGroup",
    );

    build_check_token_group_member(
        &provider,
        PROGRAM_ID,
        member_mint_pda,
        mint_pda,
        CheckTokenGroupMemberAccounts { member_mint: member_mint_pda },
    )
    .send_and_confirm()
    .expect("check_token_group_member should confirm mint/group match on the real member mint");

    let new_authority = Keypair::new().address();
    build_exercise_update_token_group(
        &provider,
        PROGRAM_ID,
        20,
        new_authority,
        ExerciseUpdateTokenGroupAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "exercise_update_token_group should succeed and self-verify max_size/authority \
         on-chain",
    );
}

/// Real end-to-end proof that `init_if_needed` re-validates `mint::decimals`
/// against an already-existing mint rather than silently accepting whatever
/// is already there â€” the gap fixed in
/// `naclac-macros/src/instruction/security.rs`'s Token/Mint Validation block
/// (previously gated on `field.init_config.is_none()` alone, which is always
/// false for an `init_if_needed` field, so this validation never ran for one
/// at all). First call creates the mint with `decimals = 6`; second call
/// against the same PDA with `decimals = 9` must now fail instead of
/// silently succeeding against the pre-existing `decimals = 6` mint.
#[test]
fn init_if_needed_mint_revalidates_decimals_against_an_existing_mint() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);
    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 601;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint2022_init_if_needed(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMint2022InitIfNeededAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("first init_if_needed call should create the mint with decimals=6");

    let result = build_create_mint2022_init_if_needed(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        9,
        CreateMint2022InitIfNeededAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm();
    assert!(
        result.is_err(),
        "second init_if_needed call with decimals=9 against the already-existing \
         decimals=6 mint must fail, not silently succeed"
    );
}

/// Real end-to-end proof of `enable_required_memo_transfers`/
/// `disable_required_memo_transfers` (`naclac-token/src/extensions/memo_transfer.rs`)
/// against real Token-2022 under the solana backend: a vault created with
/// the extension enabled reads back present and required, and
/// `disable_required_memo_transfers` leaves the extension present but flips
/// its flag â€” matching the real protocol's toggle-not-remove behavior.
#[test]
fn memo_transfer_enable_and_disable_toggle_the_real_extension_flag() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);
    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 700;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint2022(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMint2022Accounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022 should succeed");

    let owner = Keypair::new();
    let vault = Keypair::new();
    build_create_token_account_with_memo_transfer_required(
        &provider,
        PROGRAM_ID,
        CreateTokenAccountWithMemoTransferRequiredAccounts {
            payer: provider.payer.address(),
            token_account: vault.address(),
            mint: mint_pda,
            owner: owner.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .signer(&vault)
    .signer(&owner)
    .send_and_confirm()
    .expect("create_token_account_with_memo_transfer_required should succeed");

    build_check_memo_transfer(
        &provider,
        PROGRAM_ID,
        1,
        1,
        CheckMemoTransferAccounts { vault: vault.address() },
    )
    .send_and_confirm()
    .expect("check_memo_transfer must confirm the extension is present and required");

    build_exercise_memo_transfer_disable(
        &provider,
        PROGRAM_ID,
        ExerciseMemoTransferDisableAccounts {
            vault: vault.address(),
            owner: owner.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .signer(&owner)
    .send_and_confirm()
    .expect("exercise_memo_transfer_disable should succeed and self-assert the flag flipped");

    build_check_memo_transfer(
        &provider,
        PROGRAM_ID,
        1,
        0,
        CheckMemoTransferAccounts { vault: vault.address() },
    )
    .send_and_confirm()
    .expect("check_memo_transfer must confirm the extension is still present but no longer required");
}

/// Real end-to-end proof that Token-2022 actually *enforces* `MemoTransfer`
/// under the solana backend (not just that the extension is
/// readable/toggleable): a transfer into a memo-required vault with no
/// preceding memo is rejected by real Token-2022
/// (`check_previous_sibling_instruction_is_memo`, verified against the real
/// processor source), while the identical transfer preceded by a real
/// sibling CPI to the spl-memo program (v3, built via the real
/// `spl_memo_interface::instruction::build_memo` constructor) succeeds.
#[test]
fn memo_transfer_blocks_a_transfer_without_a_memo_and_allows_one_with_a_memo() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);
    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 701;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint2022(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMint2022Accounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022 should succeed");

    let source = Keypair::new();
    utils::create_token_account_with_program(
        &provider,
        &source,
        &mint_pda,
        &mint_authority_pda,
        &TOKEN_2022_PROGRAM_ID,
    )
    .expect("source vault creation should succeed");

    build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVault2022Accounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: source.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault2022 should succeed");

    let owner = Keypair::new();
    let destination = Keypair::new();
    build_create_token_account_with_memo_transfer_required(
        &provider,
        PROGRAM_ID,
        CreateTokenAccountWithMemoTransferRequiredAccounts {
            payer: provider.payer.address(),
            token_account: destination.address(),
            mint: mint_pda,
            owner: owner.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .signer(&destination)
    .signer(&owner)
    .send_and_confirm()
    .expect("create_token_account_with_memo_transfer_required should succeed");

    let memo_program: Address = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"
        .parse()
        .expect("memo v3 program id must parse");

    let blocked_result = build_transfer_tokens2022(
        &provider,
        PROGRAM_ID,
        100_000,
        TransferTokens2022Accounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            from: source.address(),
            to: destination.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    assert!(
        blocked_result.is_err(),
        "a transfer with no preceding memo must be rejected by real Token-2022 once MemoTransfer is required"
    );

    build_transfer_tokens2022_with_memo(
        &provider,
        PROGRAM_ID,
        100_000,
        TransferTokens2022WithMemoAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            from: source.address(),
            to: destination.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
            memo_program,
        },
    )
    .send_and_confirm()
    .expect("a transfer preceded by a real memo CPI must be accepted by real Token-2022");
}

/// Real end-to-end proof that `CpiGuard` (`naclac-token/src/extensions/cpi_guard.rs`,
/// read-only by design â€” see that file's own doc comment for why naclac
/// doesn't expose enable/disable CPI helpers) is both readable and actually
/// enforced by real Token-2022 under the solana backend: `Enable` is sent
/// as a genuine top-level transaction (never through `token_borsh` â€”
/// Token-2022 rejects `Enable`/`Disable` whenever invoked via CPI, confirmed
/// against the real `cpi_guard::processor::process_toggle_cpi_guard`'s
/// `in_cpi()` check), then a CPI-issued `Approve` against the guarded vault
/// is rejected while the identical CPI against a plain vault succeeds â€”
/// `Approve` is unconditionally disallowed via CPI once `CpiGuard` is
/// enabled, per the real protocol's own doc comment on
/// `CpiGuardInstruction::Enable`.
#[test]
fn cpi_guard_blocks_a_cpi_approve_and_is_readable_after_a_real_top_level_enable() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);
    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 702;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint2022(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMint2022Accounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022 should succeed");

    // Base 165 bytes + 1-byte `AccountType` marker + 4-byte TLV header +
    // 1-byte `CpiGuard` value.
    const VAULT_WITH_CPI_GUARD_SPACE: usize = 165 + 1 + 4 + 1;
    let owner = Keypair::new();
    let vault = Keypair::new();
    utils::create_token_account_with_program_and_space(
        &provider,
        &vault,
        &mint_pda,
        &owner.address(),
        &TOKEN_2022_PROGRAM_ID,
        VAULT_WITH_CPI_GUARD_SPACE,
    )
    .expect("vault creation should succeed");

    // Real, genuinely top-level `Enable` instruction â€” sent directly, never
    // through the `token_borsh` program, since Token-2022 rejects it via CPI.
    let enable_ix = spl_token_2022_interface::extension::cpi_guard::instruction::enable_cpi_guard(
        &TOKEN_2022_PROGRAM_ID,
        &vault.address(),
        &owner.address(),
        &[],
    )
    .expect("building the real enable_cpi_guard instruction should succeed");
    let payer_pubkey = provider.payer.pubkey();
    let recent_blockhash = provider.get_latest_blockhash().expect("blockhash");
    let v0_msg = solana_message::v0::Message::try_compile(
        &payer_pubkey,
        &[enable_ix],
        &[],
        recent_blockhash,
    )
    .expect("failed to compile v0 message for enable_cpi_guard");
    let versioned_message = solana_message::VersionedMessage::V0(v0_msg);
    let tx = VersionedTransaction::try_new(versioned_message, &[&*provider.payer, &owner])
        .expect("failed to sign enable_cpi_guard transaction");
    provider
        .send_transaction(&tx, None)
        .expect("a real top-level enable_cpi_guard must succeed against real Token-2022");

    build_check_cpi_guard(
        &provider,
        PROGRAM_ID,
        1,
        1,
        CheckCpiGuardAccounts { vault: vault.address() },
    )
    .send_and_confirm()
    .expect("check_cpi_guard must confirm the extension is present and locked");

    let delegate = Keypair::new();
    let blocked_result = build_approve_vault_delegate2022(
        &provider,
        PROGRAM_ID,
        100_000,
        ApproveVaultDelegate2022Accounts {
            vault: vault.address(),
            delegate: delegate.address(),
            owner: owner.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .signer(&owner)
    .send_and_confirm();
    assert!(
        blocked_result.is_err(),
        "a CPI-issued Approve must be rejected by real Token-2022 once CpiGuard is enabled"
    );

    // Negative baseline: an otherwise-identical vault with no CpiGuard
    // extension must accept the exact same CPI-issued Approve.
    let plain_owner = Keypair::new();
    let plain_vault = Keypair::new();
    utils::create_token_account_with_program(
        &provider,
        &plain_vault,
        &mint_pda,
        &plain_owner.address(),
        &TOKEN_2022_PROGRAM_ID,
    )
    .expect("plain vault creation should succeed");

    build_approve_vault_delegate2022(
        &provider,
        PROGRAM_ID,
        100_000,
        ApproveVaultDelegate2022Accounts {
            vault: plain_vault.address(),
            delegate: delegate.address(),
            owner: plain_owner.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .signer(&plain_owner)
    .send_and_confirm()
    .expect("a CPI-issued Approve against a vault with no CpiGuard must succeed");
}

/// Real end-to-end proof of `PausableConfig`'s three CPIs
/// (`naclac-token/src/extensions/pausable.rs`) against real Token-2022
/// under the solana backend: a mint created with `initialize_pausable_config`
/// reads back with the exact authority given, `pause_mint` genuinely blocks
/// `MintTo` (verified against the real processor's `PausableConfig` check
/// inside its transfer/mint path), and `resume_mint` lifts the block again.
#[test]
fn pausable_config_pause_blocks_minting_and_resume_allows_it_again() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);
    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 703;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint2022_with_pausable(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMint2022WithPausableAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_pausable should succeed");

    build_check_pausable_config(
        &provider,
        PROGRAM_ID,
        Some(mint_authority_pda),
        0,
        CheckPausableConfigAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_pausable_config must confirm the real authority and paused=false initially");

    // `Pausable` mints require every token account to carry the
    // auto-attached `PausableAccount` extension â€” confirmed against
    // `spl-token-2022-interface`'s real
    // `ExtensionType::get_required_init_account_extensions`, which maps
    // `Pausable => [PausableAccount]`. It's a 0-byte value, so it only adds
    // its own 4-byte TLV header: 165 + 1 (AccountType) + 4.
    const VAULT_WITH_PAUSABLE_ACCOUNT_SPACE: usize = 165 + 1 + 4;
    let vault = Keypair::new();
    utils::create_token_account_with_program_and_space(
        &provider,
        &vault,
        &mint_pda,
        &mint_authority_pda,
        &TOKEN_2022_PROGRAM_ID,
        VAULT_WITH_PAUSABLE_ACCOUNT_SPACE,
    )
    .expect("vault creation should succeed");

    build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        500_000,
        MintToVault2022Accounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault2022 should succeed before the mint is paused");

    build_exercise_pause_mint(
        &provider,
        PROGRAM_ID,
        ExercisePauseMintAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("exercise_pause_mint should succeed and self-assert the mint is paused");

    // Same instruction, accounts, and args as the successful `MintTo` above
    // â€” without a fresh blockhash, litesvm (faithfully reproducing real
    // Solana) would reject this resubmission as `AlreadyProcessed` rather
    // than genuinely re-running it through Token-2022 for this test to
    // observe the real `MintPaused` rejection.
    provider
        .expire_blockhash()
        .expect("expire_blockhash should succeed");
    let blocked_result = build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        500_000,
        MintToVault2022Accounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    assert!(
        blocked_result.is_err(),
        "MintTo must be rejected by real Token-2022 while the mint is paused"
    );

    build_exercise_resume_mint(
        &provider,
        PROGRAM_ID,
        ExerciseResumeMintAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("exercise_resume_mint should succeed and self-assert the mint is no longer paused");

    // Same instruction, accounts, and args as the two `MintTo`s above â€”
    // needs a fresh blockhash for the same reason as the blocked attempt.
    provider
        .expire_blockhash()
        .expect("expire_blockhash should succeed");
    build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        500_000,
        MintToVault2022Accounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("MintTo must succeed again once the mint is resumed");
}

/// Real end-to-end proof that `MintCloseAuthority`
/// (`naclac-token/src/extensions/mint_close_authority.rs`) actually permits
/// closing a supply-zero Token-2022 mint via the ordinary `close_account`
/// CPI under the solana backend, while an otherwise-identical mint with no
/// `MintCloseAuthority` extension rejects the same close attempt â€” real
/// Token-2022 refuses `CloseAccount` on a mint unless the extension is
/// present and the signer matches its registered close authority.
#[test]
fn mint_close_authority_closes_a_supply_zero_mint_and_rejects_a_plain_mint() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);
    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 704;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint2022_with_mint_close_authority(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMint2022WithMintCloseAuthorityAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_mint_close_authority should succeed");

    build_check_mint_close_authority(
        &provider,
        PROGRAM_ID,
        Some(mint_authority_pda),
        CheckMintCloseAuthorityAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_mint_close_authority must confirm the real close authority");

    let destination = Keypair::new().address();
    let lamports_before = provider
        .get_account(&destination)
        .map(|acc| acc.lamports)
        .unwrap_or(0);

    build_close_mint2022(
        &provider,
        PROGRAM_ID,
        CloseMint2022Accounts {
            mint: mint_pda,
            destination,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "close_mint2022 should succeed against a real supply-zero mint with a real \
         close authority",
    );

    let lamports_after = provider
        .get_account(&destination)
        .map(|acc| acc.lamports)
        .unwrap_or(0);
    assert!(
        lamports_after > lamports_before,
        "the mint's lamports must be transferred to the destination on close"
    );

    let closed_mint = provider.get_account(&mint_pda);
    let still_a_valid_mint = closed_mint
        .as_ref()
        .map(|acc| acc.owner == TOKEN_2022_PROGRAM_ID && acc.data.len() >= 82 && acc.data[45] != 0)
        .unwrap_or(false);
    assert!(
        !still_a_valid_mint,
        "the closed mint must no longer be readable as a valid, initialized mint"
    );

    // Negative baseline: an otherwise-identical mint with no
    // MintCloseAuthority extension must reject the same close attempt.
    let plain_id: u64 = 705;
    let (plain_mint_pda, plain_mint_bump) =
        Address::find_program_address(&[b"mint", &plain_id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint2022(
        &provider,
        PROGRAM_ID,
        plain_id,
        plain_mint_bump,
        6,
        CreateMint2022Accounts {
            payer: provider.payer.address(),
            mint: plain_mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022 (no MintCloseAuthority) should succeed");

    let blocked_result = build_close_mint2022(
        &provider,
        PROGRAM_ID,
        CloseMint2022Accounts {
            mint: plain_mint_pda,
            destination,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    assert!(
        blocked_result.is_err(),
        "closing a mint with no MintCloseAuthority extension must be rejected by real Token-2022"
    );
}

/// Real end-to-end proof of `ScaledUiAmountConfig`'s two CPIs
/// (`naclac-token/src/extensions/scaled_ui_amount.rs`) against real
/// Token-2022: a mint created with `initialize_scaled_ui_amount_config`
/// reads back with the exact authority and multiplier given, and
/// `update_scaled_ui_amount_multiplier` actually changes the on-chain
/// `new_multiplier` field.
#[test]
fn scaled_ui_amount_config_produces_a_real_readable_extension_and_update_changes_the_multiplier() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);
    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 706;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    let multiplier: f64 = 1.5;
    build_create_mint2022_with_scaled_ui_amount(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        multiplier.to_bits(),
        CreateMint2022WithScaledUiAmountAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_scaled_ui_amount should succeed");

    build_check_scaled_ui_amount_config(
        &provider,
        PROGRAM_ID,
        Some(mint_authority_pda),
        multiplier.to_bits(),
        CheckScaledUiAmountConfigAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect(
        "check_scaled_ui_amount_config must confirm the real authority and multiplier",
    );

    let new_multiplier: f64 = 2.25;
    build_exercise_update_scaled_ui_amount_multiplier(
        &provider,
        PROGRAM_ID,
        new_multiplier.to_bits(),
        0,
        ExerciseUpdateScaledUiAmountMultiplierAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "exercise_update_scaled_ui_amount_multiplier should succeed and self-assert \
         new_multiplier changed",
    );
}

/// Real end-to-end proof that `PermissionedBurnConfig`
/// (`naclac-token/src/extensions/permissioned_burn.rs`) actually blocks the
/// ordinary `Burn` instruction and only allows burning via
/// `permissioned_burn`, co-signed by the registered permissioned-burn
/// authority â€” real Token-2022 unconditionally rejects
/// `BurnInstructionVariant::Standard` once the extension is present.
#[test]
fn permissioned_burn_blocks_ordinary_burn_and_allows_permissioned_burn() {
    let provider = setup();
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);
    build_init_mint_authority(
        &provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_mint_authority should succeed");

    let id: u64 = 707;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint2022_with_permissioned_burn(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMint2022WithPermissionedBurnAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_permissioned_burn should succeed");

    build_check_permissioned_burn(
        &provider,
        PROGRAM_ID,
        Some(mint_authority_pda),
        CheckPermissionedBurnAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_permissioned_burn must confirm the real authority");

    let vault = Keypair::new();
    utils::create_token_account_with_program(
        &provider,
        &vault,
        &mint_pda,
        &mint_authority_pda,
        &TOKEN_2022_PROGRAM_ID,
    )
    .expect("vault creation should succeed");

    build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVault2022Accounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault2022 should succeed");

    let blocked_result = build_burn_vault_tokens2022(
        &provider,
        PROGRAM_ID,
        100_000,
        BurnVaultTokens2022Accounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    assert!(
        blocked_result.is_err(),
        "ordinary Burn must be rejected by real Token-2022 once PermissionedBurnConfig is set"
    );

    build_exercise_permissioned_burn(
        &provider,
        PROGRAM_ID,
        100_000,
        ExercisePermissionedBurnAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            vault: vault.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "exercise_permissioned_burn should succeed and self-assert the vault balance dropped",
    );
}
