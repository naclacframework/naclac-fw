import * as naclac from "@naclac-fw/client/kit";
import { TokenCreatorClient } from "../../clients/typescript/src/generated/token_creator";
import { AmmClient } from "../../clients/typescript/src/generated/amm";
import assert from "assert";

describe("Naclac TokenCreator Test Suite (Kit)", () => {
  let creatorClient: TokenCreatorClient;
  let ammClient: AmmClient;
  let payer: naclac.KeyPairSigner;

  let quoteMint: naclac.Address;
  let payerTokenB: naclac.Address;

  let mintAddress: naclac.Address;
  let mintBump: number;
  let launchRecord: naclac.Address;
  let launchRecordBump: number;

  let poolState: naclac.Address;
  let poolBump: number;
  let poolVaultA: naclac.Address;
  let poolVaultB: naclac.Address;
  let poolLpMint: naclac.Address;

  let launcherTokenA: naclac.Address;
  let launcherTokenB: naclac.Address;
  let launcherLp: naclac.Address;

  const id = BigInt(Math.floor(Math.random() * 100000000) + 1);
  const decimals = 6;
  const amountTokenPool = 1_000_000_000n;
  const amountTokenLauncher = 500_000_000n;
  const amountQuote = 2_000_000_000n;

  before(async () => {
    payer = await naclac.loadNodeWallet();
    creatorClient = new TokenCreatorClient("localnet", payer);
    ammClient = new AmmClient("localnet", payer);

    console.log("      Generating Quote Mint Signer...");
    const quoteMintSigner = await naclac.generateExtractableKeyPairSigner();
    quoteMint = (await naclac.createMint(creatorClient.program.provider, quoteMintSigner, naclac.TOKEN_PROGRAM_ID, payer.address, 6)) as naclac.Address;
    console.log(`      Quote Mint: ${quoteMint}`);

    payerTokenB = (await naclac.createAta(creatorClient.program.provider, quoteMint, payer.address, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    await naclac.mintTo(creatorClient.program.provider, quoteMint, payer.address, 10_000_000_000n, naclac.TOKEN_PROGRAM_ID);
    console.log(`      Payer Token B (Quote) ATA: ${payerTokenB}`);

    console.log("      Deriving Token Creator PDAs...");
    const [derivedMint, mBump] = await creatorClient.getMintPda({ id });
    mintAddress = derivedMint;
    mintBump = mBump;

    const [derivedLaunchRecord, lrBump] = await creatorClient.getLaunchRecordPda({ payer: payer.address, id });
    launchRecord = derivedLaunchRecord;
    launchRecordBump = lrBump;

    console.log(`      Token A Mint PDA: ${mintAddress} (Bump: ${mintBump})`);
    console.log(`      Launch Record PDA: ${launchRecord} (Bump: ${launchRecordBump})`);
  });

  it("1. Create Mint PDA On-chain", async () => {
    console.log("      🔥 Creating mint on-chain...");
    await creatorClient
      .createMint({
        id,
        mintBump,
        launchRecordBump,
        decimals,
      })
      .accounts({
        payer: payer.address,
        mint: mintAddress,
        launchRecord,
        tokenProgram: naclac.TOKEN_PROGRAM_ID,
        systemProgram: naclac.SYSTEM_PROGRAM_ID,
      })
      .rpc()
      .catch(naclac.logError);

    console.log("      ✅ Mint created successfully!");
  });

  it("2. Derive AMM Pool State and Create Vaults", async () => {
    console.log("      Deriving AMM Pool State PDA...");
    const [derivedPoolState, pBump] = await ammClient.getPoolStatePda({
      tokenAMint: mintAddress,
      tokenBMint: quoteMint,
      id,
    });
    poolState = derivedPoolState;
    poolBump = pBump;
    console.log(`      AMM Pool State PDA: ${poolState} (Bump: ${poolBump})`);

    const poolLpMintSigner = await naclac.generateExtractableKeyPairSigner();

    poolVaultA = (await naclac.createAta(creatorClient.program.provider, mintAddress, poolState, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    poolVaultB = (await naclac.createAta(creatorClient.program.provider, quoteMint, poolState, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    poolLpMint = (await naclac.createMint(creatorClient.program.provider, poolLpMintSigner, naclac.TOKEN_PROGRAM_ID, poolState, 6)) as naclac.Address;
    console.log(`      Pool Vault A: ${poolVaultA}`);
    console.log(`      Pool Vault B: ${poolVaultB}`);
    console.log(`      Pool LP Mint: ${poolLpMint}`);

    launcherTokenA = (await naclac.createAta(creatorClient.program.provider, mintAddress, launchRecord, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    launcherTokenB = (await naclac.createAta(creatorClient.program.provider, quoteMint, launchRecord, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    launcherLp = (await naclac.createAta(creatorClient.program.provider, poolLpMint, launchRecord, naclac.TOKEN_PROGRAM_ID)) as naclac.Address;
    console.log(`      Launcher Vault A: ${launcherTokenA}`);
    console.log(`      Launcher Vault B: ${launcherTokenB}`);
    console.log(`      Launcher LP ATA: ${launcherLp}`);
  });

  it("3. Execute launchToken CPI", async () => {
    console.log("      🔥 Sending launchToken transaction...");
    await creatorClient
      .launchToken({
        args: {
          id,
          mintBump,
          launchRecordBump,
          poolBump,
          decimals,
          amountTokenPool,
          amountTokenLauncher,
          amountQuote,
        },
      })
      .accounts({
        payer: payer.address,
        quoteMint,
        mint: mintAddress,
        launchRecord,
        launcherTokenA,
        launcherTokenB,
        launcherLp,
        payerTokenB,
        poolState,
        poolVaultA,
        poolVaultB,
        poolLpMint,
        ammProgram: ammClient.programId,
        tokenProgram: naclac.TOKEN_PROGRAM_ID,
        systemProgram: naclac.SYSTEM_PROGRAM_ID,
      })
      .rpc()
      .catch(naclac.logError);

    console.log("      ✅ launchToken transaction successful! Verifying record state...");
    const record = await creatorClient.fetchLaunchRecord(launchRecord);
    
    assert.strictEqual(record.data.creator, payer.address);
    assert.strictEqual(record.data.mint, mintAddress);
    assert.strictEqual(Number(record.data.amountToken), Number(amountTokenPool));
    assert.strictEqual(Number(record.data.amountQuote), Number(amountQuote));
    console.log("      ✅ Launch record state verified successfully!");
  });
});
