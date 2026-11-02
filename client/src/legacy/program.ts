import {
  PublicKey,
  Transaction,
  TransactionInstruction,
  Keypair,
  AddressLookupTableAccount,
  VersionedTransaction,
  type BlockhashWithExpiryBlockHeight,
} from "@solana/web3.js";
import { getBase58Codec, getBase64Codec } from "@solana/codecs";
import type { NaclacIdl, IdlInstruction, IdlAccountDef } from "../idl";
import { type LegacyProvider, type TxVersion, normalizeTxVersion } from "./provider";
import {
  compileProviderTransaction,
  requireFeePayer,
  sendAndConfirmSigned,
  signProviderTransaction,
  v1ConfigFor,
} from "./transaction";
import { encodeInstructionData, decodeDiscriminatedAccountData } from "../coder/instruction";
import { pinnedAddress, resolvePdas } from "../utils/pda";
import { resolveSlots } from "../slots";
import { translateRpcError } from "../utils/rpc";
import { resolveDiscriminator } from "../utils/hash";
import { toCamelCase } from "../utils/case";
import {
  tryDecodeEventLog,
  parseEventsFromLogs,
  parseEventsFromTransaction,
  resolveRpcInnerInstructions,
  eventFieldLayout,
  type NaclacInnerInstruction,
  type NaclacRpcResult,
} from "../events";

export type IdlInstructionName<T extends NaclacIdl> =
  T["instructions"][number]["name"];

export type IdlAccountName<T extends NaclacIdl> = T["accounts"][number]["name"];

/**
 * Inner instructions of a web3.js `getTransaction` result (fetched with
 * `maxSupportedTransactionVersion: 1`), with every account index resolved to
 * a base58 address — including accounts loaded through address lookup
 * tables, so it also serves a v0 transaction the caller sent themselves.
 * `undefined` when the node omitted them or the transaction failed (a failed
 * transaction's inner instructions were never committed, so none of them may
 * vouch for an event). Feed the result to a generated client's
 * `parseXxxEvents(logs, innerInstructions)`.
 */
export function innerInstructionsFromLegacyTransaction(
  txResult: { transaction: { message: any }; meta?: any } | null | undefined,
): NaclacInnerInstruction[] | undefined {
  if (!txResult?.meta || txResult.meta.err) return undefined;
  const meta = txResult.meta;
  if (meta.naclacInnerInstructions) {
    // litesvm mode: the native binding already resolved account keys.
    return meta.naclacInnerInstructions;
  }
  const keys: string[] = txResult.transaction.message
    .getAccountKeys({ accountKeysFromLookups: meta.loadedAddresses })
    .keySegments()
    .flat()
    .map((key: PublicKey) => key.toBase58());
  return resolveRpcInnerInstructions(keys, meta.innerInstructions);
}

/**
 * LegacyAccountFetcher — fetches and deserializes on-chain accounts.
 */
export class LegacyAccountFetcher {
  constructor(
    private readonly accountDef: IdlAccountDef,
    private readonly provider: LegacyProvider,
    private readonly programId: PublicKey,
    private readonly definedTypes: readonly any[] = [],
    private readonly isZeroCopy: boolean = false,
  ) {}

  private discriminator(): Uint8Array {
    return resolveDiscriminator(
      this.accountDef.discriminator,
      "account",
      this.accountDef.name,
    );
  }

  private decode(rawBytes: Uint8Array): Record<string, unknown> {
    return decodeDiscriminatedAccountData(
      this.accountDef.type.fields as any,
      rawBytes,
      this.discriminator(),
      this.accountDef.name,
      this.definedTypes,
      this.isZeroCopy,
    );
  }

  async fetch(pubkey: PublicKey | string): Promise<Record<string, unknown>> {
    const addr = typeof pubkey === "string" ? new PublicKey(pubkey) : pubkey;
    const commitment = this.provider.commitment;

    const accountInfo = await this.provider.connection.getAccountInfo(addr, commitment);
    if (!accountInfo) {
      throw new Error(`[Naclac] Account "${pubkey.toString()}" does not exist on-chain.`);
    }

    return this.decode(new Uint8Array(accountInfo.data));
  }

