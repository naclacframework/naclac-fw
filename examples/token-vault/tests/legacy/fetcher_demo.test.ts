import * as naclac from "@naclac-fw/client/legacy";
import { TokenVaultClient } from "../../clients/typescript/src/generated/token_vault/client-legacy";
import assert from "assert";

// Custom helper removed - using built-in SDK helpers

describe("Fetcher Demo (Legacy)", () => {
  let client: TokenVaultClient;
  let payer: naclac.Keypair;
  let mint: naclac.PublicKey;
  let user1Ata: naclac.PublicKey;
  let user2Ata: naclac.PublicKey;
  
  let vault1Pda: naclac.PublicKey;
  let vault2Pda: naclac.PublicKey;
  let vault1Ata: naclac.PublicKey;
  let vault2Ata: naclac.PublicKey;
  let user1Vault1Pda: naclac.PublicKey;
  let user2Vault1Pda: naclac.PublicKey;
  let user1Vault2Pda: naclac.PublicKey;
  
  let vault1Bump: number;
  let vault2Bump: number;
  let user1Vault1Bump: number;
  let user2Vault1Bump: number;
  let user1Vault2Bump: number;

  let user2: naclac.Keypair;
  let vaultId1: bigint;
  let vaultId2: bigint;

  before(async () => {
    payer = await naclac.loadNodeWallet();
    client = new TokenVaultClient("localnet", payer);
    user2 = naclac.Keypair.generate();

    // Generate random vault IDs to avoid collision with prior test runs
    vaultId1 = BigInt(Math.floor(Math.random() * 1_000_000_000) + 1);
    vaultId2 = BigInt(Math.floor(Math.random() * 1_000_000_000) + 1000000000);
    
    // Create mint
    const mintSigner = naclac.Keypair.generate();
    mint = (await naclac.createMint(client.program.provider, mintSigner, new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID), payer.publicKey, 0)) as naclac.PublicKey;

    // Derive PDAs
    const vault1Result = client.getVaultAccountPda({ vaultId: vaultId1 });
    vault1Pda = vault1Result[0];
    vault1Bump = vault1Result[1];

    const vault2Result = client.getVaultAccountPda({ vaultId: vaultId2 });
    vault2Pda = vault2Result[0];
    vault2Bump = vault2Result[1];

    const user1Vault1Result = client.getUserAccountPda({ vaultId: vaultId1, payer: payer.publicKey, mint });
    user1Vault1Pda = user1Vault1Result[0];
    user1Vault1Bump = user1Vault1Result[1];

    const user2Vault1Result = client.getUserAccountPda({ vaultId: vaultId1, payer: user2.publicKey, mint });
    user2Vault1Pda = user2Vault1Result[0];
    user2Vault1Bump = user2Vault1Result[1];

    const user1Vault2Result = client.getUserAccountPda({ vaultId: vaultId2, payer: payer.publicKey, mint });
    user1Vault2Pda = user1Vault2Result[0];
    user1Vault2Bump = user1Vault2Result[1];

    // Create ATAs and mint some tokens
    user1Ata = (await naclac.createAta(client.program.provider, mint, payer.publicKey, new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID))) as naclac.PublicKey;
    user2Ata = (await naclac.createAta(client.program.provider, mint, user2.publicKey, new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID))) as naclac.PublicKey;
    await naclac.mintTo(client.program.provider, mint, payer.publicKey, 1000000n, new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID));
    
    // Transfer SOL to user2 so they can pay for transactions/rent
    await naclac.transferSol(client.program.provider, user2.publicKey, 100000000);
    await naclac.mintTo(client.program.provider, mint, user2.publicKey, 1000000n, new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID));

    // Setup vault ATAs
    vault1Ata = (await naclac.createAta(client.program.provider, mint, vault1Pda, new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID))) as naclac.PublicKey;
    vault2Ata = (await naclac.createAta(client.program.provider, mint, vault2Pda, new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID))) as naclac.PublicKey;
  });

  it("1. Initialize Vaults & Deposits", async () => {
    // Init Vault 1
    await client.initialize({ vaultId: vaultId1, vaultBump: vault1Bump }).accounts({ payer: payer.publicKey, vaultAccount: vault1Pda }).rpc();
    // Init Vault 2
    await client.initialize({ vaultId: vaultId2, vaultBump: vault2Bump }).accounts({ payer: payer.publicKey, vaultAccount: vault2Pda }).rpc();

    // Deposit User 1 into Vault 1
    await client.deposit({ vaultId: vaultId1, amount: 5000n, userBump: user1Vault1Bump })
      .accounts({
        payer: payer.publicKey,
        mint,
        vaultAccount: vault1Pda,
        vaultTokenAccount: vault1Ata,
        userTokenAccount: user1Ata,
        userAccount: user1Vault1Pda,
        tokenProgram: new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID),
      }).rpc();

    // Deposit User 2 into Vault 1
    const clientUser2 = new TokenVaultClient("localnet", user2);
    await clientUser2.deposit({ vaultId: vaultId1, amount: 7000n, userBump: user2Vault1Bump })
      .accounts({
        payer: user2.publicKey,
        mint,
        vaultAccount: vault1Pda,
        vaultTokenAccount: vault1Ata,
        userTokenAccount: user2Ata,
        userAccount: user2Vault1Pda,
        tokenProgram: new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID),
      }).rpc();

    // Deposit User 1 into Vault 2
    await client.deposit({ vaultId: vaultId2, amount: 9000n, userBump: user1Vault2Bump })
      .accounts({
        payer: payer.publicKey,
        mint,
        vaultAccount: vault2Pda,
        vaultTokenAccount: vault2Ata,
        userTokenAccount: user1Ata,
        userAccount: user1Vault2Pda,
        tokenProgram: new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID),
      }).rpc();
  });

  it("2. Test Account Fetchers", async () => {
    console.log("\n=================== START LEGACY FETCHER TESTS ===================");

    // -- fetchVault & fetchUserAccount using built-in stringifyJson
    console.log("\n--- fetchVault (Single) ---");
    const vault1 = await client.fetchVault(vault1Pda);
    console.log("Vault 1:", naclac.stringifyJson(vault1, 2));

    console.log("\n--- fetchUserAccount (Single) ---");
    const user1Vault1 = await client.fetchUserAccount(user1Vault1Pda);
    console.log("User 1 in Vault 1:", naclac.stringifyJson(user1Vault1, 2));

    // -- fetchMaybeVault & fetchMaybeUserAccount (exist and not exist)
    console.log("\n--- fetchMaybeVault (Exists) ---");
    const maybeVault1 = await client.fetchMaybeVault(vault1Pda);
    console.log("Exists:", maybeVault1 !== null, "Data:", maybeVault1 !== null ? naclac.stringifyJson(maybeVault1, 2) : "N/A");

    console.log("\n--- fetchMaybeVault (Does NOT exist) ---");
    const randomAddress = naclac.Keypair.generate().publicKey;
    const maybeFakeVault = await client.fetchMaybeVault(randomAddress);
    console.log("Exists:", maybeFakeVault !== null);

    // -- fetchAllVaults & fetchAllUserAccounts (Multiple)
    console.log("\n--- fetchAllVaults (Multiple) ---");
    const allVaults = await client.fetchAllVaults([vault1Pda, vault2Pda]);
    console.log("Fetched vaults count:", allVaults.length);
    allVaults.forEach((v: any, i) => {
      if (v) {
        console.log(`Vault ${i + 1} ID:`, v.vaultId.toString());
      }
    });

    // -- Test generic map helper naclac.mapKeysToValues
    console.log("\n--- mapKeysToValues (Generic Helper) ---");
    const vaultMap = naclac.mapKeysToValues([vault1Pda, vault2Pda], allVaults);
    console.log("Vault 1 mapped ID:", (vaultMap.get(vault1Pda) as any)?.vaultId.toString());
    console.log("Vault 2 mapped ID:", (vaultMap.get(vault2Pda) as any)?.vaultId.toString());

    // -- Test OOP fetchMultipleAsMap
    console.log("\n--- fetchMultipleAsMap (OOP Helper) ---");
    const oopMap = await client.program.account.Vault.fetchMultipleAsMap([vault1Pda, vault2Pda]);
    console.log("OOP Mapped Vault 1 authority:", (oopMap.get(vault1Pda) as any).authority.toString());
    console.log("OOP Mapped Vault 2 authority:", (oopMap.get(vault2Pda) as any).authority.toString());

    // -- fetchAllByOwner
    console.log("\n--- fetchAllVaultByOwner (All Vaults owned by program) ---");
    const allVaultsByOwner = await client.fetchAllVaultByOwner();
    console.log("All Vaults by owner count:", allVaultsByOwner.length);
    allVaultsByOwner.forEach((v: any) => console.log(`PublicKey: ${v.publicKey.toString()}, Vault ID: ${v.account.vaultId}`));

    console.log("\n--- fetchAllUserAccountByOwner (All UserAccounts owned by program) ---");
    const allUserAccountsByOwner = await client.fetchAllUserAccountByOwner();
    console.log("All UserAccounts by owner count:", allUserAccountsByOwner.length);
    allUserAccountsByOwner.forEach((u: any) => console.log(`PublicKey: ${u.publicKey.toString()}, Owner: ${u.account.owner.toString()}, Balance: ${u.account.balance.toString()}`));

    console.log("=================== END LEGACY FETCHER TESTS ===================\n");
  });
});
