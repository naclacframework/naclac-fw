import {
  createMint as splCreateMint,
  getOrCreateAssociatedTokenAccount,
  mintTo as splMintTo,
  getAssociatedTokenAddress,
} from "@solana/spl-token";
import { PublicKey, Keypair } from "@solana/web3.js";
import type { LegacyProvider } from "./provider";

/**
 * Creates and initializes a new SPL Token mint using legacy web3.js + spl-token.
 */
export async function createMint(
  provider: LegacyProvider,
  mintSigner: Keypair,
  tokenProgramId: PublicKey = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
  mintAuthority: PublicKey = provider.payer ? provider.payer.publicKey : provider.publicKey!,
  decimals: number = 6,
): Promise<PublicKey> {
  const payer = provider.payer;
  if (!payer) {
    throw new Error("[Naclac] Provider must have a payer Keypair to sign local createMint transactions.");
  }

  await splCreateMint(
    provider.connection,
    payer,
    mintAuthority,
    mintAuthority,
    decimals,
    mintSigner,
    undefined,
    tokenProgramId
  );

  return mintSigner.publicKey;
}

/**
 * Creates an Associated Token Account for a given owner+mint using legacy web3.js + spl-token.
 */
export async function createAta(
  provider: LegacyProvider,
  mint: PublicKey,
  owner: PublicKey,
  tokenProgramId: PublicKey = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
): Promise<PublicKey> {
  const payer = provider.payer;
  if (!payer) {
    throw new Error("[Naclac] Provider must have a payer Keypair to sign local createAta transactions.");
  }

  const ata = await getOrCreateAssociatedTokenAccount(
    provider.connection,
    payer,
    mint,
    owner,
    true,
    undefined,
    undefined,
    tokenProgramId
  );

  return ata.address;
}

/**
 * Mints tokens to a destination owner's Associated Token Account using legacy web3.js + spl-token.
 */
export async function mintTo(
  provider: LegacyProvider,
  mint: PublicKey,
  destinationOwner: PublicKey,
  amount: bigint | number,
  tokenProgramId: PublicKey = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
): Promise<void> {
  const payer = provider.payer;
  if (!payer) {
    throw new Error("[Naclac] Provider must have a payer Keypair to sign local mintTo transactions.");
  }

  const ataAddress = await getAssociatedTokenAddress(
    mint,
    destinationOwner,
    true,
    tokenProgramId
  );

  await splMintTo(
    provider.connection,
    payer,
    mint,
    ataAddress,
    payer,
    amount,
    [],
    undefined,
    tokenProgramId
  );
}