  async fetchMultiple(
    pubkeys: Array<PublicKey | string>,
    opts: { dropCorrupted?: boolean } = {}
  ): Promise<Array<Record<string, unknown> | null>> {
    const addrs = pubkeys.map((pk) => typeof pk === "string" ? new PublicKey(pk) : pk);
    const commitment = this.provider.commitment;

    const accounts = await this.provider.connection.getMultipleAccountsInfo(addrs, commitment);
    return accounts.map((acctInfo) => {
      if (!acctInfo) return null;
      try {
        return this.decode(new Uint8Array(acctInfo.data));
      } catch (err) {
        if (opts.dropCorrupted) {
          console.warn(`[Naclac] Dropping corrupted account:`, err);
          return null;
        }
        throw err;
      }
    });
  }

  /**
   * Fetches multiple accounts in a single RPC call and returns them mapped by their PublicKey.
   * Missing accounts are returned as `null` in the map.
   *
   * @param pubkeys Array of on-chain addresses.
   * @param opts.dropCorrupted If true, corrupted accounts are silently dropped.
   */
  async fetchMultipleAsMap(
    pubkeys: Array<PublicKey | string>,
    opts: { dropCorrupted?: boolean } = {},
  ): Promise<Map<PublicKey, Record<string, unknown> | null>> {
    const results = await this.fetchMultiple(pubkeys, opts);
    const map = new Map<PublicKey, Record<string, unknown> | null>();
    pubkeys.forEach((pk, i) => {
      const pubkey = typeof pk === "string" ? new PublicKey(pk) : pk;
      map.set(pubkey, results[i]);
    });
    return map;
  }

  async all(opts: { filters?: any[]; dropCorrupted?: boolean } = {}): Promise<
    Array<{ publicKey: PublicKey; account: Record<string, unknown> }>
  > {
    const discriminator = this.discriminator();
    const base58Discriminator = getBase58Codec().decode(discriminator);
    const commitment = this.provider.commitment;

    const filters = [
      { memcmp: { offset: 0, bytes: base58Discriminator } },
      ...(opts.filters ?? []),
    ];

    const raw = await this.provider.connection.getProgramAccounts(this.programId, {
      commitment,
      filters,
    });

    const decoded = raw.map((acctInfo) => {
      try {
        const accountData = this.decode(new Uint8Array(acctInfo.account.data));
        return {
          publicKey: acctInfo.pubkey,
          account: accountData,
        };
      } catch (err) {
        if (opts.dropCorrupted) {
          console.warn(`[Naclac] Dropping corrupted account at ${acctInfo.pubkey.toString()}:`, err);
          return null;
        }
        throw err;
      }
    });

    return decoded.filter((d): d is { publicKey: PublicKey; account: Record<string, unknown> } => d !== null);
  }
}

/**
 * LegacyMethodsBuilder — the legacy Web3.js instruction pipeline.
 */
export class LegacyMethodsBuilder {
  private readonly idl: NaclacIdl;
  private readonly ixDef: IdlInstruction;
  private readonly args: Record<string, unknown>;
  private readonly provider: LegacyProvider;
  private readonly isZeroCopy: boolean;
  private readonly allocEvents: readonly string[];
  private programId: PublicKey;

  private _userAccounts: Record<string, PublicKey> = {};
  private _extraSigners: Keypair[] = [];
  private _remainingAccounts: PublicKey[] = [];
  private _preInstructions: TransactionInstruction[] = [];
  private _postInstructions: TransactionInstruction[] = [];
  private _txVersion?: TxVersion;
  private _addressLookupTableAccounts?: AddressLookupTableAccount[];
  private _computeUnitLimit?: number;
  private _loadedAccountsDataSizeLimit?: number;
  private _priorityFee?: number | bigint;

