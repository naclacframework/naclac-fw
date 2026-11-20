/**
 * @naclac/client — a Borsh/Wincode enum variant that holds another enum or a
 * defined struct, in a tuple or in named fields, encodes to the variant's
 * discriminant followed by the payload's own bytes and decodes back.
 */

import { strict as assert } from "assert";
import { getIdlCodec } from "../src/coder/types";

const definedTypes = [
  {
    name: "Color",
    type: {
      kind: "enum",
      repr: "u8",
      variants: [
        { name: "Red", discriminant: "0" },
        { name: "Blue", discriminant: "7" },
      ],
    },
  },
  {
    name: "Pair",
    type: {
      kind: "struct",
      fields: [
        { name: "left", type: "u8" },
        { name: "right", type: "u16" },
      ],
    },
  },
  {
    name: "Action",
    type: {
      kind: "enum",
      repr: "u8",
      variants: [
        { name: "Pause", discriminant: "0" },
        { name: "Paint", discriminant: "3", fields: [{ defined: "Color" }] },
        { name: "Hold", discriminant: "4", fields: [{ name: "shade", type: { defined: "Color" } }] },
        { name: "Pin", discriminant: "5", fields: [{ defined: "Pair" }] },
      ],
    },
  },
];

describe("enum variants holding another enum or a struct (Borsh layout)", () => {
  const codec = getIdlCodec({ defined: "Action" }, definedTypes, false);

  it("encodes a tuple variant holding an enum", () => {
    const encoded = codec.encode({ kind: "Paint", fields: [{ kind: "Blue" }] });
    assert.deepEqual([...encoded], [3, 7]);
    assert.deepEqual(codec.decode(encoded), { kind: "Paint", fields: [{ kind: "Blue" }] });
  });

  it("encodes a named-field variant holding an enum", () => {
    const encoded = codec.encode({ kind: "Hold", shade: { kind: "Blue" } });
    assert.deepEqual([...encoded], [4, 7]);
    assert.deepEqual(codec.decode(encoded), { kind: "Hold", shade: { kind: "Blue" } });
  });

  it("encodes a tuple variant holding a struct", () => {
    const encoded = codec.encode({ kind: "Pin", fields: [{ left: 1, right: 0x0203 }] });
    assert.deepEqual([...encoded], [5, 1, 3, 2]);
    assert.deepEqual(codec.decode(encoded), { kind: "Pin", fields: [{ left: 1, right: 0x0203 }] });
  });

  it("still encodes a unit variant", () => {
    assert.deepEqual([...codec.encode({ kind: "Pause" })], [0]);
  });
});
