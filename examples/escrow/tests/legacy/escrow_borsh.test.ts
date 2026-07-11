import * as naclac from "@naclac-fw/client/legacy";
import { EscrowBorshClient } from "../../clients/typescript/src/generated/escrow_borsh/client-legacy";

describe("Naclac EscrowBorsh Test Suite (Legacy)", () => {
  let client: EscrowBorshClient;
  let payer: naclac.Keypair;
  let makerAddress: string;

  let mintA: string;
  let mintB: string;
  let makerTokenAccountA: string;
  let makerTokenAccountB: string;

  const initialMakerA = 1_000_000_000n;
  const initialTakerB = 2_000_000_000n;

  before(async () => {
    payer = await naclac.loadNodeWallet();
    makerAddress = payer.publicKey.toBase58();

    // Use "localnet" to construct the client
    client = new EscrowBorshClient("localnet", payer);

    console.log("🚀 Starting EscrowBorsh TypeScript Integration Test Setup (Legacy)...");

    // Create mints
    const mintASigner = naclac.Keypair.generate();
    const mintBSigner = naclac.Keypair.generate();

    mintA = (await naclac.createMint(
      client.program.provider,
      mintASigner,
      undefined,
      payer.publicKey,
      9
    )).toBase58();

    mintB = (await naclac.createMint(
      client.program.provider,
      mintBSigner,
      undefined,
      payer.publicKey,
      9
    )).toBase58();

    console.log(`📦 Created Mint A: ${mintA}`);
    console.log(`📦 Created Mint B: ${mintB}`);

    // Create maker ATAs and mint tokens
    makerTokenAccountA = (await naclac.createAta(client.program.provider, new naclac.PublicKey(mintA), payer.publicKey)).toBase58();
    makerTokenAccountB = (await naclac.createAta(client.program.provider, new naclac.PublicKey(mintB), payer.publicKey)).toBase58();

    await naclac.mintTo(client.program.provider, new naclac.PublicKey(mintA), payer.publicKey, initialMakerA);
    await naclac.mintTo(client.program.provider, new naclac.PublicKey(mintB), payer.publicKey, initialTakerB);

    console.log(`✅ Minted ${initialMakerA} tokens to Maker Token A`);
    console.log(`✅ Minted ${initialTakerB} tokens to Taker Token B`);
  });

  // ===========================================================================
  // FLOW 1: Make & Cancel
  // ===========================================================================
  it("1. Run Make & Cancel Flow", async () => {
    const seedCancel = 789012n;
    const [escrowPdaCancelPubkey, escrowBumpCancel] = client.getEscrowStatePda({ maker: makerAddress, seed: seedCancel });
    const escrowPdaCancel = escrowPdaCancelPubkey.toBase58();

    // Initialize the vault ATA (idempotent)
    const vaultTokenAccountCancel = (await naclac.createAta(
      client.program.provider,
      new naclac.PublicKey(mintA),
      escrowPdaCancelPubkey
    )).toBase58();

    console.log(`      Derived Escrow PDA (Cancel): ${escrowPdaCancel} (Bump: ${escrowBumpCancel})`);
    console.log(`      Derived Vault Token Account (Cancel): ${vaultTokenAccountCancel}`);

    let alreadyInitializedCancel = false;
    try {
      const acc = await client.program.provider.connection.getAccountInfo(escrowPdaCancelPubkey);
      if (acc !== null && acc.data.length > 0) {
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
          }
        )
        .rpc()
        .catch((e: any) => { console.error(e); throw e; });

      const createdEvent = await createdEventPromise;
      if (!createdEvent) throw new Error("❌ EscrowCreated event not captured!");
      console.log("      🔔 EscrowCreated Event Fired!");
    }

    // Verify on-chain escrow state and vault balance
    const escrowState = (await client.fetchEscrowState(escrowPdaCancel)) as any;
    if (BigInt(escrowState.amountA) !== amountACancel) {
      throw new Error(`Expected amountA to be ${amountACancel}, got ${escrowState.amountA}`);
    }

    const res = await client.program.provider.connection.getTokenAccountBalance(new naclac.PublicKey(vaultTokenAccountCancel));
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
        }
      )
      .rpc()
      .catch((e: any) => { console.error(e); throw e; });

    const cancelEvent = await cancelEventPromise;
    if (!cancelEvent) throw new Error("❌ EscrowCancelled event not captured!");
    console.log("      🔔 EscrowCancelled Event Fired!");

    // Verify maker A balance was returned in full
    const makerResA = await client.program.provider.connection.getTokenAccountBalance(new naclac.PublicKey(makerTokenAccountA));
    if (BigInt(makerResA.value.amount) !== initialMakerA) {
      throw new Error(`Expected maker A balance after cancel to be ${initialMakerA}, got ${makerResA.value.amount}`);
    }

    // Verify accounts are closed
    const escrowAccountInfo = await client.program.provider.connection.getAccountInfo(escrowPdaCancelPubkey);
    if (escrowAccountInfo !== null) throw new Error("Expected escrow state PDA to be closed");

    const vaultAccountInfo = await client.program.provider.connection.getAccountInfo(new naclac.PublicKey(vaultTokenAccountCancel));
    if (vaultAccountInfo !== null) throw new Error("Expected vault token account to be closed");

    console.log("      ✅ Cancel Flow completed and verified successfully.");
  });

  // ===========================================================================
  // FLOW 2: Make & Take
  // ===========================================================================
  it("2. Run Make & Take Flow", async () => {
    const seedTake = 123456n;
    const [escrowPdaTakePubkey, escrowBumpTake] = client.getEscrowStatePda({ maker: makerAddress, seed: seedTake });
    const escrowPdaTake = escrowPdaTakePubkey.toBase58();

    console.log(`      Derived Escrow PDA (Take): ${escrowPdaTake} (Bump: ${escrowBumpTake})`);

    // When the escrow already exists from a prior run, its stored mints may differ
    // from the freshly-created mints in this run's `before()`. Read them from chain.
    let alreadyInitializedTake = false;
    let effectiveMintA = mintA;
    let effectiveMintB = mintB;
    let effectiveMakerTokenAccountA = makerTokenAccountA;
    let effectiveMakerTokenAccountB = makerTokenAccountB;

    try {
      const acc = await client.program.provider.connection.getAccountInfo(escrowPdaTakePubkey);
      if (acc !== null && acc.data.length > 0) {
        alreadyInitializedTake = true;
        console.log("      ℹ️ Escrow account for take already exists. Reading stored mints from on-chain state.");

        const existingState = (await client.fetchEscrowState(escrowPdaTake)) as any;
        effectiveMintA = existingState.mintA.toString();
        effectiveMintB = existingState.mintB.toString();

        // Re-derive the correct ATAs for the stored mints
        effectiveMakerTokenAccountA = (await naclac.createAta(client.program.provider, new naclac.PublicKey(effectiveMintA), payer.publicKey)).toBase58();
        effectiveMakerTokenAccountB = (await naclac.createAta(client.program.provider, new naclac.PublicKey(effectiveMintB), payer.publicKey)).toBase58();
      }
    } catch (e) {}

    // Initialize the vault ATA for the effective mintA (idempotent)
    const vaultTokenAccountTake = (await naclac.createAta(
      client.program.provider,
      new naclac.PublicKey(effectiveMintA),
      escrowPdaTakePubkey
    )).toBase58();

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
          }
        )
        .rpc()
        .catch((e: any) => { console.error(e); throw e; });

      const createdEvent = await createdEventPromise;
      if (!createdEvent) throw new Error("❌ EscrowCreated event not captured!");
      console.log("      🔔 EscrowCreated Event Fired!");
    }

    // Verify on-chain escrow state — compare against what is actually stored
    const escrowStateTake = (await client.fetchEscrowState(escrowPdaTake)) as any;
    if (escrowStateTake.maker.toString() !== makerAddress) throw new Error("Maker mismatch");
    if (escrowStateTake.mintA.toString() !== effectiveMintA) {
      throw new Error(`Mint A mismatch (expected ${effectiveMintA}, got ${escrowStateTake.mintA.toString()})`);
    }
    if (escrowStateTake.mintB.toString() !== effectiveMintB) {
      throw new Error(`Mint B mismatch (expected ${effectiveMintB}, got ${escrowStateTake.mintB.toString()})`);
    }

    // Use the stored amounts from on-chain state for vault balance verification
    const storedAmountA = BigInt(escrowStateTake.amountA);

    const vaultTakeRes = await client.program.provider.connection.getTokenAccountBalance(new naclac.PublicKey(vaultTokenAccountTake));
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
        }
      )
      .rpc()
      .catch((e: any) => { console.error(e); throw e; });

    const exchangedEvent = await exchangedEventPromise;
    if (!exchangedEvent) throw new Error("❌ EscrowExchanged event not captured!");
    console.log("      🔔 EscrowExchanged Event Fired!");

    // Verify accounts are closed
    const escrowAccountInfoTake = await client.program.provider.connection.getAccountInfo(escrowPdaTakePubkey);
    if (escrowAccountInfoTake !== null) throw new Error("Expected escrow state PDA to be closed");

    const vaultAccountInfoTake = await client.program.provider.connection.getAccountInfo(new naclac.PublicKey(vaultTokenAccountTake));
    if (vaultAccountInfoTake !== null) throw new Error("Expected vault token account to be closed");

    console.log("      ✅ Take Flow completed and verified successfully.");
  });
});
