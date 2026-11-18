/**
 * The naclac IDL (local form, as `naclac build` writes it) as the parser sees
 * it, and the validation of an untrusted IDL document into that shape. Only the
 * parts the parser uses are read; everything else is ignored.
 */

import { decodeBase58 } from "./base58.ts";
import { ParseError } from "./errors.ts";

export type Serialization = "borsh" | "wincode" | "bytemuck";

export interface IdlField {
  name: string;
  type: unknown;
}

export interface IdlInstructionAccount {
  name: string;
  writable: boolean;
  signer: boolean;
  optional: boolean;
  address: string | null;
}

export interface IdlInstruction {
  name: string;
  discriminator: number[];
  accounts: IdlInstructionAccount[];
  args: IdlField[];
}

export interface IdlEvent {
  name: string;
  discriminator: number[];
  fields: IdlField[];
  alloc: boolean;
}

export interface IdlErrorEntry {
  code: number;
  name: string;
  msg: string | null;
}

export interface NaclacIdl {
  address: string;
  name: string;
  serialization: Serialization;
  instructions: IdlInstruction[];
  events: IdlEvent[];
  errors: IdlErrorEntry[];
  eventCpiTag: number[] | null;
}

const SERIALIZATIONS: readonly string[] = ["borsh", "wincode", "bytemuck"];
const MAX_DISCRIMINATOR_WIDTH = 8;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function fail(path: string, problem: string): never {
  throw new ParseError(`IDL ${path} ${problem}.`, `idl.${path}`);
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (!isRecord(value)) return fail(path, "must be an object");
  return value;
}

function list(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) return fail(path, "must be a list");
  return value;
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string" || value === "") return fail(path, "must be a non-empty string");
  return value;
}

function optionalFlag(source: Record<string, unknown>, key: string, path: string): boolean {
  const value = source[key];
  if (value === undefined) return false;
  if (typeof value !== "boolean") return fail(`${path}.${key}`, "must be true or false");
  return value;
}

function bytes(value: unknown, path: string, minimum: number): number[] {
  const items = list(value, path);
  if (items.length < minimum || items.length > MAX_DISCRIMINATOR_WIDTH) {
    return fail(path, `must hold ${minimum} to ${MAX_DISCRIMINATOR_WIDTH} bytes`);
  }
  return items.map((item, i) => {
    if (typeof item !== "number" || !Number.isInteger(item) || item < 0 || item > 255) {
      return fail(`${path}[${i}]`, "must be a byte");
    }
    return item;
  });
}

function fields(value: unknown, path: string): IdlField[] {
  const seen = new Set<string>();
  return list(value, path).map((item, i) => {
    const source = record(item, `${path}[${i}]`);
    const name = text(source.name, `${path}[${i}].name`);
    if (seen.has(name)) return fail(`${path}[${i}].name`, "repeats an earlier field name");
    seen.add(name);
    return { name, type: source.type };
  });
}

function checkCategory(items: { discriminator: number[] }[], path: string): void {
  const first = items[0];
  if (first === undefined) return;
  const seen = new Set<string>();
  for (const [i, item] of items.entries()) {
    if (item.discriminator.length !== first.discriminator.length) {
      fail(`${path}[${i}].discriminator`, "has a different width than the first entry");
    }
    const key = item.discriminator.join(",");
    if (seen.has(key)) fail(`${path}[${i}].discriminator`, "repeats an earlier discriminator");
    seen.add(key);
  }
}

function instruction(value: unknown, path: string): IdlInstruction {
  const source = record(value, path);
  const accounts = list(source.accounts, `${path}.accounts`).map((item, i) => {
    const account = record(item, `${path}.accounts[${i}]`);
    const address = account.address;
    if (address !== undefined && typeof address !== "string") {
      return fail(`${path}.accounts[${i}].address`, "must be a string");
    }
    return {
      name: text(account.name, `${path}.accounts[${i}].name`),
      writable: optionalFlag(account, "writable", `${path}.accounts[${i}]`),
      signer: optionalFlag(account, "signer", `${path}.accounts[${i}]`),
      optional: optionalFlag(account, "optional", `${path}.accounts[${i}]`),
      address: address ?? null,
    };
  });
  return {
    name: text(source.name, `${path}.name`),
    discriminator: bytes(source.discriminator, `${path}.discriminator`, 1),
    accounts,
    args: fields(source.args, `${path}.args`),
  };
}

function event(value: unknown, path: string): IdlEvent {
  const source = record(value, path);
  return {
    name: text(source.name, `${path}.name`),
    discriminator: bytes(source.discriminator, `${path}.discriminator`, 1),
    fields: fields(source.fields, `${path}.fields`),
    alloc: optionalFlag(source, "alloc", path),
  };
}

function errorEntry(value: unknown, path: string): IdlErrorEntry {
  const source = record(value, path);
  const code = source.code;
  if (typeof code !== "number" || !Number.isInteger(code) || code < 0) {
    return fail(`${path}.code`, "must be a non-negative integer");
  }
  const msg = source.msg;
  if (msg !== undefined && typeof msg !== "string") return fail(`${path}.msg`, "must be a string");
  return { code, name: text(source.name, `${path}.name`), msg: msg ?? null };
}

/**
 * Validates an untrusted IDL document (already parsed from JSON) and returns
 * the parser's view of it. Throws a ParseError naming the offending path.
 */
export function parseNaclacIdl(document: unknown): NaclacIdl {
  const root = record(document, "root");
  const address = text(root.address, "address");
  if (decodeBase58(address, "idl.address").length !== 32) {
    fail("address", "must decode to 32 bytes");
  }
  const metadata = record(root.metadata, "metadata");
  const serialization = metadata.serialization;
  if (typeof serialization !== "string" || !SERIALIZATIONS.includes(serialization)) {
    fail("metadata.serialization", "must be borsh, wincode or bytemuck");
  }

  const instructions = list(root.instructions, "instructions").map((item, i) =>
    instruction(item, `instructions[${i}]`),
  );
  const events = list(root.events ?? [], "events").map((item, i) => event(item, `events[${i}]`));
  const errors = list(root.errors ?? [], "errors").map((item, i) =>
    errorEntry(item, `errors[${i}]`),
  );
  checkCategory(instructions, "instructions");
  checkCategory(events, "events");

  const tag = root.eventCpiTag;
  return {
    address,
    name: text(metadata.name, "metadata.name"),
    serialization: serialization as Serialization,
    instructions,
    events,
    errors,
    eventCpiTag: tag === undefined ? null : bytes(tag, "eventCpiTag", 1),
  };
}

/** Builds the program-id lookup the parser takes, rejecting two IDLs for one program. */
export function createIdlMap(idls: NaclacIdl[]): Map<string, NaclacIdl> {
  const map = new Map<string, NaclacIdl>();
  for (const idl of idls) {
    if (map.has(idl.address)) {
      throw new ParseError(`two IDLs were given for program ${idl.address}.`, "idl.address");
    }
    map.set(idl.address, idl);
  }
  return map;
}
