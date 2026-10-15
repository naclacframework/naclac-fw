/**
 * @naclac/client — ZcEnum account decode against real on-chain bytes
 *
 * `tests/enums/golden/state_zc.hex` is the `State` account written by the
 * compiled `enums_zc` program and read back through LiteSVM (regenerate with
 * `UPDATE_GOLDEN=1 cargo test -p enums_zc` in tests/enums; the admin address
 * is zeroed because it depends on the machine's wallet). The IDL is the one
 * `naclac build` generated for the same program.
 */

import { strict as assert } from "assert";
import { readFileSync } from "fs";
import { join } from "path";
import { getIdlCodec } from "../src/coder/types";

const enumsDir = join(__dirname, "../../tests/enums");
const idl = JSON.parse(
  readFileSync(
    join(enumsDir, "clients/typescript/src/generated/enums_zc/idl/enums_zc.json"),
    "utf8"
  )
);
const golden = readFileSync(join(enumsDir, "golden/state_zc.hex"), "utf8").trim();

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(hex.substring(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

const stateAccount = idl.accounts.find((a: any) => a.name === "State");
const definedTypes = [...idl.definedTypes, { name: "State", type: stateAccount.type }];
const codec = getIdlCodec({ defined: "State" }, definedTypes, true);
const discriminatorLength = stateAccount.discriminator.length;

describe("ZcEnum fields in a zero-copy account", () => {
  const bytes = hexToBytes(golden);
  const [state] = codec.read(bytes, discriminatorLength) as [any, number];

  it("decodes a plain enum field", () => {
    assert.deepEqual(state.color, { kind: "Blue" });
  });

  it("decodes a data-carrying enum field", () => {
    assert.equal(state.action.kind, "Transfer");
    assert.equal(BigInt(state.action.amount), 777n);
  });

  it("decodes enums with a wider tag, a negative tag and a data variant", () => {
    assert.deepEqual(state.level, { kind: "Mid" });
    assert.deepEqual(state.signed, { kind: "Neg" });
    assert.equal(state.wide.kind, "Small");
  });

  it("decodes an array of enums", () => {
    assert.deepEqual(state.history, [
      { kind: "Red" },
      { kind: "Red" },
      { kind: "Red" },
      { kind: "Red" },
    ]);
  });

  it("re-encodes to the exact bytes the program wrote", () => {
    const encoded = codec.encode(state);
    assert.equal(
      Buffer.from(encoded).toString("hex"),
      golden.slice(discriminatorLength * 2)
    );
  });

  it("rejects an enum tag with no variant", () => {
    const colorTag = discriminatorLength + 1 + 32;
    const corrupted = hexToBytes(golden);
    assert.equal(corrupted[colorTag], 7, "offset must point at Blue's tag byte");
    corrupted[colorTag] = 5;
    assert.throws(() => codec.read(corrupted, discriminatorLength));
  });

  it("names the enum and explains an all-zero slot when no variant has discriminant 0", () => {
    const levelCodec = getIdlCodec({ defined: "Level" }, definedTypes, true);
    assert.throws(
      () => levelCodec.read(new Uint8Array(2), 0),
      (error: Error) => /Level/.test(error.message) && /never set/.test(error.message)
    );
  });
});
