import {
  address,
  AccountRole,
  appendTransactionMessageInstructions,
  createTransactionMessage,
  getBase64EncodedWireTransaction,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
} from "@solana/kit";
import { getBase58Codec } from "@solana/codecs";

import type {
  Address,
  AccountMeta,
  AccountSignerMeta,
  TransactionSigner,
  Instruction,
  Blockhash,
} from "@solana/kit";

import type { NaclacProvider } from "./provider";
import type { NaclacIdl, IdlInstruction } from "../idl";
import { encodeInstructionData } from "../coder/instruction";
import { resolvePdas } from "../utils/pda";
import { translateRpcError } from "../utils/rpc";
import { toCamelCase } from "../utils/case";
import { parseEventsFromLogs } from "../events";
import {
  SYSTEM_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  ATA_PROGRAM_ID,
  SYSVAR_RENT_PUBKEY,
} from "../constants";

/**
 * The result of `.rpc()`: the confirmed transaction's signature and logs,
 * plus a convenience method to decode any events it emitted. Reading events
 * this way (rather than via `Program.addEventListener`/`waitForEvent`, a live
 * websocket subscription) is race-free — these logs belong to a transaction
 * that has already confirmed, so there's no timing window where an event
 * could be missed.
 */
export interface NaclacRpcResult {
  signature: string;
  logs: string[];
  parseEvents<T = any>(eventName: string): T[];
}

/**
 * MethodsBuilder — the fluent instruction pipeline.
 */
export class MethodsBuilder {
  private readonly idl: NaclacIdl;
  private readonly ixDef: IdlInstruction;
  private readonly args: Record<string, unknown>;
  private readonly provider: NaclacProvider;
  private readonly isZeroCopy: boolean;
  private programId: Address;

  private _userAccounts: Record<string, Address> = {};
  private _extraSigners: TransactionSigner[] = [];
  private _remainingAccounts: Address[] = [];
  private _preInstructions: Instruction<string>[] = [];
  private _postInstructions: Instruction<string>[] = [];

  constructor(
    idl: NaclacIdl,
    ixDef: IdlInstruction,
    args: Record<string, unknown>,
    provider: NaclacProvider,
    isZeroCopy = false,
  ) {
    this.idl = idl;
    this.ixDef = ixDef;
    this.args = args;
    this.provider = provider;
    this.isZeroCopy = isZeroCopy;
    this.programId = address(idl.address);
  }

  accounts(accounts: Record<string, Address | string>): this {
    const camelToRaw: Record<string, string> = {};
    for (const acct of this.ixDef.accounts) {
      camelToRaw[toCamelCase(acct.name)] = acct.name;
    }
    const normalized: Record<string, Address> = {};
    for (const [key, val] of Object.entries(accounts)) {
      normalized[camelToRaw[key] ?? key] = address(val as string);
    }
    this._userAccounts = { ...this._userAccounts, ...normalized };
    return this;
  }

  signers(signers: TransactionSigner[]): this {
    this._extraSigners.push(...signers);
    return this;
  }

  remainingAccounts(addresses: (Address | string)[]): this {
    this._remainingAccounts.push(...addresses.map((a) => address(a as string)));
    return this;
  }

  preInstructions(ixs: Instruction<string>[]): this {
    this._preInstructions.push(...ixs);
    return this;
  }

  postInstructions(ixs: Instruction<string>[]): this {
    this._postInstructions.push(...ixs);
    return this;
  }

