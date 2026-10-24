import { Connection, PublicKey, Keypair, Transaction, VersionedTransaction } from "@solana/web3.js";

/**
 * Converts web3.js wire bytes to the format naclac-litesvm's native binding expects.
 *
 * web3.js `VersionedTransaction.serialize()` layout:
 *   [compact-u16 sig count][sig_0 (64 bytes)]...[sig_N][message bytes]
 *
 * naclac-litesvm / @solana/kit layout (what wincode::deserialize_exact reads):
 *   [message bytes][sig_0 (64 bytes)]...[sig_N]
 *
 * For a legacy `Transaction` the first byte is the number of required signatures
 * in the header — always < 0x80 — so that format never clashes with versioned
 * prefix detection.
 */
function toNaclacWireBytes(web3jsTxBytes: Uint8Array | Buffer): Buffer {
  const buf = Buffer.isBuffer(web3jsTxBytes) ? web3jsTxBytes : Buffer.from(web3jsTxBytes);
  // Versioned transactions from web3.js start with the sig-count byte.
  // Legacy transactions are detected by their message header (first byte is
  // numRequiredSignatures, which is a valid header for legacy messages and
  // always < 0x80, while versioned message bytes start with 0x80|version).
  // The sig-count byte for a typical transaction is 1 — if after stripping
  // [count][sigs] the next byte is 0x80 or 0x81, it is a versioned tx in
  // web3.js format and needs to be rearranged.
  const sigCount = buf[0];
  if (sigCount > 0 && sigCount <= 127) {
    const afterSigs = 1 + sigCount * 64;
    if (afterSigs < buf.length && buf[afterSigs] === 0x81) {
      const msgBytes = buf.slice(afterSigs);
      const allSigs = buf.slice(1, afterSigs);
      return Buffer.concat([msgBytes, allSigs]);
    }
  }
  return buf;
}
import type { LegacyProvider } from "./provider";

/**
 * Reads the same raw 64-byte `[secret(32) || public(32)]` keypair JSON
 * array a web3.js `Keypair.fromSecretKey` would, and returns both the
 * `Keypair` and the raw bytes — litesvm's native binding needs the raw
 * bytes to construct a `solana_keypair::Keypair` on the Rust side.
 */
function loadNodeWalletKeypair(path?: string): { keypair: Keypair; bytes: Uint8Array } {
  let fs: any, os: any;
  try {
    fs = require("fs");
    os = require("os");
  } catch (e) {
    throw new Error("[Naclac] loadNodeWalletKeypair can only be used in Node.js environments.");
  }
  const defaultPath = `${os.homedir()}/.config/solana/id.json`;
  const bytes = new Uint8Array(JSON.parse(fs.readFileSync(path ?? defaultPath, "utf8")));
  return { keypair: Keypair.fromSecretKey(bytes), bytes };
}

/**
 * A `Connection`-shaped object backed by a `@naclac-fw/litesvm` `LiteSvm`
 * instance, implementing only the methods `LegacyAccountFetcher`/
 * `LegacyMethodsBuilder`/`LegacyProgram` actually call — not a real
 * `Connection` subclass (no benefit to one here, since none of its other
 * behavior is needed), just enough surface to satisfy those call sites.
 * Cast to `Connection` at the boundary so `LegacyProvider`'s existing,
 * strictly-typed `connection: Connection` field doesn't need to change.
 */
