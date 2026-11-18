/**
 * The SPL Token and Token-2022 programs' instructions: a 1-byte tag followed by
 * the variant's fields, as `TokenInstruction::unpack` reads them in
 * spl-token-interface 3.0.0 and spl-token-2022-interface 3.1.2. Token accepts
 * tags 0-24, 38, 45 and 255; Token-2022 adds the extension tags 25-37, 39-44
 * and 46. Extension instructions that carry a further sub-instruction are
 * named but not decoded.
 */

import type { ArgValue } from "../types.ts";
import { DecodeFailure, Reader, decodeWith, unknownInstruction, utf8 } from "./reader.ts";
import type { BuiltinDecode } from "./reader.ts";

export type TokenFlavor = "token" | "token-2022";

const LEGACY_AUTHORITY_TYPES = ["mint_tokens", "freeze_account", "account_owner", "close_account"];

const AUTHORITY_TYPES = [
  ...LEGACY_AUTHORITY_TYPES,
  "transfer_fee_config",
  "withheld_withdraw",
  "close_mint",
  "interest_rate",
  "permanent_delegate",
  "confidential_transfer_mint",
  "transfer_hook_program_id",
  "confidential_transfer_fee_config",
  "metadata_pointer",
  "group_pointer",
  "group_member_pointer",
  "scaled_ui_amount",
  "pause",
  "permissioned_burn",
];

const EXTENSION_TYPES = [
  "uninitialized",
  "transferFeeConfig",
  "transferFeeAmount",
  "mintCloseAuthority",
  "confidentialTransferMint",
  "confidentialTransferAccount",
  "defaultAccountState",
  "immutableOwner",
  "memoTransfer",
  "nonTransferable",
  "interestBearingConfig",
  "cpiGuard",
  "permanentDelegate",
  "nonTransferableAccount",
  "transferHook",
  "transferHookAccount",
  "confidentialTransferFeeConfig",
  "confidentialTransferFeeAmount",
  "metadataPointer",
  "tokenMetadata",
  "groupPointer",
  "tokenGroup",
  "groupMemberPointer",
  "tokenGroupMember",
  "confidentialMintBurn",
  "scaledUiAmount",
  "pausable",
  "pausableAccount",
  "permissionedBurn",
  "accountPaddingTest",
  "mintPaddingTest",
];

interface Variant {
  name: string;
  accounts: string[];
  args?: (reader: Reader, flavor: TokenFlavor) => Record<string, ArgValue>;
}

const amount = (r: Reader): Record<string, ArgValue> => ({ amount: r.u64("amount") });
const amountDecimals = (r: Reader): Record<string, ArgValue> => ({
  amount: r.u64("amount"),
  decimals: r.u8("decimals"),
});
const owner = (r: Reader): Record<string, ArgValue> => ({ owner: r.pubkey("owner") });
const multisigSize = (r: Reader): Record<string, ArgValue> => ({ m: r.u8("signer count") });
const mintInit = (r: Reader): Record<string, ArgValue> => ({
  decimals: r.u8("decimals"),
  mint_authority: r.pubkey("mint authority"),
  freeze_authority: r.optionPubkey("freeze authority"),
});

function extensionTypes(reader: Reader): string[] {
  if (reader.remaining % 2 !== 0) {
    throw new DecodeFailure(`the extension type list is ${reader.remaining} bytes, not a multiple of 2.`);
  }
  const names: string[] = [];
  while (reader.remaining > 0) {
    const value = reader.u16("extension type");
    const name = EXTENSION_TYPES[value];
    if (name === undefined) throw new DecodeFailure(`${value} is not an extension type.`);
    names.push(name);
  }
  return names;
}

function authorityType(reader: Reader, flavor: TokenFlavor): string {
  const value = reader.u8("authority type");
  const name = (flavor === "token" ? LEGACY_AUTHORITY_TYPES : AUTHORITY_TYPES)[value];
  if (name === undefined) throw new DecodeFailure(`${value} is not an authority type.`);
  return name;
}

