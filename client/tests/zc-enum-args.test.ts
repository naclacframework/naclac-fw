/**
 * @naclac/client — instruction args with an alignment gap, against real bytes
 *
 * `tests/enums/golden/set_mode_args_ix.hex` is the instruction data the
 * generated Rust client builds for `set_mode_with_args` (regenerate with
 * `UPDATE_GOLDEN=1 cargo test -p enums_zc` in tests/enums). `ModeArgs` is a
 * 1-byte enum followed by a `u64`, so the program reads 7 padding bytes
 * between them; the TypeScript encoder must write the same bytes.
 */

import { strict as assert } from "assert";
import { readFileSync } from "fs";
import { join } from "path";
import { encodeInstructionData } from "../src/coder/instruction";

const enumsDir = join(__dirname, "../../tests/enums");
const idl = JSON.parse(
  readFileSync(
    join(enumsDir, "artifacts/clients/typescript/src/generated/enums_zc/idl/enums_zc.json"),
    "utf8"
  )
);
function readGolden(name: string): string {
  return readFileSync(join(enumsDir, "golden", name), "utf8").trim();
}

describe("#[instruction_args] with a padding gap", () => {
  const instruction = idl.instructions.find((i: any) => i.name === "set_mode_with_args");

  it("encodes to exactly the bytes the Rust client sends", () => {
    const encoded = encodeInstructionData(
      instruction,
      { args: { mode: { kind: "Blue" }, amount: 9n } },
      idl.definedTypes,
      true
    );
    assert.equal(Buffer.from(encoded).toString("hex"), readGolden("set_mode_args_ix.hex"));
  });
});

describe("lists of text and bytes as zero-copy instruction arguments", () => {
  const instruction = idl.instructions.find((i: any) => i.name === "text_lists");

  it("encodes Vec<String> and Vec<Vec<u8>> to exactly the Rust client's bytes", () => {
    const encoded = encodeInstructionData(
      instruction,
      {
        args: { words: ["ab", "cde"] },
        names: ["x", "yz"],
        blobs: [[1, 2, 3, 4]],
      },
      idl.definedTypes,
      true
    );
    assert.equal(Buffer.from(encoded).toString("hex"), readGolden("text_lists_ix.hex"));
  });
});

describe("vectors of enums as zero-copy instruction arguments", () => {
  const instruction = idl.instructions.find((i: any) => i.name === "enum_lists");

  it("encodes a Vec and a ZcVec of enums to exactly the Rust client's bytes", () => {
    const encoded = encodeInstructionData(
      instruction,
      {
        heap: [{ kind: "Blue" }, { kind: "Red" }, { kind: "Blue" }],
        view: [{ kind: "Blue" }, { kind: "Green" }],
      },
      idl.definedTypes,
      true
    );
    assert.equal(Buffer.from(encoded).toString("hex"), readGolden("enum_lists_ix.hex"));
  });
});

describe("#[instruction_args] with dynamic fields in a zero-copy program", () => {
  const instruction = idl.instructions.find((i: any) => i.name === "dyn_args");

  it("encodes a ZcVec, ZcString, Option, heap Vec and a loose Vec to exactly the Rust client's bytes", () => {
    const encoded = encodeInstructionData(
      instruction,
      {
        args: {
          tag: 3,
          values: [10, 20, 30, 40],
          note: "hello",
          maybe: { __option: "Some", value: 500 },
          heap: [1, 2],
        },
        more: [7, 8, 9],
      },
      idl.definedTypes,
      true
    );
    assert.equal(Buffer.from(encoded).toString("hex"), readGolden("dyn_args_ix.hex"));
  });

  it("writes a None option as a zero tag followed by zeroes of the value's size", () => {
    const encoded = encodeInstructionData(
      instruction,
      {
        args: { tag: 0, values: [], note: "", maybe: null, heap: [] },
        more: [],
      },
      idl.definedTypes,
      true
    );
    const body = Buffer.from(encoded).subarray(instruction.discriminator.length).toString("hex");
    // tag(1) + values len(4) + note len(4) + option(1 + 2) + heap len(4) + loose len(4)
    assert.equal(body, "00" + "00000000" + "00000000" + "000000" + "00000000" + "00000000");
  });
});
