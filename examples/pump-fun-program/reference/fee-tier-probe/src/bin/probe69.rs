//! Settles empirically what real `pump.so`'s `set_creator`/`set_metaplex_creator`
//! actually do, before writing any implementation. Real IDL accounts for both:
//! `mint`, `metadata` (PDA under the real Metaplex program, required, not
//! optional), `bonding_curve(mut)`, `event_authority`, `program`.
//! `set_creator` additionally has `set_creator_authority(signer,
//! relations=[global])`, `global` and one arg `creator: pubkey`.
//! `set_metaplex_creator` has neither a signer nor an arg at all -- fully
//! permissionless.
//!
//! Real `Metadata`/`Key`/`Creator` layout confirmed directly from the pinned
//! `mpl-token-metadata` crate source (`5.1.2-alpha.2`, the version in
//! `reference/pump-rust-client/Cargo.lock`), fetched from the real GitHub tag
//! `mpl-token-metadata-v5.1.2-alpha.2` -- not guessed from general Metaplex
//! knowledge: `key(Key=u8 enum, MetadataV1=4), update_authority(32),
//! mint(32), name(String), symbol(String), uri(String),
//! seller_fee_basis_points(u16), creators(Option<Vec<Creator>>),
//! primary_sale_happened(bool), is_mutable(bool), edition_nonce(Option<u8>),
//! token_standard(Option<TokenStandard>), collection(Option<Collection>),
//! uses(Option<Uses>), collection_details(Option<CollectionDetails>),
//! programmable_config(Option<ProgrammableConfig>)`. Everything after
//! `is_mutable` is left `None` (single `0u8` each) in every probe account
//! here -- `set_creator`/`set_metaplex_creator` only plausibly care about
//! `creators`, and `None` needs no further struct knowledge to encode
//! correctly. `Creator { address: Pubkey(32), verified: bool(1), share: u8(1) }`.
//!
//! Open questions this probe answers:
//! 1. Does `set_metaplex_creator` (permissionless) sync `bonding_curve.creator`
//!    from `metadata.creators`? Which creator does it pick if there are
//!    several (first? first verified?)?
//! 2. What happens when `creators` is `None`, or the `metadata` account
//!    doesn't exist at all (never funded/created) -- does it no-op, leave
//!    `bonding_curve.creator` unchanged, or fail outright?
//! 3. What does `set_creator`'s `creator: pubkey` arg actually do -- is a
//!    zero/default pubkey a sentinel meaning "read from metadata instead",
//!    does the arg always win outright, or something else?

use litesvm::LiteSVM;
use solana_sdk::{
    account::Account,
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const MPL_TOKEN_METADATA_PROGRAM_ID: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const SET_CREATOR_DISCRIMINATOR: [u8; 8] = [254, 148, 255, 112, 207, 142, 170, 165];
const SET_METAPLEX_CREATOR_DISCRIMINATOR: [u8; 8] = [138, 96, 174, 217, 48, 85, 197, 246];

const BONDING_CURVE_CREATOR_OFFSET: usize = 49;

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn mint_account_data(decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    data[44] = decimals;
    data[45] = 1; // is_initialized
    data
}

// Full, real 24-field `Global`, `authority`/`set_creator_authority` set,
// everything else zeroed -- offsets confirmed via `probe21.rs`.
fn global_account_data(authority: &Pubkey, set_creator_authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1); // initialized
    data.extend_from_slice(authority.as_ref()); // authority
    data.extend_from_slice(&[0u8; 32]); // fee_recipient
    data.extend_from_slice(&[0u8; 8 * 5]); // initial_virtual_token_reserves..fee_basis_points
    data.extend_from_slice(&[0u8; 32]); // withdraw_authority
    data.push(0); // enable_migrate
    data.extend_from_slice(&[0u8; 8 * 2]); // pool_migration_fee, creator_fee_basis_points
    data.extend_from_slice(&[0u8; 32 * 7]); // fee_recipients[7]
    data.extend_from_slice(set_creator_authority.as_ref()); // set_creator_authority
    data.extend_from_slice(&[0u8; 32]); // admin_set_creator_authority
    data.push(0); // create_v2_enabled
    data.extend_from_slice(&[0u8; 32]); // whitelist_pda
    data.extend_from_slice(&[0u8; 32]); // reserved_fee_recipient
    data.push(0); // mayhem_mode_enabled
    data.extend_from_slice(&[0u8; 32 * 7]); // reserved_fee_recipients[7]
    data.push(0); // is_cashback_enabled
    data.extend_from_slice(&[0u8; 32 * 8]); // buyback_fee_recipients[8]
    data.extend_from_slice(&0u64.to_le_bytes()); // buyback_basis_points
    data.extend_from_slice(&0u64.to_le_bytes()); // initial_virtual_quote_reserves
    data.extend_from_slice(&[0u8; 32]); // whitelisted_quote_mints[1]
    assert_eq!(data.len(), 1045, "Global layout drifted from probe21's confirmed offsets");
    data
}

