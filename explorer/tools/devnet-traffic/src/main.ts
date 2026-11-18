/**
 * Entry point of the devnet traffic script. Usage:
 *
 *   node -r ts-node/register/transpile-only src/main.ts counter|escrow|all [program,program,...]
 *
 * Sends successful and deliberately failing transactions to the example
 * programs on devnet, waits for finalization, prints a report, and writes every
 * transaction to explorer/fixtures/devnet/traffic-<timestamp>.json for use as
 * parser test material. Needs DEVNET_RPC_URL, DEVNET_WS_URL and WALLET_PATH,
 * read from explorer/.env or the environment.
 */

import path from "node:path";
import { COUNTER_VARIANTS, runCounter } from "./counter";
import { createContext, loadEnvFile, waitForFinalized, writeRecords } from "./chain";
import { ESCROW_VARIANTS, runEscrow } from "./escrow";

const FINALIZE_TIMEOUT_MS = 120_000;

function usage(problem: string): never {
  throw new Error(
    `Naclac Error: ${problem} Usage: node -r ts-node/register/transpile-only src/main.ts ` +
      `counter|escrow|all [program,program,...]`,
  );
}

function selectVariants<T extends { program: string }>(
  all: readonly T[],
  filter: string[] | null,
  suite: string,
  requireMatch: boolean,
): T[] {
  const chosen = filter === null ? [...all] : all.filter((v) => filter.includes(v.program));
  if (requireMatch && chosen.length === 0) {
    usage(`None of the listed programs belongs to the ${suite} suite.`);
  }
  return chosen;
}

async function main(): Promise<void> {
  const suite = process.argv[2];
  if (suite !== "counter" && suite !== "escrow" && suite !== "all") {
    usage("The first argument must be counter, escrow or all.");
  }
  const filter = process.argv[3] ? process.argv[3].split(",") : null;
  const programNames = [...COUNTER_VARIANTS, ...ESCROW_VARIANTS].map((v) => v.program);
  for (const name of filter ?? []) {
    if (!programNames.includes(name)) usage(`Unknown program "${name}".`);
  }

  loadEnvFile(path.resolve(__dirname, "../../../.env"));
  const ctx = await createContext();
  const balance = await ctx.provider.getBalance(ctx.payer.address);
  console.log(`payer ${ctx.payer.address}, balance ${Number(balance) / 1e9} SOL`);

  if (suite === "counter" || suite === "all") {
    await runCounter(ctx, selectVariants(COUNTER_VARIANTS, filter, "counter", suite === "counter"));
  }
  if (suite === "escrow" || suite === "all") {
    await runEscrow(ctx, selectVariants(ESCROW_VARIANTS, filter, "escrow", suite === "escrow"));
  }

  console.log("\nwaiting for finalization...");
  await waitForFinalized(ctx, FINALIZE_TIMEOUT_MS);
  const file = writeRecords(ctx);

  const mismatches = ctx.records.filter((r) => !r.matchedIntent);
  const finalized = ctx.records.filter((r) => r.finalized).length;
  const landed = ctx.records.filter((r) => r.signature !== null).length;
  console.log(
    `\n${ctx.records.length} steps, ${landed} with a signature, ${finalized} finalized, ` +
      `${mismatches.length} did not match their intent.`,
  );
  for (const record of mismatches) {
    console.log(
      `  MISMATCH ${record.program} ${record.label}: expected ${record.intent}, got ${record.outcome}` +
        (record.note ? ` (${record.note})` : ""),
    );
  }
  console.log(`records written to ${file}`);
  if (mismatches.length > 0) process.exitCode = 1;
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
