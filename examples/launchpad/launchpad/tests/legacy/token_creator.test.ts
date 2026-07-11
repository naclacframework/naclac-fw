import * as naclac from "@naclac-fw/client/legacy";
import { TokenCreatorClient } from "../../clients/typescript/src/generated/token_creator/client-legacy";
import { AmmClient } from "../../clients/typescript/src/generated/amm/client-legacy";
import assert from "assert";

describe("Naclac TokenCreator Test Suite (Legacy)", () => {
  let creatorClient: TokenCreatorClient;
  let ammClient: AmmClient;
  let payer: naclac.Keypair;

  let quoteMint: naclac.PublicKey;
  let payerTokenB: naclac.PublicKey;

  let mintAddress: naclac.PublicKey;
  let mintBump: number;
  let launchRecord: naclac.PublicKey;
  let launchRecordBump: number;

  let poolState: naclac.PublicKey;
  let poolBump: number;
  let poolVaultA: naclac.PublicKey;
  let poolVaultB: naclac.PublicKey;
  let poolLpMint: naclac.PublicKey;

  let launcherTokenA: naclac.PublicKey;
  let launcherTokenB: naclac.PublicKey;
  let launcherLp: naclac.PublicKey;

  const id = BigInt(Math.floor(Math.random() * 100000000) + 1);
  const decimals = 6;
  const amountTokenPool = 1_000_000_000n;
  const amountTokenLauncher = 500_000_000n;
  const amountQuote = 2_000_000_000n;

  const tokenProgramId = new naclac.PublicKey(naclac.TOKEN_PROGRAM_ID);
  const systemProgramId = new naclac.PublicKey(naclac.SYSTEM_PROGRAM_ID);

  before(async () => {
    payer = await naclac.loadNodeWallet();
    creatorClient = new TokenCreatorClient("localnet", payer);
    ammClient = new AmmClient("localnet", payer);

    console.log("      Generating Quote Mint Keypair...");
    const quoteMintKeypair = naclac.Keypair.generate();
    quoteMint = (await naclac.createMint(creatorClient.program.provider, quoteMintKeypair, tokenProgramId, payer.publicKey, 6)) as naclac.PublicKey;
    console.log(`      Quote Mint: ${quoteMint.toBase58()}`);

    payerTokenB = (await naclac.createAta(creatorClient.program.provider, quoteMint, payer.publicKey, tokenProgramId)) as naclac.PublicKey;
    await naclac.mintTo(creatorClient.program.provider, quoteMint, payer.publicKey, 10_000_000_000n, tokenProgramId);
    console.log(`      Payer Token B (Quote) ATA: ${payerTokenB.toBase58()}`);

    console.log("      Deriving Token Creator PDAs...");
    const [derivedMint, mBump] = creatorClient.getMintPda({ id });
    mintAddress = derivedMint;
    mintBump = mBump;

    const [derivedLaunchRecord, lrBump] = creatorClient.getLaunchRecordPda({ payer: payer.publicKey, id });
    launchRecord = derivedLaunchRecord;
    launchRecordBump = lrBump;

    console.log(`      Token A Mint PDA: ${mintAddress.toBase58()} (Bump: ${mintBump})`);
    console.log(`      Launch Record PDA: ${launchRecord.toBase58()} (Bump: ${launchRecordBump})`);
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
        payer: payer.publicKey,
        mint: mintAddress,
        launchRecord,
        tokenProgram: tokenProgramId,
        systemProgram: systemProgramId,
      })
      .rpc()
      .catch(naclac.logError);

    console.log("      ✅ Mint created successfully!");
  });

  it("2. Derive AMM Pool State and Create Vaults", async () => {
    console.log("      Deriving AMM Pool State PDA...");
    const [derivedPoolState, pBump] = ammClient.getPoolStatePda({
      tokenAMint: mintAddress,
      tokenBMint: quoteMint,
      id,
    });
    poolState = derivedPoolState;
    poolBump = pBump;
    console.log(`      AMM Pool State PDA: ${poolState.toBase58()} (Bump: ${poolBump})`);

    const poolLpMintKeypair = naclac.Keypair.generate();

    poolVaultA = (await naclac.createAta(creatorClient.program.provider, mintAddress, poolState, tokenProgramId)) as naclac.PublicKey;
    poolVaultB = (await naclac.createAta(creatorClient.program.provider, quoteMint, poolState, tokenProgramId)) as naclac.PublicKey;
    poolLpMint = (await naclac.createMint(creatorClient.program.provider, poolLpMintKeypair, tokenProgramId, poolState, 6)) as naclac.PublicKey;
    console.log(`      Pool Vault A: ${poolVaultA.toBase58()}`);
    console.log(`      Pool Vault B: ${poolVaultB.toBase58()}`);
    console.log(`      Pool LP Mint: ${poolLpMint.toBase58()}`);

    launcherTokenA = (await naclac.createAta(creatorClient.program.provider, mintAddress, launchRecord, tokenProgramId)) as naclac.PublicKey;
    launcherTokenB = (await naclac.createAta(creatorClient.program.provider, quoteMint, launchRecord, tokenProgramId)) as naclac.PublicKey;
    launcherLp = (await naclac.createAta(creatorClient.program.provider, poolLpMint, launchRecord, tokenProgramId)) as naclac.PublicKey;
    console.log(`      Launcher Vault A: ${launcherTokenA.toBase58()}`);
    console.log(`      Launcher Vault B: ${launcherTokenB.toBase58()}`);
    console.log(`      Launcher LP ATA: ${launcherLp.toBase58()}`);
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
        payer: payer.publicKey,
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
        tokenProgram: tokenProgramId,
        systemProgram: systemProgramId,
      })
      .rpc()
      .catch(naclac.logError);

    console.log("      ✅ launchToken transaction successful! Verifying record state...");
    const record = (await creatorClient.fetchLaunchRecord(launchRecord)) as any;
    
    assert.strictEqual(record.creator, payer.publicKey.toBase58());
    assert.strictEqual(record.mint, mintAddress.toBase58());
    assert.strictEqual(Number(record.amountToken), Number(amountTokenPool));
    assert.strictEqual(Number(record.amountQuote), Number(amountQuote));
    console.log("      ✅ Launch record state verified successfully!");
  });
});
