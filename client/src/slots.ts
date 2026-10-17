import type { NaclacIdl } from "./idl";

type Field = { name: string; type: unknown; slots?: number };

/**
 * `type` as the codecs handle a fixed-capacity field: the plain `vec` or
 * `string` the IDL gives it, possibly inside arrays, becomes
 * `{ fixedVec: [T, slots] }` or `{ fixedString: slots }`.
 */
function fixedForm(type: unknown, slots: number): unknown {
  if (type === "string") {
    return { fixedString: slots };
  }
  if (typeof type === "object" && type !== null) {
    const shape = type as Record<string, unknown>;
    if ("vec" in shape) {
      return { fixedVec: [shape.vec, slots] };
    }
    if ("array" in shape && Array.isArray(shape.array)) {
      const [inner, length] = shape.array;
      return { array: [fixedForm(inner, slots), length] };
    }
  }
  throw new Error(`[Naclac] \`slots\` is set on a field of type ${JSON.stringify(type)}, which is not a vec or string.`);
}

function resolveFields(fields: readonly Field[], owner: string, bytemuck: boolean): void {
  for (const field of fields) {
    if (field.slots === undefined) continue;
    if (!bytemuck) {
      throw new Error(`[Naclac] "${owner}.${field.name}" has \`slots\` but the IDL's serialization is not bytemuck.`);
    }
    field.type = fixedForm(field.type, field.slots);
  }
}

/**
 * Returns a copy of `idl` in which every field with `slots` is typed as the
 * fixed-capacity type it is, so the codecs know its capacity and layout.
 * Fields without `slots` are untouched, and the input is never modified.
 */
export function resolveSlots<T extends NaclacIdl>(idl: T): T {
  const copy = JSON.parse(JSON.stringify(idl)) as {
    metadata: { serialization: string };
    accounts: { name: string; type: { fields: Field[] } }[];
    instructions: { name: string; args: Field[] }[];
    definedTypes: { name: string; type: { kind: string; fields?: Field[] } }[];
  };
  const bytemuck = copy.metadata.serialization === "bytemuck";
  for (const account of copy.accounts) {
    resolveFields(account.type.fields, account.name, bytemuck);
  }
  for (const instruction of copy.instructions) {
    resolveFields(instruction.args, instruction.name, bytemuck);
  }
  for (const defined of copy.definedTypes) {
    if (defined.type.kind === "struct" && defined.type.fields) {
      resolveFields(defined.type.fields, defined.name, bytemuck);
    }
  }
  return copy as unknown as T;
}
