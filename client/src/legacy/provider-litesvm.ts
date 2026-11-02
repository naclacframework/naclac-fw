import { Connection, PublicKey, Keypair, type Blockhash } from "@solana/web3.js";
import type { LegacyProvider } from "./provider";

type LiteSvmAccount = {
  lamports: bigint;
  owner: string;
  data: Uint8Array;
  executable: boolean;
  rentEpoch: bigint;
};

function toAccountInfo(info: LiteSvmAccount) {
  return {
    lamports: info.lamports,
    owner: new PublicKey(info.owner),
    data: info.data,
    executable: info.executable,
    rentEpoch: info.rentEpoch,
    space: BigInt(info.data.length),
  };
}

/**
 * A `Connection`-shaped object backed by a `@naclac-fw/litesvm` `LiteSvm`
 * instance, implementing only the methods `LegacyAccountFetcher`/
 * `LegacyMethodsBuilder`/`LegacyProgram` actually call — not a real
 * `Connection` subclass (no benefit to one here, since none of its other
 * behavior is needed), just enough surface to satisfy those call sites.
 * Cast to `Connection` at the boundary, so every returned shape must be kept
 * in step with the real `Connection`'s by hand.
 */
function wrapNativeConnection(svm: import("@naclac-fw/litesvm").LiteSvm): Connection {
  const fake = {
    getAccountInfo: async (pubkey: PublicKey, _commitment?: any) => {
      const info = svm.getAccountInfo(pubkey.toBase58());
      return info ? toAccountInfo(info) : null;
    },

    getMultipleAccountsInfo: async (pubkeys: PublicKey[], _commitment?: any) => {
      return pubkeys.map((pk) => {
        const info = svm.getAccountInfo(pk.toBase58());
        return info ? toAccountInfo(info) : null;
      });
    },

    getProgramAccounts: async (programId: PublicKey, _config?: any) => {
      const accounts = svm.getProgramAccounts(programId.toBase58());
      return accounts.map((a: import("@naclac-fw/litesvm").ProgramAccountResult) => ({
        pubkey: new PublicKey(a.pubkey),
        account: toAccountInfo(a),
      }));
    },

    getTokenAccountBalance: async (pubkey: PublicKey, _commitment?: any) => {
      const bal = svm.getTokenAccountBalance(pubkey.toBase58());
      const uiAmount = Number(bal.amount) / Math.pow(10, bal.decimals);
      return {
        context: { slot: 0n },
        value: {
          amount: bal.amount.toString(),
          decimals: bal.decimals,
          uiAmount,
          uiAmountString: uiAmount.toString(),
        },
      };
    },

    // litesvm has no block height; `sendAndConfirmSigned` never polls a litesvm provider.
    getLatestBlockhash: async (_commitment?: any) => {
      const bh = svm.getLatestBlockhash();
      return { blockhash: bh.blockhash as Blockhash, lastValidBlockHeight: 0n };
    },

    simulateTransaction: async (tx: { serialize: () => Uint8Array | Promise<Uint8Array> }) => {
      const wireBytes = Buffer.from(await tx.serialize());
      try {
        const result = svm.simulateTransaction(wireBytes);
        return { context: { slot: 0n }, value: { err: null, logs: result.logs, accounts: null, unitsConsumed: BigInt(result.computeUnitsConsumed), returnData: undefined } };
      } catch (e: any) {
        return { context: { slot: 0n }, value: { err: e?.message ?? "Simulation failed", logs: e?.logs ?? [], accounts: null, unitsConsumed: 0n, returnData: undefined } };
      }
    },

    sendRawTransaction: async (rawTx: Uint8Array | Array<number>, _opts?: any) => {
      const result = svm.sendTransaction(Buffer.from(rawTx));
      return result.signature;
    },

    getMinimumBalanceForRentExemption: async (dataLength: number, _commitment?: any) => {
      return svm.getMinimumBalanceForRentExemption(BigInt(dataLength));
    },

    getTransaction: async (signature: string, _config?: any) => {
      const result = svm.getTransaction(signature);
      return {
        meta: {
          err: null,
          logMessages: result.logs,
          computeUnitsConsumed: BigInt(result.computeUnitsConsumed),
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

    onLogs: (programId: PublicKey, callback: (logs: any, ctx: { slot: bigint }) => void, _commitment?: any) => {
      return svm.addLogsListener(
        programId.toBase58(),
        (_pid: string, logs: string[], signature: string, slot: number) => {
          callback({ err: null, logs, signature }, { slot: BigInt(slot) });
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
 * Synchronous on purpose, so it plugs into the generated client's
 * synchronous constructor via the `"litesvm"` cluster string. That is why
 * `payer` must already be a resolved `Keypair`: litesvm's ledger is funded
 * from its exact secret-key bytes.
 */
export function createLiteSvmProvider(payer: Keypair): LiteSvmLegacyProvider {
  const { LiteSvm } = require("@naclac-fw/litesvm");
  const svm = new LiteSvm(Buffer.from(payer.secretKey));
  const connection = wrapNativeConnection(svm);

  return {
    connection,
    publicKey: payer.publicKey,
    payer,
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