  private async _buildInstructionMetasAndData() {
    const pdaAccounts = await resolvePdas(
      this.programId,
      this.ixDef,
      this.args,
      this._userAccounts,
      undefined,
      this.idl.definedTypes,
      this.isZeroCopy,
    );

    const allAccounts: Record<string, Address> = {
      ...this._userAccounts,
      ...pdaAccounts,
    };

    const data = encodeInstructionData(this.ixDef, this.args, this.idl.definedTypes, this.isZeroCopy);

    type AnyAccountMeta =
      | AccountMeta<Address>
      | AccountSignerMeta<Address, TransactionSigner<Address>>;

    const accountMetas: AnyAccountMeta[] = this.ixDef.accounts.map(
      (acctDef) => {
        let addr = allAccounts[acctDef.name];

        if (!addr) {
          if (acctDef.address) {
            addr = address(acctDef.address);
          }
          else {
            const nameLower = acctDef.name.toLowerCase();
            if (nameLower === "systemprogram") addr = address(SYSTEM_PROGRAM_ID);
            else if (nameLower === "tokenprogram") addr = address(TOKEN_PROGRAM_ID);
            else if (nameLower === "token2022program")
              addr = address(TOKEN_2022_PROGRAM_ID);
            else if (nameLower === "ataprogram") addr = address(ATA_PROGRAM_ID);
            else if (nameLower === "rent" || nameLower === "sysvarrent")
              addr = address(SYSVAR_RENT_PUBKEY);
          }
        }

        if (!addr) {
          throw new Error(
            `[Naclac] Account "${acctDef.name}" was not provided and could not be auto-resolved.`,
          );
        }

        const isWritable =
          (acctDef as any).writable ?? (acctDef as any).isMut ?? false;
        const isSigner =
          (acctDef as any).signer ?? (acctDef as any).isSigner ?? false;

        let role: AccountRole;
        if (isSigner && isWritable) role = AccountRole.WRITABLE_SIGNER;
        else if (isSigner) role = AccountRole.READONLY_SIGNER;
        else if (isWritable) role = AccountRole.WRITABLE;
        else role = AccountRole.READONLY;

        const extraSigner = this._extraSigners.find((s) => s.address === addr);
        if (
          extraSigner &&
          (role === AccountRole.WRITABLE_SIGNER ||
            role === AccountRole.READONLY_SIGNER)
        ) {
          return {
            address: extraSigner.address,
            signer: extraSigner,
            role,
          } as unknown as AccountSignerMeta<
            Address,
            TransactionSigner<Address>
          >;
        }

        return { address: addr, role } as AccountMeta<Address>;
      },
    );

    for (const addr of this._remainingAccounts) {
      accountMetas.push({
        address: addr,
        role: AccountRole.WRITABLE,
      } as AccountMeta<Address>);
    }

    return { data, accountMetas };
  }

  async transaction() {
    const commitment = this.provider.commitment ?? "confirmed";

    const { data, accountMetas } = await this._buildInstructionMetasAndData();

    const instruction: Instruction<string> = {
      programAddress: this.programId,
      accounts: accountMetas as any,
      data,
    };

    const allInstructions = [
      ...this._preInstructions,
      instruction,
      ...this._postInstructions,
    ];

    const rpc = this.provider.rpc as any;

    const { value: latestBlockhash } = await rpc
      .getLatestBlockhash({ commitment })
      .send();

    const txMessage = pipe(
      createTransactionMessage({ version: 0 }),
      (msg) => setTransactionMessageFeePayerSigner(this.provider.signer, msg),
      (msg) =>
        setTransactionMessageLifetimeUsingBlockhash(
          {
            ...latestBlockhash,
            blockhash: latestBlockhash.blockhash as unknown as Blockhash,
          },
          msg,
        ),
      (msg) => appendTransactionMessageInstructions(allInstructions, msg),
    );

    return txMessage;
  }

  async simulate() {
    const txMessage = await this.transaction();
    const signedTx = await signTransactionMessageWithSigners(txMessage);
    const base64Tx = getBase64EncodedWireTransaction(signedTx as any);

    try {
      const response = await (this.provider.rpc as any)
        .simulateTransaction(base64Tx, { encoding: "base64" })
        .send();

      if (response.value.err) {
        const simError = new Error("Transaction simulation failed");
        (simError as any).context = { logs: response.value.logs };
        throw simError;
      }

      return response.value;
    } catch (err: any) {
      translateRpcError(err, this.idl.errors, this.ixDef.accounts);
    }
  }

  async instruction(): Promise<Instruction<string>> {
    return this.buildInstruction();
  }

