/**
 * A binary-stable wrapper for boolean values.
 * In Naclac Zero-Copy, bool is stored as a u8 (1 or 0).
 */
export interface Bool {
  value: number;
}

/**
 * A binary-stable wrapper for Opt<Address>.
 * Layout: [1 byte tag (1 when set), 32 bytes address]
 */
export interface OptPubkey {
  tag: number;
  value: string;
}

/**
 * A binary-stable wrapper for Opt<u64>.
 * Layout: [1 byte tag (1 when set), 8 bytes value]
 */
export interface OptU64 {
  tag: number;
  value: bigint | number;
}

/**
 * A standard string-represented Solana account address.
 */
export type Address = string;

/**
 * Asserts/converts a string value to a Solana Address type.
 */
export function address(value: string): Address {
  return value as Address;
}

