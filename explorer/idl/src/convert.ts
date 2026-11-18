/**
 * Converts an IDL stored on chain back to the local form `naclac build` writes.
 * The on-chain form adds `metadata.spec`, moves each account's and event's
 * fields out of `accounts`/`events` into `types`, and renames `definedTypes` to
 * `types`. Port of `to_local_idl` in idl/src/converter.rs; results are
 * null-prototype objects, since the document is untrusted.
 */

import { IdlError } from "./errors.ts";

type Entry = Record<string, unknown>;

function isRecord(value: unknown): value is Entry {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function list(root: Entry, key: string): unknown[] {
  const value = root[key];
  if (!Array.isArray(value)) throw new IdlError(`the on-chain IDL has no \`${key}\` list.`);
  return value;
}

function nameOf(entry: unknown): string {
  if (!isRecord(entry) || typeof entry.name !== "string") {
    throw new IdlError("an entry of the on-chain IDL has no name.");
  }
  return entry.name;
}

function takeType(types: unknown[], name: string): Entry | undefined {
  const position = types.findIndex((candidate) => isRecord(candidate) && candidate.name === name);
  if (position < 0) return undefined;
  const [found] = types.splice(position, 1);
  return isRecord(found) ? found : undefined;
}

function blank(): Entry {
  return Object.create(null) as Entry;
}

function localAccount(entry: unknown, types: unknown[]): Entry {
  const name = nameOf(entry);
  const declared = takeType(types, name);
  if (declared === undefined) throw new IdlError(`the account \`${name}\` has no entry in \`types\`.`);
  const local = blank();
  local.name = name;
  if (declared.docs !== undefined) local.docs = declared.docs;
  local.discriminator = (entry as Entry).discriminator;
  local.type = declared.type;
  return local;
}

function localEvent(entry: unknown, types: unknown[]): Entry {
  const name = nameOf(entry);
  const declared = takeType(types, name);
  if (declared === undefined) throw new IdlError(`the event \`${name}\` has no entry in \`types\`.`);
  const source = entry as Entry;
  const local = blank();
  local.name = name;
  if (declared.docs !== undefined) local.docs = declared.docs;
  local.discriminator = source.discriminator;
  if (source.alloc !== undefined) local.alloc = source.alloc;
  local.fields = isRecord(declared.type) ? declared.type.fields : undefined;
  return local;
}

/** True when the IDL was written by naclac: only a naclac IDL has `metadata.serialization`. */
export function isNaclacIdl(document: unknown): boolean {
  return isRecord(document) && isRecord(document.metadata) && "serialization" in document.metadata;
}

/**
 * The local IDL an on-chain naclac IDL was made from. Each account and event
 * takes its fields back from the `types` entry of the same name, and the
 * remaining types become `definedTypes`. Only for a document for which
 * `isNaclacIdl` holds, since another IDL's `types` are not ours to move.
 */
export function toLocalIdl(onchain: unknown): Entry {
  if (!isRecord(onchain)) throw new IdlError("the on-chain IDL is not a JSON object.");
  const remaining = [...list(onchain, "types")];
  const accounts = list(onchain, "accounts").map((entry) => localAccount(entry, remaining));
  const events = list(onchain, "events").map((entry) => localEvent(entry, remaining));

  const local = blank();
  for (const [key, value] of Object.entries(onchain)) {
    if (key === "metadata") {
      if (isRecord(value)) {
        const metadata = blank();
        for (const [field, fieldValue] of Object.entries(value)) {
          if (field !== "spec") metadata[field] = fieldValue;
        }
        local[key] = metadata;
      } else {
        local[key] = value;
      }
    } else if (key === "accounts") {
      local[key] = accounts;
    } else if (key === "events") {
      local[key] = events;
    } else if (key === "types") {
      local.definedTypes = remaining;
    } else {
      local[key] = value;
    }
  }
  return local;
}
