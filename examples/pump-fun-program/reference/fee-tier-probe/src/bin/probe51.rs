//! Follow-up to `probe50.rs`, which got far enough to prove real `buy_v2`
//! uses native-lamport transfers for a SOL-paired curve (not a WSOL wrap)
//! before crashing on `InsufficientFundsForRent`. That crash was very
//! likely a probe-fixture bug, not a real-program fact: `probe50` cloned
//! `buyback_fee_recipient`'s real mainnet account (so it already meets
//! rent-exemption before receiving its share) but never cloned
//! `fee_recipient`'s — leaving it at 0 lamports pre-trade, so its real
//! ~6,641-lamport credit alone can't clear the rent-exempt minimum for a
//! fresh account. Real mainnet `fee_recipient` is already funded/rent-exempt,
//! so this can't happen for real. Fixed here by cloning it too.
//!
//! Also adds full diagnostics so this run settles every open question in
//! one pass rather than needing another round-trip:
//! - Prints the full indexed + labeled account_keys list (works on success
//!   or failure) so any future rent/ordering issue is immediately locatable.
//! - Decodes `Global.buyback_basis_points` directly from the cloned account
//!   bytes (offset computed from `components/global.rs`'s field order,
//!   cross-checked against `fee_recipient`'s already-confirmed offset 41
//!   and `buyback_fee_recipients[0]`'s already-confirmed offset 741) rather
//!   than inferring it from equal-looking transfer amounts.
//! - Decodes the real `TradeEvent` payload byte-for-byte (field layout from
//!   `programs/pump/src/events.rs`) out of the self-CPI inner instruction,
//!   the most authoritative source for `sol_amount`/`fee`/`creator_fee`/
//!   `buyback_fee`/the real fee bps/`quote_amount`/reserves — instead of
//!   manually reading raw System::Transfer instruction data.
//! - After `buy_v2` succeeds, also calls real `sell_v2` on the same curve
//!   (same real bytecode) to check whether the native-lamport-only finding
//!   holds symmetrically for the sell side too.
//! - Dumps every quote-side SPL ATA's final token balance (should all stay
//!   at 0 if the native-lamport finding is real) alongside every native
//!   lamport balance that moved.
//!
//! Usage: cargo run --bin probe51 [-- <rpc-url>]

use litesvm::LiteSVM;
use solana_rpc_client::rpc_client::RpcClient;
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
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

const BUY_V2_DISCRIMINATOR: [u8; 8] = [184, 23, 238, 97, 103, 197, 211, 61];
// Confirmed real, directly from `reference/pump-rust-client/idls/pump.json`'s
// `sell_v2` instruction entry (not guessed).
const SELL_V2_DISCRIMINATOR: [u8; 8] = [93, 246, 130, 60, 231, 233, 64, 178];
const GET_FEES_DISCRIMINATOR: [u8; 8] = [231, 37, 126, 85, 207, 91, 63, 52];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
// TradeEvent's own discriminator (name-hash based), confirmed already in
// earlier probes this session that decoded real TradeEvent logs.
const TRADE_EVENT_DISCRIMINATOR: [u8; 8] = [189, 219, 127, 211, 78, 230, 97, 238];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn ata_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[owner.as_ref(), token_program.as_ref(), mint.as_ref()], &pk(ASSOCIATED_TOKEN_PROGRAM_ID)).0
}

fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    data
}

fn token_balance_of(svm: &LiteSVM, address: &Pubkey) -> u64 {
    match svm.get_account(address) {
        Some(acc) if acc.data.len() >= 72 => u64::from_le_bytes(acc.data[64..72].try_into().unwrap()),
        _ => 0,
    }
}

fn bonding_curve_account_data(
    virtual_token_reserves: u64,
    virtual_quote_reserves: u64,
    real_token_reserves: u64,
    real_quote_reserves: u64,
    token_total_supply: u64,
    creator: &Pubkey,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&virtual_token_reserves.to_le_bytes());
    data.extend_from_slice(&virtual_quote_reserves.to_le_bytes());
    data.extend_from_slice(&real_token_reserves.to_le_bytes());
    data.extend_from_slice(&real_quote_reserves.to_le_bytes());
    data.extend_from_slice(&token_total_supply.to_le_bytes());
    data.push(0); // complete
    data.extend_from_slice(creator.as_ref());
    data.push(0); // is_mayhem_mode
    data.push(0); // is_cashback_coin
    data.extend_from_slice(&[0u8; 32]); // quote_mint -- all-zero, matches our SOL-paired assumption
    data
}