  constructor(
    idl: NaclacIdl,
    ixDef: IdlInstruction,
    args: Record<string, unknown>,
    provider: LegacyProvider,
    isZeroCopy = false,
    allocEvents: readonly string[] = [],
  ) {
    this.idl = idl;
    this.ixDef = ixDef;
    this.args = args;
    this.provider = provider;
    this.isZeroCopy = isZeroCopy;
    this.allocEvents = allocEvents;
    this.programId = new PublicKey(idl.address);
  }

  accounts(accounts: Record<string, PublicKey | string>): this {
    const camelToRaw: Record<string, string> = {};
    for (const acct of this.ixDef.accounts) {
      camelToRaw[toCamelCase(acct.name)] = acct.name;
    }
    const normalized: Record<string, PublicKey> = {};
    for (const [key, val] of Object.entries(accounts)) {
      normalized[camelToRaw[key] ?? key] = typeof val === "string" ? new PublicKey(val) : val;
    }
    this._userAccounts = { ...this._userAccounts, ...normalized };
    return this;
  }

  signers(signers: Keypair[]): this {
    this._extraSigners.push(...signers);
    return this;
  }

  remainingAccounts(addresses: (PublicKey | string)[]): this {
    this._remainingAccounts.push(...addresses.map((a) => typeof a === "string" ? new PublicKey(a) : a));
    return this;
  }

  preInstructions(ixs: TransactionInstruction[]): this {
    this._preInstructions.push(...ixs);
    return this;
  }

  postInstructions(ixs: TransactionInstruction[]): this {
    this._postInstructions.push(...ixs);
    return this;
  }

  v1(): this {
    this._txVersion = "v1";
    return this;
  }

  v0(addressLookupTableAccounts?: AddressLookupTableAccount[]): this {
    this._txVersion = "v0";
    this._addressLookupTableAccounts = addressLookupTableAccounts;
    return this;
  }

  legacy(): this {
    this._txVersion = "legacy";
    return this;
  }

  version(version: TxVersion, addressLookupTableAccounts?: AddressLookupTableAccount[]): this {
    this._txVersion = version;
    if (addressLookupTableAccounts) {
      this._addressLookupTableAccounts = addressLookupTableAccounts;
    }
    return this;
  }

  computeUnitLimit(units: number): this {
    this._computeUnitLimit = units;
    return this;
  }

  loadedAccountsDataSizeLimit(bytes: number): this {
    this._loadedAccountsDataSizeLimit = bytes;
    return this;
  }

  priorityFee(lamports: number | bigint): this {
    this._priorityFee = lamports;
    return this;
  }

  private async _buildInstructionMetasAndData() {
    const userAccountsStr: Record<string, string> = {};
    for (const [k, v] of Object.entries(this._userAccounts)) {
      userAccountsStr[k] = v.toString();
    }

    const pdaAccounts = await resolvePdas(
      this.programId.toString() as any,
      this.ixDef,
      this.args,
      userAccountsStr as any,
      undefined,
      this.idl.definedTypes,
      this.isZeroCopy,
    );

    const allAccounts: Record<string, PublicKey> = {};
    for (const [k, v] of Object.entries(this._userAccounts)) {
      allAccounts[k] = v;
    }
    for (const [k, v] of Object.entries(pdaAccounts)) {
      allAccounts[k] = new PublicKey(v);
    }

    const data = encodeInstructionData(this.ixDef, this.args, this.idl.definedTypes, this.isZeroCopy);

    const accountMetas: any[] = this.ixDef.accounts.map((acctDef) => {
      let addr = allAccounts[acctDef.name];

      if (!addr) {
        const pinned = pinnedAddress(acctDef);
        if (pinned) addr = new PublicKey(pinned);
      }

      if (!addr) {
        throw new Error(
          `[Naclac] Account "${acctDef.name}" was not provided and could not be auto-resolved.`,
        );
      }

      const isWritable = (acctDef as any).writable ?? (acctDef as any).isMut ?? false;
      const isSigner = (acctDef as any).signer ?? (acctDef as any).isSigner ?? false;

      return {
        pubkey: addr,
        isWritable,
        isSigner,
      };
    });

    for (const addr of this._remainingAccounts) {
      accountMetas.push({
        pubkey: addr,
        isWritable: true,
        isSigner: false,
      });
    }

    return { data, accountMetas };
  }

