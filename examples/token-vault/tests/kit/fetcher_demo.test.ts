import * as naclac from "@naclac-fw/client/kit";
import { TokenVaultClient } from "../../clients/typescript/src/generated/token_vault";
import assert from "assert";

// Custom helper removed - using built-in SDK helpers


describe("Fetcher Demo (Kit)", () => {
  let client: TokenVaultClient;
  let payer: naclac.KeyPairSigner;
  let mint: naclac.Address;
  let user1Ata: naclac.Address;
  let user2Ata: naclac.Address;
  
  let vault1Pda: naclac.Address;
  let vault2Pda: naclac.Address;
  let vault1Ata: naclac.Address;
  let vault2Ata: naclac.Address;
  let user1Vault1Pda: naclac.Address;
  let user2Vault1Pda: naclac.Address;
  let user1Vault2Pda: naclac.Address;
  
  let vault1Bump: number;
  let vault2Bump: number;
  let user1Vault1Bump: number;
  let user2Vault1Bump: number;
  let user1Vault2Bump: number;

  let user2: naclac.KeyPairSigner;
  let vaultId1: bigint;
  let vaultId2: bigint;

  before(async () => {
    payer = await naclac.loadNodeWallet();
    client = new TokenVaultClient("localnet", payer);
    user2 = await naclac.generateExtractableKeyPairSigner();

    // Generate random vault IDs to avoid collision with prior test runs
    vaultId1 = BigInt(Math.floor(Math.random() * 1_000_000_000) + 1);
    vaultId2 = BigInt(Math.floor(Math.random() * 1_000_000_000) + 1000000000);
    
    // Create mint
    const mintSigner = await naclac.generateExtractableKeyPairSigner();
    mint = (await naclac.createMint(client.program.provider, mintSigner, naclac.TOKEN_PROGRAM_ID, payer.address, 0)) as naclac.Address;

    // Derive PDAs
    const vault1Result = await client.getVaultAccountPda({ vaultId: vaultId1 });
    vault1Pda = vault1Result[0];
    vault1Bump = vault1Result[1];

    const vault2Result = await client.getVaultAccountPda({ vaultId: vaultId2 });
    vault2Pda = vault2Result[0];
    vault2Bump = vault2Result[1];

    const user1Vault1Result = await client.getUserAccountPda({ vaultId: vaultId1, payer: payer.address, mint });
    user1Vault1Pda = user1Vault1Result[0];
    user1Vault1Bump = user1Vault1Result[1];

    const user2Vault1Result = await client.getUserAccountPda({ vaultId: vaultId1, payer: user2.address, mint });
    user2Vault1Pda = user2Vault1Result[0];
    user2Vault1Bump = user2Vault1Result[1];

    const user1Vault2Result = await client.getUserAccountPda({ vaultId: vaultId2, payer: payer.address, mint });
    user1Vault2Pda = user1Vault2Result[0];
    user1Vault2Bump = user1Vault2Result[1];

    // Create ATAs and mint some tokens
    user1Ata = (await naclac.createAta(client.program.provider, mint, payer.address, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    user2Ata = (await naclac.createAta(client.program.provider, mint, user2.address, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    await naclac.mintTo(client.program.provider, mint, payer.address, 1000000n, naclac.TOKEN_PROGRAM_ID);
    await naclac.transferSol(client.program.provider, user2.address, 100000000n);
    await naclac.mintTo(client.program.provider, mint, user2.address, 1000000n, naclac.TOKEN_PROGRAM_ID);

    // Setup vault ATAs
    vault1Ata = (await naclac.createAta(client.program.provider, mint, vault1Pda, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    vault2Ata = (await naclac.createAta(client.program.provider, mint, vault2Pda, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
  });

  it("1. Initialize Vaults & Deposits", async () => {
    // Init Vault 1
    await client.initialize({ vaultId: vaultId1, vaultBump: vault1Bump }).accounts({ payer: payer.address, vaultAccount: vault1Pda }).rpc();
    // Init Vault 2
    await client.initialize({ vaultId: vaultId2, vaultBump: vault2Bump }).accounts({ payer: payer.address, vaultAccount: vault2Pda }).rpc();

    // Deposit User 1 into Vault 1
    await client.deposit({ vaultId: vaultId1, amount: 5000n, userBump: user1Vault1Bump })
      .accounts({
        payer: payer.address,
        mint,
        vaultAccount: vault1Pda,
        vaultTokenAccount: vault1Ata,
        userTokenAccount: user1Ata,
        userAccount: user1Vault1Pda,
        tokenProgram: naclac.TOKEN_PROGRAM_ID,
      }).rpc();

    // Deposit User 2 into Vault 1 (must sign with user2 since it's user2 depositing)
    const clientUser2 = new TokenVaultClient("localnet", user2);
    await clientUser2.deposit({ vaultId: vaultId1, amount: 7000n, userBump: user2Vault1Bump })
      .accounts({
        payer: user2.address,
        mint,
        vaultAccount: vault1Pda,
        vaultTokenAccount: vault1Ata,
        userTokenAccount: user2Ata,
        userAccount: user2Vault1Pda,
        tokenProgram: naclac.TOKEN_PROGRAM_ID,
      }).rpc();

    // Deposit User 1 into Vault 2
    await client.deposit({ vaultId: vaultId2, amount: 9000n, userBump: user1Vault2Bump })
      .accounts({
        payer: payer.address,
        mint,
        vaultAccount: vault2Pda,
        vaultTokenAccount: vault2Ata,
        userTokenAccount: user1Ata,
        userAccount: user1Vault2Pda,
        tokenProgram: naclac.TOKEN_PROGRAM_ID,
      }).rpc();
  });

  it("2. Test Account Fetchers", async () => {
    console.log("\n=================== START KIT FETCHER TESTS ===================");

    // -- fetchVault & fetchUserAccount using built-in stringifyJson
    console.log("\n--- fetchVault (Single) ---");
    const vault1 = await client.fetchVault(vault1Pda);
    console.log("Vault 1:", naclac.stringifyJson(vault1.data, 2));

    console.log("\n--- fetchUserAccount (Single) ---");
    const user1Vault1 = await client.fetchUserAccount(user1Vault1Pda);
    console.log("User 1 in Vault 1:", naclac.stringifyJson(user1Vault1.data, 2));

    // -- fetchMaybeVault & fetchMaybeUserAccount (exist and not exist)
    console.log("\n--- fetchMaybeVault (Exists) ---");
    const maybeVault1 = await client.fetchMaybeVault(vault1Pda);
    console.log("Exists:", maybeVault1.exists, "Data:", maybeVault1.exists ? naclac.stringifyJson(maybeVault1.data, 2) : "N/A");

    console.log("\n--- fetchMaybeVault (Does NOT exist) ---");
    const randomAddress = (await naclac.generateExtractableKeyPairSigner()).address;
    const maybeFakeVault = await client.fetchMaybeVault(randomAddress);
    console.log("Exists:", maybeFakeVault.exists);

    // -- fetchAllVaults & fetchAllUserAccounts (Multiple)
    console.log("\n--- fetchAllVaults (Multiple) ---");
    const allVaults = await client.fetchAllVaults([vault1Pda, vault2Pda]);
    console.log("Fetched vaults count:", allVaults.length);
    allVaults.forEach((v, i) => console.log(`Vault ${i + 1} ID:`, v.data.vaultId.toString()));

    // -- Test generic map helper naclac.mapKeysToValues
    console.log("\n--- mapKeysToValues (Generic Helper) ---");
    const vaultMap = naclac.mapKeysToValues([vault1Pda, vault2Pda], allVaults);
    console.log("Vault 1 mapped ID:", vaultMap.get(vault1Pda)?.data.vaultId.toString());
    console.log("Vault 2 mapped ID:", vaultMap.get(vault2Pda)?.data.vaultId.toString());

    // -- Test OOP fetchMultipleAsMap
    console.log("\n--- fetchMultipleAsMap (OOP Helper) ---");
    const oopMap = await client.program.account.Vault.fetchMultipleAsMap([vault1Pda, vault2Pda]);
    console.log("OOP Mapped Vault 1 authority:", (oopMap.get(vault1Pda) as any).authority.toString());
    console.log("OOP Mapped Vault 2 authority:", (oopMap.get(vault2Pda) as any).authority.toString());

    console.log("\n--- fetchAllMaybeVaults (Multiple mixed) ---");
    const allMaybeVaults = await client.fetchAllMaybeVaults([vault1Pda, randomAddress]);
    console.log("Results count:", allMaybeVaults.length);
    console.log("Index 0 exists:", allMaybeVaults[0].exists);
    console.log("Index 1 exists:", allMaybeVaults[1].exists);

    // -- fetchAllByOwner
    console.log("\n--- fetchAllVaultByOwner (All Vaults owned by program) ---");
    const allVaultsByOwner = await client.fetchAllVaultByOwner();
    console.log("All Vaults by owner count:", allVaultsByOwner.length);
    allVaultsByOwner.forEach((v) => console.log(`Address: ${v.address}, Vault ID: ${v.data.vaultId}`));

    console.log("\n--- fetchAllUserAccountByOwner (All UserAccounts owned by program) ---");
    const allUserAccountsByOwner = await client.fetchAllUserAccountByOwner();
    console.log("All UserAccounts by owner count:", allUserAccountsByOwner.length);
    allUserAccountsByOwner.forEach((u) => console.log(`Address: ${u.address}, Owner: ${u.data.owner}, Balance: ${u.data.balance.toString()}`));

    console.log("=================== END KIT FETCHER TESTS ===================\n");
  });
});
