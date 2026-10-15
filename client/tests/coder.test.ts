/**
 * @naclac/client — Coder Unit Tests
 *
 * Verifies that byte-packing in encodeInstructionData EXACTLY matches
 * the discriminators and argument layout expected by the Rust backend.
 *
 * Run: pnpm test
 */

import { strict as assert } from "assert";
import {
  encodeInstructionData,
  decodeAccountData,
  decodeDiscriminatedAccountData,
} from "../src/coder/instruction";
import { tryDecodeEventLog } from "../src/events";
import { getIdlCodec } from "../src/coder/index";
import { getDiscriminator, resolveDiscriminator } from "../src/utils/hash";
import type { IdlInstruction } from "../src/idl";

// ─────────────────────────────────────────────────────────────────────────────
// IDL fixtures (mirrored from counter_test.json)
// ─────────────────────────────────────────────────────────────────────────────

const initializeIx: IdlInstruction = {
  name: "initialize",
  discriminator: [175, 175, 109, 31, 13, 152, 155, 237],
  accounts: [
    { name: "payer", isMut: true, isSigner: true },
    { name: "counterAccount", isMut: true, isSigner: false, pda: {
      seeds: [{ kind: "const", value: [99, 111, 117, 110, 116, 101, 114, 95, 118, 50] }]
    }},
    { name: "systemProgram", isMut: false, isSigner: false },
  ],
  args: [],
};

const incrementIx: IdlInstruction = {
  name: "increment",
  discriminator: [11, 18, 104, 9, 104, 174, 59, 33],
  accounts: [
    { name: "authority", isMut: false, isSigner: true },
    { name: "counterAccount", isMut: true, isSigner: false, pda: {
      seeds: [
        { kind: "const", value: [99, 111, 117, 110, 116, 101, 114, 95, 118, 50] },
        { kind: "arg", path: "bump" },
      ]
    }},
  ],
  args: [{ name: "bump", type: "u8" }],
};

const closeIx: IdlInstruction = {
  name: "close",
  discriminator: [98, 165, 201, 177, 108, 65, 206, 96],
  accounts: [],
  args: [],
};

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

describe("encodeInstructionData", () => {
  it("initialize — no args: output is exactly the 8-byte discriminator", () => {
    const result = encodeInstructionData(initializeIx, {});
    assert.equal(result.length, 8, "Should be exactly 8 bytes");
    assert.deepEqual(
      Array.from(result),
      [175, 175, 109, 31, 13, 152, 155, 237],
      "Discriminator bytes must match the IDL exactly"
    );
  });

  it("increment — with u8 bump arg: output is 8 + 1 = 9 bytes", () => {
    const result = encodeInstructionData(incrementIx, { bump: 254 });
    assert.equal(result.length, 9, "Should be 8 (discriminator) + 1 (u8 bump) = 9 bytes");

    // First 8 bytes must be the discriminator
    assert.deepEqual(
      Array.from(result.slice(0, 8)),
      [11, 18, 104, 9, 104, 174, 59, 33],
      "Discriminator bytes must match"
    );

    // Byte 9 must be the bump value (254 = 0xFE)
    assert.equal(result[8], 254, "u8 bump should encode to a single byte with value 254");
  });

  it("increment — bump 0: should encode as a single 0x00 byte", () => {
    const result = encodeInstructionData(incrementIx, { bump: 0 });
    assert.equal(result[8], 0, "bump=0 should encode as 0x00");
  });

  it("close — no args, no accounts: output only the discriminator", () => {
    const result = encodeInstructionData(closeIx, {});
    assert.deepEqual(
      Array.from(result),
      [98, 165, 201, 177, 108, 65, 206, 96]
    );
  });

  it("increment — missing arg throws a descriptive error", () => {
    assert.throws(
      () => encodeInstructionData(incrementIx, {}),
      /Missing argument "bump"/,
      "Should throw a descriptive error for missing args"
    );
  });
});

