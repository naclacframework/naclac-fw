/**
 * Shared plumbing for the devnet traffic script: environment, provider and
 * wallet setup, the devnet-only guard, sending a transaction with preflight
 * skipped so failing transactions still land on chain, status tracking, and
 * the record log written to explorer/fixtures/devnet.
 */

import * as naclac from "@naclac-fw/client/kit";
import fs from "node:fs";
import path from "node:path";

const DEVNET_GENESIS_HASH = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";

const FRAMEWORK_ERRORS: Record<number, string> = {
  1: "ConstraintMut",
  2: "ConstraintSigner",
  3: "ConstraintAddress",
  4: "ConstraintOwner",
  5: "ConstraintRentExempt",
  6: "ConstraintSeeds",
  7: "ConstraintExecutable",
  8: "ConstraintAccountIsNone",
  9: "ProgramIdMismatch",
  10: "AccountDataTooSmall",
  11: "AccountBorrowFailed",
  12: "InvalidInstructionData",
  13: "InsufficientFunds",
  14: "AccountAlreadyInitialized",
  15: "AccountNotInitialized",
  16: "NotEnoughAccountKeys",
  17: "MaxSeedLengthExceeded",
  18: "UnsupportedSysvar",
  19: "InvalidRealloc",
  20: "ArithmeticOverflow",
  21: "Unauthorized",
  22: "InvalidAccountDiscriminator",
  23: "DeserializationFailed",
  24: "SerializationFailed",
  25: "ConstraintDuplicateMutableAccount",
  26: "ConstraintClose",
  29: "TooManyExtraAccounts",
  30: "TooManyCpiAccounts",
  31: "TooManyAccountEntries",
  32: "CapacityExceeded",
  33: "InvalidUtf8",
  34: "ConstraintDuplicateAccount",
  35: "ConstraintTokenMint",
  36: "ConstraintTokenAuthority",
  37: "ConstraintMintDecimals",
  38: "ConstraintMintAuthority",
  39: "ConstraintMintFreezeAuthority",
};

export type TxVersion = "v0" | "v1" | "legacy";
export type Intent = "success" | "failure";
export type Outcome = "succeeded" | "failed" | "not-landed" | "skipped";

export interface IdlErrorEntry {
  code: number;
  name: string;
}

export interface TrafficRecord {
  suite: string;
  program: string;
  programId: string;
  label: string;
  intent: Intent;
  version: TxVersion;
  signature: string | null;
  outcome: Outcome;
  matchedIntent: boolean;
  error: unknown;
  customCode: number | null;
  customCodeMeaning: string | null;
  finalized: boolean;
  note: string | null;
}

export interface Ctx {
  provider: naclac.NaclacProvider;
  payer: naclac.KeyPairSigner;
  records: TrafficRecord[];
}

export interface RunSpec {
  suite: string;
  program: string;
  programId: string;
  idlErrors: readonly IdlErrorEntry[];
  label: string;
  intent: Intent;
  version: TxVersion;
  build: () => any;
  note?: string;
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

/**
 * Copies the `KEY=VALUE` lines of the file at `filePath` into `process.env`,
 * leaving variables that are already set untouched. Blank lines and lines
 * starting with `#` are ignored; values are taken literally, so backslashes in
 * Windows paths survive. A missing file is not an error here: `requireEnv`
 * reports every variable that is still missing.
 */
export function loadEnvFile(filePath: string): void {
  if (!fs.existsSync(filePath)) return;
  for (const line of fs.readFileSync(filePath, "utf8").split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed === "" || trimmed.startsWith("#")) continue;
    const separator = trimmed.indexOf("=");
    if (separator <= 0) continue;
    const key = trimmed.slice(0, separator).trim();
    const value = trimmed.slice(separator + 1).trim();
    if (process.env[key] === undefined) process.env[key] = value;
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Builds the provider and payer from DEVNET_RPC_URL, DEVNET_WS_URL and
 * WALLET_PATH, and throws unless the endpoint reports the devnet genesis hash,
 * so a mainnet URL can never be used by mistake. The WebSocket URL is a
 * separate setting because some providers serve subscriptions on another host
 * than their HTTP endpoint.
 */
export async function createContext(): Promise<Ctx> {
  const rpcUrl = requireEnv("DEVNET_RPC_URL");
  const wsUrl = requireEnv("DEVNET_WS_URL");
  const walletPath = requireEnv("WALLET_PATH");
  const payer = await naclac.loadNodeWallet(walletPath);
  const provider = naclac.createProvider(rpcUrl, payer, wsUrl);
  const genesisHash = await (provider.rpc as any).getGenesisHash().send();
  if (genesisHash !== DEVNET_GENESIS_HASH) {
    throw new Error(
      `Naclac Error: DEVNET_RPC_URL reports genesis hash ${genesisHash}, not devnet's ` +
        `${DEVNET_GENESIS_HASH}. Point DEVNET_RPC_URL at a devnet endpoint.`,
    );
  }
  return { provider, payer, records: [] };
}

async function waitForStatus(
  rpc: any,
  signature: string,
  minimum: "confirmed" | "finalized",
  timeoutMs: number,
): Promise<any | null> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const response = await rpc
      .getSignatureStatuses([signature], { searchTransactionHistory: true })
      .send();
    const status = response.value[0];
    if (
      status &&
      (status.confirmationStatus === "finalized" ||
        (minimum === "confirmed" && status.confirmationStatus === "confirmed"))
    ) {
      return status;
    }
    await sleep(1000);
  }
  return null;
}

