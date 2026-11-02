import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { type LegacyProvider, type TxVersion, normalizeTxVersion } from "./provider";
import { createLiteSvmProvider } from "./provider-litesvm";
import { requireFeePayer, sendInstructions } from "./transaction";

export type Cluster =
  | "mainnet"
  | "devnet"
  | "testnet"
  | "localnet"
  | "litesvm"
  | (string & {});

/**
 * Creates a LegacyProvider from a cluster moniker or custom RPC URL.
 * The `"litesvm"` cluster requires `payer`.
 */
export function createProvider(
  cluster: Cluster,
  payer?: Keypair,
  defaultTxVersion?: TxVersion,
): LegacyProvider {
  if (cluster === "litesvm") {
    if (!payer) {
      throw new Error(
        '[Naclac] createProvider("litesvm") requires a payer Keypair; load one with `await loadNodeWallet()` and pass it in.',
      );
    }
    const prov = createLiteSvmProvider(payer);
    prov.defaultTxVersion = defaultTxVersion ?? "v1";
    return prov;
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

  const commitment = "confirmed";
  const connection = new Connection(url, commitment);
  return {
    connection,
    payer,
    publicKey: payer ? payer.publicKey : undefined,
    commitment,
    defaultTxVersion: defaultTxVersion ?? "v1",
    getBalance: async (addr: string | PublicKey) => {
      const pubkey = typeof addr === "string" ? new PublicKey(addr) : addr;
      return BigInt(await connection.getBalance(pubkey, commitment));
    },
    getTokenBalance: async (addr: string | PublicKey) => {
      const pubkey = typeof addr === "string" ? new PublicKey(addr) : addr;
      const res = await connection.getTokenAccountBalance(pubkey, commitment);
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
  const fromPubkey = requireFeePayer(provider);

  const ix = SystemProgram.transfer({ fromPubkey, toPubkey, lamports });
  return sendInstructions(provider, [ix], [], normalizeTxVersion(provider.defaultTxVersion));
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
  return await Keypair.fromSecretKey(keypairBytes);
}
