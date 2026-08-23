//! `find_buy_tx` pulled a real `buy` instruction's 23-account list, but
//! manual eyeballing can't reliably map position -> account name (order
//! doesn't match the documented IDL list 1:1, and most addresses aren't
//! recognizable at a glance). This computes every expected PDA/ATA directly
//! and matches them against the 23 real addresses, rather than guessing.
//!
//! Usage: cargo run --bin cross_reference_buy_tx [-- <rpc-url>]

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use std::str::FromStr;
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

// The 23 real addresses from find_buy_tx's output, in the real instruction's
// own account order (position 0..23).
const REAL_ACCOUNTS: [&str; 23] = [
    "Gygj9QQby4j2jryqyqBHvLP7ctv2SaANgh4sCb69BUpA",
    "13ec7XdrjF3h3YcqBTFDSReRcUFwbCnJaAQspM4j6DDJ",
    "CjMAkV6eHtmRFNWR3mXUgV9mL595PHSkcBHs4wtta85Q",
    "9SZvpw4sgLEEPEX1V7f726VbXsN3qWUin3S5cGsepump",
    "6NzMzJNcdDuJwD8hkPC2A2sQ3YHygYRG34Ypy1GL5Ek1",
    "BwWK17cbHxwWBKZkUYvzxLcNQ1YVyaFezduWbtm2de6s",
    "11111111111111111111111111111111",
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
    "4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf",
    "7BmKJL627r4nVoUc6vFwq8SZQLHH2d7DfBcik1Ljidwp",
    "GesfTA3X2arioaHp8bbKdjG9vJtskViWACZoYvxp4twS",
    "HbFqquMUzsyaCXsVAYGtXj7zciBMxFPNrRadJ5KLvnBk",
    "6SomJhLYw3pWJA6XhWBfCD9LFQ55oN3tg1rvMkK3KaUT",
    "Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1",
    "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P",
    "Hq2wp8uJ9jCPsYgNHex8RtqdvMPfVGoYwjvF1ATiwn2Y",
    "FGFrX2q1iAjyAojjeyFDxXqdmvegjPpSWsrPmrJjeQ2f",
    "8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt",
    "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ",
    "8FoNgzmjuSmiy86EPCWxvv1q7oJSu2WGA7wPymwki2LJ",
    "MAyhSmzXzV1pTf7LsNkrNwkWKTo4ougAJ1PPg47MD4e",
    "GJ9w5n4n5mXwX4FXwhHp4mc7SdevhDFdwVBpkUS1NNFA",
    "GXPFM2caqTtQYC2cJ5yJRi9VDkpsYZXzYdwYpGnLmtDL",
];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn ata_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &pk(ASSOCIATED_TOKEN_PROGRAM_ID),
    )
    .0
}

fn find_and_report(label: &str, candidate: &Pubkey) {
    let candidate_str = candidate.to_string();
    match REAL_ACCOUNTS.iter().position(|&a| a == candidate_str) {
        Some(i) => println!("  [{i}] {label} = {candidate} <- MATCH"),
        None => println!("  ??? {label} = {candidate} (no match in the 23 real accounts)"),
    }
}

