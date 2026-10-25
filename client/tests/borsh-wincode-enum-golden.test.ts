/**
 * @naclac/client — Borsh and Wincode enum accounts and instruction data
 * against real bytes
 *
 * `tests/enums/golden/{borsh,wincode}_*.hex` are written by the compiled
 * `enums_borsh` / `enums_wincode` programs and the generated Rust client under
 * LiteSVM (regenerate with `UPDATE_GOLDEN=1 cargo test` in tests/enums; the
 * admin address is zeroed because it depends on the machine's wallet). The
 * IDLs are the ones `naclac build` generated for the same programs.
 */

import { strict as assert } from "assert";
import { readFileSync } from "fs";
import { join } from "path";
import { getIdlCodec } from "../src/coder/types";
import { encodeInstructionData } from "../src/coder/instruction";

const enumsDir = join(__dirname, "../../tests/enums");

function readGolden(name: string): string {
  return readFileSync(join(enumsDir, "golden", name), "utf8").trim();
}

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(hex.substring(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

for (const mode of ["borsh", "wincode"] as const) {
  const idl = JSON.parse(
    readFileSync(
      join(enumsDir, `artifacts/clients/typescript/src/generated/enums_${mode}/idl/enums_${mode}.json`),
      "utf8"
    )
  );
  const stateAccount = idl.accounts.find((a: any) => a.name === "State");
  const definedTypes = [...idl.definedTypes, { name: "State", type: stateAccount.type }];
  const codec = getIdlCodec({ defined: "State" }, definedTypes, false);
  const discriminatorLength = stateAccount.discriminator.length;

  describe(`${mode} enum account`, () => {
    const golden = readGolden(`${mode}_state.hex`);
    const [state] = codec.read(hexToBytes(golden), discriminatorLength) as [any, number];

    it("decodes plain, data-carrying, array and optional enum fields", () => {
      assert.deepEqual(state.color, { kind: "Blue" });
      assert.equal(state.action.kind, "Transfer");
      assert.equal(BigInt(state.action.amount), 777n);
      assert.deepEqual(state.history, [
        { kind: "Red" },
        { kind: "Red" },
        { kind: "Red" },
        { kind: "Red" },
      ]);
      assert.deepEqual(state.maybeColor, { __option: "None" });
      assert.equal(Number(state.counter), 0);
    });

    it("re-encodes to the exact bytes the program wrote", () => {
      const encoded = Buffer.from(codec.encode(state)).toString("hex");
      assert.equal(encoded, golden.slice(discriminatorLength * 2, discriminatorLength * 2 + encoded.length));
    });
  });

  describe(`${mode} enum instruction data`, () => {
    it("encodes a data-carrying enum argument to exactly the Rust client's bytes", () => {
      const instruction = idl.instructions.find((i: any) => i.name === "set_action");
      const encoded = encodeInstructionData(
        instruction,
        { action: { kind: "SetFee", feeBps: 321 } },
        idl.definedTypes,
        false
      );
      assert.equal(Buffer.from(encoded).toString("hex"), readGolden(`${mode}_set_fee_ix.hex`));
    });

    it("encodes a struct argument holding an enum to exactly the Rust client's bytes", () => {
      const instruction = idl.instructions.find((i: any) => i.name === "set_mode_with_args");
      const encoded = encodeInstructionData(
        instruction,
        { args: { mode: { kind: "Blue" }, amount: 9n } },
        idl.definedTypes,
        false
      );
      assert.equal(Buffer.from(encoded).toString("hex"), readGolden(`${mode}_set_mode_ix.hex`));
    });
  });
}