  async buildInstruction(): Promise<TransactionInstruction> {
    const { data, accountMetas } = await this._buildInstructionMetasAndData();
    return new TransactionInstruction({
      keys: accountMetas,
      programId: this.programId,
      data,
    });
  }

  async instruction(): Promise<TransactionInstruction> {
    return this.buildInstruction();
  }

  private async _compile(
    txVersion: 0 | 1 | "legacy",
    latestBlockhash: BlockhashWithExpiryBlockHeight,
  ): Promise<Transaction | VersionedTransaction> {
    const mainInstruction = await this.buildInstruction();
    return compileProviderTransaction({
      instructions: [...this._preInstructions, mainInstruction, ...this._postInstructions],
      feePayer: requireFeePayer(this.provider),
      latestBlockhash,
      txVersion,
      v1Config: v1ConfigFor({
        computeUnitLimit: this._computeUnitLimit,
        loadedAccountsDataSizeLimit: this._loadedAccountsDataSizeLimit,
        priorityFee: this._priorityFee,
      }),
      addressLookupTableAccounts: this._addressLookupTableAccounts,
    });
  }

  async transaction(): Promise<Transaction | VersionedTransaction> {
    const txVersion = normalizeTxVersion(this._txVersion ?? this.provider.defaultTxVersion);
    const latestBlockhash = await this.provider.connection.getLatestBlockhash(this.provider.commitment);
    return this._compile(txVersion, latestBlockhash);
  }

  /**
   * Builds a transaction against a single fetched blockhash and signs it with
   * the provider's payer/extra signers/wallet-adapter signTransaction. Shared
   * by `simulate()`, `rpc()`, `send()`, and `encoded()`. The returned
   * `latestBlockhash` is the one the transaction carries, so expiry is
   * checked against the right `lastValidBlockHeight`.
   */
  private async _signedTransaction(): Promise<{
    signedTx: Transaction | VersionedTransaction;
    commitment: LegacyProvider["commitment"];
    latestBlockhash: BlockhashWithExpiryBlockHeight;
  }> {
    const txVersion = normalizeTxVersion(this._txVersion ?? this.provider.defaultTxVersion);
    const commitment = this.provider.commitment;
    const latestBlockhash = await this.provider.connection.getLatestBlockhash(commitment);
    const tx = await this._compile(txVersion, latestBlockhash);
    const signedTx = await signProviderTransaction(this.provider, tx, this._extraSigners);
    return { signedTx, commitment, latestBlockhash };
  }

  async simulate() {
    const connection = this.provider.connection;
    const { signedTx } = await this._signedTransaction();

    try {
      const res = signedTx instanceof VersionedTransaction
        ? await connection.simulateTransaction(signedTx)
        : await connection.simulateTransaction(signedTx);
      if (res.value.err) {
        const simError = new Error("Transaction simulation failed");
        (simError as any).context = { logs: res.value.logs };
        throw simError;
      }
      return res.value;
    } catch (err: any) {
      translateRpcError(err, this.idl.errors, this.ixDef.accounts);
      throw err;
    }
  }

  /**
   * Signs the transaction and returns the base64-encoded wire transaction.
   */
  async encoded(): Promise<string> {
    const { signedTx } = await this._signedTransaction();
    return getBase64Codec().decode(await signedTx.serialize());
  }

