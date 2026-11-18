/**
 * Shared by the fixture recorders: the explorer root, `.env` loading, a plain
 * JSON-RPC call, and the devnet endpoint from DEVNET_RPC_URL, checked to really
 * be devnet before anything is recorded.
 */

import fs from "node:fs";
import path from "node:path";

export const root = path.resolve(import.meta.dirname, "../../..");

const DEVNET_GENESIS_HASH = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";
const REQUEST_TIMEOUT_MS = 20_000;

/**
 * Copies the `KEY=VALUE` lines of the file at `filePath` into `process.env`,
 * leaving variables that are already set untouched. A missing file is not an
 * error here: `requireEnv` reports every variable that is still missing.
 */
export function loadEnvFile(filePath: string): void {
  if (!fs.existsSync(filePath)) return;
  for (const line of fs.readFileSync(filePath, "utf8").split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed === "" || trimmed.startsWith("#")) continue;
    const separator = trimmed.indexOf("=");
    if (separator <= 0) continue;
    const key = trimmed.slice(0, separator).trim();
    if (process.env[key] === undefined) process.env[key] = trimmed.slice(separator + 1).trim();
  }
}

/** Returns the value of environment variable `name`, or throws an error that names it. */
export function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(
      `Naclac Error: ${name} is not set. Add it to explorer/.env (see explorer/.env.example).`,
    );
  }
  return value;
}

/** Sends one JSON-RPC request and returns the HTTP status and the response text unchanged. */
export async function rpcCall(
  url: string,
  method: string,
  params: unknown[],
): Promise<{ status: number; text: string }> {
  const response = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  return { status: response.status, text: await response.text() };
}

/** Loads explorer/.env and returns DEVNET_RPC_URL, throwing unless the endpoint reports devnet's genesis hash. */
export async function devnetUrl(): Promise<string> {
  loadEnvFile(path.join(root, ".env"));
  const url = requireEnv("DEVNET_RPC_URL");
  const genesis = JSON.parse((await rpcCall(url, "getGenesisHash", [])).text).result;
  if (genesis !== DEVNET_GENESIS_HASH) {
    throw new Error(
      `Naclac Error: DEVNET_RPC_URL reports genesis hash ${genesis}, not devnet's ` +
        `${DEVNET_GENESIS_HASH}. Point DEVNET_RPC_URL at a devnet endpoint.`,
    );
  }
  return url;
}
