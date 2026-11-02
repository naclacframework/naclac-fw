import {
  findAssociatedTokenPda,
  getCreateAssociatedTokenIdempotentInstruction,
  getInitializeMint2Instruction,
  getMintSize,
  getMintToInstruction,
} from "@solana-program/token-2022";
import { PublicKey, SystemProgram, type Keypair } from "@solana/web3.js";
import { normalizeTxVersion, type LegacyProvider } from "./provider";
import { sendInstructions } from "./transaction";

function requirePayer(provider: LegacyProvider, operation: string): Keypair {
  if (!provider.payer) {
    throw new Error(`[Naclac] Provider must have a payer Keypair to sign local ${operation} transactions.`);
  }
  return provider.payer;
}

/**
 * Creates and initializes a new SPL Token mint owned by `tokenProgramId`
 * (Token or Token-2022), with `mintAuthority` as both mint and freeze
 * authority.
 */
export async function createMint(
  provider: LegacyProvider,
  mintSigner: Keypair,
  tokenProgramId: PublicKey,
  mintAuthority: PublicKey,
  decimals: number,
): Promise<PublicKey> {
  const payer = requirePayer(provider, "createMint");

  const space = getMintSize();
  const lamports = await provider.connection.getMinimumBalanceForRentExemption(space, provider.commitment);
  const createAccountIx = SystemProgram.createAccount({
    fromPubkey: payer.publicKey,
    newAccountPubkey: mintSigner.publicKey,
    lamports,
    space,
    programId: tokenProgramId,
  });
  const initMintIx = getInitializeMint2Instruction(
    {
      mint: mintSigner.publicKey,
      decimals,
      mintAuthority,
      freezeAuthority: mintAuthority,
    },
    { programAddress: tokenProgramId },
  );

  await sendInstructions(
    provider,
    [createAccountIx, initMintIx],
    [mintSigner],
    normalizeTxVersion(provider.defaultTxVersion),
  );
  return mintSigner.publicKey;
}

/**
 * Creates the Associated Token Account for `owner` and `mint` if it does not
 * exist yet, and returns its address.
 */
export async function createAta(
  provider: LegacyProvider,
  mint: PublicKey,
  owner: PublicKey,
  tokenProgramId: PublicKey,
): Promise<PublicKey> {
  const payer = requirePayer(provider, "createAta");

  const [ata] = await findAssociatedTokenPda({ mint, owner, tokenProgram: tokenProgramId });
  const createAtaIx = getCreateAssociatedTokenIdempotentInstruction({
    payer,
    ata,
    owner,
    mint,
    tokenProgram: tokenProgramId,
  });

  await sendInstructions(provider, [createAtaIx], [], normalizeTxVersion(provider.defaultTxVersion));
  return new PublicKey(ata);
}

/**
 * Mints `amount` tokens to `destinationOwner`'s Associated Token Account,
 * creating that account first if it does not exist. The provider's payer is
 * the mint authority.
 */
export async function mintTo(
  provider: LegacyProvider,
  mint: PublicKey,
  destinationOwner: PublicKey,
  amount: bigint | number,
  tokenProgramId: PublicKey,
): Promise<void> {
  const payer = requirePayer(provider, "mintTo");

  const [ata] = await findAssociatedTokenPda({ mint, owner: destinationOwner, tokenProgram: tokenProgramId });
  const createAtaIx = getCreateAssociatedTokenIdempotentInstruction({
    payer,
    ata,
    owner: destinationOwner,
    mint,
    tokenProgram: tokenProgramId,
  });
  const mintToIx = getMintToInstruction(
    { mint, token: ata, mintAuthority: payer, amount: BigInt(amount) },
    { programAddress: tokenProgramId },
  );

  await sendInstructions(provider, [createAtaIx, mintToIx], [], normalizeTxVersion(provider.defaultTxVersion));
}
