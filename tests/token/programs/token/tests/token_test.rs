use naclac_client::*;
use token_client::{
    fetch_mint_authority, get_mint_authority_pda,
    instructions::{
        build_approve_vault_delegate, build_approve_vault_delegate2022, build_burn_vault_tokens,
        build_burn_vault_tokens2022,
        build_check_account_fields,
        build_check_ata_constraints,
        build_check_cpi_guard,
        build_check_default_account_state,
        build_check_pausable_config,
        build_check_group_member_pointer, build_check_group_pointer,
        build_check_immutable_owner,
        build_check_interest_bearing_mint,
        build_check_memo_transfer,
        build_check_metadata_pointer,
        build_check_mint_close_authority,
        build_check_permissioned_burn,
        build_check_scaled_ui_amount_config,
        build_check_mint_freeze_authority, build_check_non_transferable, build_check_permanent_delegate,
        build_check_transfer_fee_config,
        build_check_transfer_hook, build_check_vault_constraints, build_check_vault_program,
        build_close_mint2022,
        build_close_vault_account, build_create_ata, build_create_ata_idempotent, build_create_mint,
        build_create_mint2022, build_create_mint2022_dual_token_program,
        build_create_mint2022_with_default_account_state,
        build_create_mint2022_with_group_member_pointer, build_create_mint2022_with_group_pointer,
        build_create_mint2022_with_interest_bearing_mint,
        build_create_mint2022_with_metadata_pointer,
        build_create_mint2022_with_mint_close_authority,
        build_create_mint2022_with_non_transferable,
        build_create_mint2022_with_pausable,
        build_create_mint2022_with_permanent_delegate,
        build_create_mint2022_with_permissioned_burn,
        build_create_mint2022_with_scaled_ui_amount,
        build_create_mint2022_with_transfer_fee, build_create_mint2022_with_transfer_hook,
        build_create_mint_with_freeze, build_create_token_account_with_immutable_owner,
        build_create_token_account_with_memo_transfer_required,
        build_exercise_default_account_state_update,
        build_exercise_group_member_pointer_update, build_exercise_group_pointer_update,
        build_exercise_interest_bearing_mint_update_rate,
        build_exercise_memo_transfer_disable,
        build_exercise_metadata_pointer_update,
        build_exercise_pause_mint,
        build_exercise_permanent_delegate_burn,
        build_exercise_permissioned_burn,
        build_exercise_resume_mint,
        build_exercise_update_scaled_ui_amount_multiplier,
        build_exercise_transfer_fee_cpis,
        build_exercise_transfer_hook_update, build_freeze_vault_account,
        build_init_mint_authority, build_mint_to_vault, build_mint_to_vault2022,
        build_revoke_vault_delegate, build_set_mint_authority, build_set_token_account_owner,
        build_thaw_vault_account,
        build_transfer_tokens, build_transfer_tokens2022, build_transfer_tokens2022_with_memo,
        build_transfer_tokens_checked,
        build_check_token_group, build_check_token_group_member,
        build_create_mint2022_with_group_member_pointer_and_member,
        build_create_mint2022_with_group_pointer_and_group,
        build_create_mint2022_with_metadata_pointer_and_metadata,
        build_exercise_token_metadata_lifecycle,
        build_exercise_token_metadata_remove_key_and_authority,
        build_exercise_update_token_group,
        ApproveVaultDelegateAccounts, ApproveVaultDelegate2022Accounts, BurnVaultTokensAccounts,
        BurnVaultTokens2022Accounts,
        CheckAccountFieldsAccounts,
        CheckAtaConstraintsAccounts,
        CheckCpiGuardAccounts,
        CheckDefaultAccountStateAccounts,
        CheckGroupMemberPointerAccounts, CheckGroupPointerAccounts,
        CheckImmutableOwnerAccounts,
        CheckInterestBearingMintAccounts,
        CheckMemoTransferAccounts,
        CheckMetadataPointerAccounts,
        CheckMintCloseAuthorityAccounts,
        CheckMintFreezeAuthorityAccounts, CheckNonTransferableAccounts, CheckPausableConfigAccounts,
        CheckPermanentDelegateAccounts,
        CheckPermissionedBurnAccounts,
        CheckScaledUiAmountConfigAccounts,
        CheckTokenGroupAccounts, CheckTokenGroupMemberAccounts,
        CheckTransferFeeConfigAccounts, CheckTransferHookAccounts,
        CheckVaultConstraintsAccounts,
        CheckVaultProgramAccounts, CloseMint2022Accounts, CloseVaultAccountAccounts, CreateAtaAccounts,
        CreateAtaIdempotentAccounts, CreateMint2022Accounts, CreateMint2022DualTokenProgramAccounts,
        CreateMint2022WithDefaultAccountStateAccounts,
        CreateMint2022WithGroupMemberPointerAccounts, CreateMint2022WithGroupPointerAccounts,
        CreateMint2022WithGroupMemberPointerAndMemberAccounts,
        CreateMint2022WithGroupPointerAndGroupAccounts,
        CreateMint2022WithInterestBearingMintAccounts,
        CreateMint2022WithMetadataPointerAccounts,
        CreateMint2022WithMetadataPointerAndMetadataAccounts,
        CreateMint2022WithMintCloseAuthorityAccounts,
        CreateMint2022WithNonTransferableAccounts,
        CreateMint2022WithPausableAccounts,
        CreateMint2022WithPermanentDelegateAccounts,
        CreateMint2022WithPermissionedBurnAccounts,
        CreateMint2022WithScaledUiAmountAccounts,
        CreateMint2022WithTransferFeeAccounts, CreateMint2022WithTransferHookAccounts,
        CreateMintAccounts, CreateMintWithFreezeAccounts,
        CreateTokenAccountWithImmutableOwnerAccounts,
        CreateTokenAccountWithMemoTransferRequiredAccounts,
        ExerciseDefaultAccountStateUpdateAccounts,
        ExerciseGroupMemberPointerUpdateAccounts, ExerciseGroupPointerUpdateAccounts,
        ExerciseInterestBearingMintUpdateRateAccounts,
        ExerciseMemoTransferDisableAccounts,
        ExerciseMetadataPointerUpdateAccounts,
        ExercisePauseMintAccounts,
        ExercisePermanentDelegateBurnAccounts,
        ExercisePermissionedBurnAccounts,
        ExerciseResumeMintAccounts,
        ExerciseTokenMetadataLifecycleAccounts,
        ExerciseTokenMetadataRemoveKeyAndAuthorityAccounts,
        ExerciseTransferFeeCpisAccounts, ExerciseTransferHookUpdateAccounts,
        ExerciseUpdateScaledUiAmountMultiplierAccounts,
        ExerciseUpdateTokenGroupAccounts,
        FreezeVaultAccountAccounts, InitMintAuthorityAccounts, MintToVault2022Accounts,
        MintToVaultAccounts, RevokeVaultDelegateAccounts, SetMintAuthorityAccounts,
        SetTokenAccountOwnerAccounts,
        ThawVaultAccountAccounts, TransferTokens2022Accounts, TransferTokens2022WithMemoAccounts,
        TransferTokensAccounts,
        TransferTokensCheckedAccounts,
    },
    types::{
        CheckAccountFieldsArgs, CheckInterestBearingMintArgs, CheckTransferFeeConfigArgs,
        CreateMint2022WithDefaultAccountStateArgs,
        CreateMint2022WithGroupMemberPointerArgs, CreateMint2022WithGroupPointerArgs,
        CreateMint2022WithGroupMemberPointerAndMemberArgs,
        CreateMint2022WithGroupPointerAndGroupArgs,
        CreateMint2022WithInterestBearingMintArgs,
        CreateMint2022WithMetadataPointerArgs,
        CreateMint2022WithMetadataPointerAndMetadataArgs,
        CreateMint2022WithNonTransferableArgs,
        CreateMint2022WithPermanentDelegateArgs,
        CreateMint2022WithTransferFeeArgs,
        CreateMint2022WithTransferHookArgs, ExerciseTransferFeeCpisArgs, PROGRAM_ID,
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

fn read_token_account_state(provider: &NaclacProvider, address: &Address) -> u8 {
    let data = provider.get_account_data(address).expect("token account should exist");
    data[108]
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

fn setup_mint_authority_and_mint(
    provider: &NaclacProvider,
    id: u64,
) -> (Address, Address) {
    let (mint_authority_pda, _bump) = get_mint_authority_pda(&PROGRAM_ID);

    let _ = build_init_mint_authority(
        provider,
        PROGRAM_ID,
        InitMintAuthorityAccounts {
            payer: provider.payer.address(),
            mint_authority: mint_authority_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm();

    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint(
        provider,
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

    (mint_authority_pda, mint_pda)
}

/// `burn`/`burn_signed` (`naclac-token/src/token.rs`) â€” mint tokens into a
/// vault, burn part of the balance, and confirm the vault's raw SPL amount
/// field (bytes `[64..72]`, same offset `read_token_account_amount` already
/// uses) decreased by exactly the burned amount, not just "some amount".
#[test]
fn burn_reduces_vault_balance_by_exact_amount() {
    let provider = setup();
    let (mint_authority_pda, mint_pda) = setup_mint_authority_and_mint(&provider, 100);

    let vault = Keypair::new();
    create_token_account(&provider, &vault, &mint_pda, &mint_authority_pda)
        .expect("vault creation should succeed");

    build_mint_to_vault(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVaultAccounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault should succeed");

    build_burn_vault_tokens(
        &provider,
        PROGRAM_ID,
        300_000,
        BurnVaultTokensAccounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("burn_vault_tokens should succeed (real burn CPI)");

    assert_eq!(
        read_token_account_amount(&provider, &vault.address()),
        700_000,
        "vault balance must decrease by exactly the burned amount"
    );
}

/// `set_authority`/`set_authority_signed` (`naclac-token/src/token.rs`),
/// mint-authority variant: after `set_mint_authority` (authority_type = 0 =
/// `AuthorityType::MintTokens`), the mint's raw `mint_authority` `COption`
/// pubkey (bytes `[4..36]` of the SPL `Mint` layout â€” bytes `[0..4]` are the
/// `COption` discriminant, mirroring `check_mint_freeze_authority.rs`'s
/// documented `[46..50]`/`[50..82]` freeze-authority offsets shifted to the
/// mint-authority field earlier in the same struct) reflects the new
/// authority, and a subsequent `mint_to_vault` signed by the stale old PDA
/// authority is rejected by the real Token program (asserted as a generic
/// failure here, not a specific `TokenError` numeric code â€” this session
/// didn't independently verify the real `spl_token::error::TokenError` enum
/// ordering the way the framework's own hand-rolled discriminants were, so
/// asserting an exact code here would be exactly the kind of unverified
/// guess the discriminant-bug lesson this session warns against).
#[test]
fn set_mint_authority_updates_authority_and_old_authority_can_no_longer_mint() {
    let provider = setup();
    let (mint_authority_pda, mint_pda) = setup_mint_authority_and_mint(&provider, 101);

    let new_authority = Keypair::new();
    build_set_mint_authority(
        &provider,
        PROGRAM_ID,
        0, // AuthorityType::MintTokens
        SetMintAuthorityAccounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            new_authority: new_authority.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("set_mint_authority should succeed (real set_authority CPI)");

    let mint_data = provider.get_account_data(&mint_pda).expect("mint should exist");
    assert_eq!(
        &mint_data[4..36],
        new_authority.address().as_ref(),
        "the mint's raw mint_authority field must reflect the new authority"
    );

    let vault = Keypair::new();
    create_token_account(&provider, &vault, &mint_pda, &mint_authority_pda)
        .expect("vault creation should succeed");

    let stale_mint_result = build_mint_to_vault(
        &provider,
        PROGRAM_ID,
        1,
        MintToVaultAccounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    assert!(
        stale_mint_result.is_err(),
        "the old mint authority PDA must no longer be able to mint after set_mint_authority"
    );
}

/// `set_authority`/`set_authority_signed`, freeze-authority variant
/// (`authority_type = 1` = `AuthorityType::FreezeAccount`) â€” confirms the
/// mint's raw freeze-authority `COption` pubkey (bytes `[50..82]`, the same
/// offset `check_mint_freeze_authority.rs` already documents and reads)
/// reflects the newly-set authority.
///
/// Uses `build_create_mint_with_freeze` (not `setup_mint_authority_and_mint`,
/// which calls the plain `create_mint` with no freeze authority at all) â€”
/// real SPL Token semantics only allow `SetAuthority(FreezeAccount, ...)` to
/// *change* an existing freeze authority, never to add one to a mint that
/// was created without one (that call fails with `MintCannotFreeze`
/// regardless of the new authority supplied).
#[test]
fn set_freeze_authority_updates_raw_mint_layout() {
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

    let id: u64 = 102;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint_with_freeze(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMintWithFreezeAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint_with_freeze should succeed");

    let new_freeze_authority = Keypair::new();
    build_set_mint_authority(
        &provider,
        PROGRAM_ID,
        1, // AuthorityType::FreezeAccount
        SetMintAuthorityAccounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            new_authority: new_freeze_authority.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("set_mint_authority (freeze) should succeed");

    let mint_data = provider.get_account_data(&mint_pda).expect("mint should exist");
    assert_eq!(
        &mint_data[46..50],
        &[1, 0, 0, 0],
        "the freeze-authority COption discriminant must now be Some"
    );
    assert_eq!(
        &mint_data[50..82],
        new_freeze_authority.address().as_ref(),
        "the mint's raw freeze_authority field must reflect the new authority"
    );
}

/// `freeze_account`/`thaw_account` (`_signed`) (`naclac-token/src/token.rs`)
/// â€” the first test in this case to actually *call* the real freeze/thaw
/// CPIs (`check_mint_freeze_authority` only ever validated the constraint,
/// never froze anything). Uses `create_mint_with_freeze` so the
/// `mint_authority` PDA is a real freeze authority. Confirms a `transfer`
/// FROM a frozen vault fails (real SPL Token behavior â€” frozen accounts
/// reject transfers, asserted generically per the same reasoning as the
/// stale-mint-authority test above), then thaws and confirms the same
/// transfer now succeeds.
#[test]
fn freeze_blocks_transfer_and_thaw_allows_it_again() {
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

    let id: u64 = 110;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint_with_freeze(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMintWithFreezeAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint_with_freeze should succeed");

    let from_vault = Keypair::new();
    create_token_account(&provider, &from_vault, &mint_pda, &mint_authority_pda)
        .expect("from_vault creation should succeed");
    let to_vault = Keypair::new();
    create_token_account(&provider, &to_vault, &mint_pda, &mint_authority_pda)
        .expect("to_vault creation should succeed");

    build_mint_to_vault(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVaultAccounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: from_vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault should succeed");

    build_freeze_vault_account(
        &provider,
        PROGRAM_ID,
        FreezeVaultAccountAccounts {
            vault: from_vault.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("freeze_vault_account should succeed (real freeze_account CPI)");

    let frozen_transfer_result = build_transfer_tokens(
        &provider,
        PROGRAM_ID,
        1,
        TransferTokensAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            from: from_vault.address(),
            to: to_vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    assert!(
        frozen_transfer_result.is_err(),
        "a transfer from a frozen vault must be rejected by the real Token program"
    );

    build_thaw_vault_account(
        &provider,
        PROGRAM_ID,
        ThawVaultAccountAccounts {
            vault: from_vault.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("thaw_vault_account should succeed (real thaw_account CPI)");

    build_transfer_tokens(
        &provider,
        PROGRAM_ID,
        400_000,
        TransferTokensAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            from: from_vault.address(),
            to: to_vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("transfer must succeed again once the vault is thawed");

    assert_eq!(read_token_account_amount(&provider, &to_vault.address()), 400_000);
}

/// `approve`/`revoke` (`_signed`) (`naclac-token/src/token.rs`) â€” approve a
/// delegate for a limited amount on a vault, confirm the delegate can
/// transfer up to (but not more than) the approved amount via a
/// delegate-signed `transfer` (SPL Token's `Transfer` instruction accepts
/// either the account owner or an approved delegate as the signing
/// authority â€” `naclac-client`'s raw `transfer` builder is used directly
/// here, signed by the delegate keypair, since the program-level
/// `transfer_tokens` instruction always signs with the `mint_authority` PDA
/// as owner, not a delegate). Then revoke and confirm the delegate can no
/// longer transfer at all.
#[test]
fn approve_lets_delegate_transfer_up_to_the_limit_then_revoke_blocks_it() {
    let provider = setup();
    let (mint_authority_pda, mint_pda) = setup_mint_authority_and_mint(&provider, 120);

    let vault = Keypair::new();
    create_token_account(&provider, &vault, &mint_pda, &mint_authority_pda)
        .expect("vault creation should succeed");
    let destination = Keypair::new();
    create_token_account(&provider, &destination, &mint_pda, &mint_authority_pda)
        .expect("destination creation should succeed");

    build_mint_to_vault(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVaultAccounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault should succeed");

    let delegate = Keypair::new();
    build_approve_vault_delegate(
        &provider,
        PROGRAM_ID,
        200_000,
        ApproveVaultDelegateAccounts {
            vault: vault.address(),
            delegate: delegate.address(),
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("approve_vault_delegate should succeed (real approve CPI)");

    // A delegate-signed transfer over the approved amount must be rejected
    // by the real Token program (asserted generically, same reasoning as
    // the freeze/stale-authority tests above).
    let over_limit_result =
        transfer_signed(&provider, &vault.address(), &destination.address(), &delegate, 300_000);
    assert!(
        over_limit_result.is_err(),
        "a delegate transfer over the approved amount must be rejected"
    );

    transfer_signed(&provider, &vault.address(), &destination.address(), &delegate, 150_000)
        .expect("a delegate transfer within the approved amount must succeed");
    assert_eq!(read_token_account_amount(&provider, &destination.address()), 150_000);

    build_revoke_vault_delegate(
        &provider,
        PROGRAM_ID,
        RevokeVaultDelegateAccounts {
            vault: vault.address(),
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("revoke_vault_delegate should succeed (real revoke CPI)");

    let after_revoke_result =
        transfer_signed(&provider, &vault.address(), &destination.address(), &delegate, 1);
    assert!(
        after_revoke_result.is_err(),
        "the delegate must no longer be able to transfer after revoke"
    );
}

/// Raw client-side SPL `Transfer` instruction, signed by an arbitrary
/// keypair authority (a delegate, not the vault owner) â€” there is no
/// generated `build_transfer` helper that accepts a delegate signer, since
/// the program's own `transfer_tokens` instruction always signs with the
/// `mint_authority` PDA. Built via `naclac_client::InstructionBuilder`
/// directly (the same builder every generated `build_*` function uses
/// internally) plus its `.signer(...)` extra-signer hook, rather than
/// hand-rolling a raw `solana_program::instruction::Instruction` â€” this
/// test crate has no direct `solana_program`/`solana_address` dependency of
/// its own (only `naclac-client`/`token-client`, per
/// `programs/token/Cargo.toml`), and `InstructionBuilder` only needs the
/// `Address`/`AccountMeta` types already re-exported by `naclac_client::*`.
fn transfer_signed(
    provider: &NaclacProvider,
    from: &Address,
    to: &Address,
    authority: &Keypair,
    amount: u64,
) -> Result<NaclacTransactionMetadata, NaclacClientError> {
    let mut ix_data = vec![3u8]; // SPL Token `Transfer` discriminant
    ix_data.extend_from_slice(&amount.to_le_bytes());
    InstructionBuilder::new(provider, TOKEN_PROGRAM_ID, ix_data)
        .account(AccountMeta::new(*from, false), "from")
        .account(AccountMeta::new(*to, false), "to")
        .account(AccountMeta::new_readonly(authority.address(), true), "authority")
        .signer(authority)
        .send_and_confirm()
}

/// `close_account`/`close_account_signed` (`naclac-token/src/token.rs`) â€”
/// the raw SPL Token CPI that closes a token account and returns its
/// lamports, distinct from naclac's own `#[account(close = ...)]` framework
/// constraint (`tests/accounts-constraints/`'s `close_vault`, which closes a
/// naclac-owned `#[component]` account, never a real SPL token account).
/// Empties the vault via `burn`, closes it, and confirms lamports moved to
/// the destination and the account can no longer be read back as a valid
/// token account (either the account no longer exists, or its data no
/// longer decodes as an initialized SPL token account).
#[test]
fn close_vault_account_returns_lamports_and_invalidates_account() {
    let provider = setup();
    let (mint_authority_pda, mint_pda) = setup_mint_authority_and_mint(&provider, 130);

    let vault = Keypair::new();
    create_token_account(&provider, &vault, &mint_pda, &mint_authority_pda)
        .expect("vault creation should succeed");

    build_mint_to_vault(
        &provider,
        PROGRAM_ID,
        500_000,
        MintToVaultAccounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault should succeed");

    build_burn_vault_tokens(
        &provider,
        PROGRAM_ID,
        500_000,
        BurnVaultTokensAccounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("emptying the vault via burn should succeed");

    let destination = Keypair::new().address();
    let lamports_before = provider
        .get_account(&destination)
        .map(|acc| acc.lamports)
        .unwrap_or(0);

    build_close_vault_account(
        &provider,
        PROGRAM_ID,
        CloseVaultAccountAccounts {
            vault: vault.address(),
            destination,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("close_vault_account should succeed (real close_account CPI)");

    let lamports_after = provider
        .get_account(&destination)
        .map(|acc| acc.lamports)
        .unwrap_or(0);
    assert!(
        lamports_after > lamports_before,
        "the vault's lamports must be transferred to the destination on close"
    );

    let closed_vault = provider.get_account(&vault.address());
    let still_a_valid_token_account = closed_vault
        .as_ref()
        .map(|acc| acc.owner == TOKEN_PROGRAM_ID && acc.data.len() == 165 && acc.data[108] != 0)
        .unwrap_or(false);
    assert!(
        !still_a_valid_token_account,
        "the closed vault must no longer be readable as a valid, initialized token account"
    );
}

/// `transfer_checked`/`transfer_checked_signed` (`naclac-token/src/token.rs`)
/// â€” mirrors `transfer_tokens`'s existing happy path but through the
/// checked variant, which additionally validates the mint and decimals. The
/// real SPL Token `TransferChecked` instruction itself validates the
/// `decimals` argument against the mint's real decimals (set to 6 by
/// `create_mint`) and rejects a mismatch (asserted generically per the same
/// reasoning as the freeze/stale-authority tests above).
#[test]
fn transfer_checked_succeeds_with_correct_decimals_and_fails_with_wrong_decimals() {
    let provider = setup();
    let (mint_authority_pda, mint_pda) = setup_mint_authority_and_mint(&provider, 140);

    let from_vault = Keypair::new();
    create_token_account(&provider, &from_vault, &mint_pda, &mint_authority_pda)
        .expect("from_vault creation should succeed");
    let to_vault = Keypair::new();
    create_token_account(&provider, &to_vault, &mint_pda, &mint_authority_pda)
        .expect("to_vault creation should succeed");

    build_mint_to_vault(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVaultAccounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: from_vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault should succeed");

    let wrong_decimals_result = build_transfer_tokens_checked(
        &provider,
        PROGRAM_ID,
        100_000,
        5, // mint's real decimals is 6 â€” deliberately wrong
        TransferTokensCheckedAccounts {
            mint_authority: mint_authority_pda,
            from: from_vault.address(),
            mint: mint_pda,
            to: to_vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    assert!(
        wrong_decimals_result.is_err(),
        "transfer_checked with the wrong decimals must be rejected by the real Token program"
    );

    build_transfer_tokens_checked(
        &provider,
        PROGRAM_ID,
        400_000,
        6,
        TransferTokensCheckedAccounts {
            mint_authority: mint_authority_pda,
            from: from_vault.address(),
            mint: mint_pda,
            to: to_vault.address(),
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("transfer_checked with the correct decimals must succeed");

    assert_eq!(read_token_account_amount(&provider, &to_vault.address()), 400_000);
}

/// Real Token-2022 coverage: `create_mint2022`/`mint_to_vault2022`/
/// `transfer_tokens2022` parameterize `create_mint`/`mint_to_vault`/
/// `transfer_tokens`'s exact happy path over `Program<Token2022>` instead of
/// `Program<Token>`, passing `TOKEN_2022_PROGRAM_ID` (already preloaded by
/// `LiteSVM::new()` by default, confirmed in this same case's `create_ata`
/// test notes) as `token_program`. The first test in this repo to ever
/// exercise any Token-2022 code path.
#[test]
fn token_2022_mint_creation_and_cpis_work_end_to_end() {
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

    let id: u64 = 150;
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
    .expect("create_mint2022 should succeed (real CPI mint creation against Token-2022)");

    let mint_account = provider.get_account(&mint_pda).expect("mint account should exist");
    assert_eq!(
        mint_account.owner, TOKEN_2022_PROGRAM_ID,
        "the mint created via create_mint2022 must be owned by the real Token-2022 program"
    );

    // `create_token_account_with_program` lives in `naclac_client::utils` but
    // isn't re-exported at the crate root (only the `TOKEN_PROGRAM_ID`-only
    // `create_token_account` convenience wrapper is, per `naclac-client/src/lib.rs`),
    // so it's reached via the module path brought into scope by `use naclac_client::*;`.
    let vault_a = Keypair::new();
    utils::create_token_account_with_program(&provider, &vault_a, &mint_pda, &mint_authority_pda, &TOKEN_2022_PROGRAM_ID)
        .expect("client-side create_token_account_with_program (2022) for vault_a should succeed");
    let vault_b = Keypair::new();
    utils::create_token_account_with_program(&provider, &vault_b, &mint_pda, &mint_authority_pda, &TOKEN_2022_PROGRAM_ID)
        .expect("client-side create_token_account_with_program (2022) for vault_b should succeed");

    build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVault2022Accounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault_a.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault2022 should succeed (real mint_to CPI against Token-2022)");

    assert_eq!(read_token_account_amount(&provider, &vault_a.address()), 1_000_000);

    build_transfer_tokens2022(
        &provider,
        PROGRAM_ID,
        400_000,
        TransferTokens2022Accounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            from: vault_a.address(),
            to: vault_b.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("transfer_tokens2022 should succeed (real transfer CPI against Token-2022)");

    assert_eq!(read_token_account_amount(&provider, &vault_a.address()), 600_000);
    assert_eq!(read_token_account_amount(&provider, &vault_b.address()), 400_000);

    // `InterfaceAccount<TokenAccount>`'s dual owner check
    // (`Discriminator::validate_account`, `naclac-token/src/token.rs`)
    // accepts Token *or* Token-2022 (proven above by `vault_a`/`vault_b`
    // both being real Token-2022 accounts) but must still reject an account
    // owned by neither. `provider.payer.address()` is owned by the System
    // program, not the Token program, and â€” unlike `mint_authority_pda` â€”
    // isn't already referenced by another field in this same call, so it
    // doesn't also trip `ConstraintDuplicateMutableAccount`.
    let wrong_owner_result = build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVault2022Accounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: provider.payer.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    // `MintToVault2022 { mint, mint_authority, vault, token_program }` â€”
    // `vault` is field index 2; `ConstraintOwner` (4) -> 3000 + 2*100 + 4 = 3204.
    assert_custom_code(wrong_owner_result, 3204);
}

/// Regression test for the `init_cpi.rs` fix: with two token-program-shaped
/// fields in the same struct (`token_program: Program<Token>` declared
/// *before* `token_2022_program: Program<Token2022>`), `mint`'s `init` is
/// explicitly routed to `token_2022_program` via `token::program =
/// token_2022_program`. Before the fix, `init`'s CPI-target scan ignored
/// that pointer and always grabbed the first token-program field in struct
/// order â€” silently creating the mint under classic Token instead.
#[test]
fn create_mint2022_dual_token_program_targets_the_pointed_field() {
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

    let id: u64 = 250;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint2022_dual_token_program(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMint2022DualTokenProgramAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            token_2022_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_dual_token_program should succeed");

    let mint_account = provider.get_account(&mint_pda).expect("mint account should exist");
    assert_eq!(
        mint_account.owner, TOKEN_2022_PROGRAM_ID,
        "mint must be owned by Token-2022 (the field `token::program` pointed at), \
         not classic Token (the field declared first in the struct)"
    );
}

/// Companion negative case: passing classic Token's real address into the
/// `token_2022_program` slot must still be rejected â€” `Program<Token2022>`'s
/// own load-time address check (independent of the `init`-target fix above)
/// rejects it before the instruction body ever runs.
#[test]
fn create_mint2022_dual_token_program_rejects_wrong_program_in_targeted_slot() {
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

    let id: u64 = 251;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    let result = build_create_mint2022_dual_token_program(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMint2022DualTokenProgramAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            // Wrong on purpose: classic Token's address in the slot typed
            // `Program<Token2022>`.
            token_2022_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm();
    assert!(result.is_err(), "passing classic Token into the Program<Token2022> slot must fail");
}

/// Hand-assembles a real Token-2022 `Mint` (82 bytes) plus a
/// `TransferFeeConfig` extension (108 bytes) in the exact TLV wire format â€”
/// 83 bytes of zero padding from offset 82 up to the fixed extension-region
/// start at offset 165, a 1-byte `AccountType::Mint` (1) marker, then a
/// 4-byte TLV header (`type = 1` for `TransferFeeConfig`, `length = 108`)
/// followed by the 108-byte value. Every offset here was verified against
/// `spl-token-2022-interface`'s real source before writing
/// `naclac-token/src/extensions.rs` (see that file's doc comment for the
/// exact references checked) â€” this fixture exercises that same layout from
/// the opposite direction, encoding it by hand rather than reading it.
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

    // --- Mint base (82 bytes) ---
    data.extend_from_slice(&[0u8; 4]); // mint_authority COption tag: None
    data.extend_from_slice(&[0u8; 32]); // mint_authority value (unused since None)
    data.extend_from_slice(&0u64.to_le_bytes()); // supply
    data.push(6); // decimals
    data.push(1); // is_initialized
    data.extend_from_slice(&[0u8; 4]); // freeze_authority COption tag: None
    data.extend_from_slice(&[0u8; 32]); // freeze_authority value (unused since None)
    assert_eq!(data.len(), 82, "mint base must be exactly 82 bytes");

    // --- Zero padding up to the fixed extension-region start (165) ---
    data.extend_from_slice(&[0u8; 83]);
    assert_eq!(data.len(), 165);

    // --- AccountType::Mint marker ---
    data.push(1);

    // --- TLV header: type = TransferFeeConfig (1), length = 108 ---
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&108u16.to_le_bytes());

    // --- TransferFeeConfig value (108 bytes) ---
    data.extend_from_slice(&[0u8; 32]); // transfer_fee_config_authority: None
    data.extend_from_slice(&[0u8; 32]); // withdraw_withheld_authority: None
    data.extend_from_slice(&withheld_amount.to_le_bytes());
    data.extend_from_slice(&older_epoch.to_le_bytes());
    data.extend_from_slice(&older_max_fee.to_le_bytes());
    data.extend_from_slice(&older_basis_points.to_le_bytes());
    data.extend_from_slice(&newer_epoch.to_le_bytes());
    data.extend_from_slice(&newer_max_fee.to_le_bytes());
    data.extend_from_slice(&newer_basis_points.to_le_bytes());

    assert_eq!(data.len(), 278, "full fixture must be 165 + 1 + 4 + 108 = 278 bytes");
    data
}

/// Proves `InterfaceAccount<Mint>::get_extension::<TransferFeeConfig>()`
/// (`naclac-token/src/extensions.rs`) reads every field at the correct byte
/// offset, and that `calculate_fee`/`calculate_post_fee_amount` compute the
/// real Token-2022 fee formula correctly â€” including the `min(raw_fee,
/// maximum_fee)` cap, exercised here by picking a transfer amount whose
/// uncapped fee (4,000) exceeds the configured `maximum_fee` (2,000).
#[test]
fn check_transfer_fee_config_reads_extension_and_calculates_fee_correctly() {
    let provider = setup();

    let mint_bytes = build_transfer_fee_mint_bytes(
        500,    // withheld_amount
        0,      // older_transfer_fee_epoch
        1_000,  // older_transfer_fee_maximum_fee
        100,    // older_transfer_fee_basis_points (1%)
        0,      // newer_transfer_fee_epoch (already active at any current_epoch >= 0)
        2_000,  // newer_transfer_fee_maximum_fee
        200,    // newer_transfer_fee_basis_points (2%)
    );
    let mint = Keypair::new();
    provider
        .set_account(&mint.address(), mint_bytes, &TOKEN_2022_PROGRAM_ID, 10_000_000)
        .expect("set_account for the transfer-fee mint fixture should succeed");

    // 200_000 * 2% = 4_000 uncapped, but maximum_fee caps it at 2_000.
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
    // `mint` is field index 0; the handler-body mismatch returns
    // `NaclacError::ConstraintAddress.into()`, which defaults to index 0 ->
    // 3000 + 0*100 + 3 = 3003.
    assert_custom_code(wrong_result, 3003);
}

/// Hand-assembles a real Token-2022 `Mint` (82 bytes) plus a `TransferHook`
/// extension (64 bytes) in the exact TLV wire format â€” same padding/marker
/// scheme as `build_transfer_fee_mint_bytes`, `ExtensionType::TransferHook`
/// (14) instead of `TransferFeeConfig`.
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

/// Hand-assembles a real Token-2022 `TokenAccount` (165 bytes, `state = 1`
/// so it passes `validate_token_account_layout`) plus a
/// `TransferHookAccount` extension (1 byte). No padding needed â€”
/// `TokenAccount`'s own base length already equals `EXTENSION_BASE_OFFSET`.
fn build_transfer_hook_token_account_bytes(transferring: bool) -> Vec<u8> {
    let mut data = Vec::with_capacity(171);

    data.extend_from_slice(&[0u8; 32]); // mint
    data.extend_from_slice(&[0u8; 32]); // owner
    data.extend_from_slice(&0u64.to_le_bytes()); // amount
    data.extend_from_slice(&[0u8; 4]); // delegate COption: None
    data.extend_from_slice(&[0u8; 32]);
    data.push(1); // state: Initialized
    data.extend_from_slice(&[0u8; 4]); // is_native COption: None
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&0u64.to_le_bytes()); // delegated_amount
    data.extend_from_slice(&[0u8; 4]); // close_authority COption: None
    data.extend_from_slice(&[0u8; 32]);
    assert_eq!(data.len(), 165);

    data.push(2); // AccountType::Account
    data.extend_from_slice(&15u16.to_le_bytes()); // ExtensionType::TransferHookAccount
    data.extend_from_slice(&1u16.to_le_bytes());
    data.push(if transferring { 1 } else { 0 });

    assert_eq!(data.len(), 171, "165 + 1 + 4 + 1 = 171");
    data
}

/// Proves `InterfaceAccount<Mint>`/`InterfaceAccount<TokenAccount>`'s
/// `get_extension::<TransferHook>()`/`get_extension::<TransferHookAccount>()`
/// read every field at the correct byte offset, plus a rejection case
/// (mirrors `check_transfer_fee_config`'s own two-case shape).
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
    // `mint` is field index 0; the handler-body mismatch returns
    // `NaclacError::ConstraintAddress.into()`, which defaults to index 0 ->
    // 3000 + 0*100 + 3 = 3003.
    assert_custom_code(wrong_result, 3003);
}

/// Hand-assembles a real Token-2022 `Mint` (82 bytes) plus a
/// `PermanentDelegate` extension (32 bytes) in the exact TLV wire format â€”
/// same padding/marker scheme as `build_transfer_fee_mint_bytes`,
/// `ExtensionType::PermanentDelegate` (12) instead of `TransferFeeConfig`.
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
/// correct byte offset, plus a rejection case (mirrors
/// `check_transfer_hook`'s own two-case shape).
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
    // `mint` is field index 0; the handler-body mismatch returns
    // `NaclacError::ConstraintAddress.into()`, which defaults to index 0 ->
    // 3000 + 0*100 + 3 = 3003.
    assert_custom_code(wrong_result, 3003);
}

/// Proves every base `TokenAccount`/`Mint` field accessor
/// (`delegate`/`state`/`is_frozen`/`is_native`/`delegated_amount`/
/// `close_authority`/`mint_authority`/`freeze_authority`/`is_initialized`)
/// reads the correct value off a real, CPI-created mint and vault â€”
/// including the state transition a delegate approval and a freeze actually
/// produce on-chain, not a hand-crafted fixture.
#[test]
fn check_account_fields_reads_base_token_layout_correctly() {
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

    let id: u64 = 200;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);
    build_create_mint_with_freeze(
        &provider,
        PROGRAM_ID,
        id,
        mint_bump,
        6,
        CreateMintWithFreezeAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint_with_freeze should succeed");

    let vault = Keypair::new();
    create_token_account(&provider, &vault, &mint_pda, &mint_authority_pda)
        .expect("vault creation should succeed");

    let delegate = Keypair::new();
    build_approve_vault_delegate(
        &provider,
        PROGRAM_ID,
        75_000,
        ApproveVaultDelegateAccounts {
            vault: vault.address(),
            delegate: delegate.address(),
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("approve_vault_delegate should succeed");

    build_check_account_fields(
        &provider,
        PROGRAM_ID,
        CheckAccountFieldsArgs {
            expected_vault_delegate: Some(delegate.address()),
            expected_vault_delegated_amount: 75_000,
            expected_vault_state: 1, // Initialized, not yet frozen
            expected_vault_is_native: None,
            expected_vault_close_authority: None,
            expected_mint_authority: Some(mint_authority_pda),
            expected_mint_freeze_authority: Some(mint_authority_pda),
            expected_mint_is_initialized: 1,
        },
        CheckAccountFieldsAccounts { vault: vault.address(), mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_account_fields must accept the correct pre-freeze values");

    build_freeze_vault_account(
        &provider,
        PROGRAM_ID,
        FreezeVaultAccountAccounts {
            vault: vault.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("freeze_vault_account should succeed");

    build_check_account_fields(
        &provider,
        PROGRAM_ID,
        CheckAccountFieldsArgs {
            expected_vault_delegate: Some(delegate.address()),
            expected_vault_delegated_amount: 75_000,
            expected_vault_state: 2, // Frozen
            expected_vault_is_native: None,
            expected_vault_close_authority: None,
            expected_mint_authority: Some(mint_authority_pda),
            expected_mint_freeze_authority: Some(mint_authority_pda),
            expected_mint_is_initialized: 1,
        },
        CheckAccountFieldsAccounts { vault: vault.address(), mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_account_fields must accept the correct post-freeze values (state = 2)");

    let wrong_result = build_check_account_fields(
        &provider,
        PROGRAM_ID,
        CheckAccountFieldsArgs {
            expected_vault_delegate: Some(delegate.address()),
            expected_vault_delegated_amount: 999_999, // deliberately wrong
            expected_vault_state: 2,
            expected_vault_is_native: None,
            expected_vault_close_authority: None,
            expected_mint_authority: Some(mint_authority_pda),
            expected_mint_freeze_authority: Some(mint_authority_pda),
            expected_mint_is_initialized: 1,
        },
        CheckAccountFieldsAccounts { vault: vault.address(), mint: mint_pda },
    )
    .send_and_confirm();
    // `vault` is field index 0; the handler-body mismatch returns
    // `NaclacError::ConstraintAddress.into()`, which defaults to index 0 ->
    // 3000 + 0*100 + 3 = 3003.
    assert_custom_code(wrong_result, 3003);
}

/// Real end-to-end proof of `initialize_transfer_fee_config`
/// (`naclac-token/src/extensions.rs`): creates a genuine Token-2022 mint
/// with the `TransferFeeConfig` extension via `create_mint2022_with_transfer_fee`
/// (real `create_account_signed` + `InitializeTransferFeeConfig` +
/// `InitializeMint` CPIs, not a hand-assembled fixture), then reads it back
/// with `check_transfer_fee_config` â€” closing the loop the read-only test
/// (`check_transfer_fee_config_reads_extension_and_calculates_fee_correctly`)
/// couldn't: that one only proved the *reader* was correct against
/// hand-crafted bytes; this proves the *writer* (the CPI helper) produces
/// bytes the real Token-2022 program considers valid, since if the account
/// weren't genuinely initialized correctly, Token-2022 itself would have
/// rejected one of the three CPIs.
#[test]
fn create_mint2022_with_transfer_fee_produces_a_real_readable_extension() {
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

    let id: u64 = 300;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint2022_with_transfer_fee(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithTransferFeeArgs {
            id,
            mint_bump,
            decimals: 6,
            transfer_fee_basis_points: 150, // 1.5%
            maximum_fee: 5_000,
        },
        CreateMint2022WithTransferFeeAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect(
        "create_mint2022_with_transfer_fee should succeed (real create_account_signed + \
         InitializeTransferFeeConfig + InitializeMint CPIs against Token-2022)",
    );

    let mint_account = provider.get_account(&mint_pda).expect("mint account should exist");
    assert_eq!(
        mint_account.owner, TOKEN_2022_PROGRAM_ID,
        "the mint must be owned by the real Token-2022 program"
    );

    // 10_000 * 150 bps = 150; well under maximum_fee (5_000), so the
    // uncapped-fee path is exercised here (the earlier hand-crafted-fixture
    // test already exercised the capped path).
    build_check_transfer_fee_config(
        &provider,
        PROGRAM_ID,
        CheckTransferFeeConfigArgs {
            expected_withheld_amount: 0, // freshly initialized, nothing withheld yet
            expected_newer_basis_points: 150,
            expected_newer_maximum_fee: 5_000,
            current_epoch: 0,
            transfer_amount: 10_000,
            expected_fee: 150,
            expected_post_fee_amount: 9_850,
        },
        CheckTransferFeeConfigAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect(
        "check_transfer_fee_config must read back exactly what \
         create_mint2022_with_transfer_fee actually wrote on-chain",
    );
}

/// Real end-to-end proof of every ongoing-action `TransferFeeConfig` CPI in
/// `naclac-token/src/extensions.rs` â€” `exercise_transfer_fee_cpis` itself
/// asserts every intermediate balance/withheld-amount/config value against
/// a real Token-2022 mint and vaults, so a successful `send_and_confirm`
/// here is the actual proof; this test also independently re-checks the
/// final balances client-side.
#[test]
fn exercise_transfer_fee_cpis_moves_real_balances_and_fees() {
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

    let id: u64 = 301;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint2022_with_transfer_fee(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithTransferFeeArgs {
            id,
            mint_bump,
            decimals: 6,
            transfer_fee_basis_points: 1_000, // 10%, so fees below divide evenly
            maximum_fee: 1_000_000,           // high enough to never cap
        },
        CreateMint2022WithTransferFeeAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_transfer_fee should succeed");

    // 165 (base) + 1 (`AccountType` marker) + 2 (TLV type) + 2 (TLV len) + 8
    // (`TransferFeeAmount` value) = 178 â€” Token-2022 auto-requires
    // `TransferFeeAmount` space on every token account of a
    // `TransferFeeConfig` mint; the plain 165-byte
    // `create_token_account_with_program` fails `InitializeAccount3` with
    // `InvalidAccountData` against this mint. Matches
    // `naclac-token/src/extensions.rs`'s own `EXTENSION_BASE_OFFSET` (165)
    // + `AccountType` marker + TLV layout exactly.
    const VAULT_WITH_TRANSFER_FEE_SPACE: usize = 165 + 1 + 2 + 2 + 8;

    let source = Keypair::new();
    utils::create_token_account_with_program_and_space(
        &provider, &source, &mint_pda, &mint_authority_pda, &TOKEN_2022_PROGRAM_ID, VAULT_WITH_TRANSFER_FEE_SPACE,
    )
    .expect("client-side create_token_account_with_program_and_space for source should succeed");
    let vault_b = Keypair::new();
    utils::create_token_account_with_program_and_space(
        &provider, &vault_b, &mint_pda, &mint_authority_pda, &TOKEN_2022_PROGRAM_ID, VAULT_WITH_TRANSFER_FEE_SPACE,
    )
    .expect("client-side create_token_account_with_program_and_space for vault_b should succeed");
    let collector = Keypair::new();
    utils::create_token_account_with_program_and_space(
        &provider, &collector, &mint_pda, &mint_authority_pda, &TOKEN_2022_PROGRAM_ID, VAULT_WITH_TRANSFER_FEE_SPACE,
    )
    .expect("client-side create_token_account_with_program_and_space for collector should succeed");

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
    .expect("mint_to_vault2022 should succeed to fund source");

    build_exercise_transfer_fee_cpis(
        &provider,
        PROGRAM_ID,
        ExerciseTransferFeeCpisArgs {
            amount1: 10_000,
            decimals: 6,
            fee1: 1_000,
            amount2: 5_000,
            fee2: 500,
            new_basis_points: 50,
            new_maximum_fee: 999_999,
        },
        ExerciseTransferFeeCpisAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            source: source.address(),
            vault_b: vault_b.address(),
            collector: collector.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "exercise_transfer_fee_cpis should succeed end-to-end against real Token-2022 \
         (transfer_checked_with_fee, harvest_withheld_tokens_to_mint, \
         withdraw_withheld_tokens_from_mint, withdraw_withheld_tokens_from_accounts, \
         set_transfer_fee all self-assert on the way)",
    );

    assert_eq!(read_token_account_amount(&provider, &source.address()), 1_000_000 - 10_000 - 5_000);
    assert_eq!(read_token_account_amount(&provider, &vault_b.address()), 9_000 + 4_500);
    assert_eq!(read_token_account_amount(&provider, &collector.address()), 1_000 + 500);

    build_check_transfer_fee_config(
        &provider,
        PROGRAM_ID,
        CheckTransferFeeConfigArgs {
            expected_withheld_amount: 0,
            expected_newer_basis_points: 50,
            expected_newer_maximum_fee: 999_999,
            // Token-2022 only activates a `set_transfer_fee` rate change 2
            // epochs after the call â€” evaluating far enough in the future
            // guarantees the new (not old) rate is the active one here.
            current_epoch: 1_000,
            transfer_amount: 10_000,
            expected_fee: 50, // 0.5% of 10_000 at the NEW rate
            expected_post_fee_amount: 9_950,
        },
        CheckTransferFeeConfigAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_transfer_fee_config must read back set_transfer_fee's real on-chain effect");
}

/// Real end-to-end proof of `transfer_hook_update`
/// (`naclac-token/src/extensions.rs`) â€” `exercise_transfer_hook_update`
/// itself asserts the mint's `TransferHook.program_id` actually changed, so
/// a successful `send_and_confirm` here is the actual proof.
#[test]
fn exercise_transfer_hook_update_changes_the_real_hook_program() {
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

    let id: u64 = 302;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    let initial_hook_program = Keypair::new().address();
    build_create_mint2022_with_transfer_hook(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithTransferHookArgs {
            id,
            mint_bump,
            decimals: 6,
            hook_program_id: initial_hook_program,
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

    let new_hook_program = Keypair::new().address();
    build_exercise_transfer_hook_update(
        &provider,
        PROGRAM_ID,
        new_hook_program,
        ExerciseTransferHookUpdateAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "exercise_transfer_hook_update should succeed end-to-end against real Token-2022 \
         (transfer_hook_update, then self-asserts the new program id was actually written)",
    );
}

/// Real end-to-end proof that `DefaultAccountState` actually controls the
/// initial freeze state of newly created accounts against real Token-2022 â€”
/// a vault born frozen must both read back as `AccountState::Frozen` and
/// reject `MintTo` (`freeze_vault_account.rs`/`thaw_vault_account.rs` are
/// legacy-Token-only, `Account<TokenAccount>`/`Program<Token>`, so a
/// Token-2022 vault can't be thawed through them â€” the raw state byte is
/// asserted directly instead). After `update_default_account_state` flips
/// the mint's default to Initialized, a freshly created vault must read
/// back as Initialized and accept `MintTo` immediately.
#[test]
fn default_account_state_controls_new_account_freeze_state_and_update_changes_it() {
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

    let id: u64 = 303;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint2022_with_default_account_state(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithDefaultAccountStateArgs {
            id,
            mint_bump,
            decimals: 6,
            initial_state: 2, // AccountState::Frozen
        },
        CreateMint2022WithDefaultAccountStateAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_default_account_state should succeed");

    build_check_default_account_state(
        &provider,
        PROGRAM_ID,
        2,
        CheckDefaultAccountStateAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_default_account_state must accept the correct initial state");

    let vault_a = Keypair::new();
    utils::create_token_account_with_program(&provider, &vault_a, &mint_pda, &mint_authority_pda, &TOKEN_2022_PROGRAM_ID)
        .expect("vault_a creation should succeed");

    let frozen_mint_result = build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVault2022Accounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault_a.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    assert!(
        frozen_mint_result.is_err(),
        "a vault born frozen by DefaultAccountState must reject mint_to"
    );
    assert_eq!(
        read_token_account_state(&provider, &vault_a.address()),
        2,
        "a vault of a mint with initial_state = Frozen must itself be born Frozen"
    );

    build_exercise_default_account_state_update(
        &provider,
        PROGRAM_ID,
        1, // AccountState::Initialized
        ExerciseDefaultAccountStateUpdateAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "exercise_default_account_state_update should succeed end-to-end against real \
         Token-2022 (update_default_account_state, then self-asserts the new state was \
         actually written)",
    );

    let vault_b = Keypair::new();
    utils::create_token_account_with_program(&provider, &vault_b, &mint_pda, &mint_authority_pda, &TOKEN_2022_PROGRAM_ID)
        .expect("vault_b creation should succeed");
    assert_eq!(
        read_token_account_state(&provider, &vault_b.address()),
        1,
        "a vault created after update_default_account_state must be born Initialized"
    );

    build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        500_000,
        MintToVault2022Accounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault_b.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "a vault created after the update must be born Initialized (not frozen), so \
         mint_to_vault2022 must succeed without needing a thaw first",
    );
    assert_eq!(read_token_account_amount(&provider, &vault_b.address()), 500_000);
}

/// Real end-to-end proof that `ImmutableOwner` actually blocks
/// `SetAuthority(AccountOwner)` against real Token-2022 â€” not just that the
/// extension is readable. A vault created with `ImmutableOwner` rejects the
/// CPI; a normal vault (no extension) on the same mint accepts it, proving
/// the rejection is caused by the extension and not by
/// `set_token_account_owner` itself being broken.
#[test]
fn immutable_owner_blocks_set_authority_and_normal_vault_allows_it() {
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

    let id: u64 = 304;
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

    let owner_a = Keypair::new();
    let vault_a = Keypair::new();
    build_create_token_account_with_immutable_owner(
        &provider,
        PROGRAM_ID,
        CreateTokenAccountWithImmutableOwnerAccounts {
            payer: provider.payer.address(),
            token_account: vault_a.address(),
            mint: mint_pda,
            owner: owner_a.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .signer(&vault_a)
    .send_and_confirm()
    .expect("create_token_account_with_immutable_owner should succeed");

    build_check_immutable_owner(
        &provider,
        PROGRAM_ID,
        1,
        CheckImmutableOwnerAccounts { vault: vault_a.address() },
    )
    .send_and_confirm()
    .expect("check_immutable_owner must confirm the extension is present on vault_a");

    let new_owner_a = Keypair::new().address();
    let blocked_result = build_set_token_account_owner(
        &provider,
        PROGRAM_ID,
        SetTokenAccountOwnerAccounts {
            vault: vault_a.address(),
            current_owner: owner_a.address(),
            new_owner: new_owner_a,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .signer(&owner_a)
    .send_and_confirm();
    assert!(
        blocked_result.is_err(),
        "ImmutableOwner must reject SetAuthority(AccountOwner) against real Token-2022"
    );

    let owner_b = Keypair::new();
    let vault_b = Keypair::new();
    utils::create_token_account_with_program(
        &provider,
        &vault_b,
        &mint_pda,
        &owner_b.address(),
        &TOKEN_2022_PROGRAM_ID,
    )
    .expect("vault_b (no ImmutableOwner) creation should succeed");

    build_check_immutable_owner(
        &provider,
        PROGRAM_ID,
        0,
        CheckImmutableOwnerAccounts { vault: vault_b.address() },
    )
    .send_and_confirm()
    .expect("check_immutable_owner must confirm the extension is absent on vault_b");

    let new_owner_b = Keypair::new().address();
    build_set_token_account_owner(
        &provider,
        PROGRAM_ID,
        SetTokenAccountOwnerAccounts {
            vault: vault_b.address(),
            current_owner: owner_b.address(),
            new_owner: new_owner_b,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .signer(&owner_b)
    .send_and_confirm()
    .expect(
        "a normal vault with no ImmutableOwner extension must accept \
         SetAuthority(AccountOwner)",
    );
}

/// Real end-to-end proof of `enable_required_memo_transfers`/
/// `disable_required_memo_transfers` (`naclac-token/src/extensions/memo_transfer.rs`)
/// against real Token-2022: a vault created with the extension enabled
/// reads back present and required, and `disable_required_memo_transfers`
/// leaves the extension present but flips its flag â€” matching the real
/// protocol's toggle-not-remove behavior.
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

    let id: u64 = 505;
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
/// (not just that the extension is readable/toggleable): a transfer into a
/// memo-required vault with no preceding memo is rejected by real
/// Token-2022 (`check_previous_sibling_instruction_is_memo`, verified
/// against the real processor source), while the identical transfer
/// preceded by a real sibling CPI to the spl-memo program (v3) succeeds.
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

    let id: u64 = 506;
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
/// enforced by real Token-2022: `Enable` is sent as a genuine top-level
/// transaction (never through the `token` program â€” Token-2022 rejects
/// `Enable`/`Disable` whenever invoked via CPI, confirmed against the real
/// `cpi_guard::processor::process_toggle_cpi_guard`'s `in_cpi()` check),
/// then a CPI-issued `Approve` against the guarded vault is rejected while
/// the identical CPI against a plain vault succeeds â€” `Approve` is
/// unconditionally disallowed via CPI once `CpiGuard` is enabled, per the
/// real protocol's own doc comment on `CpiGuardInstruction::Enable`.
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

    let id: u64 = 507;
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
    // through the `token` program, since Token-2022 rejects it via CPI.
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
/// (`naclac-token/src/extensions/pausable.rs`) against real Token-2022: a
/// mint created with `initialize_pausable_config` reads back with the exact
/// authority given, `pause_mint` genuinely blocks `MintTo` (verified against
/// the real processor's `PausableConfig` check inside its transfer/mint
/// path), and `resume_mint` lifts the block again.
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

    let id: u64 = 508;
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
/// CPI, while an otherwise-identical mint with no `MintCloseAuthority`
/// extension rejects the same close attempt â€” real Token-2022 refuses
/// `CloseAccount` on a mint unless the extension is present and the signer
/// matches its registered close authority.
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

    let id: u64 = 509;
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
    let plain_id: u64 = 510;
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

/// Real end-to-end proof of `InterestBearingConfig`'s two CPIs
/// (`naclac-token/src/extensions/interest_bearing_mint.rs`) against real
/// Token-2022: a mint created with `initialize_interest_bearing_mint`
/// reads back with the exact rate authority and rate given, and
/// `update_interest_bearing_mint_rate` actually changes the on-chain rate.
#[test]
fn interest_bearing_mint_produces_a_real_readable_extension_and_update_changes_the_rate() {
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

    let id: u64 = 305;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint2022_with_interest_bearing_mint(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithInterestBearingMintArgs {
            id,
            mint_bump,
            decimals: 6,
            rate: 500,
        },
        CreateMint2022WithInterestBearingMintAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_interest_bearing_mint should succeed");

    build_check_interest_bearing_mint(
        &provider,
        PROGRAM_ID,
        CheckInterestBearingMintArgs {
            expected_rate_authority: Some(mint_authority_pda),
            expected_current_rate: 500,
        },
        CheckInterestBearingMintAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_interest_bearing_mint must accept the correct initial rate authority/rate");

    build_exercise_interest_bearing_mint_update_rate(
        &provider,
        PROGRAM_ID,
        750,
        ExerciseInterestBearingMintUpdateRateAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "exercise_interest_bearing_mint_update_rate should succeed end-to-end against real \
         Token-2022 (update_interest_bearing_mint_rate, then self-asserts the new rate was \
         actually written)",
    );

    build_check_interest_bearing_mint(
        &provider,
        PROGRAM_ID,
        CheckInterestBearingMintArgs {
            expected_rate_authority: Some(mint_authority_pda),
            expected_current_rate: 750,
        },
        CheckInterestBearingMintAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_interest_bearing_mint must accept the updated rate");
}

/// Real end-to-end proof of `MetadataPointer`'s two CPIs
/// (`naclac-token/src/extensions/metadata_pointer.rs`) against real
/// Token-2022 â€” including `update_metadata_pointer`, a gap fill beyond
/// anchor-spl-v2's own module, which only exposes `initialize`.
#[test]
fn metadata_pointer_produces_a_real_readable_extension_and_update_changes_it() {
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

    let id: u64 = 306;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    let initial_metadata_address = Keypair::new().address();
    build_create_mint2022_with_metadata_pointer(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithMetadataPointerArgs {
            id,
            mint_bump,
            decimals: 6,
            metadata_address: initial_metadata_address,
        },
        CreateMint2022WithMetadataPointerAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_metadata_pointer should succeed");

    build_check_metadata_pointer(
        &provider,
        PROGRAM_ID,
        Some(mint_authority_pda),
        Some(initial_metadata_address),
        CheckMetadataPointerAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_metadata_pointer must accept the correct initial authority/address");

    let new_metadata_address = Keypair::new().address();
    build_exercise_metadata_pointer_update(
        &provider,
        PROGRAM_ID,
        new_metadata_address,
        ExerciseMetadataPointerUpdateAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "exercise_metadata_pointer_update should succeed end-to-end against real Token-2022 \
         (update_metadata_pointer, then self-asserts the new address was actually written)",
    );

    build_check_metadata_pointer(
        &provider,
        PROGRAM_ID,
        Some(mint_authority_pda),
        Some(new_metadata_address),
        CheckMetadataPointerAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_metadata_pointer must accept the updated address");
}

/// Real end-to-end proof of `GroupPointer`'s two CPIs
/// (`naclac-token/src/extensions/group_pointer.rs`) against real
/// Token-2022.
#[test]
fn group_pointer_produces_a_real_readable_extension_and_update_changes_it() {
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

    let id: u64 = 307;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    let initial_group_address = Keypair::new().address();
    build_create_mint2022_with_group_pointer(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithGroupPointerArgs {
            id,
            mint_bump,
            decimals: 6,
            group_address: initial_group_address,
        },
        CreateMint2022WithGroupPointerAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_group_pointer should succeed");

    build_check_group_pointer(
        &provider,
        PROGRAM_ID,
        Some(mint_authority_pda),
        Some(initial_group_address),
        CheckGroupPointerAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_group_pointer must accept the correct initial authority/address");

    let new_group_address = Keypair::new().address();
    build_exercise_group_pointer_update(
        &provider,
        PROGRAM_ID,
        new_group_address,
        ExerciseGroupPointerUpdateAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "exercise_group_pointer_update should succeed end-to-end against real Token-2022 \
         (update_group_pointer, then self-asserts the new address was actually written)",
    );

    build_check_group_pointer(
        &provider,
        PROGRAM_ID,
        Some(mint_authority_pda),
        Some(new_group_address),
        CheckGroupPointerAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_group_pointer must accept the updated address");
}

/// Real end-to-end proof of `GroupMemberPointer`'s two CPIs
/// (`naclac-token/src/extensions/group_member_pointer.rs`) against real
/// Token-2022.
#[test]
fn group_member_pointer_produces_a_real_readable_extension_and_update_changes_it() {
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

    let id: u64 = 308;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    let initial_member_address = Keypair::new().address();
    build_create_mint2022_with_group_member_pointer(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithGroupMemberPointerArgs {
            id,
            mint_bump,
            decimals: 6,
            member_address: initial_member_address,
        },
        CreateMint2022WithGroupMemberPointerAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_group_member_pointer should succeed");

    build_check_group_member_pointer(
        &provider,
        PROGRAM_ID,
        Some(mint_authority_pda),
        Some(initial_member_address),
        CheckGroupMemberPointerAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_group_member_pointer must accept the correct initial authority/address");

    let new_member_address = Keypair::new().address();
    build_exercise_group_member_pointer_update(
        &provider,
        PROGRAM_ID,
        new_member_address,
        ExerciseGroupMemberPointerUpdateAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "exercise_group_member_pointer_update should succeed end-to-end against real \
         Token-2022 (update_group_member_pointer, then self-asserts the new address was \
         actually written)",
    );

    build_check_group_member_pointer(
        &provider,
        PROGRAM_ID,
        Some(mint_authority_pda),
        Some(new_member_address),
        CheckGroupMemberPointerAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_group_member_pointer must accept the updated address");
}

/// Real end-to-end proof of `initialize_permanent_delegate`
/// (`naclac-token/src/extensions/permanent_delegate.rs`) â€” this CPI had
/// zero test coverage before this test (`check_permanent_delegate_reads_extension_correctly`
/// only ever read a hand-crafted fixture, never a mint the real CPI
/// actually created). Two things are proven here, not one:
///
/// 1. The CPI actually runs against real Token-2022 and the resulting mint
///    reads back with the exact delegate given (`check_permanent_delegate`
///    against the real `mint_pda`, not `provider.set_account`).
/// 2. The extension actually *does* something: the delegate can `Burn`
///    from an account it doesn't own and never signed for, and the exact
///    same burn is rejected against an otherwise-identical mint that
///    lacks the extension â€” isolating that the extension itself, not some
///    other authority rule, is what grants the power.
#[test]
fn permanent_delegate_lets_the_delegate_burn_from_an_account_it_does_not_own() {
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

    let id: u64 = 309;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint2022_with_permanent_delegate(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithPermanentDelegateArgs { id, mint_bump, decimals: 6 },
        CreateMint2022WithPermanentDelegateAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_permanent_delegate should succeed");

    build_check_permanent_delegate(
        &provider,
        PROGRAM_ID,
        Some(mint_authority_pda),
        CheckPermanentDelegateAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_permanent_delegate must read the delegate off the real, CPI-created mint");

    let stranger = Keypair::new();
    let vault = Keypair::new();
    utils::create_token_account_with_program(&provider, &vault, &mint_pda, &stranger.address(), &TOKEN_2022_PROGRAM_ID)
        .expect("vault owned by a stranger (never signs anything below) should succeed");

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

    build_exercise_permanent_delegate_burn(
        &provider,
        PROGRAM_ID,
        400_000,
        ExercisePermanentDelegateBurnAccounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            vault: vault.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "the mint's permanent delegate must be able to burn from a vault it doesn't own and \
         never signed for, proven against real Token-2022",
    );
    assert_eq!(read_token_account_amount(&provider, &vault.address()), 600_000);

    // Negative baseline: an otherwise-identical mint with no PermanentDelegate
    // extension must reject the exact same burn attempt.
    let plain_id: u64 = 310;
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
    .expect("create_mint2022 (no PermanentDelegate) should succeed");

    let plain_stranger = Keypair::new();
    let plain_vault = Keypair::new();
    utils::create_token_account_with_program(
        &provider,
        &plain_vault,
        &plain_mint_pda,
        &plain_stranger.address(),
        &TOKEN_2022_PROGRAM_ID,
    )
    .expect("plain vault owned by a stranger should succeed");

    build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVault2022Accounts {
            mint: plain_mint_pda,
            mint_authority: mint_authority_pda,
            vault: plain_vault.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault2022 should succeed");

    let blocked_result = build_exercise_permanent_delegate_burn(
        &provider,
        PROGRAM_ID,
        400_000,
        ExercisePermanentDelegateBurnAccounts {
            mint_authority: mint_authority_pda,
            mint: plain_mint_pda,
            vault: plain_vault.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    assert!(
        blocked_result.is_err(),
        "without PermanentDelegate, mint_authority is neither the vault's owner nor an \
         approved delegate, so the identical burn must be rejected by real Token-2022"
    );
}

/// Real end-to-end proof of `initialize_non_transferable_mint`
/// (`naclac-token/src/extensions/non_transferable.rs`) â€” this CPI was
/// entirely missing before this test: an earlier pass wrongly concluded
/// `NonTransferable` had no dedicated init CPI at all. Two things are
/// proven, not one: the CPI actually runs against real Token-2022 and the
/// resulting mint reads back with the extension present, and the extension
/// actually *does* something â€” `transfer`/`transfer_checked` against
/// tokens of a `NonTransferable` mint is rejected by real Token-2022, while
/// the identical transfer succeeds against an otherwise-identical mint
/// lacking the extension, isolating that the extension itself is what
/// blocks it.
#[test]
fn non_transferable_blocks_transfers_and_plain_mint_allows_it() {
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

    let id: u64 = 311;
    let (mint_pda, mint_bump) =
        Address::find_program_address(&[b"mint", &id.to_le_bytes()], &PROGRAM_ID);

    build_create_mint2022_with_non_transferable(
        &provider,
        PROGRAM_ID,
        CreateMint2022WithNonTransferableArgs { id, mint_bump, decimals: 6 },
        CreateMint2022WithNonTransferableAccounts {
            payer: provider.payer.address(),
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            token_program: TOKEN_2022_PROGRAM_ID,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("create_mint2022_with_non_transferable should succeed");

    build_check_non_transferable(
        &provider,
        PROGRAM_ID,
        1,
        CheckNonTransferableAccounts { mint: mint_pda },
    )
    .send_and_confirm()
    .expect("check_non_transferable must confirm the extension is present on the real mint");

    // `NonTransferable` requires *two* account-side extensions, not one â€”
    // confirmed against `spl-token-2022-interface`'s real
    // `ExtensionType::required_init_account_extensions`, which maps
    // `NonTransferable => [NonTransferableAccount, ImmutableOwner]`. Both
    // are 0-byte values, so each only adds its own 4-byte TLV header:
    // 165 + 1 (AccountType) + 4 (NonTransferableAccount) + 4 (ImmutableOwner).
    const VAULT_WITH_NON_TRANSFERABLE_ACCOUNT_SPACE: usize = 165 + 1 + 4 + 4;
    let vault_a = Keypair::new();
    utils::create_token_account_with_program_and_space(
        &provider,
        &vault_a,
        &mint_pda,
        &mint_authority_pda,
        &TOKEN_2022_PROGRAM_ID,
        VAULT_WITH_NON_TRANSFERABLE_ACCOUNT_SPACE,
    )
    .expect("vault_a creation should succeed");
    let vault_b = Keypair::new();
    utils::create_token_account_with_program_and_space(
        &provider,
        &vault_b,
        &mint_pda,
        &mint_authority_pda,
        &TOKEN_2022_PROGRAM_ID,
        VAULT_WITH_NON_TRANSFERABLE_ACCOUNT_SPACE,
    )
    .expect("vault_b creation should succeed");

    build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVault2022Accounts {
            mint: mint_pda,
            mint_authority: mint_authority_pda,
            vault: vault_a.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault2022 should succeed (minting is still allowed on a NonTransferable mint)");

    let blocked_result = build_transfer_tokens2022(
        &provider,
        PROGRAM_ID,
        400_000,
        TransferTokens2022Accounts {
            mint_authority: mint_authority_pda,
            mint: mint_pda,
            from: vault_a.address(),
            to: vault_b.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm();
    assert!(
        blocked_result.is_err(),
        "a transfer of NonTransferable-mint tokens must be rejected by real Token-2022"
    );

    // Negative baseline: an otherwise-identical mint with no NonTransferable
    // extension must accept the exact same transfer.
    let plain_id: u64 = 312;
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
    .expect("create_mint2022 (no NonTransferable) should succeed");

    let plain_vault_a = Keypair::new();
    utils::create_token_account_with_program(&provider, &plain_vault_a, &plain_mint_pda, &mint_authority_pda, &TOKEN_2022_PROGRAM_ID)
        .expect("plain_vault_a creation should succeed");
    let plain_vault_b = Keypair::new();
    utils::create_token_account_with_program(&provider, &plain_vault_b, &plain_mint_pda, &mint_authority_pda, &TOKEN_2022_PROGRAM_ID)
        .expect("plain_vault_b creation should succeed");

    build_mint_to_vault2022(
        &provider,
        PROGRAM_ID,
        1_000_000,
        MintToVault2022Accounts {
            mint: plain_mint_pda,
            mint_authority: mint_authority_pda,
            vault: plain_vault_a.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("mint_to_vault2022 should succeed");

    build_transfer_tokens2022(
        &provider,
        PROGRAM_ID,
        400_000,
        TransferTokens2022Accounts {
            mint_authority: mint_authority_pda,
            mint: plain_mint_pda,
            from: plain_vault_a.address(),
            to: plain_vault_b.address(),
            token_program: TOKEN_2022_PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect(
        "without NonTransferable, the identical transfer must succeed against real Token-2022",
    );
}

/// Real end-to-end proof of `initialize_token_metadata`, `update_token_metadata_field`,
/// `remove_token_metadata_key`, `update_token_metadata_authority`, and
/// `emit_token_metadata` (`naclac-token/src/extensions/token_metadata.rs`),
/// exercised against the pinocchio backend (this test program's own
/// default feature). `TokenMetadata` has no fixed byte layout naclac can
/// read via `get_extension` (unlike every other extension in this crate) â€”
/// instead, each mutating step ends with `emit_token_metadata`, and this
/// test decodes the transaction's real on-chain return data via
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

    let id: u64 = 503;
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
/// `update_token_group_max_size`, `update_token_group_authority`, and
/// `initialize_token_group_member` (`naclac-token/src/extensions/token_group.rs`),
/// exercised against the pinocchio backend. Unlike `TokenMetadata`,
/// `TokenGroup`/`TokenGroupMember` are fixed-size `Pod` structs naclac reads
/// directly via `get_extension` (`check_token_group`/
/// `check_token_group_member`'s own on-chain assertions), so no return-data
/// decoding is needed here.
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

    let id: u64 = 504;
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
    let member_seed: u64 = 505;
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

    let id: u64 = 511;
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

    let id: u64 = 512;
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