  /**
   * Signs and sends the transaction without waiting for confirmation.
   * Equivalent to Anchor's .send().
   */
  async send(options: { skipPreflight?: boolean } = {}): Promise<string> {
    const connection = this.provider.connection;
    const { signedTx, commitment } = await this._signedTransaction();

    try {
      return await connection.sendRawTransaction(await signedTx.serialize(), {
        skipPreflight: options.skipPreflight ?? false,
        preflightCommitment: commitment,
      });
    } catch (err: any) {
      translateRpcError(err, this.idl.errors, this.ixDef.accounts);
      throw err;
    }
  }

  /**
   * Sends the transaction to a specific RPC URL (e.g. TEE / Rollup endpoint).
   * Automatically handles signing and base64 encoding.
   *
   * @param url - The custom RPC URL (can include tokens).
   * @param opts - Options including wait for confirmation.
   */
  async sendTo(
    url: string,
    opts: { confirm?: boolean; timeoutMs?: number } = {},
  ): Promise<string> {
    const base64Tx = await this.encoded();

    const response = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "sendTransaction",
        params: [base64Tx, { encoding: "base64", skipPreflight: true }],
      }),
    });

    const data = (await response.json()) as any;
    if (data.error) {
      throw new Error(`[Naclac] Remote RPC error: ${JSON.stringify(data.error)}`);
    }

    const signature = data.result as string;

    if (opts.confirm) {
      const deadline = Date.now() + (opts.timeoutMs ?? 30000);
      while (Date.now() < deadline) {
        await new Promise((r) => setTimeout(r, 1000));
        const statusRes = await fetch(url, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            jsonrpc: "2.0",
            id: 1,
            method: "getSignatureStatuses",
            params: [[signature], { searchTransactionHistory: true }],
          }),
        });
        const statusData = (await statusRes.json()) as any;
        const status = statusData?.result?.value?.[0];

        if (status?.err) {
          const err = new Error(
            `[Naclac] tx failed on-chain: ${JSON.stringify(status.err)} | sig: ${signature}`,
          );
          translateRpcError(err, this.idl.errors, this.ixDef.accounts);
          throw err;
        }
        if (
          status?.confirmationStatus === "confirmed" ||
          status?.confirmationStatus === "finalized"
        ) {
          return signature;
        }
      }
      throw new Error(
        `[Naclac] Transaction confirmation timed out after ${opts.timeoutMs ?? 30000}ms`,
      );
    }

    return signature;
  }

  async pubkeys(): Promise<Record<string, PublicKey>> {
    const { accountMetas } = await this._buildInstructionMetasAndData();
    const result: Record<string, PublicKey> = {};
    this.ixDef.accounts.forEach((acct, i) => {
      result[toCamelCase(acct.name)] = accountMetas[i].pubkey;
    });
    return result;
  }

  /** The program's `event_authority` PDA (base58) that signs `emit_cpi!` self-CPIs. */
  private async eventAuthority(): Promise<string> {
    const [pda] = await PublicKey.findProgramAddress(
      [new TextEncoder().encode("__event_authority")],
      this.programId,
    );
    return pda.toBase58();
  }

  async rpc(): Promise<NaclacRpcResult> {
    const connection = this.provider.connection;
    const { signedTx, commitment, latestBlockhash } = await this._signedTransaction();

    let signature: string;
    try {
      signature = await sendAndConfirmSigned(this.provider, signedTx, latestBlockhash.lastValidBlockHeight);
    } catch (err: any) {
      translateRpcError(err, this.idl.errors, this.ixDef.accounts);
      throw err;
    }

    // Fetch the now-confirmed transaction's logs directly — no race, since
    // this transaction has already landed and its logs already exist (unlike
    // `connection.onLogs(...)`, which can miss events broadcast before the
    // subscription handshake finishes connecting).
    let logs: string[] = [];
    let innerInstructions: NaclacInnerInstruction[] | undefined;
    try {
      const txResult = await connection.getTransaction(signature, {
        commitment: commitment === "finalized" ? "finalized" : "confirmed",
        maxSupportedTransactionVersion: 1,
      });
      logs = [...(txResult?.meta?.logMessages ?? [])];
      innerInstructions = innerInstructionsFromLegacyTransaction(txResult);
    } catch {
      // Logs are a best-effort convenience; a failure fetching them shouldn't
      // fail an already-confirmed transaction.
    }

    const eventAuthority = await this.eventAuthority();
    return {
      signature,
      logs,
      innerInstructions,
      parseEvents: <T = any>(eventName: string) =>
        parseEventsFromTransaction<T>(
          this.idl,
          eventName,
          { logs, innerInstructions, eventAuthority },
          eventFieldLayout(this.isZeroCopy, this.allocEvents, eventName),
        ),
    };
  }
}

