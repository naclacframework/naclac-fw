import {
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  sendAndConfirmTransactionFactory,
  createKeyPairSignerFromBytes,
  address,
  createTransactionMessage,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  appendTransactionMessageInstruction,
  signTransactionMessageWithSigners,
  getBase64EncodedWireTransaction,
  AccountRole,
  pipe,
  type Address,
} from "@solana/kit";
import type { NaclacProvider } from "./provider";
import { createLiteSvmProvider } from "./provider-litesvm";
import { SYSTEM_PROGRAM_ID } from "../constants";

export type Cluster =
  | "mainnet"
  | "devnet"
  | "testnet"
  | "localnet"
  | "litesvm"
  | (string & {});

/**
 * Creates a NaclacProvider from a cluster moniker or custom RPC URL using standard @solana/kit clients.
 * `signer` must already be resolved (e.g. `await loadNodeWallet()`) — same
 * requirement for every cluster, litesvm included.
 */
export function createProvider(
  cluster: Cluster,
  signer: any,
  clusterSubscriptions?: Cluster,
): NaclacProvider {
  if (cluster === "litesvm") {
    return createLiteSvmProvider(signer);
  }

  let httpUrl: string;
  let wsUrl: string;

  if (cluster === "mainnet") {
    httpUrl = "https://api.mainnet-beta.solana.com";
    wsUrl = "wss://api.mainnet-beta.solana.com";
  } else if (cluster === "devnet") {
    httpUrl = "https://api.devnet.solana.com";
    wsUrl = "wss://api.devnet.solana.com";
  } else if (cluster === "testnet") {
    httpUrl = "https://api.testnet.solana.com";
    wsUrl = "wss://api.testnet.solana.com";
  } else if (cluster === "localnet" || cluster.includes("localhost") || cluster.includes("127.0.0.1")) {
    httpUrl = "http://127.0.0.1:8899";
    wsUrl = "ws://127.0.0.1:8900";
  } else {
    httpUrl = cluster;
    wsUrl = cluster.replace("https://", "wss://").replace("http://", "ws://");
  }

  const rpc = createSolanaRpc(httpUrl);
  
  let subWsUrl = wsUrl;
  if (clusterSubscriptions) {
    if (clusterSubscriptions === "mainnet") {
      subWsUrl = "wss://api.mainnet-beta.solana.com";
    } else if (clusterSubscriptions === "devnet") {
      subWsUrl = "wss://api.devnet.solana.com";
    } else if (clusterSubscriptions === "testnet") {
      subWsUrl = "wss://api.testnet.solana.com";
    } else if (clusterSubscriptions === "localnet") {
      subWsUrl = "ws://127.0.0.1:8900";
    } else {
      subWsUrl = clusterSubscriptions;
    }
  }
  const rpcSubscriptions = createSolanaRpcSubscriptions(subWsUrl);

  const sendAndConfirm = sendAndConfirmTransactionFactory({
    rpc,
    rpcSubscriptions,
  });

  return {
    rpc: rpc as any,
    rpcSubscriptions: rpcSubscriptions as any,
    signer,
    commitment: "confirmed",
    _sendAndConfirm: (tx: any) => sendAndConfirm(tx, { commitment: "confirmed" }),
    getBalance: async (addr: string | Address) => {
      const res = await rpc.getAccountInfo(address(addr as string), { commitment: "confirmed" }).send();
      return res.value?.lamports ?? 0n;
    },
    getTokenBalance: async (addr: string | Address) => {
      const res = await (rpc as any).getTokenAccountBalance(address(addr as string)).send();
      return Number(res.value.amount) / Math.pow(10, res.value.decimals);
    }
  };
}

/**
 * Safely loads a local keypair file (Node.js only).
 * Prevents Webpack/Vite from crashing in frontend React apps.
 */
export async function loadNodeWallet(path?: string) {
  let fs: any, os: any;
  try {
    fs = require("fs");
    os = require("os");
  } catch (e) {
    throw new Error("[Naclac] loadNodeWallet can only be used in Node.js environments.");
  }
  const defaultPath = `${os.homedir()}/.config/solana/id.json`;
  const keypairBytes = new Uint8Array(JSON.parse(fs.readFileSync(path ?? defaultPath, "utf8")));
  return createKeyPairSignerFromBytes(keypairBytes);
}

/**
 * Transfers SOL from the provider signer to a destination address.
 */
export async function transferSol(
  provider: NaclacProvider,
  to: Address | string,
  lamports: bigint | number
): Promise<void> {
  const toAddress = address(to as string);
  const amount = BigInt(lamports);

  const data = new Uint8Array(12);
  const view = new DataView(data.buffer);
  view.setUint32(0, 2, true);
  view.setBigUint64(4, amount, true);

  const instruction = {
    programAddress: SYSTEM_PROGRAM_ID as Address,
    accounts: [
      { address: provider.signer.address, role: AccountRole.WRITABLE_SIGNER },
      { address: toAddress, role: AccountRole.WRITABLE },
    ],
    data,
  };

  const { value: latestBlockhash } = await (provider.rpc as any)
    .getLatestBlockhash({ commitment: provider.commitment ?? "confirmed" })
    .send();

  const txMessage = pipe(
    createTransactionMessage({ version: 0 }),
    (msg) => setTransactionMessageFeePayerSigner(provider.signer, msg),
    (msg) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, msg),
    (msg) => appendTransactionMessageInstruction(instruction, msg)
  );

  const signedTx = await signTransactionMessageWithSigners(txMessage);

  if (provider.litesvm) {
    // litesvm executes and finalizes `sendTransaction` synchronously —
    // there is no separate confirm step to wait for, and no real
    // websocket for `sendAndConfirmTransactionFactory` to use anyway.
    await (provider.rpc as any)
      .sendTransaction(getBase64EncodedWireTransaction(signedTx as any), { encoding: "base64" })
      .send();
    return;
  }

  const sendAndConfirm = sendAndConfirmTransactionFactory({
    rpc: provider.rpc as any,
    rpcSubscriptions: provider.rpcSubscriptions as any,
  });

  await sendAndConfirm(signedTx as any, {
    commitment: provider.commitment ?? "confirmed",
  });
}
