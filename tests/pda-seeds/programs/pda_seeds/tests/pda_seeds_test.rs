use naclac_client::*;
use pda_seeds_client::{
    get_child_pda, get_entry_pda, get_registry_pda,
    instructions::{
        build_init_child, build_init_entry, build_init_registry, build_init_tagged_child,
        build_read_config_entry_bare_bump, build_touch_config_entry_bare_bump,
        build_touch_config_entry_bare_bump_with_args, build_touch_entry_bare_bump,
        build_touch_registry_explicit_bump, InitChildAccounts, InitEntryAccounts,
        InitRegistryAccounts, InitTaggedChildAccounts, ReadConfigEntryBareBumpAccounts,
        TouchConfigEntryBareBumpAccounts, TouchConfigEntryBareBumpWithArgsAccounts,
        TouchEntryBareBumpAccounts, TouchRegistryExplicitBumpAccounts,
    },
    types::PROGRAM_ID,
    ConfigEntry, CONFIGENTRY_DISCRIMINATOR,
};

const SEED_REGISTRY: &[u8] = b"registry";
const SEED_ENTRY: &[u8] = b"entry";
const SEED_CHILD: &[u8] = b"child";
const SEED_TAGGED_CHILD: &[u8] = b"tagged_child";
const SEED_CONFIG_ENTRY: &[u8] = b"config_entry";

fn discriminated_bytes<T: bytemuck::Pod>(disc: [u8; 8], value: &T) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + core::mem::size_of::<T>());
    out.extend_from_slice(&disc);
    out.extend_from_slice(bytemuck::bytes_of(value));
    out
}

/// Asserts a transaction failed with exactly the given `Custom` error code
/// — not just "any error", the specific numeric `NaclacError` (framework
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

fn load_program(provider: &NaclacProvider) {
    let mut so_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.pop(); // programs
    so_path.pop(); // pda-seeds workspace root
    so_path.push("target/deploy/pda_seeds.so");

    provider
        .add_program(&PROGRAM_ID, so_path.to_str().unwrap())
        .expect("Failed to load pda_seeds program binary");
}

