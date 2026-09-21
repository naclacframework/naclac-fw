import type { NaclacIdl } from "../idl";
import type { NaclacProvider } from "./provider";
import { MethodsBuilder } from "./methods";
import { AccountFetcher } from "./account";
import { address, type Address } from "@solana/kit";

import { getDiscriminator } from "../utils/hash";
import { resolvePdas } from "../utils/pda";
import { tryDecodeEventLog, parseEventsFromLogs } from "../events";

export type IdlInstructionName<T extends NaclacIdl> =
  T["instructions"][number]["name"];

export type IdlAccountName<T extends NaclacIdl> = T["accounts"][number]["name"];

/**
 * Program<TIdl> — the main entry point for every Naclac SDK using @solana/kit.
 */
export class Program<TIdl extends NaclacIdl = NaclacIdl> {
  readonly idl: TIdl;
  readonly provider: NaclacProvider;
  /** See the constructor's own doc comment — a codegen-time-only fact, never part of `idl`. */
  readonly isZeroCopy: boolean;

  readonly methods: {
    [Name in IdlInstructionName<TIdl>]: (
      args?: Record<string, unknown>,
    ) => MethodsBuilder;
  };

  readonly account: {
    [Name in IdlAccountName<TIdl>]: AccountFetcher;
  };

  private _listenerIdCounter = 0;
  private _eventListeners: Map<number, AbortController | { nativeListenerId: number }> = new Map();

  get constants(): Record<string, any> {
    const constants: Record<string, any> = {};
    for (const c of this.idl.constants) {
      constants[c.name] = c.value;
    }
    return constants;
  }

  async pda(
    accountName: string,
    seeds: Record<string, any> = {},
    instructionName?: string,
  ): Promise<Address> {
    const tryResolve = async (ix: any): Promise<Address | null> => {
      const acct = ix.accounts.find(
        (a: any) => a.name === accountName && a.pda,
      );
      if (!acct) return null;

      const args: Record<string, unknown> = {};
      const accounts: Record<string, Address> = {};
      for (const seed of acct.pda.seeds as any[]) {
        const val = seeds[seed.path];
        if (val === undefined) continue;
        if (seed.kind === "account") {
          accounts[seed.path] = val as Address;
        } else if (seed.kind === "arg") {
          args[seed.path] = val;
        }
      }

      const resolved = await resolvePdas(
        address(this.idl.address),
        ix,
        args,
        accounts,
        accountName,
        this.idl.definedTypes,
        this.isZeroCopy,
      );
      return resolved[accountName] ?? null;
    };

    if (instructionName) {
      const ix = (this.idl.instructions as any[]).find(
        (i) => i.name === instructionName,
      );
      if (ix) {
        const result = await tryResolve(ix);
        if (result) return result;
      }
    }

    for (const ix of this.idl.instructions as any[]) {
      const result = await tryResolve(ix);
      if (result) return result;
    }

    throw new Error(
      `[Naclac] No PDA definition found for account "${accountName}" in IDL instructions.`,
    );
  }

  async getAccountInfo(address: string | Address) {
    const { value } = await this.provider.rpc
      .getAccountInfo(address as Address, {
        commitment: this.provider.commitment ?? "confirmed",
        encoding: "base64",
      })
      .send();
    return value;
  }

  /**
   * `isZeroCopy` is a generation-time fact (was this program built with the
   * `pinocchio` feature, or without `borsh`?), never part of the IDL itself
   * — the generated `client.ts` passes it as a literal here, the same way
   * `naclac-client-gen`'s own `idl.is_zero_copy` only ever conditions what
   * source code gets written (Cargo feature defaults, `cfg_attr` branches),
   * never round-tripped through IDL data. Determines whether a
   * `definedTypes` struct/enum's wire layout is real C-layout-with-padding
   * or tightly-packed Borsh — see `client/src/coder/types.ts`'s
   * `getIdlCodec`.
   */
  constructor(idl: TIdl, provider: NaclacProvider, isZeroCopy = false) {
    this.idl = idl;
    this.provider = provider;
    this.isZeroCopy = isZeroCopy;

    this.methods = {} as any;
    for (const ixDef of idl.instructions) {
      (this.methods as any)[ixDef.name] = (
        args: Record<string, unknown> = {},
      ) => {
        return new MethodsBuilder(idl, ixDef, args, provider, isZeroCopy);
      };
    }

    this.account = {} as any;
    for (const acctDef of idl.accounts) {
      (this.account as any)[acctDef.name] = new AccountFetcher(
        acctDef,
        provider,
        address(idl.address),
        idl.definedTypes,
        isZeroCopy,
      );
    }
  }