describe("decodeAccountData", () => {
  it("should decode a Counter account correctly (skip 8-byte discriminator)", () => {
    /**
     * Layout: [disc(8)] [authority: publicKey(32)] [count: u64(8)]
     *
     * We'll use a zeroed-out authority and count=42n.
     */
    const disc     = new Uint8Array(8).fill(0xAB);
    const authority = new Uint8Array(32).fill(0); // all-zero public key
    const countBuf  = new Uint8Array(8);

    // Write count=42 as little-endian u64
    const view = new DataView(countBuf.buffer);
    view.setBigUint64(0, 42n, true /* little-endian */);

    const raw = new Uint8Array([...disc, ...authority, ...countBuf]);

    const fields = [
      { name: "authority", type: "publicKey" },
      { name: "count",     type: "u64" },
    ];

    const decoded = decodeAccountData(fields, raw);

    assert.ok(decoded.authority !== undefined, "authority should be present");
    assert.equal(decoded.count, 42n, "count should decode to BigInt 42n");
  });
});

describe("custom-width discriminators", () => {
  const countFields = [{ name: "count", type: "u64" }];

  function accountBytes(disc: number[], count: bigint): Uint8Array {
    const body = new Uint8Array(8);
    new DataView(body.buffer).setBigUint64(0, count, true);
    return new Uint8Array([...disc, ...body]);
  }

  it("encodeInstructionData writes a 1-byte and a 3-byte discriminator verbatim", () => {
    const oneByte: IdlInstruction = { name: "a", discriminator: [7], accounts: [], args: [{ name: "bump", type: "u8" }] };
    assert.deepEqual(Array.from(encodeInstructionData(oneByte, { bump: 9 })), [7, 9]);

    const threeByte: IdlInstruction = { name: "b", discriminator: [1, 2, 3], accounts: [], args: [] };
    assert.deepEqual(Array.from(encodeInstructionData(threeByte, {})), [1, 2, 3]);
  });

  it("decodeAccountData starts reading at the given offset", () => {
    assert.equal(decodeAccountData(countFields, accountBytes([5], 42n), undefined, false, 1).count, 42n);
    assert.equal(decodeAccountData(countFields, accountBytes([1, 2, 3], 99n), undefined, false, 3).count, 99n);
  });

  it("decodeDiscriminatedAccountData accepts a matching discriminator of any width", () => {
    const one = decodeDiscriminatedAccountData(countFields, accountBytes([5], 7n), Uint8Array.from([5]), "Thing");
    assert.equal(one.count, 7n);
    const three = decodeDiscriminatedAccountData(countFields, accountBytes([1, 2, 3], 8n), Uint8Array.from([1, 2, 3]), "Thing");
    assert.equal(three.count, 8n);
  });

  it("decodeDiscriminatedAccountData rejects a different account type", () => {
    assert.throws(
      () => decodeDiscriminatedAccountData(countFields, accountBytes([6], 7n), Uint8Array.from([5]), "Thing"),
      /does not start with the "Thing" discriminator/,
    );
  });

  it("decodeDiscriminatedAccountData rejects data shorter than the discriminator", () => {
    assert.throws(
      () => decodeDiscriminatedAccountData(countFields, Uint8Array.from([1, 2]), Uint8Array.from([1, 2, 3]), "Thing"),
      /does not start with the "Thing" discriminator/,
    );
  });

  it("resolveDiscriminator prefers the IDL value and falls back to the name-derived default", () => {
    assert.deepEqual(Array.from(resolveDiscriminator([9, 9], "event", "Foo")), [9, 9]);
    assert.deepEqual(
      Array.from(resolveDiscriminator(undefined, "event", "Foo")),
      Array.from(getDiscriminator("event", "Foo")),
    );
  });

  it("tryDecodeEventLog matches a 1-byte event discriminator and skips others", () => {
    const eventDef = { fields: [{ name: "value", type: "u64" }] };
    const payload = new Uint8Array(8);
    new DataView(payload.buffer).setBigUint64(0, 123n, true);
    const b64 = (bytes: Uint8Array) => Buffer.from(bytes).toString("base64");

    const matching = `Program data: ${b64(Uint8Array.from([4]))} ${b64(payload)}`;
    assert.equal(tryDecodeEventLog(eventDef, Uint8Array.from([4]), matching)?.value, 123n);

    const other = `Program data: ${b64(Uint8Array.from([5]))} ${b64(payload)}`;
    assert.equal(tryDecodeEventLog(eventDef, Uint8Array.from([4]), other), null);

    const tooShort = `Program data: ${b64(Uint8Array.from([4]))}`;
    assert.equal(tryDecodeEventLog(eventDef, Uint8Array.from([4, 4]), tooShort), null);
  });
});

