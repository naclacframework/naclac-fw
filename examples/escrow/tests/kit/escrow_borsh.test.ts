import * as naclac from "@naclac-fw/client/kit";
import { EscrowBorshClient } from "../../clients/typescript/src/generated/escrow_borsh";

describe("Naclac EscrowBorsh Test Suite", () => {
  let client: EscrowBorshClient;
  let payer: naclac.KeyPairSigner;
  let makerAddress: string;

  let mintA: string;
  let mintB: string;
  let makerTokenAccountA: string;
  let makerTokenAccountB: string;

  const initialMakerA = 1_000_000_000n;
  const initialTakerB = 2_000_000_000n;

  before(async () => {
    payer = await naclac.loadNodeWallet();
    makerAddress = payer.address;

    // Use "localnet" to construct the client
    client = new EscrowBorshClient("localnet", payer);

    console.log("🚀 Starting EscrowBorsh TypeScript Integration Test Setup...");

    // Create mints
    const mintASigner = await naclac.generateKeyPairSigner();
    const mintBSigner = await naclac.generateKeyPairSigner();

    mintA = await naclac.createMint(
      client.program.provider,
      mintASigner,
      naclac.TOKEN_PROGRAM_ID,
      makerAddress,
      9
    );

    mintB = await naclac.createMint(
      client.program.provider,
      mintBSigner,
      naclac.TOKEN_PROGRAM_ID,
      makerAddress,
      9
    );

    console.log(`📦 Created Mint A: ${mintA}`);
    console.log(`📦 Created Mint B: ${mintB}`);

    // Derive maker ATAs and mint tokens
    const [ataA] = await naclac.getAssociatedTokenAddress(mintA, makerAddress, naclac.TOKEN_PROGRAM_ID);
    const [ataB] = await naclac.getAssociatedTokenAddress(mintB, makerAddress, naclac.TOKEN_PROGRAM_ID);
    makerTokenAccountA = ataA;
    makerTokenAccountB = ataB;

    await naclac.mintTo(client.program.provider, mintA, makerAddress, initialMakerA, naclac.TOKEN_PROGRAM_ID);
    await naclac.mintTo(client.program.provider, mintB, makerAddress, initialTakerB, naclac.TOKEN_PROGRAM_ID);

    console.log(`✅ Minted ${initialMakerA} tokens to Maker Token A`);
    console.log(`✅ Minted ${initialTakerB} tokens to Taker Token B`);
  });

  // ===========================================================================
  // FLOW 1: Make & Cancel
  // ===========================================================================
  it("1. Run Make & Cancel Flow", async () => {
    const seedCancel = 789012n;
    const [escrowPdaCancel, escrowBumpCancel] = await client.getEscrowStatePda({ maker: makerAddress, seed: seedCancel });

    // Initialize the vault ATA (idempotent)
    const vaultTokenAccountCancel = await naclac.createAta(
      client.program.provider,
      mintA,
      escrowPdaCancel,
      naclac.TOKEN_PROGRAM_ID
    );

    console.log(`      Derived Escrow PDA (Cancel): ${escrowPdaCancel} (Bump: ${escrowBumpCancel})`);
    console.log(`      Derived Vault Token Account (Cancel): ${vaultTokenAccountCancel}`);

    let alreadyInitializedCancel = false;
    try {
      const acc = await client.program.provider.rpc.getAccountInfo(escrowPdaCancel).send();
      if (acc && acc.value) {
        alreadyInitializedCancel = true;
        console.log("      ℹ️ Escrow account for cancel already exists. Skipping Make.");
      }
    } catch (e) {}

    const amountACancel = 400_000_000n;
    const amountBCancel = 800_000_000n;

    if (!alreadyInitializedCancel) {
      console.log(`      🤝 Sending MAKE transaction for Cancel Flow (A = ${amountACancel}, B = ${amountBCancel})...`);

      const createdEventPromise = client.waitForEscrowCreated({ timeoutMs: 10000 });

      await client
        .make(
          { seed: seedCancel, escrowBump: escrowBumpCancel, amountA: amountACancel, amountB: amountBCancel },
          {
            maker: makerAddress,
            mintA,
            mintB,
            escrowState: escrowPdaCancel,
            vaultTokenAccount: vaultTokenAccountCancel,
            makerTokenAccountA,
            tokenProgram: naclac.TOKEN_PROGRAM_ID,
            systemProgram: naclac.SYSTEM_PROGRAM_ID,
          }
        )
        .rpc()
        .catch(naclac.logError);

      const createdEvent = await createdEventPromise;
      if (!createdEvent) throw new Error("❌ EscrowCreated event not captured!");
      console.log("      🔔 EscrowCreated Event Fired!");
    }

    // Verify on-chain escrow state and vault balance
    const escrowState = await client.fetchEscrowState(escrowPdaCancel);
    if (BigInt(escrowState.data.amountA) !== amountACancel) {
      throw new Error(`Expected amountA to be ${amountACancel}, got ${escrowState.data.amountA}`);
    }

    const res = await client.program.provider.rpc.getTokenAccountBalance(naclac.address(vaultTokenAccountCancel)).send();
    if (BigInt(res.value.amount) !== amountACancel) {
      throw new Error(`Expected vault balance to be ${amountACancel}, got ${res.value.amount}`);
    }

    console.log("      🛑 Sending CANCEL transaction...");
    const cancelEventPromise = client.waitForEscrowCancelled({ timeoutMs: 10000 });

    await client
      .cancel(
        { seed: seedCancel },
        {
          maker: makerAddress,
          mintA,
          escrowState: escrowPdaCancel,
          vaultTokenAccount: vaultTokenAccountCancel,
          makerTokenAccountA,
          tokenProgram: naclac.TOKEN_PROGRAM_ID,
          systemProgram: naclac.SYSTEM_PROGRAM_ID,
        }
      )
      .rpc()
      .catch(naclac.logError);

    const cancelEvent = await cancelEventPromise;
    if (!cancelEvent) throw new Error("❌ EscrowCancelled event not captured!");
    console.log("      🔔 EscrowCancelled Event Fired!");

    // Verify maker A balance was returned in full
    const makerResA = await client.program.provider.rpc.getTokenAccountBalance(naclac.address(makerTokenAccountA)).send();
    if (BigInt(makerResA.value.amount) !== initialMakerA) {
      throw new Error(`Expected maker A balance after cancel to be ${initialMakerA}, got ${makerResA.value.amount}`);
    }

    // Verify accounts are closed
    const escrowAccountInfo = await client.program.provider.rpc.getAccountInfo(naclac.address(escrowPdaCancel)).send();
    if (escrowAccountInfo.value !== null) throw new Error("Expected escrow state PDA to be closed");

    const vaultAccountInfo = await client.program.provider.rpc.getAccountInfo(naclac.address(vaultTokenAccountCancel)).send();
    if (vaultAccountInfo.value !== null) throw new Error("Expected vault token account to be closed");

    console.log("      ✅ Cancel Flow completed and verified successfully.");
  });

  // ===========================================================================
  // FLOW 2: Make & Take
  // ===========================================================================
  it("2. Run Make & Take Flow", async () => {
    const seedTake = 123456n;
    const [escrowPdaTake, escrowBumpTake] = await client.getEscrowStatePda({ maker: makerAddress, seed: seedTake });

    console.log(`      Derived Escrow PDA (Take): ${escrowPdaTake} (Bump: ${escrowBumpTake})`);

    // When the escrow already exists from a prior run, its stored mints may differ
    // from the freshly-created mints in this run's `before()`. Read them from chain.
    let alreadyInitializedTake = false;
    let effectiveMintA = mintA;
    let effectiveMintB = mintB;
    let effectiveMakerTokenAccountA = makerTokenAccountA;
    let effectiveMakerTokenAccountB = makerTokenAccountB;

    try {
      const acc = await client.program.provider.rpc.getAccountInfo(escrowPdaTake).send();
      if (acc && acc.value) {
        alreadyInitializedTake = true;
        console.log("      ℹ️ Escrow account for take already exists. Reading stored mints from on-chain state.");

        const existingState = await client.fetchEscrowState(escrowPdaTake);
        effectiveMintA = existingState.data.mintA;
        effectiveMintB = existingState.data.mintB;

        // Re-derive the correct ATAs for the stored mints
        const [ataA] = await naclac.getAssociatedTokenAddress(effectiveMintA, makerAddress, naclac.TOKEN_PROGRAM_ID);
        const [ataB] = await naclac.getAssociatedTokenAddress(effectiveMintB, makerAddress, naclac.TOKEN_PROGRAM_ID);
        effectiveMakerTokenAccountA = ataA;
        effectiveMakerTokenAccountB = ataB;
      }
    } catch (e) {}

    // Initialize the vault ATA for the effective mintA (idempotent)
    const vaultTokenAccountTake = await naclac.createAta(
      client.program.provider,
      effectiveMintA,
      escrowPdaTake,
      naclac.TOKEN_PROGRAM_ID
    );

    console.log(`      Derived Vault Token Account (Take): ${vaultTokenAccountTake}`);

    const amountATake = 500_000_000n;
    const amountBTake = 1_000_000_000n;

    if (!alreadyInitializedTake) {
      console.log(`      🤝 Sending MAKE transaction for Take Flow (A = ${amountATake}, B = ${amountBTake})...`);

      const createdEventPromise = client.waitForEscrowCreated({ timeoutMs: 10000 });

      await client
        .make(
          { seed: seedTake, escrowBump: escrowBumpTake, amountA: amountATake, amountB: amountBTake },
          {
            maker: makerAddress,
            mintA: effectiveMintA,
            mintB: effectiveMintB,
            escrowState: escrowPdaTake,
            vaultTokenAccount: vaultTokenAccountTake,
            makerTokenAccountA: effectiveMakerTokenAccountA,
            tokenProgram: naclac.TOKEN_PROGRAM_ID,
            systemProgram: naclac.SYSTEM_PROGRAM_ID,
          }
        )
        .rpc()
        .catch(naclac.logError);

      const createdEvent = await createdEventPromise;
      if (!createdEvent) throw new Error("❌ EscrowCreated event not captured!");
      console.log("      🔔 EscrowCreated Event Fired!");
    }

    // Verify on-chain escrow state — compare against what is actually stored
    const escrowStateTake = await client.fetchEscrowState(escrowPdaTake);
    if (escrowStateTake.data.maker !== makerAddress) throw new Error("Maker mismatch");
    if (escrowStateTake.data.mintA !== effectiveMintA) {
      throw new Error(`Mint A mismatch (expected ${effectiveMintA}, got ${escrowStateTake.data.mintA})`);
    }
    if (escrowStateTake.data.mintB !== effectiveMintB) {
      throw new Error(`Mint B mismatch (expected ${effectiveMintB}, got ${escrowStateTake.data.mintB})`);
    }

    // Use the stored amounts from on-chain state for vault balance verification
    const storedAmountA = BigInt(escrowStateTake.data.amountA);

    const vaultTakeRes = await client.program.provider.rpc.getTokenAccountBalance(naclac.address(vaultTokenAccountTake)).send();
    if (BigInt(vaultTakeRes.value.amount) !== storedAmountA) {
      throw new Error(`Expected vault balance to be ${storedAmountA}, got ${vaultTakeRes.value.amount}`);
    }

    console.log("      🎬 Sending TAKE transaction...");
    const exchangedEventPromise = client.waitForEscrowExchanged({ timeoutMs: 10000 });

    await client
      .take(
        { seed: seedTake },
        {
          taker: makerAddress,
          maker: makerAddress,
          mintA: effectiveMintA,
          mintB: effectiveMintB,
          escrowState: escrowPdaTake,
          vaultTokenAccount: vaultTokenAccountTake,
          takerTokenAccountA: effectiveMakerTokenAccountA,
          takerTokenAccountB: effectiveMakerTokenAccountB,
          makerTokenAccountB: effectiveMakerTokenAccountB,
          tokenProgram: naclac.TOKEN_PROGRAM_ID,
          systemProgram: naclac.SYSTEM_PROGRAM_ID,
        }
      )
      .rpc()
      .catch(naclac.logError);

    const exchangedEvent = await exchangedEventPromise;
    if (!exchangedEvent) throw new Error("❌ EscrowExchanged event not captured!");
    console.log("      🔔 EscrowExchanged Event Fired!");

    // Verify accounts are closed
    const escrowAccountInfoTake = await client.program.provider.rpc.getAccountInfo(naclac.address(escrowPdaTake)).send();
    if (escrowAccountInfoTake.value !== null) throw new Error("Expected escrow state PDA to be closed");

    const vaultAccountInfoTake = await client.program.provider.rpc.getAccountInfo(naclac.address(vaultTokenAccountTake)).send();
    if (vaultAccountInfoTake.value !== null) throw new Error("Expected vault token account to be closed");

    console.log("      ✅ Take Flow completed and verified successfully.");
  });
});