// Full, real `BondingCurve`, `creator` set to whatever's passed in --
// offsets/field order confirmed via `probe23.rs`'s own working
// `bonding_curve_account_data`.
fn bonding_curve_account_data(creator: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&0u64.to_le_bytes()); // virtual_token_reserves
    data.extend_from_slice(&0u64.to_le_bytes()); // virtual_quote_reserves
    data.extend_from_slice(&0u64.to_le_bytes()); // real_token_reserves
    data.extend_from_slice(&0u64.to_le_bytes()); // real_quote_reserves
    data.extend_from_slice(&0u64.to_le_bytes()); // token_total_supply
    data.push(0); // complete
    data.extend_from_slice(creator.as_ref()); // creator
    data.push(0); // is_mayhem_mode
    data.push(0); // is_cashback_coin
    data.extend_from_slice(&[0u8; 32]); // quote_mint
    data.extend_from_slice(&[0u8; 36]); // _reserved_trailing
    data
}

fn borsh_string(s: &str) -> Vec<u8> {
    let mut out = (s.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(s.as_bytes());
    out
}

struct CreatorSpec {
    address: Pubkey,
    verified: bool,
    share: u8,
}

// Real `Metadata` account bytes, per the real `mpl-token-metadata` v5.1.2-alpha.2
// crate source (see module doc). `creators: None` when `creators` is empty.
fn metadata_account_data(mint: &Pubkey, creators: &[CreatorSpec]) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(4); // Key::MetadataV1
    data.extend_from_slice(&[0u8; 32]); // update_authority
    data.extend_from_slice(mint.as_ref()); // mint
    data.extend_from_slice(&borsh_string("probe")); // name
    data.extend_from_slice(&borsh_string("PRB")); // symbol
    data.extend_from_slice(&borsh_string("https://example.com")); // uri
    data.extend_from_slice(&0u16.to_le_bytes()); // seller_fee_basis_points
    if creators.is_empty() {
        data.push(0); // creators: None
    } else {
        data.push(1); // creators: Some
        data.extend_from_slice(&(creators.len() as u32).to_le_bytes());
        for c in creators {
            data.extend_from_slice(c.address.as_ref());
            data.push(c.verified as u8);
            data.push(c.share);
        }
    }
    data.push(0); // primary_sale_happened: false
    data.push(1); // is_mutable: true
    data.push(0); // edition_nonce: None
    data.push(0); // token_standard: None
    data.push(0); // collection: None
    data.push(0); // uses: None
    data.push(0); // collection_details: None
    data.push(0); // programmable_config: None
    data
}

struct Scenario<'a> {
    pump_program: Pubkey,
    global: Pubkey,
    bonding_curve: Pubkey,
    metadata: Pubkey,
    event_authority: Pubkey,
    mint: Pubkey,
    authority: &'a Keypair,
}

fn run_set_metaplex_creator(svm: &mut LiteSVM, s: &Scenario, label: &str) {
    println!("=== set_metaplex_creator: {label} ===");
    let accounts = vec![
        AccountMeta::new_readonly(s.mint, false),
        AccountMeta::new_readonly(s.metadata, false),
        AccountMeta::new(s.bonding_curve, false),
        AccountMeta::new_readonly(s.event_authority, false),
        AccountMeta::new_readonly(s.pump_program, false),
    ];
    let ix = Instruction { program_id: s.pump_program, accounts, data: SET_METAPLEX_CREATOR_DISCRIMINATOR.to_vec() };
    run_and_report(svm, s, ix);
}

fn run_set_creator(svm: &mut LiteSVM, s: &Scenario, label: &str, creator_arg: Pubkey) {
    println!("=== set_creator: {label} (arg={creator_arg}) ===");
    let mut data = SET_CREATOR_DISCRIMINATOR.to_vec();
    data.extend_from_slice(creator_arg.as_ref());
    let accounts = vec![
        AccountMeta::new_readonly(s.authority.pubkey(), true),
        AccountMeta::new_readonly(s.global, false),
        AccountMeta::new_readonly(s.mint, false),
        AccountMeta::new_readonly(s.metadata, false),
        AccountMeta::new(s.bonding_curve, false),
        AccountMeta::new_readonly(s.event_authority, false),
        AccountMeta::new_readonly(s.pump_program, false),
    ];
    let ix = Instruction { program_id: s.pump_program, accounts, data };
    run_and_report(svm, s, ix);
}

