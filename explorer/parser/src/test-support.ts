/**
 * Shared helpers of the parser's tests: fixture locations, JSON readers, the
 * recorded devnet transactions with their index, and the counter IDLs.
 */

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { createIdlMap, parseNaclacIdl } from "./idl.ts";
import type { NaclacIdl } from "./idl.ts";
import type { RpcTransactionResult } from "./types.ts";

export const fixtures = path.resolve(import.meta.dirname, "../../fixtures");
export const rawDir = path.join(fixtures, "devnet/raw");
export const idlDir = path.join(fixtures, "devnet/idl");
export const mainnetSignature =
  "61CGEEKgqxqfnSimYajSkTN3oLuFNtexxrAYFmBnCdZgqPd4XqthjU15jmBguAoKwX9h18YBDckFkooPzY4hz2V4";

export interface IndexEntry {
  program: string;
  label: string;
  intent: string;
  requestedVersion: string;
  outcome: string;
}

export function readJson(file: string): any {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

export const index: Record<string, IndexEntry> = readJson(path.join(rawDir, "index.json"));

export function mainnetRaw(): RpcTransactionResult {
  return readJson(path.join(fixtures, `${mainnetSignature}.rpc.json`)).result;
}

export function devnetRaw(signature: string): RpcTransactionResult {
  return readJson(path.join(rawDir, `${signature}.rpc.json`)).result;
}

export function at<T>(items: readonly T[], position: number): T {
  const item = items[position];
  assert.ok(item !== undefined, `no item at position ${position}`);
  return item;
}

export function counterIdlDocuments(): Record<string, any> {
  const documents: Record<string, any> = {};
  for (const file of fs.readdirSync(idlDir)) {
    documents[file.replace(/\.json$/, "")] = readJson(path.join(idlDir, file));
  }
  return documents;
}

export function counterIdls(): Map<string, NaclacIdl> {
  return createIdlMap(Object.values(counterIdlDocuments()).map((doc) => parseNaclacIdl(doc)));
}
