/**
 * Decoding of what a naclac program did, from its IDL: the instruction a call
 * invoked (name, arguments, named accounts), the `emit!` events in the logs,
 * and the error a failed transaction ended with. Anything that cannot be
 * decoded is returned with a status and a reason; nothing is guessed.
 */

import { nameAccounts } from "./accounts.ts";
import { decodeBase58 } from "./base58.ts";
import { ParseError } from "./errors.ts";
import type { FlatInstruction } from "./flatten.ts";
import { FRAMEWORK_ERROR_BASE, FRAMEWORK_ERRORS, USER_ERROR_BASE } from "./framework-errors.ts";
import type { NaclacIdl } from "./idl.ts";
import { decodeFields } from "./layout.ts";
import type { LogFailure, WalkedLogs } from "./logs.ts";
import type {
  AccountKey,
  DecodedError,
  DecodedInstructionData,
  DecodeStatus,
  ParsedEvent,
  ParsedInstruction,
  UndecodedLogData,
} from "./types.ts";

export interface InstructionDecoding {
  programName: string | null;
  instructionName: string | null;
  decoded: DecodedInstructionData | null;
  decodeStatus: DecodeStatus;
  decodeReason: string | null;
}

function startsWith(bytes: Uint8Array, prefix: number[]): boolean {
  return bytes.length >= prefix.length && prefix.every((byte, i) => bytes[i] === byte);
}

function hex(bytes: number[]): string {
  return bytes.map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function failure(
  idl: NaclacIdl,
  instructionName: string | null,
  status: DecodeStatus,
  reason: string,
): InstructionDecoding {
  return {
    programName: idl.name,
    instructionName,
    decoded: null,
    decodeStatus: status,
    decodeReason: reason,
  };
}

/**
 * Decodes one instruction against the IDL of the program it calls. `idl` is
 * undefined when none was supplied for the program.
 */
export function decodeInstruction(
  idl: NaclacIdl | undefined,
  instruction: FlatInstruction,
  keys: AccountKey[],
): InstructionDecoding {
  if (idl === undefined) {
    return {
      programName: null,
      instructionName: null,
      decoded: null,
      decodeStatus: "no-idl",
      decodeReason: "no IDL was supplied for this program.",
    };
  }
  const data = decodeBase58(instruction.rawData, "instruction data");
  const definition = idl.instructions.find((candidate) => startsWith(data, candidate.discriminator));
  if (definition === undefined) {
    const width = idl.instructions[0]?.discriminator.length ?? 0;
    return failure(
      idl,
      null,
      "unknown-instruction",
      `no instruction of ${idl.name} has the discriminator 0x${hex(Array.from(data.subarray(0, width)))}.`,
    );
  }
  const args = decodeFields(
    definition.args,
    data,
    definition.discriminator.length,
    idl.serialization,
  );
  if (!args.ok) return failure(idl, definition.name, args.status, args.reason);

  const { accounts, missing } = nameAccounts(
    definition.accounts,
    instruction,
    keys,
    "remainingAccount",
  );
  return {
    programName: idl.name,
    instructionName: definition.name,
    decoded: {
      args: args.values,
      accounts,
      missingAccounts: missing,
      trailingBytes: data.length - args.end,
    },
    decodeStatus: "decoded",
    decodeReason: null,
  };
}

function joinChunks(chunks: string[]): Uint8Array {
  const parts = chunks.map((chunk) => {
    const binary = atob(chunk);
    return Uint8Array.from(binary, (char) => char.charCodeAt(0));
  });
  const joined = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    joined.set(part, offset);
    offset += part.length;
  }
  return joined;
}

/**
 * Decodes the `emit!` events in the logs against the IDL of the program that
 * wrote each one. A `Program data:` line that cannot be decoded is returned in
 * `undecoded` with the reason.
 */