  async pubkeys(): Promise<Record<string, Address>> {
    const { accountMetas } = await this._buildInstructionMetasAndData();
    const result: Record<string, Address> = {};
    this.ixDef.accounts.forEach((acct, i) => {
      result[toCamelCase(acct.name)] = accountMetas[i].address;
    });
    return result;
  }

  async encoded(): Promise<string> {
    const txMessage = await this.transaction();
    const signedTx = await signTransactionMessageWithSigners(txMessage);
    return getBase64EncodedWireTransaction(signedTx as any);
  }

  async send(options: { skipPreflight?: boolean } = {}): Promise<string> {
    const commitment = this.provider.commitment ?? "confirmed";
    const txMessage = await this.transaction();
    const signedTx = await signTransactionMessageWithSigners(txMessage);

    const rpc = this.provider.rpc as any;
    const base64Tx = getBase64EncodedWireTransaction(signedTx as any);

    try {
      const response = await rpc
        .sendTransaction(base64Tx, {
          encoding: "base64",
          preflightCommitment: commitment,
          skipPreflight: options.skipPreflight ?? false,
        })
        .send();
      return response;
    } catch (err: any) {
      translateRpcError(err, this.idl.errors, this.ixDef.accounts);
      throw err;
    }
  }

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
      const err = new Error(
        `[Naclac] Remote RPC error: ${JSON.stringify(data.error)}`,
      );
      throw err;
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

  async rpc(): Promise<NaclacRpcResult> {
    const commitment = this.provider.commitment ?? "confirmed";
    const txMessage = await this.transaction();

    const signedTx = await signTransactionMessageWithSigners(txMessage);

    const rpc = this.provider.rpc as any;
    let signature: string;

    if (this.provider.litesvm) {
      // litesvm executes and finalizes `sendTransaction` synchronously —
      // there is no separate confirm step to wait for, and no real
      // websocket for `sendAndConfirmTransactionFactory` to use anyway.
      const base64Tx = getBase64EncodedWireTransaction(signedTx as any);
      try {
        signature = await rpc.sendTransaction(base64Tx, { encoding: "base64" }).send();
      } catch (err: any) {
        translateRpcError(err, this.idl.errors, this.ixDef.accounts);
        throw err;
      }
    } else {
      const rpcSubscriptions = this.provider.rpcSubscriptions as any;
      const sendAndConfirm = sendAndConfirmTransactionFactory({
        rpc,
        rpcSubscriptions,
      });

      try {
        await sendAndConfirm(signedTx as any, { commitment });
      } catch (err: any) {
        translateRpcError(err, this.idl.errors, this.ixDef.accounts);
        throw err;
      }

      const signatureMap = (
        signedTx as { signatures: Record<string, Uint8Array | null> }
      ).signatures;
      const signatures = Object.values(signatureMap).filter(
        (sig): sig is Uint8Array => sig !== null,
      );

      const sigBytes = signatures[0];
      signature = sigBytes ? getBase58Codec().decode(sigBytes) : "(no signature)";
    }

    // Fetch the now-confirmed transaction's logs directly — no race, since
    // this transaction has already landed and its logs already exist (unlike
    // a live `logsSubscribe` subscription, which can miss events broadcast
    // before the subscription handshake finishes connecting).
    let logs: string[] = [];
    try {
      const txResult = await rpc
        .getTransaction(signature, {
          commitment: commitment === "finalized" ? "finalized" : "confirmed",
          maxSupportedTransactionVersion: 0,
        })
        .send();
      logs = (txResult?.meta?.logMessages as string[] | undefined) ?? [];
    } catch {
      // Logs are a best-effort convenience; a failure fetching them shouldn't
      // fail an already-confirmed transaction.
    }

    return {
      signature,
      logs,
      parseEvents: <T = any>(eventName: string) =>
        parseEventsFromLogs<T>(this.idl, eventName, logs, this.isZeroCopy),
    };
  }

  async buildInstruction(): Promise<Instruction<string>> {
    const { data, accountMetas } = await this._buildInstructionMetasAndData();
    return {
      programAddress: this.programId,
      accounts: accountMetas as any,
      data,
    };
  }
}