fn main() {
    let rpc_url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    let pump_program = pk(PUMP_PROGRAM_ID);
    let pump_fees_program = pk(PUMP_FEES_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let token_2022_program = pk(TOKEN_2022_PROGRAM_ID);

    // Confirmed by vanity suffix in find_buy_tx's output.
    let mint = pk("9SZvpw4sgLEEPEX1V7f726VbXsN3qWUin3S5cGsepump");

    println!("=== Fixed/recognizable accounts (already visible directly) ===");
    println!("  [6]  system_program = 11111111111111111111111111111111");
    println!("  [7]  token_program (this tx) = TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb (Token-2022!)");
    println!("  [14] program (self) = {pump_program}");
    println!("  [18] fee_program = {pump_fees_program}");
    println!("  [3]  mint (vanity suffix) = {mint}");

    println!("\n=== Computed PDAs ===");
    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    find_and_report("global", &global);

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    find_and_report("bonding_curve", &bonding_curve);

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);
    find_and_report("event_authority", &event_authority);

    let (global_volume_accumulator, _) = Pubkey::find_program_address(&[b"global_volume_accumulator"], &pump_program);
    find_and_report("global_volume_accumulator", &global_volume_accumulator);

    let (fee_config, _) = Pubkey::find_program_address(&[b"fee_config", pump_program.as_ref()], &pump_fees_program);
    find_and_report("fee_config", &fee_config);

    // associated_bonding_curve: try both classic Token and Token-2022 as the
    // owning program, since this tx's own token_program resolved to Token-2022.
    let assoc_bc_classic = ata_address(&bonding_curve, &mint, &token_program);
    find_and_report("associated_bonding_curve (classic Token)", &assoc_bc_classic);
    let assoc_bc_2022 = ata_address(&bonding_curve, &mint, &token_2022_program);
    find_and_report("associated_bonding_curve (Token-2022)", &assoc_bc_2022);

    // Fetch the real bonding_curve account to read its .creator, then derive
    // creator_vault/associated_creator_vault.
    println!("\n=== Fetching real bonding_curve account for .creator ===");
    match client.get_account(&bonding_curve) {
        Ok(account) => {
            println!("bonding_curve exists, {} bytes, owner={}", account.data.len(), account.owner);
            // discriminator(8) + 5*u64(40) + complete(1) = 49, creator starts at 49
            if account.data.len() >= 49 + 32 {
                let creator = Pubkey::try_from(&account.data[49..81]).unwrap();
                println!("bonding_curve.creator = {creator}");
                let (creator_vault, _) = Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
                find_and_report("creator_vault", &creator_vault);
                let assoc_cv_classic = ata_address(&creator_vault, &mint, &token_program);
                find_and_report("associated_creator_vault (classic, wrong mint on purpose skip)", &assoc_cv_classic);
                let wsol = pk("So11111111111111111111111111111111111111112");
                let assoc_cv_wsol = ata_address(&creator_vault, &wsol, &token_program);
                find_and_report("associated_creator_vault (WSOL, classic Token)", &assoc_cv_wsol);
            }
        }
        Err(e) => println!("Failed to fetch bonding_curve: {e:?}"),
    }

    // Try every still-unmatched position as a `user` candidate (fee payer
    // might not be `user` if this tx was relayer/bot-submitted) — report any
    // candidate whose derived associated_user/user_volume_accumulator lands
    // on ANOTHER real position, which would confirm it decisively.
    println!("\n=== Trying every position as a `user` candidate ===");
    let already_matched: Vec<&str> = vec![
        REAL_ACCOUNTS[3], REAL_ACCOUNTS[6], REAL_ACCOUNTS[7], REAL_ACCOUNTS[8],
        REAL_ACCOUNTS[9], REAL_ACCOUNTS[11], REAL_ACCOUNTS[12], REAL_ACCOUNTS[13],
        REAL_ACCOUNTS[14], REAL_ACCOUNTS[15], REAL_ACCOUNTS[17], REAL_ACCOUNTS[18],
    ];
    for (i, addr_str) in REAL_ACCOUNTS.iter().enumerate() {
        if already_matched.contains(addr_str) {
            continue;
        }
        let candidate = pk(addr_str);
        let assoc_2022 = ata_address(&candidate, &mint, &token_2022_program);
        let assoc_classic = ata_address(&candidate, &mint, &token_program);
        let (uva, _) = Pubkey::find_program_address(&[b"user_volume_accumulator", candidate.as_ref()], &pump_program);
        let assoc_2022_str = assoc_2022.to_string();
        let assoc_classic_str = assoc_classic.to_string();
        let uva_str = uva.to_string();
        let hit_2022 = REAL_ACCOUNTS.iter().position(|&a| a == assoc_2022_str);
        let hit_classic = REAL_ACCOUNTS.iter().position(|&a| a == assoc_classic_str);
        let hit_uva = REAL_ACCOUNTS.iter().position(|&a| a == uva_str);
        if hit_2022.is_some() || hit_classic.is_some() || hit_uva.is_some() {
            println!("  [{i}] candidate={candidate}");
            if let Some(h) = hit_2022 { println!("       -> associated_user (Token-2022) MATCHES [{h}]"); }
            if let Some(h) = hit_classic { println!("       -> associated_user (classic Token) MATCHES [{h}]"); }
            if let Some(h) = hit_uva { println!("       -> user_volume_accumulator MATCHES [{h}]"); }
        }
    }

    // fee_recipient should equal global's own stored fee_recipient field —
    // fetch global directly to read it rather than guess by elimination.
    println!("\n=== Fetching real global account for .fee_recipient ===");
    match client.get_account(&global) {
        Ok(account) => {
            // discriminator(8) + initialized(1) + authority(32) = 41, fee_recipient starts at 41
            if account.data.len() >= 41 + 32 {
                let fee_recipient = Pubkey::try_from(&account.data[41..73]).unwrap();
                println!("global.fee_recipient = {fee_recipient}");
                find_and_report("fee_recipient", &fee_recipient);
            }
        }
        Err(e) => println!("Failed to fetch global: {e:?}"),
    }

    // buyback_fee_recipients[8] starts at struct-relative offset 733 (8-byte
    // disc + 733 = account.data offset 741), 8 * 32 bytes — computed from the
    // full real Global field list (pump-public-docs/idl/pump.json).
    println!("\n=== Fetching real global account for .buyback_fee_recipients[8] ===");
    match client.get_account(&global) {
        Ok(account) => {
            let start = 8 + 733;
            if account.data.len() >= start + 256 {
                for i in 0..8 {
                    let off = start + i * 32;
                    let recipient = Pubkey::try_from(&account.data[off..off + 32]).unwrap();
                    if recipient != Pubkey::default() {
                        find_and_report(&format!("buyback_fee_recipients[{i}]"), &recipient);
                        let assoc = ata_address(&recipient, &pk("So11111111111111111111111111111111111111112"), &token_program);
                        find_and_report(&format!("associated_buyback_fee_recipients[{i}] (WSOL)"), &assoc);
                    }
                }
            } else {
                println!("global account too short for buyback_fee_recipients at this offset ({} bytes)", account.data.len());
            }
        }
        Err(e) => println!("Failed to fetch global: {e:?}"),
    }

    // sell's own docstring: "For cashback coins, pass as remaining_accounts:
    // [0] user_volume_accumulator, [1] bonding_curve_v2." Testing whether
    // this mint is cashback-enabled, and whether a `bonding_curve_v2`-seeded
    // PDA appears among the unmatched positions.
    println!("\n=== Checking is_cashback_coin / is_mayhem_mode on this bonding_curve ===");
    if let Ok(account) = client.get_account(&bonding_curve) {
        if account.data.len() >= 8 + 40 + 1 + 32 + 2 {
            let is_mayhem_mode = account.data[8 + 40 + 1 + 32];
            let is_cashback_coin = account.data[8 + 40 + 1 + 32 + 1];
            println!("is_mayhem_mode = {is_mayhem_mode}, is_cashback_coin = {is_cashback_coin}");
        }
    }

    println!("\n=== Testing candidate bonding_curve_v2 seed variants ===");
    for seed in [&b"bonding-curve-v2"[..], &b"bonding_curve_v2"[..], &b"bonding-curve_v2"[..]] {
        let (candidate, _) = Pubkey::find_program_address(&[seed, mint.as_ref()], &pump_program);
        find_and_report(&format!("bonding_curve_v2 (seed={:?})", String::from_utf8_lossy(seed)), &candidate);
    }

    // Global's OTHER pubkey arrays (legacy fee_recipients[7], and
    // reserved_fee_recipients[7]) — check if any entry matches an unmapped position.
    println!("\n=== Checking Global.fee_recipients[7] (legacy array, offset 154) and reserved_fee_recipients[7] (offset 508) ===");
    if let Ok(account) = client.get_account(&global) {
        for (label, start) in [("fee_recipients", 8 + 154), ("reserved_fee_recipients", 8 + 508)] {
            if account.data.len() >= start + 224 {
                for i in 0..7 {
                    let off = start + i * 32;
                    let candidate = Pubkey::try_from(&account.data[off..off + 32]).unwrap();
                    if candidate != Pubkey::default() {
                        find_and_report(&format!("{label}[{i}]"), &candidate);
                    }
                }
            }
        }
    }

    println!("\n=== Remaining unmatched positions after the above (print full list again for reference) ===");
    for (i, a) in REAL_ACCOUNTS.iter().enumerate() {
        println!("  [{i}] {a}");
    }
}
