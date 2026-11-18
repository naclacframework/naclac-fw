/**
 * Base58 (Bitcoin alphabet) encoding and decoding of byte arrays, as used for
 * Solana addresses.
 */

const ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const DIGIT_OF = new Map<string, number>(Array.from(ALPHABET, (char, index) => [char, index]));

/** Decodes base58 `text` to bytes, or returns null when it holds a character outside the alphabet. */
export function decodeBase58(text: string): Uint8Array | null {
  const bytes: number[] = [];
  for (const char of text) {
    let carry = DIGIT_OF.get(char);
    if (carry === undefined) return null;
    for (let i = 0; i < bytes.length; i++) {
      carry += (bytes[i] ?? 0) * 58;
      bytes[i] = carry & 0xff;
      carry >>= 8;
    }
    while (carry > 0) {
      bytes.push(carry & 0xff);
      carry >>= 8;
    }
  }
  let zeros = 0;
  while (zeros < text.length && text.charAt(zeros) === "1") zeros++;
  const result = new Uint8Array(zeros + bytes.length);
  bytes.forEach((byte, i) => {
    result[result.length - 1 - i] = byte;
  });
  return result;
}

/** Encodes `bytes` as base58. */
export function encodeBase58(bytes: Uint8Array): string {
  const digits: number[] = [];
  for (const byte of bytes) {
    let carry = byte;
    for (let i = 0; i < digits.length; i++) {
      carry += (digits[i] ?? 0) << 8;
      digits[i] = carry % 58;
      carry = Math.floor(carry / 58);
    }
    while (carry > 0) {
      digits.push(carry % 58);
      carry = Math.floor(carry / 58);
    }
  }
  let zeros = 0;
  while (zeros < bytes.length && bytes[zeros] === 0) zeros++;
  return "1".repeat(zeros) + digits.reverse().map((digit) => ALPHABET.charAt(digit)).join("");
}
