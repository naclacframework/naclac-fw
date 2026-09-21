import {
  address,
  createTransactionMessage,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  appendTransactionMessageInstruction,
  appendTransactionMessageInstructions,
  signTransactionMessageWithSigners,
  sendAndConfirmTransactionFactory,
} from "@solana/kit";
import { getCreateAccountInstruction } from "@solana-program/system";
import {
  getInitializeMintInstruction,
  getMintSize,
  getCreateAssociatedTokenIdempotentInstruction,
  getMintToInstruction,
} from "@solana-program/token-2022";
import type { NaclacProvider } from "./provider";
import { getAssociatedTokenAddress } from "../utils/pda";
export { getAssociatedTokenAddress };

function getSendAndConfirm(provider: NaclacProvider) {
  if (provider._sendAndConfirm) return provider._sendAndConfirm;

  const sendAndConfirm = sendAndConfirmTransactionFactory({
    rpc: provider.rpc as any,
    rpcSubscriptions: provider.rpcSubscriptions as any,
  });
  return (tx: any) => sendAndConfirm(tx, { commitment: provider.commitment ?? "confirmed" });
}

export async function createMint(
  provider: NaclacProvider,
  mintSigner: any,
  tokenProgram: any = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
  mintAuthority: string = provider.signer.address as string,
  decimals: number = 6,
): Promise<string> {
  const rpc = provider.rpc as any;
  const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

  const space = getMintSize();
  const lamports = await rpc.getMinimumBalanceForRentExemption(BigInt(space)).send();

  const createAccountIx = getCreateAccountInstruction({
    space,
    lamports,
    newAccount: mintSigner,
    payer: provider.signer,
    programAddress: address(tokenProgram),
  });

  const initMintIx = getInitializeMintInstruction(
    {
      mint: mintSigner.address,
      mintAuthority: address(mintAuthority),
      freezeAuthority: address(mintAuthority),
      decimals,
    },
    { programAddress: address(tokenProgram) },
  );

  const txMessage = pipe(
    createTransactionMessage({ version: "legacy" }),
    (msg) => setTransactionMessageFeePayerSigner(provider.signer, msg),
    (msg) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, msg),
    (msg) => appendTransactionMessageInstructions([createAccountIx, initMintIx], msg)
  );

  const signedTx = await signTransactionMessageWithSigners(txMessage);
  await getSendAndConfirm(provider)(signedTx as any);
  return mintSigner.address as string;
}

export async function createAta(
  provider: NaclacProvider,
  mint: string,
  owner: string,
  tokenProgram: any = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
): Promise<string> {
  const rpc = provider.rpc as any;

  const [ata] = await getAssociatedTokenAddress(
    mint,
    owner,
    tokenProgram,
  );

  const existing = await rpc.getAccountInfo(ata, { encoding: "base64" }).send();
  if (existing.value !== null) return ata as string;

  const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

  const createAtaIx = getCreateAssociatedTokenIdempotentInstruction({
    payer: provider.signer,
    ata,
    owner: address(owner),
    mint: address(mint),
    tokenProgram: address(tokenProgram),
  });

  const txMessage = pipe(
    createTransactionMessage({ version: "legacy" }),
    (msg) => setTransactionMessageFeePayerSigner(provider.signer, msg),
    (msg) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, msg),
    (msg) => appendTransactionMessageInstruction(createAtaIx, msg)
  );

  const signedTx = await signTransactionMessageWithSigners(txMessage);
  await getSendAndConfirm(provider)(signedTx as any);
  return ata as string;
}

export async function mintTo(
  provider: NaclacProvider,
  mint: string,
  destinationOwner: string,
  amount: bigint,
  tokenProgram: any = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
): Promise<void> {
  const rpc = provider.rpc as any;
  const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

  const [ata] = await getAssociatedTokenAddress(
    mint,
    destinationOwner,
    tokenProgram,
  );

  const createAtaIx = getCreateAssociatedTokenIdempotentInstruction({
    payer: provider.signer,
    ata,
    owner: address(destinationOwner),
    mint: address(mint),
    tokenProgram: address(tokenProgram),
  });

  const mintToIx = getMintToInstruction(
    {
      mint: address(mint),
      token: ata,
      mintAuthority: provider.signer,
      amount,
    },
    { programAddress: address(tokenProgram) }
  );

  const txMessage = pipe(
    createTransactionMessage({ version: "legacy" }),
    (msg) => setTransactionMessageFeePayerSigner(provider.signer, msg),
    (msg) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, msg),
    (msg) => appendTransactionMessageInstructions([createAtaIx, mintToIx], msg)
  );

  const signedTx = await signTransactionMessageWithSigners(txMessage);
  await getSendAndConfirm(provider)(signedTx as any);
}
