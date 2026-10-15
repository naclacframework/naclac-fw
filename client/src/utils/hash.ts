import { sha256 } from "@noble/hashes/sha2.js";

/** The default, name-derived discriminator: `sha256("<namespace>:<name>")[0..8]`. */
export function getDiscriminator(namespace: string, name: string): Uint8Array {
  const bytes = new TextEncoder().encode(`${namespace}:${name}`);
  const hash = sha256(bytes);
  return hash.slice(0, 8);
}

/**
 * The discriminator the program's IDL records for an item, which is the
 * name-derived default unless the program declares a custom one. Falls back
 * to the default only for an IDL that predates recorded discriminators.
 */
export function resolveDiscriminator(
  recorded: readonly number[] | undefined,
  namespace: string,
  name: string,
): Uint8Array {
  return recorded ? Uint8Array.from(recorded) : getDiscriminator(namespace, name);
}
