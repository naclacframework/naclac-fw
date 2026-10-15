import {
  AddressLookupTableProgram,
  Connection,
  Keypair,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";

/**
 * Creates an address lookup table holding `addresses` on `connection`'s
 * cluster and waits until those addresses are usable by a transaction (a
 * table's entries only become usable from the slot after they were added).
 */
export async function createLookupTable(
  connection: Connection,
  payer: Keypair,
  addresses: PublicKey[],
): Promise<PublicKey> {
  const recentSlot = await connection.getSlot();
  const [createInstruction, table] = AddressLookupTableProgram.createLookupTable({
    authority: payer.publicKey,
    payer: payer.publicKey,
    recentSlot,
  });
  const extendInstruction = AddressLookupTableProgram.extendLookupTable({
    lookupTable: table,
    authority: payer.publicKey,
    payer: payer.publicKey,
    addresses,
  });

  const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash();
  const message = new TransactionMessage({
    payerKey: payer.publicKey,
    recentBlockhash: blockhash,
    instructions: [createInstruction, extendInstruction],
  }).compileToV0Message();
  const transaction = new VersionedTransaction(message);
  transaction.sign([payer]);
  const signature = await connection.sendTransaction(transaction);
  await connection.confirmTransaction({ signature, blockhash, lastValidBlockHeight }, "confirmed");

  const extendedAt = await connection.getSlot();
  while ((await connection.getSlot()) <= extendedAt) {
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  return table;
}
