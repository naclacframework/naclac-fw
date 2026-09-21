import type { Address, EncodedAccount, Account, Lamports, Commitment } from "@solana/kit";
import { address } from "@solana/kit";
import { getBase58Codec } from "@solana/codecs";
import type { IdlAccountDef } from "../idl";
import type { NaclacProvider } from "./provider";
import { decodeAccountData } from "../coder/instruction";
import { getDiscriminator } from "../utils/hash";
import { decodeBase64 } from "../utils/common";

/**
 * AccountFetcher — fetches and deserializes on-chain accounts for a single
 * account type defined in the program IDL.
 *
 * Created automatically by `Program<TIdl>` for every entry in `idl.accounts`.
 */
export class AccountFetcher {
  private readonly accountDef: IdlAccountDef;
  private readonly provider: NaclacProvider;
  private readonly programId: Address;

  private readonly definedTypes: readonly any[];
  private readonly isZeroCopy: boolean;

  constructor(
    accountDef: IdlAccountDef,
    provider: NaclacProvider,
    programId: Address,
    definedTypes: readonly any[] = [],
    isZeroCopy: boolean = false,
  ) {
    this.accountDef = accountDef;
    this.provider = provider;
    this.programId = programId;
    this.definedTypes = definedTypes;
    this.isZeroCopy = isZeroCopy;
  }

  /**
   * Fetches and deserializes a single account by its on-chain address.
   *
   * @param pubkey The on-chain address of the account.
   * @throws If the account does not exist at the given address.
   */
  async fetch(pubkey: Address | string): Promise<Record<string, unknown>> {
    const addr = address(pubkey as string);
    const commitment = this.provider.commitment ?? "confirmed";

    const { value: accountInfo } = await (
      this.provider.rpc as {
        getAccountInfo: (
          addr: Address,
          opts?: { commitment: string; encoding: string },
        ) => {
          send: () => Promise<{ value: { data: [string, string] } | null }>;
        };
      }
    )
      .getAccountInfo(addr, { commitment, encoding: "base64" })
      .send();

    if (!accountInfo) {
      throw new Error(
        `[Naclac] Account "${pubkey}" does not exist on-chain (commitment: ${commitment}).`,
      );
    }

    const rawBase64 = (accountInfo.data as [string, string])[0];
    const rawBytes = decodeBase64(rawBase64);

    return decodeAccountData(
      this.accountDef.type.fields as any,
      new Uint8Array(rawBytes),
      this.definedTypes,
      this.isZeroCopy,
    );
  }

