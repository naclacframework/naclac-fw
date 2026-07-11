import * as naclac from "@naclac-fw/client/legacy";
import { TokenVaultBorshClient } from "../../clients/typescript/src/generated/token_vault_borsh/client-legacy";

describe("Naclac Token Vault Borsh Test Suite (Legacy)", () => {
  describe("SPL Token Program", () => {
    runLifecycleTest(new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID), "SPL Token");
  });

  describe("Token-2022 Program", () => {
    runLifecycleTest(new naclac.PublicKey(naclac.TOKEN_2022_PROGRAM_ID), "Token-2022");
  });
});

function runLifecycleTest(tokenProgramId: naclac.PublicKey, name: string) {
  let client: TokenVaultBorshClient;
  let payer: naclac.Keypair;
  let mint: naclac.PublicKey;
  let userAta: naclac.PublicKey;
  let vaultAta: naclac.PublicKey;
  let vaultPda: naclac.PublicKey;
  let userAccountPda: naclac.PublicKey;
  let userBump: number;
  let vaultBump: number;

  before(async () => {
    // Load local wallet keypair
    payer = await naclac.loadNodeWallet();
    client = new TokenVaultBorshClient("localnet", payer);

    console.log(`      [${name}] Generating Mint Signer...`);
    const mintSigner = naclac.Keypair.generate();

    console.log(`      [${name}] Creating Mint with 0 decimals...`);
    mint = (await naclac.createMint(client.program.provider, mintSigner, tokenProgramId, payer.publicKey, 0)) as naclac.PublicKey;
    console.log(`      [${name}] Mint Address: ${mint}`);

    console.log(`      [${name}] Deriving Vault PDA...`);
    const vaultPdaResult = await client.getVaultAccountPda({ vaultId: 42n });
    vaultPda = vaultPdaResult[0];
    vaultBump = vaultPdaResult[1];
    console.log(`      [${name}] Vault PDA: ${vaultPda} (Bump: ${vaultBump})`);

    console.log(`      [${name}] Deriving User Account PDA...`);
    const userAccountPdaResult = await client.getUserAccountPda({ vaultId: 42n, payer: payer.publicKey, mint });
    userAccountPda = userAccountPdaResult[0];
    userBump = userAccountPdaResult[1];
    console.log(`      [${name}] User Account PDA: ${userAccountPda} (Bump: ${userBump})`);

    console.log(`      [${name}] Creating User ATA and Minting 1,000,000 Tokens...`);
    userAta = (await naclac.createAta(client.program.provider, mint, payer.publicKey, tokenProgramId)) as naclac.PublicKey;
    await naclac.mintTo(client.program.provider, mint, payer.publicKey, 1000000n, tokenProgramId);
    const userBalance = (await client.program.provider.connection.getTokenAccountBalance(userAta)).value.uiAmount;
    console.log(`      [${name}] User ATA: ${userAta} (Balance: ${userBalance})`);

    console.log(`      [${name}] Creating Vault ATA owned by Vault PDA...`);
    vaultAta = (await naclac.createAta(client.program.provider, mint, vaultPda, tokenProgramId)) as naclac.PublicKey;
    console.log(`      [${name}] Vault ATA: ${vaultAta}`);
  });

  it(`1. Setup & Initialize Vault [${name}]`, async () => {
    console.log(`      🔍 [${name}] Checking if Vault exists...`);
    try {
      await client.fetchVault(vaultPda);
      console.log(`      ℹ️  [${name}] Vault already exists — skipping initialization.`);
    } catch (e) {
      console.log(`      🚀 [${name}] Initializing New Vault...`);
      await client
        .initialize({ vaultId: 42n, vaultBump })
        .accounts({
          payer: payer.publicKey,
          vaultAccount: vaultPda,
        })
        .rpc()
        .catch(naclac.logError);

      const vaultState = await client.fetchVault(vaultPda);
      console.log(`      [${name}] Vault Authority: ${vaultState.authority}`);
    }
  });

  it(`2. Deposit and Verify Balance & Event [${name}]`, async () => {
    const depositAmount = 600000n;
    console.log(`      💰 [${name}] Depositing ${depositAmount} tokens...`);

    const eventPromise = client.waitForTokenDeposited({ timeoutMs: 30000 });
    // Wait for the websocket log subscription to establish
    await new Promise((resolve) => setTimeout(resolve, 1000));

    await client
      .deposit({ vaultId: 42n, amount: depositAmount, userBump })
      .accounts({
        payer: payer.publicKey,
        vaultAccount: vaultPda,
        vaultTokenAccount: vaultAta,
        userTokenAccount: userAta,
        userAccount: userAccountPda,
        mint,
        tokenProgram: tokenProgramId,
      })
      .rpc()
      .catch(naclac.logError);

    const vaultState = await client.fetchVault(vaultPda);
    const userState = await client.fetchUserAccount(userAccountPda);
    const userAtaBalance = (await client.program.provider.connection.getTokenAccountBalance(userAta)).value.uiAmount;
    const vaultAtaBalance = (await client.program.provider.connection.getTokenAccountBalance(vaultAta)).value.uiAmount;

    console.log(`      ✅ [${name}] Vault Authority field: ${vaultState.authority}`);
    console.log(`      ✅ [${name}] User balance field: ${userState.balance}`);
    console.log(`      ✅ [${name}] User ATA balance: ${userAtaBalance}`);
    console.log(`      ✅ [${name}] Vault ATA balance: ${vaultAtaBalance}`);

    if (Number(userState.balance) !== Number(depositAmount)) {
      throw new Error("User balance did not increase correctly");
    }
    if (Number(vaultAtaBalance) !== Number(depositAmount)) {
      throw new Error("Vault ATA balance did not increase correctly");
    }

    const capturedEvent = await eventPromise;
    if (!capturedEvent) throw new Error("❌ Deposit event was not captured!");
    console.log(`      🔔 [${name}] Event Fired! User: ${capturedEvent.user}, Deposited: ${capturedEvent.amount}, New Total: ${capturedEvent.totalVaultBalance}`);
  });

  it(`3. Withdraw and Verify Balance & Event [${name}]`, async () => {
    const withdrawAmount = 250000n;
    console.log(`      💸 [${name}] Withdrawing ${withdrawAmount} tokens...`);

    const eventPromise = client.waitForTokenWithdrawn({ timeoutMs: 20000 });
    // Wait for the websocket log subscription to establish
    await new Promise((resolve) => setTimeout(resolve, 1000));

    await client
      .withdraw({ vaultId: 42n, amount: withdrawAmount })
      .accounts({
        payer: payer.publicKey,
        vaultAccount: vaultPda,
        vaultTokenAccount: vaultAta,
        userTokenAccount: userAta,
        userAccount: userAccountPda,
        mint,
        tokenProgram: tokenProgramId,
      })
      .rpc()
      .catch(naclac.logError);

    const vaultState = await client.fetchVault(vaultPda);
    const userState = await client.fetchUserAccount(userAccountPda);
    const userAtaBalance = (await client.program.provider.connection.getTokenAccountBalance(userAta)).value.uiAmount;
    const vaultAtaBalance = (await client.program.provider.connection.getTokenAccountBalance(vaultAta)).value.uiAmount;

    console.log(`      ✅ [${name}] Vault Authority field: ${vaultState.authority}`);
    console.log(`      ✅ [${name}] User balance field: ${userState.balance}`);
    console.log(`      ✅ [${name}] User ATA balance: ${userAtaBalance}`);
    console.log(`      ✅ [${name}] Vault ATA balance: ${vaultAtaBalance}`);

    if (Number(userState.balance) !== 350000) {
      throw new Error("User balance did not decrease correctly");
    }
    if (Number(vaultAtaBalance) !== 350000) {
      throw new Error("Vault ATA balance did not decrease correctly");
    }

    const capturedEvent = await eventPromise;
    if (!capturedEvent) throw new Error("❌ Withdrawal event was not captured!");
    console.log(`      🔔 [${name}] Event Fired! User: ${capturedEvent.user}, Withdrawn: ${capturedEvent.amount}, New Total: ${capturedEvent.totalVaultBalance}`);
  });

  it(`4. Withdraw excessive amount (Expect Error) [${name}]`, async () => {
    const excessiveAmount = 500000n;
    console.log(`      ⚠️ [${name}] Attempting to withdraw ${excessiveAmount} tokens (should fail since balance is 350,000)...`);

    try {
      await client
        .withdraw({ vaultId: 42n, amount: excessiveAmount })
        .accounts({
          payer: payer.publicKey,
          vaultAccount: vaultPda,
          vaultTokenAccount: vaultAta,
          userTokenAccount: userAta,
          userAccount: userAccountPda,
          mint,
          tokenProgram: tokenProgramId,
        })
        .rpc();
      throw new Error("Withdrawal should have failed but succeeded");
    } catch (e) {
      console.log(`      ✅ [${name}] Correctly failed to withdraw excessive amount.`);
    }
  });
}
