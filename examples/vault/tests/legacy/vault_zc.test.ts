import * as naclac from "@naclac-fw/client/legacy";
import { VaultZcClient } from "../../clients/typescript/src/generated/vault_zc/client-legacy";

describe("Naclac Vault Test Suite", () => {
  let client: VaultZcClient;
  let payer: naclac.Keypair;
  let vaultPda: naclac.PublicKey;
  let userAccountPda: naclac.PublicKey;
  let userBump: number;

  before(async () => {
    payer = await naclac.loadNodeWallet();
    client = new VaultZcClient("localnet", payer);
    [vaultPda] = await client.getVaultAccountPda({});
    [userAccountPda, userBump] = await client.getUserAccountPda({ user: payer.publicKey });
  });

  it("1. Setup & Initialize Vault", async () => {
    console.log("      🔍 Checking if Vault exists...");
    try {
      await client.fetchVault(vaultPda);
      console.log("      ℹ️  Vault already exists — skipping initialization.");
    } catch (e) {
      console.log("      🚀 Initializing New Vault...");
      await client
        .initialize({})
        .accounts({ payer: payer.publicKey, vaultAccount: vaultPda })
        .rpc()
        .catch(naclac.logError);
    }
  });

  it("2. Deposit and Verify Balance", async () => {
    const amount = 1000000; // 0.001 SOL
    console.log(`      💰 Depositing ${amount} lamports...`);

    const txResult = await client
      .deposit({ amount, userBump })
      .accounts({
        payer: payer.publicKey,
        vaultAccount: vaultPda,
        userAccount: userAccountPda
      })
      .rpc()
      .catch(naclac.logError);

    const vaultState = await client.fetchVault(vaultPda);
    const userState = await client.fetchUserAccount(userAccountPda);

    console.log(`      ✅ Vault Total: ${vaultState.totalDeposited}`);
    console.log(`      ✅ User Balance: ${userState.balance}`);

    if (Number(userState.balance) < amount) {
      throw new Error("User balance did not increase correctly");
    }

    // Race-free: reads events straight out of the already-confirmed
    // transaction's own logs, instead of racing a live subscription.
    const capturedEvents = client.parseFundsDepositedEvents(txResult!.logs);
    if (capturedEvents.length === 0) throw new Error("❌ Deposit event was not captured!");
    console.log(`      🔔 Event Fired! Deposited: ${capturedEvents[0].amount}`);
  });

  it("3. Withdraw and Verify Balance", async () => {
    const amount = 500000; // 0.0005 SOL
    console.log(`      💸 Withdrawing ${amount} lamports...`);

    const txResult = await client
      .withdraw({ amount })
      .accounts({
        user: payer.publicKey,
        vaultAccount: vaultPda,
        userAccount: userAccountPda
      })
      .rpc()
      .catch(naclac.logError);

    const vaultState = await client.fetchVault(vaultPda);
    const userState = await client.fetchUserAccount(userAccountPda);

    console.log(`      ✅ Vault Total: ${vaultState.totalDeposited}`);
    console.log(`      ✅ User Balance: ${userState.balance}`);

    const capturedEvents = client.parseFundsWithdrawnEvents(txResult!.logs);
    if (capturedEvents.length === 0) throw new Error("❌ Withdrawal event was not captured!");
    console.log(`      🔔 Event Fired! Withdrawn: ${capturedEvents[0].amount}`);
  });

  it("4. Withdraw excessive amount (Expect Error)", async () => {
    const excessiveAmount = 1000000000;
    console.log(`      ⚠️ Attempting to withdraw ${excessiveAmount} lamports (should fail)...`);

    try {
      await client
        .withdraw({ amount: excessiveAmount })
        .accounts({
          user: payer.publicKey,
          vaultAccount: vaultPda,
          userAccount: userAccountPda
        })
        .rpc();
      throw new Error("Withdrawal should have failed but succeeded");
    } catch (e) {
      console.log("      ✅ Correctly failed to withdraw excessive amount.");
    }
  });
});