  /**
   * Fetches and deserializes multiple accounts in a single RPC call.
   * Missing accounts are returned as `null`.
   *
   * @param pubkeys Array of on-chain addresses.
   * @param opts.dropCorrupted If true, corrupted accounts are silently dropped
   *                           instead of throwing. Useful for exploratory scans.
   */
  async fetchMultiple(
    pubkeys: Array<Address | string>,
    opts: { dropCorrupted?: boolean } = {},
  ): Promise<Array<Record<string, unknown> | null>> {
    const addrs = pubkeys.map((pk) => address(pk as string));
    const commitment = this.provider.commitment ?? "confirmed";

    const { value: accounts } = await (
      this.provider.rpc as {
        getMultipleAccounts: (
          addrs: Address[],
          opts?: { commitment: string; encoding: string },
        ) => {
          send: () => Promise<{
            value: Array<{ data: [string, string] } | null>;
          }>;
        };
      }
    )
      .getMultipleAccounts(addrs, { commitment, encoding: "base64" })
      .send();

    return accounts.map((acctInfo) => {
      if (!acctInfo) return null;
      try {
        const rawBase64 = (acctInfo.data as [string, string])[0];
        const rawBytes = decodeBase64(rawBase64);
        return decodeAccountData(
          this.accountDef.type.fields as any,
          new Uint8Array(rawBytes),
          this.definedTypes,
          this.isZeroCopy,
        );
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
   * Fetches multiple accounts in a single RPC call and returns them mapped by their address.
   * Missing accounts are returned as `null` in the map.
   *
   * @param pubkeys Array of on-chain addresses.
   * @param opts.dropCorrupted If true, corrupted accounts are silently dropped.
   */
  async fetchMultipleAsMap(
    pubkeys: Array<Address | string>,
    opts: { dropCorrupted?: boolean } = {},
  ): Promise<Map<Address, Record<string, unknown> | null>> {
    const results = await this.fetchMultiple(pubkeys, opts);
    const map = new Map<Address, Record<string, unknown> | null>();
    pubkeys.forEach((pk, i) => {
      map.set(address(pk as string), results[i]);
    });
    return map;
  }

  /**
   * Fetches all on-chain accounts of this type owned by the program.
   * Uses a memcmp filter on the 8-byte account discriminator for precision.
   *
   * @param opts.filters Additional memcmp or dataSize RPC filters to narrow results.
   * @param opts.dropCorrupted If true, accounts that fail to deserialise are silently dropped.
   */
  async all(opts: { filters?: any[]; dropCorrupted?: boolean } = {}): Promise<
    Array<{ publicKey: Address; account: Record<string, unknown> }>
  > {
    const discriminator = getDiscriminator("account", this.accountDef.name);
    // getBase58Codec().decode(bytes) → base58 string, which is what memcmp.bytes expects.
    const base58Discriminator = getBase58Codec().decode(discriminator);
    const commitment = this.provider.commitment ?? "confirmed";

    const filters = [
      { memcmp: { offset: 0, bytes: base58Discriminator } },
      ...(opts.filters ?? []),
    ];

    const raw = await (this.provider.rpc as any)
      .getProgramAccounts(this.programId, {
        commitment,
        encoding: "base64",
        filters,
      })
      .send();

    const accounts: any[] = Array.isArray(raw) ? raw : (raw?.value ?? []);

    const decoded = accounts.map((acctInfo: any) => {
      try {
        const rawBase64 = acctInfo.account.data[0];
        const rawBytes = decodeBase64(rawBase64);
        const accountData = decodeAccountData(
          this.accountDef.type.fields as any,
          new Uint8Array(rawBytes),
          this.definedTypes,
          this.isZeroCopy,
        );
        return {
          publicKey: acctInfo.pubkey as Address,
          account: accountData,
        };
      } catch (err) {
        if (opts.dropCorrupted) {
          console.warn(`[Naclac] Dropping corrupted account at ${acctInfo.pubkey}:`, err);
          return null;
        }
        throw err;
      }
    });

    return decoded.filter(
      (d): d is { publicKey: Address; account: Record<string, unknown> } =>
        d !== null,
    );
  }
}

/**
 * Fetches all accounts owned by a program that match a specific discriminator.
 * A utility helper to keep generated TS clients extremely lean and clean.
 */
export async function fetchAllAccountsByProgram<
  TAccount extends object | Uint8Array = object | Uint8Array,
  TAddress extends string = string,
>(
  rpc: any,
  programAddress: Address,
  discriminator: Uint8Array,
  decodeFn: (encodedAccount: EncodedAccount<TAddress>) => Account<TAccount, TAddress>,
  options?: { commitment?: Commitment; filters?: unknown[] },
): Promise<Array<Account<TAccount, TAddress>>> {
  const discBase58 = getBase58Codec().decode(discriminator);
  const baseFilters = [{ memcmp: { offset: 0, bytes: discBase58 } }];
  const allFilters = [...baseFilters, ...(options?.filters ?? [])];
  const raw = await rpc.getProgramAccounts(programAddress, {
    commitment: options?.commitment ?? "confirmed",
    encoding: "base64",
    filters: allFilters,
  }).send();
  const items: any[] = Array.isArray(raw) ? raw : (raw?.value ?? []);
  return items.map((item: any) => {
    const dataBase64: string = Array.isArray(item.account.data)
      ? item.account.data[0]
      : item.account.data;
    const rawBytes = decodeBase64(dataBase64);
    return decodeFn({
      address: item.pubkey as Address<TAddress>,
      data: rawBytes,
      executable: item.account.executable,
      lamports: BigInt(item.account.lamports) as Lamports,
      programAddress: item.account.owner as Address,
      space: BigInt(rawBytes.length),
    });
  });
}

