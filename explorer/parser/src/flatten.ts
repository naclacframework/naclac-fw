/**
 * Turns a transaction's top-level and inner instructions into one flat list in
 * execution order, with account indexes resolved to addresses. A top-level
 * instruction has `innerInstructionIndex: null`; the inner instructions it made
 * follow it, numbered 0.. in order, and `stackHeight` gives their depth.
 */

import { ParseError } from "./errors.ts";
import type { AccountKey, RpcInnerInstructionSet, RpcInstruction } from "./types.ts";

/** One instruction of the flat list, before any decoding; `accountIndexes` index the full account list. */
export interface FlatInstruction {
  instructionIndex: number;
  innerInstructionIndex: number | null;
  stackHeight: number | null;
  programId: string;
  rawAccounts: string[];
  rawData: string;
  accountIndexes: number[];
}

function addressAt(keys: AccountKey[], index: number, field: string): string {
  const key = Number.isInteger(index) ? keys[index] : undefined;
  if (key === undefined) {
    throw new ParseError(
      `${field} is ${index}, but the transaction has ${keys.length} account keys.`,
      field,
    );
  }
  return key.address;
}

function build(
  instruction: RpcInstruction,
  keys: AccountKey[],
  instructionIndex: number,
  innerInstructionIndex: number | null,
  where: string,
): FlatInstruction {
  return {
    instructionIndex,
    innerInstructionIndex,
    stackHeight: instruction.stackHeight ?? null,
    programId: addressAt(keys, instruction.programIdIndex, `${where}.programIdIndex`),
    rawAccounts: instruction.accounts.map((index, position) =>
      addressAt(keys, index, `${where}.accounts[${position}]`),
    ),
    rawData: instruction.data,
    accountIndexes: [...instruction.accounts],
  };
}

/**
 * Returns the flat instruction list. Throws a ParseError when an account index
 * is out of range, or when an inner-instruction set names a missing or
 * repeated top-level instruction.
 */
export function flattenInstructions(
  topLevel: RpcInstruction[],
  innerSets: RpcInnerInstructionSet[],
  keys: AccountKey[],
): FlatInstruction[] {
  const innerByParent = new Map<number, RpcInstruction[]>();
  for (const set of innerSets) {
    if (!Number.isInteger(set.index) || set.index < 0 || set.index >= topLevel.length) {
      throw new ParseError(
        `inner instructions name top-level instruction ${set.index}, but there are ${topLevel.length}.`,
        "meta.innerInstructions",
      );
    }
    if (innerByParent.has(set.index)) {
      throw new ParseError(
        `inner instructions of top-level instruction ${set.index} appear twice.`,
        "meta.innerInstructions",
      );
    }
    innerByParent.set(set.index, set.instructions);
  }

  const flat: FlatInstruction[] = [];
  topLevel.forEach((instruction, index) => {
    flat.push(build(instruction, keys, index, null, `message.instructions[${index}]`));
    (innerByParent.get(index) ?? []).forEach((inner, position) => {
      flat.push(
        build(
          inner,
          keys,
          index,
          position,
          `meta.innerInstructions[${index}].instructions[${position}]`,
        ),
      );
    });
  });
  return flat;
}