describe("Borsh/Wincode enum tag is the variant's real discriminant", () => {
  const definedTypes = [
    {
      name: "Mode",
      type: {
        kind: "enum",
        repr: "u8",
        variants: [
          { name: "Off", discriminant: "0" },
          { name: "On", discriminant: "1" },
          { name: "Auto", discriminant: "7" },
          {
            name: "Rect",
            discriminant: "8",
            fields: [{ name: "width", type: "u16" }],
          },
        ],
      },
    },
  ];
  const codec = getIdlCodec({ defined: "Mode" }, definedTypes, false);

  it("a unit variant with an explicit discriminant writes that discriminant, not its position", () => {
    assert.deepEqual(Array.from(codec.encode({ kind: "Auto" })), [7]);
  });

  it("a variant after an explicit discriminant continues from it", () => {
    assert.deepEqual(Array.from(codec.encode({ kind: "Rect", width: 513 })), [8, 1, 2]);
  });

  it("decode maps the discriminant back to the variant", () => {
    assert.deepEqual(codec.decode(Uint8Array.from([7])), { kind: "Auto" });
    assert.deepEqual(codec.decode(Uint8Array.from([8, 1, 2])), { kind: "Rect", width: 513 });
  });

  it("an unknown discriminant is rejected", () => {
    assert.throws(() => codec.decode(Uint8Array.from([2])), /Invalid enum discriminant 2/);
  });
});

describe("Vec codec matches Borsh's wire format", () => {
  it("Vec<u8> is a u32 LE element count followed by the bytes", () => {
    const codec = getIdlCodec({ vec: "u8" }, [], false);
    const wire = [3, 0, 0, 0, 1, 2, 3];
    assert.deepEqual(Array.from(codec.encode([1, 2, 3])), wire);
    assert.deepEqual(Array.from(codec.decode(Uint8Array.from(wire)) as ArrayLike<number>), [1, 2, 3]);
  });

  it("Vec<u32> counts elements, not bytes", () => {
    const codec = getIdlCodec({ vec: "u32" }, [], false);
    const wire = [2, 0, 0, 0, 7, 0, 0, 0, 9, 0, 0, 0];
    assert.deepEqual(Array.from(codec.encode([7, 9])), wire);
    assert.deepEqual(Array.from(codec.decode(Uint8Array.from(wire)) as ArrayLike<number>), [7, 9]);
  });

  it("an empty Vec is just a zero count", () => {
    const codec = getIdlCodec({ vec: "u8" }, [], false);
    assert.deepEqual(Array.from(codec.encode([])), [0, 0, 0, 0]);
  });

  it("an #[event(alloc)] log line with a Vec<u8> field decodes with the sequential layout", () => {
    const eventDef = { fields: [{ name: "data", type: { vec: "u8" } }] };
    const raw = Uint8Array.from([9, 3, 0, 0, 0, 1, 2, 3]);
    const log = `Program data: ${Buffer.from(raw).toString("base64")}`;
    const decoded = tryDecodeEventLog(eventDef, Uint8Array.from([9]), log, [], false);
    assert.deepEqual(Array.from(decoded?.data as ArrayLike<number>), [1, 2, 3]);
  });
});