#[test]
fn seed_expression_shapes_all_resolve_correctly() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);

    // --- Case 1: literal seed, bare bump, init ---
    let (registry_pda, registry_bump) = Address::find_program_address(&[SEED_REGISTRY], &PROGRAM_ID);

    build_init_registry(
        &provider,
        PROGRAM_ID,
        InitRegistryAccounts {
            payer: provider.payer.address(),
            registry: registry_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_registry should succeed");

    // --- Case 2: dynamic seed via whole-account `.as_ref()`, init ---
    // On-chain, `registry.as_ref()` yields the registry PDA's own address
    // bytes (Account<T>: AsRefByteSlice) — mirror that here.
    let (entry_pda, entry_bump) =
        Address::find_program_address(&[SEED_ENTRY, registry_pda.as_ref()], &PROGRAM_ID);

    build_init_entry(
        &provider,
        PROGRAM_ID,
        entry_bump,
        InitEntryAccounts {
            payer: provider.payer.address(),
            registry: registry_pda,
            entry: entry_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect(
        "init_entry should succeed — this is the exact seed shape \
         ZERO_COPY_BORSH_PARITY_AUDIT.md finding #3 broke on \
         (`token_a_mint.as_ref()` misrewritten as a Deref field access)",
    );

    // --- Case 3: dynamic seed + bare bump, existing (non-init) account ---
    // The confirmed zero-coverage gap from ZERO_COPY_CONSISTENCY_PLAN.md
    // Part 6e — forces the macro to read `entry.bump` back out of the
    // account's own already-loaded data at runtime.
    build_touch_entry_bare_bump(
        &provider,
        PROGRAM_ID,
        TouchEntryBareBumpAccounts {
            registry: registry_pda,
            entry: entry_pda,
        },
    )
    .send_and_confirm()
    .expect("touch_entry_bare_bump should succeed (auto-bump on existing zero-copy account)");

    // --- Case 4: explicit bump, existing account — positive then negative ---
    build_touch_registry_explicit_bump(
        &provider,
        PROGRAM_ID,
        registry_bump,
        TouchRegistryExplicitBumpAccounts {
            registry: registry_pda,
        },
    )
    .send_and_confirm()
    .expect("touch_registry_explicit_bump with the correct bump should succeed");

    let wrong_bump = registry_bump.wrapping_sub(1);
    let wrong_result = build_touch_registry_explicit_bump(
        &provider,
        PROGRAM_ID,
        wrong_bump,
        TouchRegistryExplicitBumpAccounts {
            registry: registry_pda,
        },
    )
    .send_and_confirm();
    // `TouchRegistryExplicitBump { registry }` — `registry` is field index
    // 0; a wrong explicit `bump` mismatch emits `ConstraintSeeds` (6) ->
    // 3000 + 0*100 + 6 = 3006.
    assert_custom_code(wrong_result, 3006);

    // --- Case 5: nested field-access + method-chain seed, init ---
    // `registry.bump.to_le_bytes().as_ref()` — distinct from case 2's
    // whole-account access; exercises the generic seed-normalization
    // fallback's handling of a field-then-method chain.
    let registry_bump_bytes = registry_bump.to_le_bytes();
    let (child_pda, child_bump) =
        Address::find_program_address(&[SEED_CHILD, registry_bump_bytes.as_ref()], &PROGRAM_ID);

    build_init_child(
        &provider,
        PROGRAM_ID,
        child_bump,
        InitChildAccounts {
            payer: provider.payer.address(),
            registry: registry_pda,
            child: child_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_child should succeed (nested field-access + method-chain seed)");
}

/// Closes the gap the other test above doesn't cover: that test hand-derives
/// every PDA via `Address::find_program_address` directly, so it proves the
/// *on-chain program's* validation logic is correct but never actually calls
/// the *generated SDK helpers* (`get_registry_pda`/`get_entry_pda`/
/// `get_child_pda`) at all — a bug in the generator itself (like the one
/// `get_child_pda` had: it used to type `registry_bump` as a 32-byte
/// `Address` and derive a completely wrong PDA) could exist here
/// undetected. This test calls the generated helpers directly and confirms
/// the SDK and the real deployed on-chain program agree — not just that the
/// numbers match, but that a transaction built with the SDK-derived address
/// is actually accepted by the program.
#[test]
fn generated_pda_helpers_agree_with_on_chain_program() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);

    // Hand-derived, as ground truth to compare the generated helpers against.
    let (registry_pda_manual, registry_bump) =
        Address::find_program_address(&[SEED_REGISTRY], &PROGRAM_ID);

    // The generated helper — this is the actual thing under test.
    let (registry_pda, registry_bump_from_sdk) = get_registry_pda(&PROGRAM_ID);
    assert_eq!(
        registry_pda, registry_pda_manual,
        "get_registry_pda must match the hand-derived PDA"
    );
    assert_eq!(registry_bump_from_sdk, registry_bump);

    // Use the SDK-derived address for the actual transaction — if the SDK
    // were wrong, the on-chain program would reject this outright.
    build_init_registry(
        &provider,
        PROGRAM_ID,
        InitRegistryAccounts {
            payer: provider.payer.address(),
            registry: registry_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_registry with the SDK-derived registry PDA should be accepted on-chain");

    let (entry_pda_manual, entry_bump) =
        Address::find_program_address(&[SEED_ENTRY, registry_pda.as_ref()], &PROGRAM_ID);
    let (entry_pda, entry_bump_from_sdk) = get_entry_pda(&PROGRAM_ID, &registry_pda);
    assert_eq!(
        entry_pda, entry_pda_manual,
        "get_entry_pda must match the hand-derived PDA"
    );
    assert_eq!(entry_bump_from_sdk, entry_bump);

    build_init_entry(
        &provider,
        PROGRAM_ID,
        entry_bump,
        InitEntryAccounts {
            payer: provider.payer.address(),
            registry: registry_pda,
            entry: entry_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_entry with the SDK-derived entry PDA should be accepted on-chain");

    // This is the specific case that was broken: `get_child_pda` used to
    // take `registry_bump: &Address` (wrong — 32 bytes) instead of
    // `registry_bump: u8` (correct — 1 byte, matching the real on-chain
    // seed `registry.bump.to_le_bytes()`).
    let registry_bump_bytes = registry_bump.to_le_bytes();
    let (child_pda_manual, child_bump) =
        Address::find_program_address(&[SEED_CHILD, registry_bump_bytes.as_ref()], &PROGRAM_ID);
    let (child_pda, child_bump_from_sdk) = get_child_pda(&PROGRAM_ID, registry_bump);
    assert_eq!(
        child_pda, child_pda_manual,
        "get_child_pda must match the hand-derived PDA — this is the exact case that used \
         to silently compute a completely wrong address"
    );
    assert_eq!(child_bump_from_sdk, child_bump);

    build_init_child(
        &provider,
        PROGRAM_ID,
        child_bump,
        InitChildAccounts {
            payer: provider.payer.address(),
            registry: registry_pda,
            child: child_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_child with the SDK-derived child PDA should be accepted on-chain");
}

/// The other half of the on-chain/IDL/SDK consistency check: `init_child`
/// above proves a field-dependent seed with a *primitive* field type
/// (`u8`) gets a correctly-generated helper. This proves the opposite case
/// — a field-dependent seed whose field type is *not* a plain primitive
/// (`registry.label`, a `[u8; 4]` array) must NOT get a generated helper at
/// all. naclac-syn has no way to mechanically know how to turn an
/// arbitrary array/defined type into seed bytes the way it does for
/// primitives, so guessing would risk silently generating a helper that
/// computes the wrong PDA (exactly the bug `get_child_pda` had before it
/// was fixed) — the correct behavior is to skip it and leave a comment,
/// which this test asserts on the generated source directly.
#[test]
fn non_primitive_field_seed_pda_helper_is_correctly_skipped() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);

    let (registry_pda, registry_bump) = Address::find_program_address(&[SEED_REGISTRY], &PROGRAM_ID);

    build_init_registry(
        &provider,
        PROGRAM_ID,
        InitRegistryAccounts {
            payer: provider.payer.address(),
            registry: registry_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_registry should succeed");
    let _ = registry_bump;

    // `init_registry` sets `registry.label = [0, 0, 0, 0]`, so hand-derive
    // against that same value — the on-chain program itself handles this
    // seed shape fine; only the *generated client helper* is intentionally
    // absent.
    let registry_label = [0u8, 0, 0, 0];
    let (tagged_child_pda, tagged_child_bump) =
        Address::find_program_address(&[SEED_TAGGED_CHILD, registry_label.as_ref()], &PROGRAM_ID);

    build_init_tagged_child(
        &provider,
        PROGRAM_ID,
        tagged_child_bump,
        InitTaggedChildAccounts {
            payer: provider.payer.address(),
            registry: registry_pda,
            tagged_child: tagged_child_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect(
        "init_tagged_child should succeed on-chain even though no SDK PDA helper \
         exists for it — this seed shape is handled by the generic runtime \
         seed-normalization path, not a generated helper",
    );

    let generated_lib_rs = include_str!("../../../clients/rust/pda_seeds/src/lib.rs");
    assert!(
        !generated_lib_rs.contains("pub fn get_tagged_child_pda"),
        "the generator must NOT invent a get_tagged_child_pda helper for a seed \
         that depends on a non-primitive field type (`registry.label: [u8; 4]`) — \
         guessing here risks silently generating a helper that computes the wrong \
         PDA, exactly like the original get_child_pda bug"
    );
    assert!(
        generated_lib_rs.contains("get_tagged_child_pda intentionally not generated"),
        "the generator should explain *why* the helper is missing, not just \
         silently omit it"
    );
}

/// Isolates one specific shape from `touch_entry_bare_bump`'s (Case 3 above):
/// there, the dynamic seed is a sibling *typed* `Account<Registry>` field
/// (`registry.as_ref()`), which `seed_binding_tokens` resolves via its
/// cross-field-reference special case. Here the sibling is a plain,
/// unchecked `AccountInfo` with `.address()` called on it
/// (`config_program_id.address().as_ref()`) — not a zero-copy account field,
/// so it falls through to the generic expression-peeling path instead. Both
/// are verify-only (never `init`), bare `bump`, reading the bump stored on
/// the existing account — this checks whether that mechanism holds for the
/// *generic* path the same way it already does for the cross-field one.
#[test]
fn bare_bump_verify_with_account_info_seed_reference() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);

    let config_program_id = Keypair::new().address();
    let (config_entry_pda, bump) = Address::find_program_address(
        &[SEED_CONFIG_ENTRY, config_program_id.as_ref()],
        &PROGRAM_ID,
    );

    let fixture = ConfigEntry { bump, value: 5 };
    let data = discriminated_bytes(CONFIGENTRY_DISCRIMINATOR, &fixture);
    provider
        .set_account(&config_entry_pda, data, &PROGRAM_ID, 10_000_000)
        .expect("inject config_entry fixture");

    build_touch_config_entry_bare_bump(
        &provider,
        PROGRAM_ID,
        TouchConfigEntryBareBumpAccounts {
            config_program_id,
            config_entry: config_entry_pda,
        },
    )
    .send_and_confirm()
    .expect(
        "touch_config_entry_bare_bump should succeed — bare bump, verify-only, \
         seed referencing a plain AccountInfo sibling field via .address()",
    );

    let raw = provider
        .get_account_data(&config_entry_pda)
        .expect("config_entry should be readable after touch");
    let updated: ConfigEntry = *bytemuck::from_bytes(&raw[8..]);
    assert_eq!(updated.value, 6, "touch should have incremented value");
}

/// Same seed shape as `bare_bump_verify_with_account_info_seed_reference`,
/// but the instruction also takes a `u128` arg ahead of the bare-bump
/// account check — matching `pump_fees::get_fees`'s exact arg shape
/// (`Bool, u128, u64, Bool` before its `AccountInfo`-seeded, bare-bump
/// `fee_config` account). Isolates whether combining instruction-arg parsing
/// (particularly a `u128`, given this framework's two independent
/// arg-wire-format parsers) with this seed shape breaks the seeds check —
/// something no other passing seed-shape test exercises.
#[test]
fn bare_bump_verify_with_account_info_seed_reference_and_ix_args() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);

    let config_program_id = Keypair::new().address();
    let (config_entry_pda, bump) = Address::find_program_address(
        &[SEED_CONFIG_ENTRY, config_program_id.as_ref()],
        &PROGRAM_ID,
    );

    let fixture = ConfigEntry { bump, value: 5 };
    let data = discriminated_bytes(CONFIGENTRY_DISCRIMINATOR, &fixture);
    provider
        .set_account(&config_entry_pda, data, &PROGRAM_ID, 10_000_000)
        .expect("inject config_entry fixture");

    build_touch_config_entry_bare_bump_with_args(
        &provider,
        PROGRAM_ID,
        Bool::from(false),
        500_000_000u128,
        0u64,
        Bool::from(false),
        TouchConfigEntryBareBumpWithArgsAccounts {
            config_program_id,
            config_entry: config_entry_pda,
        },
    )
    .send_and_confirm()
    .expect(
        "touch_config_entry_bare_bump_with_args should succeed — same seed shape as \
         the args-free version, but with get_fees's exact ix-arg shape ahead of it",
    );

    let raw = provider
        .get_account_data(&config_entry_pda)
        .expect("config_entry should be readable after touch");
    let updated: ConfigEntry = *bytemuck::from_bytes(&raw[8..]);
    assert_eq!(updated.value, 6, "touch should have incremented value");
}

/// The one remaining structural difference from `get_fees.rs`: that
/// account is verify-only *and never `mut`* (a plain read), whereas every
/// other seed-shape test so far marks the account `mut` so it can write to
/// it. Isolates whether a non-`mut`, bare-bump, `AccountInfo`-seeded
/// account being only read (never written) — with the same ix-arg shape —
/// changes how/when the seeds check runs.
#[test]
fn read_only_bare_bump_verify_with_account_info_seed_reference() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer);
    load_program(&provider);

    let config_program_id = Keypair::new().address();
    let (config_entry_pda, bump) = Address::find_program_address(
        &[SEED_CONFIG_ENTRY, config_program_id.as_ref()],
        &PROGRAM_ID,
    );

    let fixture = ConfigEntry { bump, value: 42 };
    let data = discriminated_bytes(CONFIGENTRY_DISCRIMINATOR, &fixture);
    provider
        .set_account(&config_entry_pda, data, &PROGRAM_ID, 10_000_000)
        .expect("inject config_entry fixture");

    let result = build_read_config_entry_bare_bump(
        &provider,
        PROGRAM_ID,
        Bool::from(false),
        500_000_000u128,
        0u64,
        Bool::from(false),
        ReadConfigEntryBareBumpAccounts {
            config_program_id,
            config_entry: config_entry_pda,
        },
    )
    .send_and_confirm()
    .expect(
        "read_config_entry_bare_bump should succeed — non-mut, verify-only, bare bump, \
         seed referencing a plain AccountInfo sibling field via .address()",
    );

    let value: u64 = *bytemuck::from_bytes(&result.return_data);
    assert_eq!(value, 42, "should read back the injected fixture's value");
}