/**
 * LegacyProgram<TIdl> — the legacy Web3.js OOP client.
 */
export class LegacyProgram<TIdl extends NaclacIdl = NaclacIdl> {
  readonly idl: TIdl;
  readonly provider: LegacyProvider;
  readonly programId: PublicKey;
  /** True when the IDL's `metadata.serialization` is `bytemuck`. */
  readonly isZeroCopy: boolean;
  /** Names of this program's `alloc` events. */
  readonly allocEvents: readonly string[];

  readonly methods: {
    [Name in IdlInstructionName<TIdl>]: (
      args?: Record<string, unknown>,
    ) => LegacyMethodsBuilder;
  };

  readonly account: {
    [Name in IdlAccountName<TIdl>]: LegacyAccountFetcher;
  };

  private _eventListeners: Map<number, number> = new Map();

  get constants(): Record<string, any> {
    const constants: Record<string, any> = {};
    for (const c of this.idl.constants) {
      constants[c.name] = c.value;
    }
    return constants;
  }

  /**
   * Derive a PDA for a given account name using the instruction definitions in the IDL.
   *
   * Accepts a flat `seeds` object whose keys match seed paths from the IDL.
   * Internally splits them into `args` and `accounts` maps as required by resolvePdas.
   *
   * @param accountName     - The camelCase name of the account in the IDL.
   * @param seeds           - Flat map of every seed value keyed by its IDL path.
   * @param instructionName - Optional: pin to a specific instruction's seed schema.
   */
  async pda(
    accountName: string,
    seeds: Record<string, any> = {},
    instructionName?: string,
  ): Promise<PublicKey> {
    const tryResolve = async (ix: any): Promise<PublicKey | null> => {
      const acct = ix.accounts.find(
        (a: any) => a.name === accountName && a.pda,
      );
      if (!acct) return null;

      const args: Record<string, unknown> = {};
      const accounts: Record<string, string> = {};
      for (const seed of acct.pda.seeds as any[]) {
        const val = seeds[seed.path];
        if (val === undefined) continue;
        if (seed.kind === "account") {
          accounts[seed.path] = val.toString();
        } else if (seed.kind === "arg") {
          args[seed.path] = val;
        }
      }

      const resolved = await resolvePdas(
        this.programId.toString() as any,
        ix,
        args,
        accounts as any,
        accountName,
        this.idl.definedTypes,
        this.isZeroCopy,
      );
      return resolved[accountName] ? new PublicKey(resolved[accountName]) : null;
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

  /**
   * The wire layout comes from the IDL itself — see `Program`'s (the
   * @solana/kit equivalent) constructor doc comment.
   */
  constructor(rawIdl: TIdl, provider: LegacyProvider) {
    const idl = resolveSlots(rawIdl);
    this.idl = idl;
    this.provider = provider;
    this.isZeroCopy = idl.metadata.serialization === "bytemuck";
    const isZeroCopy = this.isZeroCopy;
    this.allocEvents = idl.events.filter((e) => e.alloc).map((e) => e.name);
    const allocEvents = this.allocEvents;
    this.programId = new PublicKey(idl.address);

    this.methods = {} as any;
    for (const ixDef of idl.instructions) {
      (this.methods as any)[ixDef.name] = (
        args: Record<string, unknown> = {},
      ) => {
        return new LegacyMethodsBuilder(idl, ixDef, args, provider, isZeroCopy, allocEvents);
      };
    }

    this.account = {} as any;
    for (const acctDef of idl.accounts) {
      (this.account as any)[acctDef.name] = new LegacyAccountFetcher(
        acctDef,
        provider,
        this.programId,
        idl.definedTypes,
        isZeroCopy,
      );
    }
  }

  /**
   * Calls `callback` for every `eventName` event a live log subscription sees.
   * Only events emitted with `emit!` reach it: an `emit_cpi!` event is in the
   * transaction's inner instructions, not its logs — read that one from
   * `.rpc()`'s `parseEvents()` instead.
   */
  addEventListener(
    eventName: string,
    callback: (event: any, slot: bigint, signature: string) => void,
  ): number {
    const eventDef = this.idl.events?.find((e) => e.name === eventName);
    if (!eventDef) {
      throw new Error(`[Naclac] Event "${eventName}" not found in IDL.`);
    }

    const discriminator = resolveDiscriminator(eventDef.discriminator, "event", eventName);
    const connection = this.provider.connection;
    const commitment = this.provider.commitment;

    const listenerId = connection.onLogs(
      this.programId,
      (logs, ctx) => {
        if (logs.err) return;
        for (const log of logs.logs) {
          const decoded = tryDecodeEventLog(
            eventDef as any,
            discriminator,
            log,
            this.idl.definedTypes,
            eventFieldLayout(this.isZeroCopy, this.allocEvents, eventName),
          );
          if (decoded === null) continue;
          callback(decoded, ctx.slot, logs.signature);
        }
      },
      commitment
    );

    this._eventListeners.set(listenerId, listenerId);
    return listenerId;
  }

  /**
   * Decodes every occurrence of `eventName` found in an already-fetched list
   * of transaction log lines (e.g. from `.rpc()`'s returned `logs`). Prefer
   * this (or `.rpc()`'s own `parseEvents()`) over `waitForEvent`/
   * `addEventListener` when you already know which transaction you care
   * about — it's race-free, since it reads logs of a transaction that has
   * already confirmed rather than racing `connection.onLogs(...)` against it.
   * Reserve `waitForEvent`/`addEventListener` for reacting to events across
   * future transactions whose signatures you don't know yet (an indexer, a bot, ...).
   */
  parseEvents<T = any>(eventName: string, logs: readonly string[]): T[] {
    return parseEventsFromLogs<T>(
      this.idl,
      eventName,
      logs,
      eventFieldLayout(this.isZeroCopy, this.allocEvents, eventName),
    );
  }

  /**
   * Like `parseEvents`, but also decodes events the program emitted as
   * self-CPIs (`emit_cpi!`), which never appear in the logs. Async because
   * the program's `event_authority` PDA has to be derived. Pass the inner
   * instructions of a *successful* transaction, e.g. `.rpc()`'s returned
   * `innerInstructions`.
   */
  async parseEventsFromTransaction<T = any>(
    eventName: string,
    tx: {
      logs: readonly string[];
      innerInstructions?: readonly NaclacInnerInstruction[];
    },
  ): Promise<T[]> {
    const [pda] = await PublicKey.findProgramAddress(
      [new TextEncoder().encode("__event_authority")],
      this.programId,
    );
    return parseEventsFromTransaction<T>(
      this.idl,
      eventName,
      { ...tx, eventAuthority: pda.toBase58() },
      eventFieldLayout(this.isZeroCopy, this.allocEvents, eventName),
    );
  }

  removeEventListener(listenerId: number): void {
    if (this._eventListeners.has(listenerId)) {
      this.provider.connection.removeOnLogsListener(listenerId).catch(() => {});
      this._eventListeners.delete(listenerId);
    }
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

  async getAccountInfo(address: string | PublicKey) {
    const addr = typeof address === "string" ? new PublicKey(address) : address;
    const info = await this.provider.connection.getAccountInfo(addr, this.provider.commitment);
    if (!info) return null;
    return {
      ...info,
      data: [getBase64Codec().decode(info.data), "base64"] as [string, string],
    };
  }
}

