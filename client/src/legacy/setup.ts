import { Connection, Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import type { LegacyProvider } from "./provider";
import { createLiteSvmProvider } from "./provider-litesvm";

export type Cluster =
  | "mainnet"
  | "devnet"
  | "testnet"
  | "localnet"
  | "litesvm"
  | (string & {});

/**
 * Creates a LegacyProvider from a cluster moniker or custom RPC URL.
 */
export function createProvider(
  cluster: Cluster,
  payer?: Keypair,
): LegacyProvider {
  if (cluster === "litesvm") {
    return createLiteSvmProvider(payer);
  }

  let url: string;
  if (cluster === "mainnet") {
    url = "https://api.mainnet-beta.solana.com";
  } else if (cluster === "devnet") {
    url = "https://api.devnet.solana.com";
  } else if (cluster === "testnet") {
    url = "https://api.testnet.solana.com";
  } else if (cluster === "localnet" || cluster.includes("localhost") || cluster.includes("127.0.0.1")) {
    url = "http://127.0.0.1:8899";
  } else {
    url = cluster;
  }

  const connection = new Connection(url, "confirmed");
  return {
    connection,
    payer,
    publicKey: payer ? payer.publicKey : undefined,
    commitment: "confirmed",
    getBalance: async (addr: string | PublicKey) => {
      const pubkey = typeof addr === "string" ? new PublicKey(addr) : addr;
      const lamports = await connection.getBalance(pubkey, "confirmed");
      return BigInt(lamports);
    },
    getTokenBalance: async (addr: string | PublicKey) => {
      const pubkey = typeof addr === "string" ? new PublicKey(addr) : addr;
      const res = await connection.getTokenAccountBalance(pubkey, "confirmed");
      return Number(res.value.amount) / Math.pow(10, res.value.decimals);
    },
  };
}

/**
 * Transfers SOL from the provider to a destination address.
 */
export async function transferSol(
  provider: LegacyProvider,
  to: PublicKey | string,
  lamports: bigint | number
): Promise<string> {
  const toPubkey = new PublicKey(to);
  const amount = Number(lamports);

  const fromPubkey = provider.payer ? provider.payer.publicKey : provider.publicKey;
  if (!fromPubkey) {
    throw new Error("[Naclac] Provider has no payer or publicKey configured.");
  }

  const transaction = new Transaction().add(
    SystemProgram.transfer({
      fromPubkey,
      toPubkey,
      lamports: amount,
    })
  );

  const { blockhash, lastValidBlockHeight } = await provider.connection.getLatestBlockhash(
    (provider.commitment as any) ?? "confirmed"
  );
  transaction.recentBlockhash = blockhash;
  transaction.feePayer = fromPubkey;

  let signedTx: Transaction;
  if (provider.payer) {
    transaction.sign(provider.payer);
    signedTx = transaction;
  } else if (provider.signTransaction) {
    signedTx = await provider.signTransaction(transaction);
  } else {
    throw new Error("[Naclac] No signing capability found on legacy provider.");
  }

  const signature = await provider.connection.sendRawTransaction(signedTx.serialize());
  await provider.connection.confirmTransaction({
    signature,
    blockhash,
    lastValidBlockHeight,
  }, (provider.commitment as any) ?? "confirmed");

  return signature;
}

/**
 * Safely loads a local legacy Keypair file (Node.js only).
 * Reads from `~/.config/solana/id.json` by default.
 *
 * @param path Optional absolute path to a keypair JSON file.
 * @returns A `Keypair` instance ready for signing transactions.
 * @throws If called outside a Node.js environment.
 */
export async function loadNodeWallet(path?: string): Promise<Keypair> {
  let fs: any, os: any;
  try {
    fs = require("fs");
    os = require("os");
  } catch (e) {
    throw new Error("[Naclac] loadNodeWallet can only be used in Node.js environments.");
  }
  const defaultPath = `${os.homedir()}/.config/solana/id.json`;
  const keypairBytes = new Uint8Array(JSON.parse(fs.readFileSync(path ?? defaultPath, "utf8")));
  return Keypair.fromSecretKey(keypairBytes);
}

