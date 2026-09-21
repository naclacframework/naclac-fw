export interface IdlSeedAta {
  readonly kind: "ata";
  readonly mint: string;
  readonly authority: string;
  readonly tokenProgram?: string;
}

/** A PDA seed — one of four kinds emitted by the Naclac Rust compiler. */
export type IdlSeed =
  | { readonly kind: "const"; readonly value: readonly number[]; readonly name?: string }
  | { readonly kind: "arg"; readonly path: string }
  | { readonly kind: "account"; readonly path: string }
  | IdlSeedAta;

/** PDA descriptor attached to an account in the IDL. */
export interface IdlPda {
  readonly seeds: readonly IdlSeed[];
}

/** Top-level PDA definition — lists all discoverable PDAs for the program. */
export interface IdlPdaDef {
  readonly name: string;
  readonly seeds: readonly IdlSeed[];
}

/** A single account in an instruction's accounts array. */
export interface IdlAccount {
  name: string;
  /** Whether this account must be writable for the instruction to succeed. */
  writable?: boolean;
  /** Whether this account must sign the transaction. */
  signer?: boolean;
  /**
   * Whether this account is optional and may be omitted by the caller.
   * When omitted, the `optionalAccountStrategy` on the instruction determines the fill-in value.
   */
  optional?: boolean;
  /** Legacy aliases — isMut/isSigner kept for backward compatibility with older IDLs. */
  isMut?: boolean;
  isSigner?: boolean;
  /** If set, this account is a PDA derivable from these seeds. */
  pda?: IdlPda;
  /** If set, this account is always the given fixed address (e.g. system program). */
  address?: string;
}

/** A single field (instruction arg or account struct field). */
export interface IdlField {
  name: string;
  type: string | Record<string, unknown>;
}

/** A single instruction definition. */
export interface IdlInstruction {
  readonly name: string;
  readonly discriminator: readonly number[];
  readonly accounts: readonly IdlAccount[];
  readonly args: readonly IdlField[];
  /**
   * Strategy for filling optional accounts that the caller did not provide.
   * - "programId": use the current program's own address as a placeholder.
   * - "omitted": omit the account from the account list entirely (advanced).
   */
  readonly optionalAccountStrategy?: "programId" | "omitted";
}

/** A named account type definition (on-chain data layout). */
export interface IdlAccountDef {
  name: string;
  /**
   * 8-byte discriminator prefix stored at byte 0 of every account's data.
   * Used to verify the account type before deserialising.
   */
  discriminator?: readonly number[];
  readonly type: {
    readonly kind: string;
    readonly fields: readonly IdlField[];
  };
}

/** An event field in the IDL. */
export interface IdlEventField {
  name: string;
  type: string | Record<string, unknown>;
  index: boolean;
}

/** An event definition. */
export interface IdlEventDef {
  readonly name: string;
  /**
   * 8-byte discriminator used to identify this event in transaction logs.
   * Computed as sha256("event:<EventName>")[0..8].
   */
  readonly discriminator?: readonly number[];
  readonly fields: readonly IdlEventField[];
}

/** An error definition. */
export interface IdlErrorDef {
  code: number;
  name: string;
  /** Human-readable error description. Preferred over the deprecated `msg` field. */
  message?: string;
  /** @deprecated Use `message` instead. Kept for backward compatibility with older IDLs. */
  msg?: string;
}

/** A constant definition. */
export interface IdlConstant {
  name: string;
  type: string | Record<string, unknown>;
  value: string;
}

/**
 * A data-carrying enum variant's fields — named (`Foo { x: u64 }`, an
 * `IdlField[]`) or tuple (`Foo(u64, String)`, a bare array of raw type
 * descriptors with no field names) — matches `naclac-idl`'s untagged
 * `IdlEnumFields::{Named, Tuple}` exactly. Absent entirely for a unit
 * variant.
 */
export type IdlEnumFields =
  | readonly IdlField[]
  | ReadonlyArray<string | Record<string, unknown>>;

/** A user-defined type (struct or enum) referenced in accounts or instruction args. */
export interface IdlDefinedType {
  name: string;
  type:
    | {
        kind: "struct";
        fields: readonly IdlField[];
      }
    | {
        kind: "enum";
        variants: ReadonlyArray<{
          name: string;
          fields?: IdlEnumFields;
          /**
           * This variant's real, fully-resolved discriminant value
           * (decimal string, possibly negative) — see
           * `naclac_syn::types::NaclacEnumVariant::discriminant`'s doc
           * comment for the full rationale. Used by the zero-copy enum
           * codec (`client/src/coder/types.ts`) to map a tag byte back to
           * its variant; Borsh mode uses positional index instead and
           * ignores this field entirely (confirmed against real
           * `borsh-derive` source).
           */
          discriminant: string;
        }>;
        /**
         * The enum's `#[repr(uN)]` discriminant type (e.g. `"u8"`,
         * `"u16"`), or absent if never written explicitly — absent means
         * the same `u8` default `#[defined_type]`'s zero-copy branch
         * applies on-chain (`naclac-macros/src/lib.rs`).
         */
        repr?: string;
      };
}

/** The root IDL object emitted by `naclac build`. */
export interface NaclacIdl {
  readonly address: string;
  readonly metadata: {
    readonly name: string;
    readonly version: string;
    readonly description: string;
  };
  readonly instructions: readonly IdlInstruction[];
  readonly accounts: readonly IdlAccountDef[];
  readonly events: readonly IdlEventDef[];
  readonly errors: readonly IdlErrorDef[];
  readonly constants: readonly IdlConstant[];
  /**
   * User-defined types (structs/enums) used in this program's accounts or args.
   * Renamed from `types` to `definedTypes` to match ecosystem conventions.
   */
  readonly definedTypes: readonly IdlDefinedType[];
  /**
   * Top-level PDAs discoverable without scanning instruction accounts.
   * Each entry mirrors the embedded `pda` on instruction accounts but is lifted
   * to the root for tooling that needs a quick lookup.
   */
  readonly pdas: readonly IdlPdaDef[];
  /** @deprecated Use `definedTypes` instead. Kept for backward compatibility. */
  readonly types?: readonly IdlDefinedType[];
}
