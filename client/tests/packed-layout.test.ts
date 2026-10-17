/**
 * @naclac/client — the packed zero-copy layout (alignment 1, no padding),
 * against byte sequences worked out by hand from the layout rules:
 * fields in declaration order, a data-carrying enum as its tag, the active
 * variant's fields, then zeros up to the largest variant, `Bool` read as
 * true for any non-zero byte.
 *
 * Run: pnpm test
 */

import { strict as assert } from "assert";
import { getIdlCodec } from "../src/coder/types";

const definedTypes = [
  {
    name: "Mixed",
    type: {
      kind: "struct",
      fields: [
        { name: "a", type: "u8" },
        { name: "b", type: "u64" },
        { name: "c", type: "u16" },
      ],
    },
  },
  {
    name: "Shape",
    type: {
      kind: "enum",
      repr: "u8",
      variants: [
        { name: "Empty", discriminant: "0" },
        { name: "Wide", discriminant: "1", fields: [{ name: "x", type: "u32" }] },
        {
          name: "Both",
          discriminant: "2",
          fields: [
            { name: "y", type: "u8" },
            { name: "z", type: "u64" },
          ],
        },
      ],
    },
  },
  {
    name: "Holder",
    type: {
      kind: "struct",
      fields: [
        { name: "flag", type: "u8" },
        { name: "shape", type: { defined: "Shape" } },
        { name: "tail", type: "u16" },
      ],
    },
  },
] as const;

const codecOf = (type: unknown) => getIdlCodec(type, definedTypes as any, true);

describe("packed zero-copy layout", () => {
  it("lays a struct out with no padding between or after fields", () => {
    const codec = codecOf({ defined: "Mixed" });
    const bytes = codec.encode({ a: 7, b: 0x0102030405060708n, c: 0x0a0b }) as Uint8Array;
    assert.deepEqual([...bytes], [7, 8, 7, 6, 5, 4, 3, 2, 1, 0x0b, 0x0a]);
    assert.deepEqual(codec.decode(bytes), { a: 7, b: 0x0102030405060708n, c: 0x0a0b });
  });

  it("stores an enum as tag, active fields and zeros up to the largest variant", () => {
    const codec = codecOf({ defined: "Shape" });

    const empty = codec.encode({ kind: "Empty" }) as Uint8Array;
    assert.deepEqual([...empty], [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    const wide = codec.encode({ kind: "Wide", x: 0x01020304 }) as Uint8Array;
    assert.deepEqual([...wide], [1, 4, 3, 2, 1, 0, 0, 0, 0, 0]);

    const both = codec.encode({ kind: "Both", y: 9, z: 1n }) as Uint8Array;
    assert.deepEqual([...both], [2, 9, 1, 0, 0, 0, 0, 0, 0, 0]);

    assert.deepEqual(codec.decode(both), { kind: "Both", y: 9, z: 1n });
    assert.deepEqual(codec.decode(wide), { kind: "Wide", x: 0x01020304 });
  });

  it("packs a struct that holds an enum", () => {
    const codec = codecOf({ defined: "Holder" });
    const bytes = codec.encode({ flag: 5, shape: { kind: "Wide", x: 1 }, tail: 0x0102 }) as Uint8Array;
    assert.equal(bytes.length, 1 + 10 + 2);
    assert.deepEqual([...bytes], [5, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 2, 1]);
  });

  it("rejects an enum tag that names no variant", () => {
    const codec = codecOf({ defined: "Shape" });
    assert.throws(() => codec.decode(Uint8Array.of(9, 0, 0, 0, 0, 0, 0, 0, 0, 0)), /Invalid enum discriminant 9/);
  });

  it("reads any non-zero Bool byte as true and writes exactly 1 or 0", () => {
    const codec = codecOf("bool");
    assert.equal(codec.decode(Uint8Array.of(2)), true);
    assert.equal(codec.decode(Uint8Array.of(255)), true);
    assert.equal(codec.decode(Uint8Array.of(0)), false);
    assert.deepEqual([...(codec.encode(true) as Uint8Array)], [1]);
    assert.deepEqual([...(codec.encode(false) as Uint8Array)], [0]);
  });

  it("stores a FixedVec of structs with a two-byte length and no padding", () => {
    const codec = codecOf({ fixedVec: [{ defined: "Mixed" }, 2] });
    const bytes = codec.encode([{ a: 1, b: 2n, c: 3 }]) as Uint8Array;
    assert.equal(bytes.length, 2 + 2 * 11);
    assert.deepEqual([...bytes.slice(0, 13)], [1, 0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0]);
    assert.ok(bytes.slice(13).every((b) => b === 0));
  });
});
