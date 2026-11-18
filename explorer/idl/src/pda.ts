/**
 * Program derived addresses: SHA-256 over the seeds, a bump, the program id and
 * a fixed marker, accepted only when the hash is not a valid ed25519 point.
 * Uses the web crypto API for SHA-256 and BigInt for the curve check, so it
 * runs unchanged in Node, Deno and browsers.
 */

import { encodeBase58 } from "./base58.ts";
import { IdlError } from "./errors.ts";

const MAX_SEEDS = 16;
const MAX_SEED_LENGTH = 32;
const MARKER = new TextEncoder().encode("ProgramDerivedAddress");

const P = (1n << 255n) - 19n;

function modPow(base: bigint, exponent: bigint, modulus: bigint): bigint {
  let result = 1n;
  let factor = base % modulus;
  let remaining = exponent;
  while (remaining > 0n) {
    if (remaining & 1n) result = (result * factor) % modulus;
    factor = (factor * factor) % modulus;
    remaining >>= 1n;
  }
  return result;
}

const D = ((P - 121665n) * modPow(121666n, P - 2n, P)) % P;

/**
 * True when the 32 `bytes` decode to a point of the ed25519 curve, as
 * curve25519-dalek's `CompressedEdwardsY::decompress` decides it: the x
 * coordinate's square exists, `(y^2 - 1) / (d * y^2 + 1)` being a square.
 */
export function isOnCurve(bytes: Uint8Array): boolean {
  if (bytes.length !== 32) throw new IdlError(`a curve point is 32 bytes, not ${bytes.length}.`);
  let y = 0n;
  for (let i = 31; i >= 0; i--) y = (y << 8n) | BigInt(bytes[i] ?? 0);
  y = (y & ((1n << 255n) - 1n)) % P;
  const ySquared = (y * y) % P;
  const numerator = (ySquared + P - 1n) % P;
  if (numerator === 0n) return true;
  const denominator = (D * ySquared + 1n) % P;
  const ratio = (numerator * modPow(denominator, P - 2n, P)) % P;
  return modPow(ratio, (P - 1n) / 2n, P) === 1n;
}

async function sha256(data: Uint8Array): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", data as BufferSource));
}

function checkSeeds(seeds: Uint8Array[]): void {
  if (seeds.length > MAX_SEEDS) {
    throw new IdlError(`a program address takes at most ${MAX_SEEDS} seeds, not ${seeds.length}.`);
  }
  for (const seed of seeds) {
    if (seed.length > MAX_SEED_LENGTH) {
      throw new IdlError(`a seed is at most ${MAX_SEED_LENGTH} bytes, not ${seed.length}.`);
    }
  }
}

function concat(parts: Uint8Array[]): Uint8Array {
  const joined = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    joined.set(part, offset);
    offset += part.length;
  }
  return joined;
}

/**
 * The address for exactly these `seeds` (a bump included, if any) under
 * `programId`, or null when the hash lands on the curve and so is not a valid
 * program address.
 */
export async function createProgramAddress(
  seeds: Uint8Array[],
  programId: Uint8Array,
): Promise<string | null> {
  checkSeeds(seeds);
  const hash = await sha256(concat([...seeds, programId, MARKER]));
  return isOnCurve(hash) ? null : encodeBase58(hash);
}

/** The program address for `seeds` under `programId` with the highest bump that gives a valid address. */
export async function findProgramAddress(
  seeds: Uint8Array[],
  programId: Uint8Array,
): Promise<{ address: string; bump: number }> {
  checkSeeds(seeds);
  for (let bump = 255; bump >= 0; bump--) {
    const address = await createProgramAddress([...seeds, Uint8Array.of(bump)], programId);
    if (address !== null) return { address, bump };
  }
  throw new IdlError("no bump gives a valid program address for these seeds.");
}
