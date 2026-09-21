use naclac_client::*;
use token_zc_client::{
    fetch_mint_authority, get_mint_authority_pda,
    instructions::{
        build_check_ata_constraints, build_check_mint_freeze_authority, build_check_vault_constraints,
        build_check_vault_program, build_create_ata, build_create_ata_idempotent, build_create_mint,
        build_create_mint_with_freeze, build_init_mint_authority, build_mint_to_vault,
        build_transfer_tokens, CheckAtaConstraintsAccounts, CheckMintFreezeAuthorityAccounts,
        CheckVaultConstraintsAccounts, CheckVaultProgramAccounts, CreateAtaAccounts,
        CreateAtaIdempotentAccounts, CreateMintAccounts, CreateMintWithFreezeAccounts,
        InitMintAuthorityAccounts, MintToVaultAccounts, TransferTokensAccounts,
    },
    types::PROGRAM_ID,
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