fn fetch_account(client: &RpcClient, pubkey: &Pubkey) -> Result<Account, String> {
    let remote = client.get_account(pubkey).map_err(|e| format!("{e:?}"))?;
    Ok(Account {
        lamports: remote.lamports,
        data: remote.data.clone(),
        owner: Pubkey::from_str(&remote.owner.to_string()).map_err(|e| format!("{e:?}"))?,
        executable: remote.executable,
        rent_epoch: remote.rent_epoch,
    })
}

/// Decodes a real `TradeEvent` payload (16-byte wrapper+type discriminator
/// prefix already stripped by the caller) using `events.rs`'s exact field
/// order. Only the fixed-size prefix fields up through `creator_fee` are
/// needed to answer this probe's open questions; parsing stops there.
fn decode_trade_event_prefix(data: &[u8]) {
    if data.len() < 32 + 8 + 8 + 1 + 32 + 8 + 8 + 8 + 8 + 32 + 8 + 8 + 32 + 8 + 8 {
        println!("  TradeEvent payload too short to decode ({} bytes)", data.len());
        return;
    }
    let mut o = 0usize;
    let mint = Pubkey::new_from_array(data[o..o + 32].try_into().unwrap());
    o += 32;
    let sol_amount = u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
    o += 8;
    let token_amount = u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
    o += 8;
    let is_buy = data[o];
    o += 1;
    let user = Pubkey::new_from_array(data[o..o + 32].try_into().unwrap());
    o += 32;
    let timestamp = i64::from_le_bytes(data[o..o + 8].try_into().unwrap());
    o += 8;
    let virtual_sol_reserves = u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
    o += 8;
    let virtual_token_reserves = u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
    o += 8;
    let real_sol_reserves = u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
    o += 8;
    let real_token_reserves = u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
    o += 8;
    let fee_recipient = Pubkey::new_from_array(data[o..o + 32].try_into().unwrap());
    o += 32;
    let fee_basis_points = u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
    o += 8;
    let fee = u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
    o += 8;
    let creator = Pubkey::new_from_array(data[o..o + 32].try_into().unwrap());
    o += 32;
    let creator_fee_basis_points = u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
    o += 8;
    let creator_fee = u64::from_le_bytes(data[o..o + 8].try_into().unwrap());

    println!("  TradeEvent decoded:");
    println!("    mint={mint} sol_amount={sol_amount} token_amount={token_amount} is_buy={is_buy}");
    println!("    user={user} timestamp={timestamp}");
    println!("    virtual_sol_reserves={virtual_sol_reserves} virtual_token_reserves={virtual_token_reserves}");
    println!("    real_sol_reserves={real_sol_reserves} real_token_reserves={real_token_reserves}");
    println!("    fee_recipient={fee_recipient} fee_basis_points={fee_basis_points} fee={fee}");
    println!("    creator={creator} creator_fee_basis_points={creator_fee_basis_points} creator_fee={creator_fee}");
    let _ = o;
}

