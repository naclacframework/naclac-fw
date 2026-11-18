/**
 * Shared helpers of the IDL reader's tests: fixture access, and a test-only
 * port of the Rust `to_onchain_idl`, used to build on-chain documents from the
 * local counter IDLs.
 */

import fs from "node:fs";
import path from "node:path";

export const fixtures = path.resolve(import.meta.dirname, "../../fixtures");
export const idlDir = path.join(fixtures, "devnet/idl");
export const rawDir = path.join(fixtures, "devnet/raw");

export function readJson(file: string): any {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

export function counterLocalIdls(): Record<string, any> {
  const idls: Record<string, any> = {};
  for (const file of fs.readdirSync(idlDir)) {
    idls[file.replace(/\.json$/, "")] = readJson(path.join(idlDir, file));
  }
  return idls;
}

function typeEntry(name: string, docs: unknown, type: unknown): any {
  const entry: any = { name };
  if (Array.isArray(docs) && docs.length > 0) entry.docs = docs;
  entry.type = type;
  return entry;
}

/** Builds the on-chain form of a local IDL, as idl/src/converter.rs `to_onchain_idl` does. */
export function toOnchain(local: any): any {
  const types = [
    ...local.accounts.map((a: any) => typeEntry(a.name, a.docs, a.type)),
    ...local.events.map((e: any) => typeEntry(e.name, e.docs, { kind: "struct", fields: e.fields })),
    ...local.definedTypes,
  ];
  const onchain: any = {};
  for (const [key, value] of Object.entries(local)) {
    if (key === "metadata") {
      onchain.metadata = { ...(value as object), spec: "0.1.0" };
    } else if (key === "accounts") {
      onchain.accounts = local.accounts.map((a: any) => ({
        name: a.name,
        discriminator: a.discriminator,
      }));
    } else if (key === "events") {
      onchain.events = local.events.map((e: any) => ({
        name: e.name,
        discriminator: e.discriminator,
        ...(e.alloc ? { alloc: true } : {}),
      }));
    } else if (key === "definedTypes") {
      onchain.types = types;
    } else {
      onchain[key] = value;
    }
  }
  return onchain;
}

export const pmpDir = path.join(fixtures, "devnet/pmp");

export interface RecordedMetadata {
  name: string;
  programId: string;
  metadataAddress: string;
  account: { owner: string; data: Uint8Array } | null;
}

/** The metadata accounts recorded from devnet by tools/record-fixture/src/record-idl.ts. */
export function recordedMetadata(): RecordedMetadata[] {
  const index = readJson(path.join(pmpDir, "index.json"));
  return Object.entries(index).map(([programId, entry]: [string, any]) => {
    const value = readJson(path.join(pmpDir, `${programId}.account.json`)).result.value;
    return {
      name: entry.name,
      programId,
      metadataAddress: entry.metadataAddress,
      account:
        value === null
          ? null
          : { owner: value.owner, data: new Uint8Array(Buffer.from(value.data[0], "base64")) },
    };
  });
}

export function plain(value: unknown): unknown {
  return JSON.parse(JSON.stringify(value));
}
