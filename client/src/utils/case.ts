/**
 * Converts a snake_case IDL identifier (field name, account-role name) to
 * camelCase for the TypeScript-facing surface — the on-disk IDL JSON and
 * every raw runtime lookup key (`program.methods.<name>`, PDA-seed pool
 * keys, etc.) stay snake_case exactly as emitted from the Rust source;
 * only the property name a caller actually types/reads is converted, here
 * and via `heck::ToLowerCamelCase` on the Rust codegen side — both must
 * agree, since one produces a generated TS interface's declared property
 * name and the other produces the same object's actual runtime key.
 */
export function toCamelCase(raw: string): string {
  return raw.replace(/_([a-zA-Z0-9])/g, (_, c) => c.toUpperCase());
}
