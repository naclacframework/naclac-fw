import * as naclac from "@naclac-fw/client/legacy";
import { AmmClient } from "../../clients/typescript/src/generated/amm/client-legacy";
import assert from "assert";

describe("Naclac AMM Test Suite (Legacy)", () => {
  let client: AmmClient;
  let payer: naclac.Keypair;
  let tokenAMint: naclac.PublicKey;
  let tokenBMint: naclac.PublicKey;
  let userTokenA: naclac.PublicKey;
  let userTokenB: naclac.PublicKey;
  let userLp: naclac.PublicKey;
  let poolState: naclac.PublicKey;
  let poolBump: number;
  let vaultA: naclac.PublicKey;
  let vaultB: naclac.PublicKey;
  let lpMint: naclac.PublicKey;

  const id = 100n;
  const amountA = 2_000_000_000n;
  const amountB = 4_000_000_000n;
  const addAmountA = 500_000_000n;
  const addAmountB = 1_000_000_000n;
  const swapIn = 100_000_000n;
  const minOut = 150_000_000n;
  const removeLpAmount = 500_000_000n;

  const tokenProgramId = new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID);
  const systemProgramId = new naclac.PublicKey(naclac.SYSTEM_PROGRAM_ID);

  before(async () => {
    payer = await naclac.loadNodeWallet();
    client = new AmmClient("localnet", payer);

    console.log("      Generating Mint Keypairs...");
    const mintAKeypair = naclac.Keypair.generate();
    const mintBKeypair = naclac.Keypair.generate();

    tokenAMint = (await naclac.createMint(client.program.provider, mintAKeypair, tokenProgramId, payer.publicKey, 6)) as naclac.PublicKey;
    tokenBMint = (await naclac.createMint(client.program.provider, mintBKeypair, tokenProgramId, payer.publicKey, 6)) as naclac.PublicKey;
    console.log(`      Token A Mint: ${tokenAMint.toBase58()}`);
    console.log(`      Token B Mint: ${tokenBMint.toBase58()}`);

    userTokenA = (await naclac.createAta(client.program.provider, tokenAMint, payer.publicKey, tokenProgramId)) as naclac.PublicKey;
    userTokenB = (await naclac.createAta(client.program.provider, tokenBMint, payer.publicKey, tokenProgramId)) as naclac.PublicKey;

    await naclac.mintTo(client.program.provider, tokenAMint, payer.publicKey, 10_000_000_000n, tokenProgramId);
    await naclac.mintTo(client.program.provider, tokenBMint, payer.publicKey, 10_000_000_000n, tokenProgramId);
    console.log("      Minted initial Token A and B supply to user.");

    console.log("      Deriving Pool State PDA...");
    const [derivedPoolState, bump] = client.getPoolStatePda({
      tokenAMint,
      tokenBMint,
      id,
    });
    poolState = derivedPoolState;
    poolBump = bump;
    console.log(`      Pool State PDA: ${poolState.toBase58()} (Bump: ${poolBump})`);

    const lpMintKeypair = naclac.Keypair.generate();

    vaultA = (await naclac.createAta(client.program.provider, tokenAMint, poolState, tokenProgramId)) as naclac.PublicKey;
    vaultB = (await naclac.createAta(client.program.provider, tokenBMint, poolState, tokenProgramId)) as naclac.PublicKey;
    lpMint = (await naclac.createMint(client.program.provider, lpMintKeypair, tokenProgramId, poolState, 6)) as naclac.PublicKey;
    console.log(`      Vault A: ${vaultA.toBase58()}`);
    console.log(`      Vault B: ${vaultB.toBase58()}`);
    console.log(`      LP Mint: ${lpMint.toBase58()}`);

    userLp = (await naclac.createAta(client.program.provider, lpMint, payer.publicKey, tokenProgramId)) as naclac.PublicKey;
    console.log(`      User LP ATA: ${userLp.toBase58()}`);
  });

  it("1. Initialize Pool", async () => {
    console.log("      🔥 Sending initialize pool transaction...");
    await client
      .initialize({
        id,
        poolBump,
        amountA,
        amountB,
      })
      .accounts({
        payer: payer.publicKey,
        tokenAMint,
        tokenBMint,
        poolState,
        vaultA,
        vaultB,
        lpMint,
        depositorTokenA: userTokenA,
        depositorTokenB: userTokenB,
        depositorLp: userLp,
        depositorAuthority: payer.publicKey,
        tokenProgram: tokenProgramId,
        systemProgram: systemProgramId,
      })
      .rpc()
      .catch(naclac.logError);

    console.log("      ✅ Pool initialized! Verifying balances...");
    const vaultABal = (await client.program.provider.connection.getTokenAccountBalance(vaultA)).value.amount;
    const vaultBBal = (await client.program.provider.connection.getTokenAccountBalance(vaultB)).value.amount;
    const userLpBal = (await client.program.provider.connection.getTokenAccountBalance(userLp)).value.amount;

    assert.strictEqual(Number(vaultABal), Number(amountA));
    assert.strictEqual(Number(vaultBBal), Number(amountB));

    const expectedLp = Math.floor(Math.sqrt(Number(amountA) * Number(amountB)));
    assert.strictEqual(Number(userLpBal), expectedLp);
    console.log(`      📊 Initial balances verified. LP minted: ${userLpBal}`);
  });

  it("2. Add Liquidity", async () => {
    console.log("      🔥 Adding liquidity...");
    await client
      .addLiquidity({
        maxAmountA: addAmountA,
        maxAmountB: addAmountB,
      })
      .accounts({
        user: payer.publicKey,
        poolState,
        vaultA,
        vaultB,
        lpMint,
        userTokenA,
        userTokenB,
        userLp,
        tokenProgram: tokenProgramId,
      })
      .rpc()
      .catch(naclac.logError);

    console.log("      ✅ Liquidity added! Verifying new balances...");
    const vaultABal = (await client.program.provider.connection.getTokenAccountBalance(vaultA)).value.amount;
    const userLpBal = (await client.program.provider.connection.getTokenAccountBalance(userLp)).value.amount;

    assert.strictEqual(Number(vaultABal), Number(amountA + addAmountA));
    assert.ok(Number(userLpBal) > Math.floor(Math.sqrt(Number(amountA) * Number(amountB))));
    console.log(`      📊 Post-add balances verified. LP count: ${userLpBal}`);
  });

  it("3. Execute Swap", async () => {
    console.log("      🔥 Executing swap...");
    await client
      .swap({
        amountIn: swapIn,
        minimumAmountOut: minOut,
      })
      .accounts({
        user: payer.publicKey,
        poolState,
        poolSourceVault: vaultA,
        poolDestinationVault: vaultB,
        userSourceToken: userTokenA,
        userDestinationToken: userTokenB,
        tokenProgram: tokenProgramId,
      })
      .rpc()
      .catch(naclac.logError);

    console.log("      ✅ Swap executed! Verifying Vault A balance...");
    const vaultABal = (await client.program.provider.connection.getTokenAccountBalance(vaultA)).value.amount;
    assert.strictEqual(Number(vaultABal), Number(amountA + addAmountA + swapIn));
    console.log(`      📊 Post-swap vault A balance: ${vaultABal}`);
  });

  it("4. Remove Liquidity", async () => {
    const userLpBefore = (await client.program.provider.connection.getTokenAccountBalance(userLp)).value.amount;
    console.log("      🔥 Removing liquidity...");
    await client
      .removeLiquidity({
        lpAmount: removeLpAmount,
      })
      .accounts({
        user: payer.publicKey,
        poolState,
        vaultA,
        vaultB,
        lpMint,
        userTokenA,
        userTokenB,
        userLp,
        tokenProgram: tokenProgramId,
      })
      .rpc()
      .catch(naclac.logError);

    console.log("      ✅ Liquidity removed! Verifying LP balance...");
    const userLpAfter = (await client.program.provider.connection.getTokenAccountBalance(userLp)).value.amount;
    assert.strictEqual(Number(userLpAfter), Number(userLpBefore) - Number(removeLpAmount));
    console.log(`      📊 Final LP Balance: ${userLpAfter}`);
  });
});