function customCodeOf(err: any): number | null {
  const instructionError = err?.InstructionError;
  if (!Array.isArray(instructionError)) return null;
  const detail = instructionError[1];
  const code = detail && typeof detail === "object" ? detail.Custom : undefined;
  return code === undefined ? null : Number(code);
}

function describeCode(code: number, idlErrors: readonly IdlErrorEntry[]): string {
  if (code >= 10_000) {
    const relative = code - 10_000;
    const accountIndex = Math.floor(relative / 100);
    const type = relative % 100;
    const name = FRAMEWORK_ERRORS[type] ?? `unknown type ${type}`;
    return `framework ${name} (account index ${accountIndex})`;
  }
  const entry = idlErrors.find((e) => e.code === code);
  return entry ? `program error ${entry.name}` : `unrecognised custom code ${code}`;
}

/**
 * Sends the transaction built by `spec.build()` as `spec.version` with
 * preflight skipped, waits for it to be confirmed, and appends a record of what
 * happened to `ctx.records`. A transaction that fails on chain is a normal
 * result and is recorded; nothing is thrown for it.
 */
export async function run(ctx: Ctx, spec: RunSpec): Promise<TrafficRecord> {
  const rpc = ctx.provider.rpc as any;
  let signature: string | null = null;
  let status: any = null;
  let note = spec.note ?? null;

  try {
    signature = await spec.build().version(spec.version).send({ skipPreflight: true });
    status = await waitForStatus(rpc, signature as string, "confirmed", 60_000);
    if (!status) note = [note, "not confirmed within 60s"].filter(Boolean).join("; ");
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    note = [note, `send failed: ${message}`].filter(Boolean).join("; ");
  }

  const landed = status !== null;
  const failed = landed && status.err !== null;
  const outcome: Outcome = !landed ? "not-landed" : failed ? "failed" : "succeeded";
  const code = failed ? customCodeOf(status.err) : null;

  const record: TrafficRecord = {
    suite: spec.suite,
    program: spec.program,
    programId: spec.programId,
    label: spec.label,
    intent: spec.intent,
    version: spec.version,
    signature,
    outcome,
    matchedIntent:
      (spec.intent === "success" && outcome === "succeeded") ||
      (spec.intent === "failure" && outcome === "failed"),
    error: failed ? status.err : null,
    customCode: code,
    customCodeMeaning: code === null ? null : describeCode(code, spec.idlErrors),
    finalized: false,
    note,
  };
  ctx.records.push(record);
  console.log(
    `  ${record.matchedIntent ? "ok  " : "MISMATCH"} ${spec.program} ${spec.label} -> ${outcome}` +
      (record.customCodeMeaning ? ` (${record.customCodeMeaning})` : ""),
  );
  return record;
}

/** Records a step that was deliberately not run, with the reason. */
export function skipped(
  ctx: Ctx,
  spec: Omit<RunSpec, "build" | "version"> & { version?: TxVersion },
  reason: string,
): void {
  ctx.records.push({
    suite: spec.suite,
    program: spec.program,
    programId: spec.programId,
    label: spec.label,
    intent: spec.intent,
    version: spec.version ?? "v1",
    signature: null,
    outcome: "skipped",
    matchedIntent: true,
    error: null,
    customCode: null,
    customCodeMeaning: null,
    finalized: false,
    note: reason,
  });
  console.log(`  skip ${spec.program} ${spec.label}: ${reason}`);
}

/** Waits up to `timeoutMs` for every recorded signature to be finalized and marks the ones that are. */
export async function waitForFinalized(ctx: Ctx, timeoutMs: number): Promise<void> {
  const rpc = ctx.provider.rpc as any;
  const pending = ctx.records.filter((r) => r.signature !== null && r.outcome !== "not-landed");
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const open = pending.filter((r) => !r.finalized);
    if (open.length === 0) return;
    for (let i = 0; i < open.length; i += 100) {
      const batch = open.slice(i, i + 100);
      const response = await rpc
        .getSignatureStatuses(batch.map((r) => r.signature), { searchTransactionHistory: true })
        .send();
      batch.forEach((record, index) => {
        if (response.value[index]?.confirmationStatus === "finalized") record.finalized = true;
      });
    }
    await sleep(5000);
  }
}

/** Writes the records to explorer/fixtures/devnet/traffic-<timestamp>.json and returns the path. */
export function writeRecords(ctx: Ctx): string {
  const dir = path.resolve(__dirname, "../../../fixtures/devnet");
  fs.mkdirSync(dir, { recursive: true });
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const file = path.join(dir, `traffic-${stamp}.json`);
  const replacer = (_key: string, value: unknown) =>
    typeof value === "bigint" ? value.toString() : value;
  fs.writeFileSync(file, JSON.stringify(ctx.records, replacer, 2));
  return file;
}