fn run_and_report(svm: &mut LiteSVM, s: &Scenario, ix: Instruction) {
    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&s.authority.pubkey()));
    let tx = Transaction::new(&[s.authority], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("  RESULT: SUCCESS ({} CU)", meta.compute_units_consumed);
            for line in &meta.logs {
                if line.starts_with("Program log:") {
                    println!("  LOG: {line}");
                }
            }
        }
        Err(e) => {
            println!("  RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                if line.starts_with("Program log:") {
                    println!("  LOG: {line}");
                }
            }
        }
    }

    let bc = svm.get_account(&s.bonding_curve).unwrap();
    let creator = Pubkey::try_from(&bc.data[BONDING_CURVE_CREATOR_OFFSET..BONDING_CURVE_CREATOR_OFFSET + 32]).unwrap();
    println!("  bonding_curve.creator after call = {creator}\n");
}

fn main() {
    let pump_program = pk(PUMP_PROGRAM_ID);
    let mpl_program = pk(MPL_TOKEN_METADATA_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);

    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_program, &so_path).unwrap_or_else(|e| panic!("load pump.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account { lamports: 10_000_000, data: global_account_data(&authority.pubkey(), &authority.pubkey()), owner: pump_program, executable: false, rent_epoch: 0 },
    ).unwrap();

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    let already_set_creator = Pubkey::new_unique();
    let metadata_creator_verified = Pubkey::new_unique();
    let metadata_creator_unverified = Pubkey::new_unique();
    println!("already_set_creator (seeded on bonding_curve for the already-set scenarios) = {already_set_creator}");
    println!("metadata_creator_verified (creators[1], verified)  = {metadata_creator_verified}");
    println!("metadata_creator_unverified (creators[0], unverified) = {metadata_creator_unverified}\n");

    // `creator` seeds `bonding_curve.creator` BEFORE the call --
    // `Pubkey::default()` simulates a bonding curve that has never had a
    // creator set; a real pubkey simulates one that already has (real
    // pump.so, per direct user observation, no-ops with "Bonding curve has
    // creator already set" when called against one of these -- scenarios H1/H2
    // confirm that directly here rather than assume it applies to both
    // instructions identically).
    let make_scenario = |svm: &mut LiteSVM, mint: Pubkey, creator: &Pubkey| -> Scenario {
        svm.set_account(mint, Account { lamports: 10_000_000, data: mint_account_data(6), owner: token_program, executable: false, rent_epoch: 0 }).unwrap();
        let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
        svm.set_account(bonding_curve, Account { lamports: 10_000_000, data: bonding_curve_account_data(creator), owner: pump_program, executable: false, rent_epoch: 0 }).unwrap();
        let (metadata, _) = Pubkey::find_program_address(&[b"metadata", mpl_program.as_ref(), mint.as_ref()], &mpl_program);
        Scenario { pump_program, global, bonding_curve, metadata, event_authority, mint, authority: &authority }
    };

    // Scenario A: bonding_curve.creator unset (default), metadata does not
    // exist at all (never funded/created).
    let mint_a = Pubkey::new_unique();
    let s = make_scenario(&mut svm, mint_a, &Pubkey::default());
    run_set_metaplex_creator(&mut svm, &s, "creator unset, metadata account does not exist");

    // Scenario B: bonding_curve.creator unset, metadata exists, creators = None.
    let mint_b = Pubkey::new_unique();
    let s = make_scenario(&mut svm, mint_b, &Pubkey::default());
    svm.set_account(s.metadata, Account { lamports: 10_000_000, data: metadata_account_data(&mint_b, &[]), owner: mpl_program, executable: false, rent_epoch: 0 }).unwrap();
    run_set_metaplex_creator(&mut svm, &s, "creator unset, metadata exists, creators = None");

    // Scenario C: bonding_curve.creator unset, metadata exists, creators =
    // Some([unverified, verified]) -- does it pick creators[0] (first), or
    // specifically the verified one?
    let mint_c = Pubkey::new_unique();
    let s = make_scenario(&mut svm, mint_c, &Pubkey::default());
    svm.set_account(
        s.metadata,
        Account {
            lamports: 10_000_000,
            data: metadata_account_data(&mint_c, &[
                CreatorSpec { address: metadata_creator_unverified, verified: false, share: 50 },
                CreatorSpec { address: metadata_creator_verified, verified: true, share: 50 },
            ]),
            owner: mpl_program,
            executable: false,
            rent_epoch: 0,
        },
    ).unwrap();
    run_set_metaplex_creator(&mut svm, &s, "creator unset, metadata exists, creators = [unverified, verified]");

    // Scenario J: `metadata` supplied is a REAL, valid, mpl_program-owned
    // Metadata account with real creators -- but it's the canonical PDA for
    // a DIFFERENT mint, not the one this bonding curve belongs to. Does real
    // pump.so verify `metadata` is the exact PDA for THIS mint (an implicit
    // Anchor `pda`-seeds check, since the real IDL marks `metadata` as
    // seeded), or does it only check the account's owner and blindly trust
    // whatever address is supplied (mirroring the `buyback_fee_recipients`
    // no-independent-verification finding from `probe66.rs`)?
    let mint_j = Pubkey::new_unique(); // the real subject mint
    let mint_k = Pubkey::new_unique(); // a different mint, whose metadata we'll substitute in
    let s = make_scenario(&mut svm, mint_j, &Pubkey::default());
    let (wrong_metadata, _) = Pubkey::find_program_address(&[b"metadata", mpl_program.as_ref(), mint_k.as_ref()], &mpl_program);
    let metadata_creator_wrong_mint = Pubkey::new_unique();
    println!("mint_j (subject) = {mint_j}, mint_k (different mint whose metadata we substitute) = {mint_k}");
    println!("metadata_creator_wrong_mint (creators[0] on mint_k's metadata) = {metadata_creator_wrong_mint}\n");
    svm.set_account(
        wrong_metadata,
        Account { lamports: 10_000_000, data: metadata_account_data(&mint_k, &[CreatorSpec { address: metadata_creator_wrong_mint, verified: true, share: 100 }]), owner: mpl_program, executable: false, rent_epoch: 0 },
    ).unwrap();
    let s = Scenario { metadata: wrong_metadata, ..s };
    run_set_metaplex_creator(&mut svm, &s, "metadata account is a REAL Metadata PDA, but for a DIFFERENT mint (wrong-mint substitution)");

    // Scenario H1: bonding_curve.creator ALREADY set (non-default), metadata
    // exists with a different creator -- direct confirmation of the
    // already-set no-op the user observed live for one of these instructions.
    let mint_h1 = Pubkey::new_unique();
    let s = make_scenario(&mut svm, mint_h1, &already_set_creator);
    svm.set_account(
        s.metadata,
        Account { lamports: 10_000_000, data: metadata_account_data(&mint_h1, &[CreatorSpec { address: metadata_creator_verified, verified: true, share: 100 }]), owner: mpl_program, executable: false, rent_epoch: 0 },
    ).unwrap();
    run_set_metaplex_creator(&mut svm, &s, "creator ALREADY set, metadata has a different creator (expect no-op)");

    // Scenario D: set_creator, bonding_curve.creator unset, creator arg =
    // Pubkey::default() (all zeros), metadata exists with a verified creator
    // -- does the zero arg act as a "read from metadata" sentinel?
    let mint_d = Pubkey::new_unique();
    let s = make_scenario(&mut svm, mint_d, &Pubkey::default());
    svm.set_account(
        s.metadata,
        Account { lamports: 10_000_000, data: metadata_account_data(&mint_d, &[CreatorSpec { address: metadata_creator_verified, verified: true, share: 100 }]), owner: mpl_program, executable: false, rent_epoch: 0 },
    ).unwrap();
    run_set_creator(&mut svm, &s, "creator unset, arg = default/zero, metadata has a verified creator", Pubkey::default());

    // Scenario E: set_creator, bonding_curve.creator unset, explicit non-zero
    // creator arg, metadata ALSO exists with a DIFFERENT creator -- does the
    // explicit arg win outright, or does metadata still take priority? (Real
    // mainnet txs `63bj2x...`/`3uff6J...` already show the arg landing
    // directly in the emitted event's `creator` field, but neither of those
    // had metadata to compete with -- this isolates that.)
    let mint_e = Pubkey::new_unique();
    let explicit_arg_creator = Pubkey::new_unique();
    println!("explicit_arg_creator (scenario E's set_creator arg) = {explicit_arg_creator}\n");
    let s = make_scenario(&mut svm, mint_e, &Pubkey::default());
    svm.set_account(
        s.metadata,
        Account { lamports: 10_000_000, data: metadata_account_data(&mint_e, &[CreatorSpec { address: metadata_creator_verified, verified: true, share: 100 }]), owner: mpl_program, executable: false, rent_epoch: 0 },
    ).unwrap();
    run_set_creator(&mut svm, &s, "creator unset, arg = explicit non-zero, metadata also has a (different) creator", explicit_arg_creator);

    // Scenario F: set_creator, bonding_curve.creator unset, explicit non-zero
    // creator arg, metadata does NOT exist at all.
    let mint_f = Pubkey::new_unique();
    let s = make_scenario(&mut svm, mint_f, &Pubkey::default());
    run_set_creator(&mut svm, &s, "creator unset, arg = explicit non-zero, metadata does not exist", explicit_arg_creator);

    // Scenario I: set_creator, bonding_curve.creator unset, metadata EXISTS
    // but creators = None (empty), explicit non-zero arg -- tests the real
    // IDL's own doc comment ("set the bonding curve creator from Metaplex
    // metadata OR input argument") as a literal fallback: does the arg get
    // used when metadata has nothing usable?
    let mint_i = Pubkey::new_unique();
    let s = make_scenario(&mut svm, mint_i, &Pubkey::default());
    svm.set_account(s.metadata, Account { lamports: 10_000_000, data: metadata_account_data(&mint_i, &[]), owner: mpl_program, executable: false, rent_epoch: 0 }).unwrap();
    run_set_creator(&mut svm, &s, "creator unset, metadata exists with creators = None, arg = explicit non-zero", explicit_arg_creator);

    // Scenario I2: same as I but arg is ALSO default/zero -- nothing usable
    // anywhere. Does it no-op like set_metaplex_creator's scenario B, or
    // fail, or write the zero arg literally?
    let mint_i2 = Pubkey::new_unique();
    let s = make_scenario(&mut svm, mint_i2, &Pubkey::default());
    svm.set_account(s.metadata, Account { lamports: 10_000_000, data: metadata_account_data(&mint_i2, &[]), owner: mpl_program, executable: false, rent_epoch: 0 }).unwrap();
    run_set_creator(&mut svm, &s, "creator unset, metadata exists with creators = None, arg = default/zero (nothing usable anywhere)", Pubkey::default());

    // Scenario H2: set_creator, bonding_curve.creator ALREADY set, metadata
    // exists with a creator -- does set_creator ALSO no-op the same way
    // set_metaplex_creator does (H1), or does an explicit arg override an
    // already-set creator where the metadata-driven sync would not? (Fixed
    // from the first run: originally had no metadata account at all, so it
    // only proved metadata is unconditionally required, same as A/F -- not
    // what this scenario was meant to isolate.)
    let mint_h2 = Pubkey::new_unique();
    let s = make_scenario(&mut svm, mint_h2, &already_set_creator);
    svm.set_account(
        s.metadata,
        Account { lamports: 10_000_000, data: metadata_account_data(&mint_h2, &[CreatorSpec { address: metadata_creator_verified, verified: true, share: 100 }]), owner: mpl_program, executable: false, rent_epoch: 0 },
    ).unwrap();
    run_set_creator(&mut svm, &s, "creator ALREADY set, metadata has a creator, arg = explicit non-zero (does the arg override it?)", explicit_arg_creator);

    // Scenario G: set_creator signed by someone other than
    // global.set_creator_authority -- confirms the authority check fires.
    // (Fixed from the first run: also needs a real metadata account so the
    // failure is attributable to the signer check, not to metadata being
    // missing -- same issue as H2 above.)
    let mint_g = Pubkey::new_unique();
    let not_authority = Keypair::new();
    svm.airdrop(&not_authority.pubkey(), 10_000_000_000).unwrap();
    let s_orig = make_scenario(&mut svm, mint_g, &Pubkey::default());
    svm.set_account(
        s_orig.metadata,
        Account { lamports: 10_000_000, data: metadata_account_data(&mint_g, &[CreatorSpec { address: metadata_creator_verified, verified: true, share: 100 }]), owner: mpl_program, executable: false, rent_epoch: 0 },
    ).unwrap();
    let s = Scenario { authority: &not_authority, ..s_orig };
    run_set_creator(&mut svm, &s, "signed by a non-set_creator_authority key (expect failure)", explicit_arg_creator);
}
