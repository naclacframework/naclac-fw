/**
 * The error every parser check throws when the raw transaction is missing a
 * field or is inconsistent. `field` names the offending part of the raw
 * result, so a caller can report exactly what the provider returned wrongly.
 */
export class ParseError extends Error {
  readonly field: string;

  constructor(message: string, field: string) {
    super(`Naclac Error: ${message}`);
    this.name = "ParseError";
    this.field = field;
  }
}
