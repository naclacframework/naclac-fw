/**
 * The error thrown for input the IDL reader cannot work with: a malformed
 * program id, or an on-chain IDL document that cannot be converted back.
 * Expected states of a program's metadata (absent, unfinished, unreadable) are
 * returned as results by `resolveIdl`, not thrown.
 */
export class IdlError extends Error {
  constructor(message: string) {
    super(`Naclac Error: ${message}`);
    this.name = "IdlError";
  }
}