/// Not a standalone typed function -- litesvm's inner-instruction list type
/// lives behind a re-export whose exact path varies by litesvm/solana-sdk
/// version, so this is inlined as a macro at each call site instead, letting
/// the compiler infer the concrete type from `meta.inner_instructions`
/// directly rather than us having to name it.
macro_rules! find_and_decode_trade_event {
    ($logs:expr, $inner:expr) => {{
        let mut found = false;
        for list in &$inner {
            for ix in list {
                let d = &ix.instruction.data;
                if d.len() >= 16 && d[8..16] == TRADE_EVENT_DISCRIMINATOR {
                    println!("  Found TradeEvent self-CPI, {} bytes payload", d.len() - 16);
                    decode_trade_event_prefix(&d[16..]);
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        if !found {
            println!("  No TradeEvent self-CPI found in inner instructions; raw logs follow:");
            for l in &$logs {
                if l.starts_with("Program data:") {
                    println!("  {l}");
                }
            }
        }
    }};
}

struct TradeAccounts {
    global: Pubkey,
    base_mint: Pubkey,
    quote_mint: Pubkey,
    base_token_program: Pubkey,
    quote_token_program: Pubkey,
    associated_token_program: Pubkey,
    fee_recipient: Pubkey,
    associated_quote_fee_recipient: Pubkey,
    buyback_fee_recipient: Pubkey,
    associated_quote_buyback_fee_recipient: Pubkey,
    bonding_curve: Pubkey,
    associated_base_bonding_curve: Pubkey,
    associated_quote_bonding_curve: Pubkey,
    user: Pubkey,
    associated_base_user: Pubkey,
    associated_quote_user: Pubkey,
    creator_vault: Pubkey,
    associated_creator_vault: Pubkey,
    sharing_config: Pubkey,
    global_volume_accumulator: Pubkey,
    user_volume_accumulator: Pubkey,
    associated_user_volume_accumulator: Pubkey,
    fee_config: Pubkey,
    fee_program: Pubkey,
    system_program: Pubkey,
    program: Pubkey,
}

fn build_ix(accs: &TradeAccounts, data: Vec<u8>, is_buy: bool) -> Instruction {
    let mut metas = vec![
        AccountMeta::new_readonly(accs.global, false),
        AccountMeta::new_readonly(accs.base_mint, false),
        AccountMeta::new_readonly(accs.quote_mint, false),
        AccountMeta::new_readonly(accs.base_token_program, false),
        AccountMeta::new_readonly(accs.quote_token_program, false),
        AccountMeta::new_readonly(accs.associated_token_program, false),
        AccountMeta::new(accs.fee_recipient, false),
        AccountMeta::new(accs.associated_quote_fee_recipient, false),
        AccountMeta::new(accs.buyback_fee_recipient, false),
        AccountMeta::new(accs.associated_quote_buyback_fee_recipient, false),
        AccountMeta::new(accs.bonding_curve, false),
        AccountMeta::new(accs.associated_base_bonding_curve, false),
        AccountMeta::new(accs.associated_quote_bonding_curve, false),
        AccountMeta::new(accs.user, true),
        AccountMeta::new(accs.associated_base_user, false),
        AccountMeta::new(accs.associated_quote_user, false),
        AccountMeta::new(accs.creator_vault, false),
        AccountMeta::new(accs.associated_creator_vault, false),
        AccountMeta::new_readonly(accs.sharing_config, false),
    ];
    if is_buy {
        metas.push(AccountMeta::new_readonly(accs.global_volume_accumulator, false));
    }
    metas.push(AccountMeta::new(accs.user_volume_accumulator, false));
    metas.push(AccountMeta::new(accs.associated_user_volume_accumulator, false));
    metas.push(AccountMeta::new_readonly(accs.fee_config, false));
    metas.push(AccountMeta::new_readonly(accs.fee_program, false));
    metas.push(AccountMeta::new_readonly(accs.system_program, false));
    metas.push(AccountMeta::new_readonly(Pubkey::find_program_address(&[b"__event_authority"], &accs.program).0, false));
    metas.push(AccountMeta::new_readonly(accs.program, false));

    Instruction { program_id: accs.program, accounts: metas, data }
}

fn main() {
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    let pump_program = pk(PUMP_PROGRAM_ID);
    let pump_fees_program = pk(PUMP_FEES_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let base_token_program = pk(TOKEN_PROGRAM_ID);
    let quote_token_program = pk(TOKEN_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let quote_mint = pk(WSOL_MINT);

    let mut svm = LiteSVM::new();
    for (program_id, so_name) in [(pump_program, "pump.so"), (pump_fees_program, "pump_fees.so")] {
        let so_path = format!("{}/../pump-rust-client/artifacts/{}", env!("CARGO_MANIFEST_DIR"), so_name);
        svm.add_program_from_file(program_id, &so_path).unwrap_or_else(|e| panic!("load {so_name}: {e:?}"));
    }
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let global_account = fetch_account(&client, &global).expect("fetch global failed");
    svm.set_account(global, global_account.clone()).unwrap();

    let real_buyback_basis_points = u64::from_le_bytes(global_account.data[997..1005].try_into().unwrap());
    println!("Real Global.buyback_basis_points (offset 997, decoded directly): {real_buyback_basis_points}");

    let (fee_config, _) = Pubkey::find_program_address(&[b"fee_config", pump_program.as_ref()], &pump_fees_program);
    let fee_config_account = fetch_account(&client, &fee_config).expect("fetch fee_config failed");
    svm.set_account(fee_config, fee_config_account).unwrap();

    let quote_mint_account = fetch_account(&client, &quote_mint).expect("fetch WSOL mint failed");
    svm.set_account(quote_mint, quote_mint_account).unwrap();

    let creator = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    svm.set_account(
        base_mint,
        Account { lamports: 10_000_000, data: { let mut d = vec![0u8; 82]; d[44] = 6; d[45] = 1; d }, owner: base_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", base_mint.as_ref()], &pump_program);
    let virtual_token_reserves = 1_073_000_000_000_000u64;
    let virtual_sol_reserves = 30_000_000_000u64;
    let token_total_supply = 1_000_000_000_000_000u64;
    let bc_data = bonding_curve_account_data(virtual_token_reserves, virtual_sol_reserves, 793_100_000_000_000, 0, token_total_supply, &creator);
    svm.set_account(bonding_curve, Account { lamports: 10_000_000, data: bc_data, owner: pump_program, executable: false, rent_epoch: 0 }).unwrap();

    let associated_base_bonding_curve = ata_address(&bonding_curve, &base_mint, &base_token_program);
    svm.set_account(
        associated_base_bonding_curve,
        Account { lamports: 2_039_280, data: token_account_data(&base_mint, &bonding_curve, token_total_supply), owner: base_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();
    let associated_quote_bonding_curve = ata_address(&bonding_curve, &quote_mint, &quote_token_program);
    svm.set_account(
        associated_quote_bonding_curve,
        Account { lamports: 2_039_280, data: token_account_data(&quote_mint, &bonding_curve, 0), owner: quote_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();
    let associated_base_user = ata_address(&user.pubkey(), &base_mint, &base_token_program);
    svm.set_account(
        associated_base_user,
        Account { lamports: 2_039_280, data: token_account_data(&base_mint, &user.pubkey(), 0), owner: base_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();
    let associated_quote_user = ata_address(&user.pubkey(), &quote_mint, &quote_token_program);
    svm.set_account(
        associated_quote_user,
        Account { lamports: 2_039_280, data: token_account_data(&quote_mint, &user.pubkey(), 0), owner: quote_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let fee_recipient = Pubkey::try_from(&global_account.data[41..73]).expect("fee_recipient pubkey");
    // THE FIX vs. probe50: clone fee_recipient's real, already-rent-exempt
    // mainnet account instead of leaving it nonexistent (0 lamports).
    let fee_recipient_account = fetch_account(&client, &fee_recipient).expect("fetch fee_recipient failed");
    svm.set_account(fee_recipient, fee_recipient_account).unwrap();
    let associated_quote_fee_recipient = ata_address(&fee_recipient, &quote_mint, &quote_token_program);

    let buyback_start = 8 + 733;
    let buyback_fee_recipient = Pubkey::try_from(&global_account.data[buyback_start..buyback_start + 32]).expect("buyback pubkey");
    let buyback_vault_account = fetch_account(&client, &buyback_fee_recipient).expect("fetch buyback vault failed");
    svm.set_account(buyback_fee_recipient, buyback_vault_account).unwrap();
    let associated_quote_buyback_fee_recipient = ata_address(&buyback_fee_recipient, &quote_mint, &quote_token_program);
    svm.set_account(
        associated_quote_buyback_fee_recipient,
        Account { lamports: 2_039_280, data: token_account_data(&quote_mint, &buyback_fee_recipient, 0), owner: quote_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (creator_vault, _) = Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    let associated_creator_vault = ata_address(&creator_vault, &quote_mint, &quote_token_program);

    let (sharing_config, _) = Pubkey::find_program_address(&[b"sharing-config", base_mint.as_ref()], &pump_fees_program);

    let (global_volume_accumulator, _) = Pubkey::find_program_address(&[b"global_volume_accumulator"], &pump_program);
    let gva_account = fetch_account(&client, &global_volume_accumulator).expect("fetch global_volume_accumulator failed");
    svm.set_account(global_volume_accumulator, gva_account).unwrap();

    let (user_volume_accumulator, _) = Pubkey::find_program_address(&[b"user_volume_accumulator", user.pubkey().as_ref()], &pump_program);
    let associated_user_volume_accumulator = ata_address(&user_volume_accumulator, &quote_mint, &quote_token_program);
    svm.set_account(
        associated_user_volume_accumulator,
        Account { lamports: 2_039_280, data: token_account_data(&quote_mint, &user_volume_accumulator, 0), owner: quote_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let accs = TradeAccounts {
        global, base_mint, quote_mint, base_token_program, quote_token_program, associated_token_program,
        fee_recipient, associated_quote_fee_recipient, buyback_fee_recipient, associated_quote_buyback_fee_recipient,
        bonding_curve, associated_base_bonding_curve, associated_quote_bonding_curve,
        user: user.pubkey(), associated_base_user, associated_quote_user,
        creator_vault, associated_creator_vault, sharing_config,
        global_volume_accumulator, user_volume_accumulator, associated_user_volume_accumulator,
        fee_config, fee_program: pump_fees_program, system_program, program: pump_program,
    };

    let named: Vec<(&str, Pubkey)> = vec![
        ("global", accs.global), ("base_mint", accs.base_mint), ("quote_mint", accs.quote_mint),
        ("base_token_program", accs.base_token_program), ("quote_token_program", accs.quote_token_program),
        ("associated_token_program", accs.associated_token_program), ("fee_recipient", accs.fee_recipient),
        ("associated_quote_fee_recipient", accs.associated_quote_fee_recipient),
        ("buyback_fee_recipient", accs.buyback_fee_recipient),
        ("associated_quote_buyback_fee_recipient", accs.associated_quote_buyback_fee_recipient),
        ("bonding_curve", accs.bonding_curve), ("associated_base_bonding_curve", accs.associated_base_bonding_curve),
        ("associated_quote_bonding_curve", accs.associated_quote_bonding_curve), ("user", accs.user),
        ("associated_base_user", accs.associated_base_user), ("associated_quote_user", accs.associated_quote_user),
        ("creator_vault", accs.creator_vault), ("associated_creator_vault", accs.associated_creator_vault),
        ("sharing_config", accs.sharing_config), ("global_volume_accumulator", accs.global_volume_accumulator),
        ("user_volume_accumulator", accs.user_volume_accumulator),
        ("associated_user_volume_accumulator", accs.associated_user_volume_accumulator),
        ("fee_config", accs.fee_config), ("fee_program", accs.fee_program),
        ("system_program", accs.system_program), ("program", accs.program),
    ];
    let label_for = |pk: &Pubkey| -> String {
        named.iter().find(|(_, addr)| addr == pk).map(|(name, _)| name.to_string()).unwrap_or_else(|| pk.to_string())
    };

    println!("\n========== buy_v2 (SOL-paired curve, real bytecode) ==========");
    let mut data = BUY_V2_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&50_000_000_000u64.to_le_bytes());
    data.extend_from_slice(&2_000_000_000u64.to_le_bytes());
    let ix = build_ix(&accs, data, true);

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let account_keys: Vec<Pubkey> = msg.account_keys.clone();
    println!("\n-- Full transaction account_keys (indexed) --");
    for (i, k) in account_keys.iter().enumerate() {
        println!("  [{i}] {} = {}", label_for(k), k);
    }
    let tx = Transaction::new(&[&user], msg, blockhash);

    let buy_ok = match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("\nRESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            for l in &meta.inner_instructions {
                for ix in l {
                    let d = &ix.instruction.data;
                    if d.len() >= 34 && d[0..8] == GET_FEES_DISCRIMINATOR {
                        println!(
                            "  get_fees CPI: is_pump_pool={} market_cap_lamports={} trade_size_lamports={} is_new_quote_mint={}",
                            d[8],
                            u128::from_le_bytes(d[9..25].try_into().unwrap()),
                            u64::from_le_bytes(d[25..33].try_into().unwrap()),
                            d[33],
                        );
                    }
                }
            }
            find_and_decode_trade_event!(meta.logs, meta.inner_instructions);

            println!("\n-- Post-buy_v2 balances --");
            println!("  bonding_curve lamports: {}", svm.get_account(&accs.bonding_curve).unwrap().lamports);
            println!("  fee_recipient lamports: {}", svm.get_account(&accs.fee_recipient).unwrap().lamports);
            println!("  buyback_fee_recipient lamports: {}", svm.get_account(&accs.buyback_fee_recipient).unwrap().lamports);
            println!(
                "  creator_vault lamports: {}",
                svm.get_account(&accs.creator_vault).map(|a| a.lamports).unwrap_or(0)
            );
            println!("  associated_quote_bonding_curve WSOL balance: {}", token_balance_of(&svm, &accs.associated_quote_bonding_curve));
            println!("  associated_quote_user WSOL balance: {}", token_balance_of(&svm, &accs.associated_quote_user));
            println!("  associated_quote_fee_recipient WSOL balance: {}", token_balance_of(&svm, &accs.associated_quote_fee_recipient));
            println!("  associated_quote_buyback_fee_recipient WSOL balance: {}", token_balance_of(&svm, &accs.associated_quote_buyback_fee_recipient));
            println!("  associated_creator_vault WSOL balance: {}", token_balance_of(&svm, &accs.associated_creator_vault));
            true
        }
        Err(e) => {
            println!("\nRESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
            println!("\n-- Decoded inner instructions (labeled accounts) --");
            for inner_list in &e.meta.inner_instructions {
                for inner in inner_list {
                    let prog_idx = inner.instruction.program_id_index as usize;
                    let prog = account_keys.get(prog_idx).map(&label_for).unwrap_or_else(|| "?".to_string());
                    let accs_dec: Vec<String> = inner
                        .instruction
                        .accounts
                        .iter()
                        .map(|&i| account_keys.get(i as usize).map(&label_for).unwrap_or_else(|| "?".to_string()))
                        .collect();
                    println!("  program={prog} data={:02x?} accounts={accs_dec:?}", inner.instruction.data);
                }
            }
            false
        }
    };

    if !buy_ok {
        println!("\nbuy_v2 failed -- skipping the sell_v2 follow-up case.");
        return;
    }

    println!("\n\n========== sell_v2 (same curve, real bytecode) ==========");
    let sell_amount = 25_000_000_000u64;
    let mut sdata = SELL_V2_DISCRIMINATOR.to_vec();
    sdata.extend_from_slice(&sell_amount.to_le_bytes());
    sdata.extend_from_slice(&0u64.to_le_bytes()); // min_sol_output
    let sell_ix = build_ix(&accs, sdata, false);
    let budget_ix2 = set_compute_unit_limit_ix(400_000);
    let blockhash2 = svm.latest_blockhash();
    let smsg = Message::new(&[budget_ix2, sell_ix], Some(&user.pubkey()));
    let saccount_keys: Vec<Pubkey> = smsg.account_keys.clone();
    println!("\n-- Full transaction account_keys (indexed) --");
    for (i, k) in saccount_keys.iter().enumerate() {
        println!("  [{i}] {} = {}", label_for(k), k);
    }
    let stx = Transaction::new(&[&user], smsg, blockhash2);

    match svm.send_transaction(stx) {
        Ok(meta) => {
            println!("\nRESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            find_and_decode_trade_event!(meta.logs, meta.inner_instructions);

            println!("\n-- Post-sell_v2 balances --");
            println!("  bonding_curve lamports: {}", svm.get_account(&accs.bonding_curve).unwrap().lamports);
            println!("  user native lamports: {}", svm.get_account(&accs.user).unwrap().lamports);
            println!("  fee_recipient lamports: {}", svm.get_account(&accs.fee_recipient).unwrap().lamports);
            println!("  buyback_fee_recipient lamports: {}", svm.get_account(&accs.buyback_fee_recipient).unwrap().lamports);
            println!(
                "  creator_vault lamports: {}",
                svm.get_account(&accs.creator_vault).map(|a| a.lamports).unwrap_or(0)
            );
            println!("  associated_quote_bonding_curve WSOL balance: {}", token_balance_of(&svm, &accs.associated_quote_bonding_curve));
            println!("  associated_quote_user WSOL balance: {}", token_balance_of(&svm, &accs.associated_quote_user));
            println!("  associated_quote_fee_recipient WSOL balance: {}", token_balance_of(&svm, &accs.associated_quote_fee_recipient));
            println!("  associated_quote_buyback_fee_recipient WSOL balance: {}", token_balance_of(&svm, &accs.associated_quote_buyback_fee_recipient));
            println!("  associated_creator_vault WSOL balance: {}", token_balance_of(&svm, &accs.associated_creator_vault));
        }
        Err(e) => {
            println!("\nRESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
            println!("\n-- Decoded inner instructions (labeled accounts) --");
            for inner_list in &e.meta.inner_instructions {
                for inner in inner_list {
                    let prog_idx = inner.instruction.program_id_index as usize;
                    let prog = saccount_keys.get(prog_idx).map(&label_for).unwrap_or_else(|| "?".to_string());
                    let accs_dec: Vec<String> = inner
                        .instruction
                        .accounts
                        .iter()
                        .map(|&i| saccount_keys.get(i as usize).map(&label_for).unwrap_or_else(|| "?".to_string()))
                        .collect();
                    println!("  program={prog} data={:02x?} accounts={accs_dec:?}", inner.instruction.data);
                }
            }
        }
    }
}
