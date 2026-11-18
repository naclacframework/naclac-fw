/**
 * @naclac/client — fixed-capacity fields (`FixedVec<T, N>`, `FixedString<N>`):
 * an IDL field's `slots` makes the plain `vec`/`string` a fixed-capacity one,
 * with the exact bytes the program's C layout has.
 *
 * Run: pnpm test
 */

import { strict as assert } from "assert";
import { getAccountCodec } from "../src/coder/account";
import { getStructFieldsCodec } from "../src/coder/types";
import { resolveSlots } from "../src/slots";
import type { NaclacIdl } from "../src/idl";

function idlWith(serialization: "bytemuck" | "borsh", slots: boolean): NaclacIdl {
  return {
    address: "11111111111111111111111111111111",
    metadata: { name: "p", version: "0.1.0", description: "d", serialization },
    instructions: [],
    accounts: [
      {
        name: "Profile",
        type: {
          kind: "struct",
          fields: [
            { name: "scores", type: { vec: "u64" }, ...(slots ? { slots: 8 } : {}) },
            { name: "name", type: "string", ...(slots ? { slots: 32 } : {}) },
          ],
        },
      },
    ],
    events: [],
    errors: [],
    constants: [],
    definedTypes: [],
    pdas: [],
  } as unknown as NaclacIdl;
}

describe("fixed-capacity fields", () => {
  it("writes a FixedVec and FixedString as a two-byte length plus every slot, with no padding", () => {
    const codec = getAccountCodec(idlWith("bytemuck", true), "Profile");
    const bytes = codec.encode({ scores: [1n, 2n], name: "hi" });

    assert.equal(bytes.length, 2 + 8 * 8 + 2 + 32);
    assert.deepEqual([...bytes.slice(0, 2)], [2, 0]);
    assert.deepEqual([...bytes.slice(2, 18)], [1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]);
    assert.ok(bytes.slice(18, 66).every((b) => b === 0));
    assert.deepEqual([...bytes.slice(66, 70)], [2, 0, 104, 105]);
    assert.ok(bytes.slice(70).every((b) => b === 0));
  });

  it("reads back only the first len items, and rejects a length above the capacity", () => {
    const codec = getAccountCodec(idlWith("bytemuck", true), "Profile");
    const bytes = codec.encode({ scores: [1n, 2n], name: "hi" });
    assert.deepEqual(codec.decode(bytes), { scores: [1n, 2n], name: "hi" });

    const forged = new Uint8Array(bytes);
    forged[0] = 0xff;
    forged[1] = 0xff;
    assert.throws(() => codec.decode(forged), /FixedVec stores a length of 65535, above its capacity of 8/);

    const forgedName = new Uint8Array(bytes);
    forgedName[66] = 33;
    assert.throws(() => codec.decode(forgedName), /FixedString stores a length of 33, above its capacity of 32/);
  });

  it("rejects a list or string that does not fit", () => {
    const codec = getAccountCodec(idlWith("bytemuck", true), "Profile");
    assert.throws(() => codec.encode({ scores: new Array(9).fill(1n), name: "" }));
    assert.throws(() => codec.encode({ scores: [], name: "x".repeat(33) }));
  });

  it("counts a string's slots in bytes", () => {
    const codec = getAccountCodec(idlWith("bytemuck", true), "Profile");
    assert.throws(() => codec.encode({ scores: [], name: "é".repeat(17) }));
    assert.doesNotThrow(() => codec.encode({ scores: [], name: "é".repeat(16) }));
  });

  it("keeps the same fields as a Borsh count-prefixed vec and string when there are no slots", () => {
    const codec = getAccountCodec(idlWith("borsh", false), "Profile");
    const bytes = codec.encode({ scores: [1n], name: "hi" });
    assert.deepEqual([...bytes], [1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 104, 105]);
  });

  it("refuses slots in a program that is not bytemuck", () => {
    assert.throws(() => resolveSlots(idlWith("borsh", true)), /not bytemuck/);
  });

  it("does not modify the IDL it resolves", () => {
    const idl = idlWith("bytemuck", true);
    resolveSlots(idl);
    assert.deepEqual((idl.accounts[0].type.fields[0] as any).type, { vec: "u64" });
  });
});

function idlWithEvent(serialization: "bytemuck" | "borsh"): NaclacIdl {
  return {
    address: "11111111111111111111111111111111",
    metadata: { name: "p", version: "0.1.0", description: "d", serialization },
    instructions: [],
    accounts: [],
    events: [
      {
        name: "Log",
        discriminator: [1],
        fields: [
          { name: "scores", type: { vec: "u64" }, slots: 4 },
          { name: "label", type: "string", slots: 16 },
          { name: "total", type: "u64" },
        ],
      },
    ],
    errors: [],
    constants: [],
    definedTypes: [],
    pdas: [],
  } as unknown as NaclacIdl;
}

describe("fixed-capacity event fields", () => {
  for (const layout of [true, "alloc"] as const) {
    it(`encodes and decodes a ${layout === true ? "plain" : "alloc"} event with a FixedVec and a FixedString`, () => {
      const idl = resolveSlots(idlWithEvent("bytemuck"));
      const codec = getStructFieldsCodec(idl.events[0].fields as any[], idl.definedTypes, layout);
      const bytes = codec.encode({ scores: [1n, 2n], label: "hi", total: 7n });

      assert.equal(bytes.length, 2 + 4 * 8 + (2 + 16) + 8);
      assert.deepEqual([...bytes.slice(0, 2)], [2, 0]);
      assert.deepEqual([...bytes.slice(34, 38)], [2, 0, 104, 105]);
      assert.deepEqual(codec.decode(bytes), { scores: [1n, 2n], label: "hi", total: 7n });
    });
  }

  it("refuses slots on an event field in a program that is not bytemuck", () => {
    assert.throws(() => resolveSlots(idlWithEvent("borsh")), /Log\.scores.*not bytemuck/);
  });
});
