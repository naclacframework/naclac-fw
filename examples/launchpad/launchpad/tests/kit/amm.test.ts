import * as naclac from "@naclac-fw/client/kit";
import { AmmClient } from "../../clients/typescript/src/generated/amm";
import assert from "assert";

describe("Naclac AMM Test Suite (Kit)", () => {
  let client: AmmClient;
  let payer: naclac.KeyPairSigner;
  let tokenAMint: naclac.Address;
  let tokenBMint: naclac.Address;
  let userTokenA: naclac.Address;
  let userTokenB: naclac.Address;
  let userLp: naclac.Address;
  let poolState: naclac.Address;
  let poolBump: number;
  let vaultA: naclac.Address;
  let vaultB: naclac.Address;
  let lpMint: naclac.Address;

  const id = 100n;
  const amountA = 2_000_000_000n;
  const amountB = 4_000_000_000n;
  const addAmountA = 500_000_000n;
  const addAmountB = 1_000_000_000n;
  const swapIn = 100_000_000n;
  const minOut = 150_000_000n;
  const removeLpAmount = 500_000_000n;

  before(async () => {
    payer = await naclac.loadNodeWallet();
    client = new AmmClient("localnet", payer);

    console.log("      Generating Mint Signers...");
    const mintASigner = await naclac.generateExtractableKeyPairSigner();
    const mintBSigner = await naclac.generateExtractableKeyPairSigner();

    tokenAMint = (await naclac.createMint(client.program.provider, mintASigner, naclac.TOKEN_PROGRAM_ID, payer.address, 6)) as naclac.Address;
    tokenBMint = (await naclac.createMint(client.program.provider, mintBSigner, naclac.TOKEN_PROGRAM_ID, payer.address, 6)) as naclac.Address;
    console.log(`      Token A Mint: ${tokenAMint}`);
    console.log(`      Token B Mint: ${tokenBMint}`);

    userTokenA = (await naclac.createAta(client.program.provider, tokenAMint, payer.address, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    userTokenB = (await naclac.createAta(client.program.provider, tokenBMint, payer.address, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;

    await naclac.mintTo(client.program.provider, tokenAMint, payer.address, 10_000_000_000n, naclac.TOKEN_PROGRAM_ID);
    await naclac.mintTo(client.program.provider, tokenBMint, payer.address, 10_000_000_000n, naclac.TOKEN_PROGRAM_ID);
    console.log("      Minted initial Token A and B supply to user.");

    console.log("      Deriving Pool State PDA...");
    const [derivedPoolState, bump] = await client.getPoolStatePda({
      tokenAMint,
      tokenBMint,
      id,
    });
    poolState = derivedPoolState;
    poolBump = bump;
    console.log(`      Pool State PDA: ${poolState} (Bump: ${poolBump})`);

    const lpMintSigner = await naclac.generateExtractableKeyPairSigner();

    vaultA = (await naclac.createAta(client.program.provider, tokenAMint, poolState, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    vaultB = (await naclac.createAta(client.program.provider, tokenBMint, poolState, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    lpMint = (await naclac.createMint(client.program.provider, lpMintSigner, naclac.TOKEN_PROGRAM_ID, poolState, 6)) as naclac.Address;
    console.log(`      Vault A: ${vaultA}`);
    console.log(`      Vault B: ${vaultB}`);
    console.log(`      LP Mint: ${lpMint}`);

    userLp = (await naclac.createAta(client.program.provider, lpMint, payer.address, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    console.log(`      User LP ATA: ${userLp}`);
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
        payer: payer.address,
        tokenAMint,
        tokenBMint,
        poolState,
        vaultA,
        vaultB,
        lpMint,
        depositorTokenA: userTokenA,
        depositorTokenB: userTokenB,
        depositorLp: userLp,
        depositorAuthority: payer.address,
        tokenProgram: naclac.TOKEN_PROGRAM_ID,
        systemProgram: naclac.SYSTEM_PROGRAM_ID,
      })
      .rpc()
      .catch(naclac.logError);

    console.log("      ✅ Pool initialized! Verifying balances...");
    const vaultABal = await client.program.provider.getTokenBalance(vaultA);
    const vaultBBal = await client.program.provider.getTokenBalance(vaultB);
    const userLpBal = await client.program.provider.getTokenBalance(userLp);

    assert.strictEqual(Number(vaultABal), Number(amountA) / 1_000_000);
    assert.strictEqual(Number(vaultBBal), Number(amountB) / 1_000_000);

    const expectedLp = Math.floor(Math.sqrt(Number(amountA) * Number(amountB)));
    assert.strictEqual(Number(userLpBal), expectedLp / 1_000_000);
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
        user: payer.address,
        poolState,
        vaultA,
        vaultB,
        lpMint,
        userTokenA,
        userTokenB,
        userLp,
        tokenProgram: naclac.TOKEN_PROGRAM_ID,
      })
      .rpc()
      .catch(naclac.logError);

    console.log("      ✅ Liquidity added! Verifying new balances...");
    const vaultABal = await client.program.provider.getTokenBalance(vaultA);
    const userLpBal = await client.program.provider.getTokenBalance(userLp);

    assert.strictEqual(Number(vaultABal), Number(amountA + addAmountA) / 1_000_000);
    assert.ok(Number(userLpBal) > Math.floor(Math.sqrt(Number(amountA) * Number(amountB))) / 1_000_000);
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
        user: payer.address,
        poolState,
        poolSourceVault: vaultA,
        poolDestinationVault: vaultB,
        userSourceToken: userTokenA,
        userDestinationToken: userTokenB,
        tokenProgram: naclac.TOKEN_PROGRAM_ID,
      })
      .rpc()
      .catch(naclac.logError);

    console.log("      ✅ Swap executed! Verifying Vault A balance...");
    const vaultABal = await client.program.provider.getTokenBalance(vaultA);
    assert.strictEqual(Number(vaultABal), Number(amountA + addAmountA + swapIn) / 1_000_000);
    console.log(`      📊 Post-swap vault A balance: ${vaultABal}`);
  });

  it("4. Remove Liquidity", async () => {
    const userLpBefore = await client.program.provider.getTokenBalance(userLp);
    console.log("      🔥 Removing liquidity...");
    await client
      .removeLiquidity({
        lpAmount: removeLpAmount,
      })
      .accounts({
        user: payer.address,
        poolState,
        vaultA,
        vaultB,
        lpMint,
        userTokenA,
        userTokenB,
        userLp,
        tokenProgram: naclac.TOKEN_PROGRAM_ID,
      })
      .rpc()
      .catch(naclac.logError);

    console.log("      ✅ Liquidity removed! Verifying LP balance...");
    const userLpAfter = await client.program.provider.getTokenBalance(userLp);
    assert.strictEqual(Number(userLpAfter), Number(userLpBefore) - Number(removeLpAmount) / 1_000_000);
    console.log(`      📊 Final LP Balance: ${userLpAfter}`);
  });
});