const VARIANTS = new Map<number, Variant>([
  [0, { name: "initialize_mint", accounts: ["mint", "rent_sysvar"], args: mintInit }],
  [1, { name: "initialize_account", accounts: ["account", "mint", "owner", "rent_sysvar"] }],
  [2, { name: "initialize_multisig", accounts: ["multisig", "rent_sysvar"], args: multisigSize }],
  [3, { name: "transfer", accounts: ["source", "destination", "authority"], args: amount }],
  [4, { name: "approve", accounts: ["source", "delegate", "owner"], args: amount }],
  [5, { name: "revoke", accounts: ["source", "owner"] }],
  [
    6,
    {
      name: "set_authority",
      accounts: ["account", "current_authority"],
      args: (r, flavor) => ({
        authority_type: authorityType(r, flavor),
        new_authority: r.optionPubkey("new authority"),
      }),
    },
  ],
  [7, { name: "mint_to", accounts: ["mint", "destination", "mint_authority"], args: amount }],
  [8, { name: "burn", accounts: ["source", "mint", "authority"], args: amount }],
  [9, { name: "close_account", accounts: ["account", "destination", "owner"] }],
  [10, { name: "freeze_account", accounts: ["account", "mint", "freeze_authority"] }],
  [11, { name: "thaw_account", accounts: ["account", "mint", "freeze_authority"] }],
  [
    12,
    {
      name: "transfer_checked",
      accounts: ["source", "mint", "destination", "authority"],
      args: amountDecimals,
    },
  ],
  [
    13,
    {
      name: "approve_checked",
      accounts: ["source", "mint", "delegate", "owner"],
      args: amountDecimals,
    },
  ],
  [
    14,
    {
      name: "mint_to_checked",
      accounts: ["mint", "destination", "mint_authority"],
      args: amountDecimals,
    },
  ],
  [15, { name: "burn_checked", accounts: ["source", "mint", "authority"], args: amountDecimals }],
  [16, { name: "initialize_account_2", accounts: ["account", "mint", "rent_sysvar"], args: owner }],
  [17, { name: "sync_native", accounts: ["account"] }],
  [18, { name: "initialize_account_3", accounts: ["account", "mint"], args: owner }],
  [19, { name: "initialize_multisig_2", accounts: ["multisig"], args: multisigSize }],
  [20, { name: "initialize_mint_2", accounts: ["mint"], args: mintInit }],
  [
    21,
    {
      name: "get_account_data_size",
      accounts: ["mint"],
      args: (r, flavor): Record<string, ArgValue> =>
        flavor === "token-2022" ? { extension_types: extensionTypes(r) } : {},
    },
  ],
  [22, { name: "initialize_immutable_owner", accounts: ["account"] }],
  [23, { name: "amount_to_ui_amount", accounts: ["mint"], args: amount }],
  [
    24,
    {
      name: "ui_amount_to_amount",
      accounts: ["mint"],
      args: (r) => ({ ui_amount: utf8(r.rest(), "the UI amount") }),
    },
  ],
  [38, { name: "withdraw_excess_lamports", accounts: ["source", "destination", "authority"] }],
  [
    45,
    {
      name: "unwrap_lamports",
      accounts: ["source", "destination", "authority"],
      args: (r) => ({ amount: r.optionU64("amount") }),
    },
  ],
]);

const TOKEN_2022_VARIANTS = new Map<number, Variant>([
  [
    25,
    {
      name: "initialize_mint_close_authority",
      accounts: ["mint"],
      args: (r) => ({ close_authority: r.optionPubkey("close authority") }),
    },
  ],
  [
    29,
    {
      name: "reallocate",
      accounts: ["account", "payer", "system_program", "owner"],
      args: (r) => ({ extension_types: extensionTypes(r) }),
    },
  ],
  [
    31,
    {
      name: "create_native_mint",
      accounts: ["funding_account", "native_mint", "system_program"],
    },
  ],
  [
    35,
    {
      name: "initialize_permanent_delegate",
      accounts: ["mint"],
      args: (r) => ({ delegate: r.pubkey("delegate") }),
    },
  ],
]);

const EXTENSION_ONLY = new Map<number, string>([
  [26, "transfer_fee_extension"],
  [27, "confidential_transfer_extension"],
  [28, "default_account_state_extension"],
  [30, "memo_transfer_extension"],
  [32, "initialize_non_transferable_mint"],
  [33, "interest_bearing_mint_extension"],
  [34, "cpi_guard_extension"],
  [36, "transfer_hook_extension"],
  [37, "confidential_transfer_fee_extension"],
  [39, "metadata_pointer_extension"],
  [40, "group_pointer_extension"],
  [41, "group_member_pointer_extension"],
  [42, "confidential_mint_burn_extension"],
  [43, "scaled_ui_amount_extension"],
  [44, "pausable_extension"],
  [46, "permissioned_burn_extension"],
]);

function unsupported(name: string, reason: string): BuiltinDecode {
  return { ok: false, status: "unsupported-type", name, reason };
}

/** Decodes an instruction of the Token program (`flavor` "token") or of Token-2022. */
export function decodeToken(flavor: TokenFlavor, data: Uint8Array): BuiltinDecode {
  const tag = data[0];
  if (tag === undefined) return unknownInstruction("the data is empty.");

  const variant = VARIANTS.get(tag) ?? (flavor === "token-2022" ? TOKEN_2022_VARIANTS.get(tag) : undefined);
  if (variant !== undefined) {
    return decodeWith(variant.name, data, 1, (reader) => ({
      args: variant.args ? variant.args(reader, flavor) : {},
      accounts: variant.accounts,
    }));
  }
  if (tag === 255) {
    return unsupported("batch", "a batch holds several instructions, which are not decoded yet.");
  }
  const extensionName = flavor === "token-2022" ? EXTENSION_ONLY.get(tag) : undefined;
  if (extensionName !== undefined) {
    return unsupported(extensionName, "this extension instruction's fields are not decoded yet.");
  }
  return unknownInstruction(`${tag} is not an instruction of the ${flavor} program.`);
}