function wrapNativeConnection(svm: import("@naclac-fw/litesvm").LiteSvm): Connection {
  const fake = {
    getAccountInfo: async (pubkey: PublicKey, _commitment?: any) => {
      const info = svm.getAccountInfo(pubkey.toBase58());
      if (!info) return null;
      return {
        lamports: Number(info.lamports),
        owner: new PublicKey(info.owner),
        data: info.data,
        executable: info.executable,
        rentEpoch: Number(info.rentEpoch),
      };
    },

    getMultipleAccountsInfo: async (pubkeys: PublicKey[], _commitment?: any) => {
      return pubkeys.map((pk) => {
        const info = svm.getAccountInfo(pk.toBase58());
        if (!info) return null;
        return {
          lamports: Number(info.lamports),
          owner: new PublicKey(info.owner),
          data: info.data,
          executable: info.executable,
          rentEpoch: Number(info.rentEpoch),
        };
      });
    },

    getProgramAccounts: async (programId: PublicKey, _config?: any) => {
      const accounts = svm.getProgramAccounts(programId.toBase58());
      return accounts.map((a: import("@naclac-fw/litesvm").ProgramAccountResult) => ({
        pubkey: new PublicKey(a.pubkey),
        account: {
          lamports: Number(a.lamports),
          owner: new PublicKey(a.owner),
          data: a.data,
          executable: a.executable,
          rentEpoch: Number(a.rentEpoch),
        },
      }));
    },

    getTokenAccountBalance: async (pubkey: PublicKey, _commitment?: any) => {
      const bal = svm.getTokenAccountBalance(pubkey.toBase58());
      const uiAmount = Number(bal.amount) / Math.pow(10, bal.decimals);
      return {
        context: { slot: 0 },
        value: {
          amount: bal.amount.toString(),
          decimals: bal.decimals,
          uiAmount,
          uiAmountString: uiAmount.toString(),
        },
      };
    },

    getLatestBlockhash: async (_commitment?: any) => {
      const bh = svm.getLatestBlockhash();
      // litesvm mode's confirm path never reads `lastValidBlockHeight` (it
      // skips `pollForConfirmation`, the only thing that consumes it), so
      // 0 here is inert.
      return { blockhash: bh.blockhash, lastValidBlockHeight: 0 };
    },

    simulateTransaction: async (tx: VersionedTransaction | any) => {
      const raw = Buffer.from(
        "serialize" in tx && typeof tx.serialize === "function"
          ? tx.serialize()
          : VersionedTransaction.deserialize(tx).serialize(),
      );
      const wireBytes = toNaclacWireBytes(raw);
      try {
        const result = svm.simulateTransaction(wireBytes);
        return { context: { slot: 0 }, value: { err: null, logs: result.logs, accounts: null, unitsConsumed: Number(result.computeUnitsConsumed), returnData: undefined } };
      } catch (e: any) {
        return { context: { slot: 0 }, value: { err: e?.message ?? "Simulation failed", logs: e?.logs ?? [], accounts: null, unitsConsumed: 0, returnData: undefined } };
      }
    },

    sendRawTransaction: async (rawTx: Buffer | Uint8Array | Array<number>, _opts?: any) => {
      const wireBytes = toNaclacWireBytes(Buffer.from(rawTx as any));
      const result = svm.sendTransaction(wireBytes);
      return result.signature;
    },

    // Used by `@solana/spl-token`'s `sendAndConfirmTransaction` (called
    // internally by its `createMint`/`getOrCreateAssociatedTokenAccount`/
    // `mintTo`, which `legacy/token.ts` delegates straight through to) —
    // distinct from `sendRawTransaction`: takes an *unsigned* legacy
    // `Transaction` plus its signers, not already-serialized bytes.
    sendTransaction: async (
      transaction: Transaction | VersionedTransaction,
      signersOrOptions?: Keypair[] | any,
      _options?: any,
    ) => {
      if ("version" in transaction) {
        const wireBytes = toNaclacWireBytes(Buffer.from(transaction.serialize()));
        const result = svm.sendTransaction(wireBytes);
        return result.signature;
      }
      const signers: Keypair[] = Array.isArray(signersOrOptions) ? signersOrOptions : [];
      const bh = svm.getLatestBlockhash();
      transaction.recentBlockhash = bh.blockhash;
      transaction.feePayer = transaction.feePayer ?? signers[0]?.publicKey;
      transaction.sign(...signers);
      const result = svm.sendTransaction(Buffer.from(transaction.serialize()));
      return result.signature;
    },

    getMinimumBalanceForRentExemption: async (dataLength: number, _commitment?: any) => {
      return Number(svm.getMinimumBalanceForRentExemption(BigInt(dataLength)));
    },

    getTransaction: async (signature: string, _config?: any) => {
      const result = svm.getTransaction(signature);
      return {
        meta: {
          err: null,
          logMessages: result.logs,
          computeUnitsConsumed: Number(result.computeUnitsConsumed),
          // The native binding resolves account keys itself (litesvm keeps
          // only compiled instructions), so `.rpc()` reads this ready-made
          // list instead of resolving `innerInstructions` against a message.
          // Absent means the binding couldn't supply them.
          naclacInnerInstructions: result.innerInstructions?.map((call) => ({
            outerIndex: call.outerIndex,
            stackHeight: call.stackHeight,
            programId: call.programId,
            accounts: call.accounts,
            data: new Uint8Array(call.data),
          })),
        },
      } as any;
    },

    // `sendRawTransaction` already executed and finalized the transaction
    // synchronously by the time this could be called — nothing left to
    // wait for, so this is a no-op success (used by `transferSol`).
    confirmTransaction: async (_strategyOrSignature: any, _commitment?: any) => {
      return { context: { slot: 0 }, value: { err: null } };
    },

    onLogs: (programId: PublicKey, callback: (logs: any, ctx: { slot: number }) => void, _commitment?: any) => {
      return svm.addLogsListener(
        programId.toBase58(),
        (_pid: string, logs: string[], signature: string, slot: number) => {
          callback({ err: null, logs, signature }, { slot });
        },
      );
    },

    removeOnLogsListener: async (listenerId: number) => {
      svm.removeLogsListener(listenerId);
    },
  };

  return fake as unknown as Connection;
}

