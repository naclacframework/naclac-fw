import { getBase64EncodedWireTransaction, type Address, type TransactionSigner } from "@solana/kit";
import type { NaclacProvider } from "./provider";

/**
 * Reads the same raw 64-byte `[secret(32) || public(32)]` keypair JSON
 * array `loadNodeWallet` does, but returns the bytes themselves instead of
 * an opaque `TransactionSigner` — litesvm's native binding needs the raw
 * bytes to construct a `solana_keypair::Keypair` on the Rust side, and a
 * `TransactionSigner` (often WebCrypto/`CryptoKeyPair`-backed) doesn't
 * generally expose its private key material back out once constructed.
 */
function loadNodeWalletBytes(path?: string): Uint8Array {
  let fs: any, os: any;
  try {
    fs = require("fs");
    os = require("os");
  } catch (e) {
    throw new Error("[Naclac] loadNodeWalletBytes can only be used in Node.js environments.");
  }
  const defaultPath = `${os.homedir()}/.config/solana/id.json`;
  return new Uint8Array(JSON.parse(fs.readFileSync(path ?? defaultPath, "utf8")));
}

/**
 * Wraps a `@naclac-fw/litesvm` `LiteSvm` instance's methods in Kit's own
 * `.method(...).send()` chainable shape, so `methods.ts`/`setup.ts`'s
 * existing `rpc.<method>(...).send()` call sites work completely unchanged
 * against litesvm — no litesvm-specific branching needed for reads, account
 * fetches, or simulate/send themselves. The only place that DOES need to
 * know it's talking to litesvm is `methods.ts`'s confirm step (litesvm
 * executes synchronously — there is nothing to wait for) and
 * `program.ts`'s event-listener path (no real websocket to subscribe to) —
 * both detect litesvm via `provider.litesvm`, the raw native instance this
 * module also exposes on the returned provider.
 */
function wrapNativeRpc(svm: import("@naclac-fw/litesvm").LiteSvm) {
  const send = <T>(fn: () => T) => ({ send: async () => fn() });

  return {
    getAccountInfo: (addr: Address, _opts?: { commitment?: string }) =>
      send(() => {
        const info = svm.getAccountInfo(addr.toString());
        if (!info) return { value: null };
        return {
          value: {
            lamports: info.lamports,
            owner: info.owner,
            data: [info.data.toString("base64"), "base64"] as [string, string],
            executable: info.executable,
            rentEpoch: info.rentEpoch,
          },
        };
      }),

    getMultipleAccounts: (addrs: Address[], _opts?: { commitment?: string }) =>
      send(() => ({
        value: addrs.map((addr) => {
          const info = svm.getAccountInfo(addr.toString());
          if (!info) return null;
          return {
            lamports: info.lamports,
            owner: info.owner,
            data: [info.data.toString("base64"), "base64"] as [string, string],
            executable: info.executable,
            rentEpoch: info.rentEpoch,
          };
        }),
      })),

    getProgramAccounts: (programId: Address, _opts?: { commitment?: string; filters?: unknown[] }) =>
      send(() =>
        svm.getProgramAccounts(programId.toString()).map((a: import("@naclac-fw/litesvm").ProgramAccountResult) => ({
          pubkey: a.pubkey,
          account: {
            data: [a.data.toString("base64"), "base64"] as [string, string],
            lamports: a.lamports,
            owner: a.owner,
            executable: a.executable,
            rentEpoch: a.rentEpoch,
          },
        })),
      ),

    getTokenAccountBalance: (addr: Address) =>
      send(() => {
        const bal = svm.getTokenAccountBalance(addr.toString());
        return {
          value: {
            amount: bal.amount.toString(),
            decimals: bal.decimals,
            uiAmount: Number(bal.amount) / Math.pow(10, bal.decimals),
            uiAmountString: (Number(bal.amount) / Math.pow(10, bal.decimals)).toString(),
          },
        };
      }),

    getMinimumBalanceForRentExemption: (size: bigint) =>
      send(() => svm.getMinimumBalanceForRentExemption(size)),

    getLatestBlockhash: (_opts?: { commitment?: string }) =>
      send(() => {
        const bh = svm.getLatestBlockhash();
        // litesvm doesn't track a meaningful block height — litesvm mode's
        // confirm path (methods.ts) never reads this, only real-cluster
        // confirmation polling does, so 0n here is inert, not a lie anyone acts on.
        return { value: { blockhash: bh.blockhash, lastValidBlockHeight: 0n } };
      }),

    simulateTransaction: (base64Tx: string, _opts?: { encoding?: string }) =>
      send(() => {
        try {
          const result = svm.simulateTransaction(Buffer.from(base64Tx, "base64"));
          return { value: { err: null, logs: result.logs } };
        } catch (e: any) {
          return { value: { err: e?.message ?? "Simulation failed", logs: e?.logs ?? [] } };
        }
      }),

    sendTransaction: (
      base64Tx: string,
      _opts?: { encoding?: string; preflightCommitment?: string; skipPreflight?: boolean },
    ) =>
      send(() => {
        const result = svm.sendTransaction(Buffer.from(base64Tx, "base64"));
        return result.signature;
      }),

    getTransaction: (
      signature: string,
      _opts?: { commitment?: string; maxSupportedTransactionVersion?: number },
    ) =>
      send(() => {
        const result = svm.getTransaction(signature);
        return {
          meta: {
            logMessages: result.logs,
            computeUnitsConsumed: result.computeUnitsConsumed,
          },
        };
      }),
  };
}