export function decodeLogEvents(
  walked: WalkedLogs,
  flat: FlatInstruction[],
  idls: ReadonlyMap<string, NaclacIdl>,
): { events: ParsedEvent[]; undecoded: UndecodedLogData[] } {
  const events: ParsedEvent[] = [];
  const undecoded: UndecodedLogData[] = [];

  for (const entry of walked.data) {
    const target = flat[entry.invocation];
    const attributed = target !== undefined && target.programId === entry.programId;
    const where = {
      instructionIndex: attributed ? target.instructionIndex : null,
      innerInstructionIndex: attributed ? target.innerInstructionIndex : null,
    };
    const skip = (reason: string): void => {
      undecoded.push({ ...where, programId: entry.programId, reason, base64: entry.chunks });
    };

    const idl = idls.get(entry.programId);
    if (idl === undefined) {
      skip("no IDL was supplied for this program.");
      continue;
    }
    let bytes: Uint8Array;
    try {
      bytes = joinChunks(entry.chunks);
    } catch {
      skip("the log data is not valid base64.");
      continue;
    }
    const definition = idl.events.find((candidate) => startsWith(bytes, candidate.discriminator));
    if (definition === undefined) {
      skip(`no event of ${idl.name} has this discriminator.`);
      continue;
    }
    if (definition.alloc) {
      skip(`event ${definition.name} is an alloc event, which is not decoded yet.`);
      continue;
    }
    const decoded = decodeFields(
      definition.fields,
      bytes,
      definition.discriminator.length,
      idl.serialization,
    );
    if (!decoded.ok) {
      skip(decoded.reason);
      continue;
    }
    events.push({
      ...where,
      programId: entry.programId,
      programName: idl.name,
      name: definition.name,
      discriminator: definition.discriminator,
      data: decoded.values,
      trailingBytes: bytes.length - decoded.end,
      source: "log",
    });
  }
  return { events, undecoded };
}

const CUSTOM_REASON = /^custom program error: 0x([0-9a-fA-F]+)$/;

function customCodeOf(err: unknown): { position: number; code: number } | null {
  if (typeof err !== "object" || err === null || !("InstructionError" in err)) return null;
  const detail = err.InstructionError;
  if (!Array.isArray(detail) || detail.length !== 2) return null;
  const [position, error] = detail as [unknown, unknown];
  if (typeof position !== "number" || typeof error !== "object" || error === null) return null;
  if (!("Custom" in error)) return null;
  const code = error.Custom;
  return typeof code === "number" && Number.isInteger(code) ? { position, code } : null;
}

/**
 * Resolves a failed transaction's custom error against the IDL of the program
 * that raised it, taken from the first `failed:` log line. Returns null when
 * the error is not a custom program error. Throws a ParseError when the log's
 * error code disagrees with the transaction's.
 */
export function decodeFailure(
  err: unknown,
  logFailure: LogFailure | null,
  idls: ReadonlyMap<string, NaclacIdl>,
  instructions: ParsedInstruction[],
): DecodedError | null {
  const custom = customCodeOf(err);
  if (custom === null) return null;
  const { position, code } = custom;

  const logged = logFailure === null ? null : CUSTOM_REASON.exec(logFailure.reason);
  if (logged !== null && Number.parseInt(logged[1] as string, 16) !== code) {
    throw new ParseError(
      `the failed log line reports code 0x${logged[1]}, but the transaction reports ${code}.`,
      "meta.logMessages",
    );
  }

  const programId = logFailure?.programId ?? null;
  const idl = programId === null ? undefined : idls.get(programId);
  const base = {
    instructionIndex: position,
    programId,
    programName: idl?.name ?? null,
    code,
  };
  const unrecognised: DecodedError = {
    ...base,
    kind: "unrecognised",
    name: null,
    msg: null,
    accountIndex: null,
    accountName: null,
  };
  if (idl === undefined) return unrecognised;

  if (code >= FRAMEWORK_ERROR_BASE) {
    const relative = code - FRAMEWORK_ERROR_BASE;
    const accountIndex = Math.floor(relative / 100);
    const name = FRAMEWORK_ERRORS.get(relative % 100);
    if (name === undefined) return unrecognised;
    const failing = instructions.find(
      (i) =>
        i.instructionIndex === position &&
        i.innerInstructionIndex === null &&
        i.programId === programId,
    );
    return {
      ...base,
      kind: "framework",
      name,
      msg: null,
      accountIndex,
      accountName: failing?.decoded?.accounts[accountIndex]?.name ?? null,
    };
  }
  if (code >= USER_ERROR_BASE) {
    const entry = idl.errors.find((candidate) => candidate.code === code);
    if (entry !== undefined) {
      return {
        ...base,
        kind: "program",
        name: entry.name,
        msg: entry.msg,
        accountIndex: null,
        accountName: null,
      };
    }
  }
  return unrecognised;
}