export interface LiteSvmLegacyProvider extends LegacyProvider {
  /** Present only on a litesvm-backed provider — its presence is how the
   * legacy `LegacyProgram`/`LegacyMethodsBuilder` detect litesvm mode and
   * take the synchronous-confirm / emulated-event-listener paths instead
   * of the real-cluster ones. */
  litesvm: import("@naclac-fw/litesvm").LiteSvm;
}

/**
 * Creates a `LegacyProvider` backed by an in-process litesvm instance
 * instead of a real cluster — auto-loads this workspace's own program(s)
 * from `artifacts/deploy/` (same as the Rust `NaclacProvider::new("litesvm", ...)`),
 * with `addProgram`/`addProgramFromCluster` available on `.litesvm` for
 * anything external.
 *
 * Synchronous on purpose (`require`, not a dynamic `import()`) — unlike
 * kit's `TransactionSigner` (WebCrypto-backed, generally non-extractable),
 * a web3.js `Keypair`'s `secretKey` is a plain, directly-readable 64-byte
 * array, so this plugs directly into the generated client's synchronous
 * constructor via the `"litesvm"` cluster string, the same way
 * `"devnet"`/`"localnet"`/etc. already do.
 *
 * If `payer` is given, its exact bytes fund litesvm's ledger — no
 * assumption that some other file matches it. Without one, falls back to
 * reading `walletPath` (default `~/.config/solana/id.json`), same as
 * `loadNodeWallet`.
 */
export function createLiteSvmProvider(payer?: Keypair, walletPath?: string): LiteSvmLegacyProvider {
  const { LiteSvm } = require("@naclac-fw/litesvm");
  const keypair = payer ?? loadNodeWalletKeypair(walletPath).keypair;
  const bytes = keypair.secretKey;

  const svm = new LiteSvm(Buffer.from(bytes));
  const connection = wrapNativeConnection(svm);

  return {
    connection,
    publicKey: keypair.publicKey,
    payer: keypair,
    commitment: "confirmed",
    defaultTxVersion: "v1",
    litesvm: svm,
    getBalance: async (addr) => svm.getBalance(addr.toString()),
    getTokenBalance: async (addr) => {
      const bal = svm.getTokenAccountBalance(addr.toString());
      return Number(bal.amount) / Math.pow(10, bal.decimals);
    },
  };
}
