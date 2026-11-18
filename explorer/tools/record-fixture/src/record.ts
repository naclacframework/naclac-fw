/**
 * Saves the raw getTransaction result of every landed transaction listed in a
 * devnet traffic record file (written by tools/devnet-traffic) to
 * explorer/fixtures/devnet/raw/<signature>.rpc.json, exactly as the node
 * returned it, and maintains raw/index.json describing each signature. Prints
 * which transaction versions came back and the JSON keys of each version's
 * transaction, message and meta, so the parser's types can be checked against
 * real data. Needs no installed packages: run it with Node 22.18 or later.
 *
 *   node src/record.ts <traffic-file.json> [output-folder]
 *
 * The output folder is a name under fixtures/devnet and defaults to "raw".
 *
 * Reads DEVNET_RPC_URL from explorer/.env or the environment. Existing files
 * are kept, never refetched. A signature that cannot be fetched is reported
 * with the reason; nothing is retried or substituted.
 */

import fs from "node:fs";
import path from "node:path";
import { devnetUrl, root, rpcCall } from "./common.ts";

const MAX_CONSECUTIVE_FAILURES = 5;
const CONCURRENCY = 2;

interface TrafficRecord {
  program: string;
  label: string;
  intent: string;
  version: string;
  signature: string | null;
  outcome: string;
}

interface IndexEntry {
  program: string;
  label: string;
  intent: string;
  requestedVersion: string;
  outcome: string;
}

type Fetched = { ok: true; text: string } | { ok: false; reason: string };

async function fetchTransaction(url: string, signature: string): Promise<Fetched> {
  let reply: { status: number; text: string };
  try {
    reply = await rpcCall(url, "getTransaction", [
      signature,
      { encoding: "json", maxSupportedTransactionVersion: 1, commitment: "finalized" },
    ]);
  } catch (error) {
    return { ok: false, reason: error instanceof Error ? error.message : String(error) };
  }
  if (reply.status !== 200) {
    return { ok: false, reason: `HTTP ${reply.status}: ${reply.text.slice(0, 200)}` };
  }
  let body: any;
  try {
    body = JSON.parse(reply.text);
  } catch {
    return { ok: false, reason: `response is not JSON: ${reply.text.slice(0, 200)}` };
  }
  if (body.error) return { ok: false, reason: `RPC error: ${JSON.stringify(body.error)}` };
  if (body.result === null || body.result === undefined) {
    return { ok: false, reason: "not found at finalized commitment" };
  }
  return { ok: true, text: reply.text };
}

function keysOf(value: unknown): string {
  return value && typeof value === "object" ? Object.keys(value).sort().join(", ") : String(value);
}

async function main(): Promise<void> {
  const trafficFile = process.argv[2];
  if (!trafficFile) {
    throw new Error(
      "Naclac Error: pass the traffic record file. Usage: node src/record.ts <traffic-file.json>",
    );
  }
  const url = await devnetUrl();

  const records: TrafficRecord[] = JSON.parse(fs.readFileSync(path.resolve(trafficFile), "utf8"));
  const landed = records.filter(
    (r) => r.signature !== null && (r.outcome === "succeeded" || r.outcome === "failed"),
  );
  const outName = process.argv[3] ?? "raw";
  if (!/^[A-Za-z0-9_-]+$/.test(outName)) {
    throw new Error("Naclac Error: the output folder name may hold only letters, digits, - and _.");
  }
  const outDir = path.join(root, "fixtures/devnet", outName);
  fs.mkdirSync(outDir, { recursive: true });
  const indexPath = path.join(outDir, "index.json");
  const index: Record<string, IndexEntry> = fs.existsSync(indexPath)
    ? JSON.parse(fs.readFileSync(indexPath, "utf8"))
    : {};

  const failures: string[] = [];
  const versionCounts: Record<string, number> = {};
  const shapes: Record<string, string> = {};
  let fetched = 0;
  let kept = 0;
  let consecutiveFailures = 0;
  let stopped = false;

  const note = (text: string): void => {
    const body = JSON.parse(text).result;
    const version = body.version === undefined ? "absent" : String(body.version);
    versionCounts[version] = (versionCounts[version] ?? 0) + 1;
    if (!(version in shapes)) {
      shapes[version] =
        `transaction: [${keysOf(body.transaction)}]\n` +
        `    message: [${keysOf(body.transaction?.message)}]\n` +
        `    meta: [${keysOf(body.meta)}]`;
    }
  };

  let next = 0;
  const worker = async (): Promise<void> => {
    while (!stopped) {
      const record = landed[next++];
      if (!record) return;
      const signature = record.signature as string;
      const file = path.join(outDir, `${signature}.rpc.json`);
      index[signature] = {
        program: record.program,
        label: record.label,
        intent: record.intent,
        requestedVersion: record.version,
        outcome: record.outcome,
      };
      if (fs.existsSync(file)) {
        kept++;
        note(fs.readFileSync(file, "utf8"));
        continue;
      }
      const result = await fetchTransaction(url, signature);
      if (!result.ok) {
        failures.push(`${signature} (${record.program} ${record.label}): ${result.reason}`);
        if (++consecutiveFailures >= MAX_CONSECUTIVE_FAILURES) {
          stopped = true;
        }
        continue;
      }
      consecutiveFailures = 0;
      fs.writeFileSync(file, result.text);
      fetched++;
      note(result.text);
    }
  };
  await Promise.all(Array.from({ length: CONCURRENCY }, worker));

  fs.writeFileSync(indexPath, JSON.stringify(index, null, 2));
  console.log(
    `${landed.length} landed transactions: ${fetched} fetched, ${kept} already saved, ` +
      `${failures.length} failed.`,
  );
  console.log(`versions seen: ${JSON.stringify(versionCounts)}`);
  for (const [version, shape] of Object.entries(shapes)) {
    console.log(`\nversion ${version}:\n    ${shape}`);
  }
  if (stopped) {
    console.log(`\nstopped after ${MAX_CONSECUTIVE_FAILURES} failures in a row.`);
  }
  for (const failure of failures) console.log(`  FAILED ${failure}`);
  console.log(`\nraw transactions are in ${outDir}`);
  if (failures.length > 0) process.exitCode = 1;
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