  addEventListener(
    eventName: string,
    callback: (event: any, slot: number, signature: string) => void,
  ): number {
    const eventDef = this.idl.events?.find((e) => e.name === eventName);
    if (!eventDef) {
      throw new Error(`[Naclac] Event "${eventName}" not found in IDL.`);
    }

    const discriminator = getDiscriminator("event", eventName);
    const listenerId = this._listenerIdCounter++;

    if (this.provider.litesvm) {
      // No real validator to subscribe to — the native binding fires
      // synchronously, in-process, the instant a transaction mentioning
      // this program executes via `sendTransaction`.
      const nativeListenerId = this.provider.litesvm.addLogsListener(
        this.idl.address,
        (_programId: string, logs: string[], signature: string, slot: number) => {
          for (const log of logs) {
            const decoded = tryDecodeEventLog(eventDef as any, discriminator, log, this.idl.definedTypes, this.isZeroCopy);
            if (decoded === null) continue;
            callback(decoded, slot, signature);
          }
        },
      );
      this._eventListeners.set(listenerId, { nativeListenerId });
      return listenerId;
    }

    const abortController = new AbortController();
    this._eventListeners.set(listenerId, abortController);

    const rpcSubscriptions = this.provider.rpcSubscriptions as any;
    const programId = address(this.idl.address);

    (async () => {
      try {
        const subscription = await rpcSubscriptions
          .logsSubscribe({ mentions: [programId] }, { commitment: "confirmed" })
          .subscribe({ abortSignal: abortController.signal });

        for await (const notification of subscription) {
          const logs = notification.value.logs;
          if (!logs) continue;

          for (const log of logs) {
            const decoded = tryDecodeEventLog(eventDef as any, discriminator, log, this.idl.definedTypes, this.isZeroCopy);
            if (decoded === null) continue;
            callback(
              decoded,
              notification.context.slot,
              notification.value.signature,
            );
          }
        }
      } catch (err: any) {
        if (err.name === "AbortError") return;
        console.error(
          `[Naclac] Subscription error for event ${eventName}:`,
          err,
        );
      }
    })();

    return listenerId;
  }

  removeEventListener(listenerId: number): void {
    const entry = this._eventListeners.get(listenerId);
    if (!entry) return;
    if (entry instanceof AbortController) {
      entry.abort();
    } else if (this.provider.litesvm) {
      this.provider.litesvm.removeLogsListener(entry.nativeListenerId);
    }
    this._eventListeners.delete(listenerId);
  }

  /**
   * Decodes every occurrence of `eventName` found in an already-fetched list
   * of transaction log lines (e.g. from `.rpc()`'s returned `logs`). Prefer
   * this (or `.rpc()`'s own `parseEvents()`) over `waitForEvent`/
   * `addEventListener` when you already know which transaction you care
   * about — it's race-free, since it reads logs of a transaction that has
   * already confirmed rather than racing a live subscription against it.
   * Reserve `waitForEvent`/`addEventListener` for reacting to events across
   * future transactions whose signatures you don't know yet (an indexer, a bot, ...).
   */
  parseEvents<T = any>(eventName: string, logs: readonly string[]): T[] {
    return parseEventsFromLogs<T>(this.idl, eventName, logs, this.isZeroCopy);
  }

  async waitForEvent<T = any>(
    eventName: string,
    options: { timeoutMs?: number } = {},
  ): Promise<T | null> {
    const timeoutMs = options.timeoutMs ?? 10000;
    return new Promise((resolve) => {
      let resolved = false;
      const listenerId = this.addEventListener(eventName, (event) => {
        if (!resolved) {
          resolved = true;
          this.removeEventListener(listenerId);
          resolve(event);
        }
      });

      setTimeout(() => {
        if (!resolved) {
          resolved = true;
          this.removeEventListener(listenerId);
          resolve(null);
        }
      }, timeoutMs);
    });
  }
}