export interface LiteSvmProvider extends NaclacProvider {
  /** The raw native litesvm instance — present only on a litesvm-backed
   * provider. `methods.ts`/`program.ts` check this field to detect litesvm
   * mode and take the synchronous-confirm / emulated-event-listener paths
   * instead of the real-cluster ones. */
  litesvm: import("@naclac-fw/litesvm").LiteSvm;
}

/**
 * Creates a `NaclacProvider` backed by an in-process litesvm instance
 * instead of a real cluster — auto-loads this workspace's own program(s)
 * from `target/deploy/` (same as the Rust `NaclacProvider::new("litesvm", ...)`),
 * with `addProgram`/`addProgramFromCluster` available on `.litesvm` for
 * anything external.
 *
 * Synchronous on purpose (`require`, not a dynamic `import()`), so this
 * plugs directly into the generated client's synchronous constructor via
 * the `"litesvm"` cluster string, the same way `"devnet"`/`"localnet"`/etc.
 * already do — `signer` is the already-resolved payer the caller passes in
 * exactly like every other cluster already requires (e.g.
 * `payer = await loadNodeWallet(); new XClient("localnet", payer)`), so
 * there is no signer construction left to do here, async or otherwise.
 *
 * `signer` is used as `provider.signer` (what actually signs transactions);
 * separately, `walletPath` (defaulting to the same `~/.config/solana/id.json`
 * `loadNodeWallet` reads) is read again, synchronously, purely to fund
 * litesvm's ledger via the native binding's constructor. These two MUST
 * resolve to the same underlying keypair (the default: call
 * `loadNodeWallet()` for `signer` and leave `walletPath` unset) — if they
 * don't, transactions signed by `signer` will fail with insufficient funds,
 * since litesvm only auto-funds whichever identity its own constructor was
 * given.
 */
export function createLiteSvmProvider(signer: TransactionSigner, walletPath?: string): LiteSvmProvider {
  const { LiteSvm } = require("@naclac-fw/litesvm");
  const payerBytes = loadNodeWalletBytes(walletPath);

  const svm = new LiteSvm(Buffer.from(payerBytes));
  const rpc = wrapNativeRpc(svm);

  return {
    rpc: rpc as any,
    rpcSubscriptions: undefined as any,
    signer,
    commitment: "confirmed",
    litesvm: svm,
    // `client/src/kit/token.ts`'s `getSendAndConfirm` checks this before
    // falling back to `sendAndConfirmTransactionFactory` (which needs a
    // real websocket `rpcSubscriptions` litesvm doesn't have) — same
    // synchronous-execution bypass as `.rpc()`'s, just reached through a
    // different existing hook instead of a new litesvm-mode branch.
    _sendAndConfirm: async (tx: any) => {
      const base64Tx = getBase64EncodedWireTransaction(tx);
      return svm.sendTransaction(Buffer.from(base64Tx, "base64")).signature;
    },
    getBalance: async (addr) => svm.getBalance(addr.toString()),
    getTokenBalance: async (addr) => {
      const bal = svm.getTokenAccountBalance(addr.toString());
      return Number(bal.amount) / Math.pow(10, bal.decimals);
    },
  };
}
