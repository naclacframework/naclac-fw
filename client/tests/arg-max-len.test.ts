/**
 * @naclac/client: an argument with `maxLen` in the IDL is length-checked
 * before it is encoded. A string counts UTF-8 bytes, a vec counts elements,
 * and an argument without `maxLen` is never checked.
 */

import { strict as assert } from "assert";
import type { IdlInstruction } from "../src/idl";
import { encodeInstructionData } from "../src/coder/instruction";

const instruction: IdlInstruction = {
  name: "set_title",
  discriminator: [1, 2, 3, 4, 5, 6, 7, 8],
  accounts: [],
  args: [
    { name: "title", type: "string", maxLen: 4 },
    { name: "tags", type: { vec: "u8" }, maxLen: 2 },
    { name: "note", type: "string" },
  ],
};

function encode(args: Record<string, unknown>): Uint8Array {
  return encodeInstructionData(instruction, args, undefined, true);
}

describe("encodeInstructionData with maxLen", () => {
  it("accepts values at their limit", () => {
    const data = encode({ title: "abcd", tags: [1, 2], note: "x" });
    assert.ok(data.length > 8);
  });

  it("rejects a string longer than its limit and names the argument", () => {
    assert.throws(
      () => encode({ title: "abcde", tags: [], note: "" }),
      /Argument "title" for instruction "set_title" is 5 bytes long, but the program accepts at most 4/
    );
  });

  it("counts UTF-8 bytes, not characters", () => {
    assert.doesNotThrow(() => encode({ title: "éé", tags: [], note: "" }));
    assert.throws(() => encode({ title: "ééé", tags: [], note: "" }), /is 6 bytes long/);
  });

  it("rejects a vec with more elements than its limit", () => {
    assert.throws(
      () => encode({ title: "a", tags: [1, 2, 3], note: "" }),
      /Argument "tags".*is 3 elements long, but the program accepts at most 2/
    );
    assert.throws(
      () => encode({ title: "a", tags: new Uint8Array(3), note: "" }),
      /Argument "tags"/
    );
  });

  it("never checks an argument without maxLen", () => {
    assert.doesNotThrow(() => encode({ title: "a", tags: [], note: "n".repeat(1000) }));
  });
});
