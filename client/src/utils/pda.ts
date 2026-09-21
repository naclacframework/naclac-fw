import type { Address } from "@solana/kit";
import { getProgramDerivedAddress, address, getAddressEncoder } from "@solana/kit";
import type { IdlSeed } from "../idl";
import { getBase58Encoder } from "@solana/codecs";
import { TOKEN_PROGRAM_ID, ATA_PROGRAM_ID } from "../constants";
import { getIdlCodec } from "../coder/types";
import type { IdlInstruction } from "../idl";

const addressEncoder = getAddressEncoder();

/** Resolved PDA result containing the address and canonical bump. */
export interface ResolvedPda {
  address: Address;
  bump: number;
}

/**
 * Pure @solana/kit helper to derive an Associated Token Account (ATA) address.
 * No need to mess with getBase58Encoder() manually!
 *
 * @param mint - Base58 address of the token mint
 * @param owner - Base58 address of the wallet/account owning the ATA
 * @param tokenProgram - Base58 address of the Token program (defaults to standard SPL Token)
 */
export async function getAssociatedTokenAddress(
  mint: string | Address,
  owner: string | Address,
  tokenProgram: string | Address = TOKEN_PROGRAM_ID
): Promise<[Address, number]> {
  const encoder = getBase58Encoder();
  const result = await getProgramDerivedAddress({
    programAddress: address(ATA_PROGRAM_ID),
    seeds: [
      encoder.encode(owner as string),
      encoder.encode(tokenProgram as string),
      encoder.encode(mint as string),
    ],
  });
  return result as unknown as [Address, number];
}

function resolveLeafType(
  path: string,
  ixDef: IdlInstruction,
  definedTypes?: readonly any[]
): any {
  const parts = path.split(".");
  const rootArg = ixDef.args.find((a) => a.name === parts[0]);
  if (!rootArg) return undefined;

  let currentType = rootArg.type;
  for (let i = 1; i < parts.length; i++) {
    if (typeof currentType === "object" && currentType !== null && "defined" in currentType) {
      const typeName = currentType.defined;
      const def = definedTypes?.find((d) => d.name === typeName);
      if (!def || def.type.kind !== "struct") {
        return undefined;
      }
      const field = def.type.fields.find((f: any) => f.name === parts[i]);
      if (!field) return undefined;
      currentType = field.type;
    } else {
      return undefined;
    }
  }
  return currentType;
}

/**
 * Resolves a single seed from the IDL into a Uint8Array for PDA derivation.
 *
 * Seed kinds:
 *  - "const"   → raw bytes array from the IDL (e.g. b"counter_v2")
 *  - "arg"     → the user-supplied argument value, encoded as a single byte (for u8 bumps)
 *  - "account" → the public key bytes of a resolved account
 *
 * @param seed      - The IDL seed descriptor.
 * @param args      - The instruction's argument map (for "arg" seeds).
 * @param accounts  - The resolved account address map (for "account" seeds).
 */
function resolveSeed(
  seed: IdlSeed,
  ixDef: IdlInstruction,
  args: Record<string, unknown>,
  accounts: Record<string, Address>,
  definedTypes?: readonly any[],
  isZeroCopy = false
): Uint8Array {
  switch (seed.kind) {
    case "const":
      return new Uint8Array(seed.value);

    case "arg": {
      const val = seed.path.split('.').reduce((acc, part) => (acc as any)?.[part], args);
      if (val === undefined) {
        throw new Error(
          `[Naclac/PDA] Arg seed references "${seed.path}" but it was not found in the provided args.`
        );
      }

      // Look up the argument type in the IDL to use the correct codec
      const leafType = resolveLeafType(seed.path, ixDef, definedTypes);
      if (!leafType) {
        throw new Error(
          `[Naclac/PDA] Arg seed references "${seed.path}" but it is not defined in the instruction arguments.`
        );
      }

      const codec = getIdlCodec(leafType, definedTypes, isZeroCopy);
      return new Uint8Array(codec.encode(val));
    }

    case "account": {
      const addr = accounts[seed.path];
      if (!addr) {
        throw new Error(
          `[Naclac/PDA] Account seed references "${seed.path}" but it was not found in the provided accounts.`
        );
      }
      return new Uint8Array(addressEncoder.encode(addr));
    }

    case "ata": {
      throw new Error(
        `[Naclac/PDA] ATA seeds cannot be resolved via resolveSeed. They must be handled in resolvePdas.`
      );
    }

    default: {
      const exhaustive: never = seed;
      throw new Error(`[Naclac/PDA] Unknown seed kind: ${JSON.stringify(exhaustive)}`);
    }
  }
}

/**
 * Automatically derives PDA addresses for all accounts in an instruction
 * that have a `pda` field in the IDL.
 *
 * @param programId - The on-chain program address.
 * @param idlAccounts   - The instruction's accounts array from the IDL.
 * @param args      - The instruction's argument map.
 * @param userAccounts - Accounts explicitly provided by the developer (non-PDA accounts).
 * @returns A map of account name → resolved Address for all PDA accounts.
 */
export async function resolvePdas(
  programId: Address,
  ixDef: IdlInstruction,
  args: Record<string, unknown>,
  userAccounts: Record<string, Address>,
  targetAccountName?: string,
  definedTypes?: readonly any[],
  isZeroCopy = false
): Promise<Record<string, Address>> {
  const resolved: Record<string, Address> = {};
  const idlAccounts = ixDef.accounts;

  for (const acct of idlAccounts) {
    if (!acct.pda) continue;
    
    // Safety: if we are only resolving one specific account, skip the others.
    if (targetAccountName && acct.name !== targetAccountName) continue;

    // Safely skip auto-resolution if the developer explicitly provided the PDA
    if (userAccounts[acct.name]) {
      continue;
    }

    const basePdfSeeds = acct.pda.seeds.filter(
      (s) => !(s.kind === "arg" && s.path === "bump")
    );
    
    const isAta = basePdfSeeds.some((s) => s.kind === "ata");
    const derivationProgramId = isAta ? address(ATA_PROGRAM_ID) : programId;
    
    const seeds = basePdfSeeds.flatMap((seed) => {
      if (seed.kind === "ata") {
        const pool = { ...userAccounts, ...resolved };
        const authorityAddr = pool[seed.authority];
        const mintAddr = pool[seed.mint];
        const tokenProgramAddr = seed.tokenProgram
          ? pool[seed.tokenProgram]
          : TOKEN_PROGRAM_ID;

        if (!authorityAddr) {
          throw new Error(`[Naclac/PDA] ATA seed missing authority account "${seed.authority}"`);
        }
        if (!mintAddr) {
          throw new Error(`[Naclac/PDA] ATA seed missing mint account "${seed.mint}"`);
        }
        if (!tokenProgramAddr) {
          throw new Error(`[Naclac/PDA] ATA seed missing token program account "${seed.tokenProgram}"`);
        }

        const encoder = getBase58Encoder();
        return [
          new Uint8Array(encoder.encode(authorityAddr)),
          new Uint8Array(encoder.encode(tokenProgramAddr as string)),
          new Uint8Array(encoder.encode(mintAddr)),
        ];
      }

      return [resolveSeed(seed, ixDef, args, { ...userAccounts, ...resolved }, definedTypes, isZeroCopy)];
    });

    const [pdaAddress] = await getProgramDerivedAddress({
      programAddress: derivationProgramId,
      seeds,
    });

    resolved[acct.name] = pdaAddress;
  }

  return resolved;
}
