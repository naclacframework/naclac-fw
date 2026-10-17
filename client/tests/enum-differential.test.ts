/**
 * @naclac/client — Zero-copy enum differential test
 *
 * Cross-checks the TS zero-copy enum codec (`computeEnumLayout` /
 * `getZeroCopyEnumCodec`, `src/coder/types.ts`) against fixed byte strings
 * that the Rust side pins as well: `pod/tests/enum_fixtures.rs` encodes the
 * same enums and asserts the identical hex, so the two codecs are tied to one
 * set of bytes instead of each to its own arithmetic.
 *
 * Layout: the tag (the enum's `repr`, little-endian), the active variant's
 * fields in declaration order with no padding, then zeros up to the size of
 * the largest variant. `StepMode` is 1 + 8 = 9 bytes, `WideMode` 2 + 4 = 6.
 *
 * Run: pnpm test
 */

import { strict as assert } from "assert";
import { getIdlCodec } from "../src/coder/types";

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(hex.substring(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array | Readonly<Uint8Array>): string {
  return Array.from(bytes as Uint8Array)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

// ─────────────────────────────────────────────────────────────────────────────
// IDL fixtures
// ─────────────────────────────────────────────────────────────────────────────

const definedTypes = [
  {
    name: "InnerMode",
    type: {
      kind: "enum",
      variants: [
        { name: "A", discriminant: "0" },
        { name: "B", discriminant: "1" },
      ],
    },
  },
  {
    name: "StepMode",
    type: {
      kind: "enum",
      variants: [
        { name: "Simple", discriminant: "0" },
        { name: "Stepped", fields: ["u64"], discriminant: "1" },
        {
          name: "Named",
          fields: [
            { name: "rate", type: "u32" },
            { name: "mode", type: { defined: "InnerMode" } },
          ],
          discriminant: "10",
        },
      ],
    },
  },
  {
    name: "WideMode",
    type: {
      kind: "enum",
      repr: "u16",
      variants: [
        { name: "Alpha", discriminant: "0" },
        { name: "Beta", fields: ["u32"], discriminant: "1" },
      ],
    },
  },
];

// The same strings are asserted in pod/tests/enum_fixtures.rs.
const FIXTURES = {
  // tag 0 = Simple, no fields: the tag and eight zero bytes
  stepModeSimple: "0".repeat(18),
  stepModeStepped42: "012a00000000000000",
  // tag 10, rate 7 (u32), mode B (1), then three zero bytes up to the size
  stepModeNamedRate7ModeB: "0a0700000001000000",
  stepModeNamedRateMaxModeA: "0affffffff00000000",
  wideModeAlpha: "000000000000",
  wideModeBeta99: "010063000000",
  wideModeBetaMax: "0100ffffffff",
};

describe("Zero-copy enum codec — differential test against the byte strings Rust pins", () => {
  it("all fixtures have the packed size (9 for StepMode, 6 for WideMode)", () => {
    assert.equal(hexToBytes(FIXTURES.stepModeSimple).length, 9);
    assert.equal(hexToBytes(FIXTURES.stepModeStepped42).length, 9);
    assert.equal(hexToBytes(FIXTURES.stepModeNamedRate7ModeB).length, 9);
    assert.equal(hexToBytes(FIXTURES.stepModeNamedRateMaxModeA).length, 9);
    assert.equal(hexToBytes(FIXTURES.wideModeAlpha).length, 6);
    assert.equal(hexToBytes(FIXTURES.wideModeBeta99).length, 6);
    assert.equal(hexToBytes(FIXTURES.wideModeBetaMax).length, 6);
  });

  describe("StepMode", () => {
    const codec = getIdlCodec({ defined: "StepMode" }, definedTypes, true);

    it("decodes real Rust bytes for Simple correctly", () => {
      const [value] = codec.read(hexToBytes(FIXTURES.stepModeSimple), 0);
      assert.deepEqual(value, { kind: "Simple" });
    });

    it("encodes Simple to the exact real Rust bytes", () => {
      const encoded = codec.encode({ kind: "Simple" });
      assert.equal(bytesToHex(encoded), FIXTURES.stepModeSimple);
    });

    it("decodes real Rust bytes for Stepped(42) correctly", () => {
      const [value] = codec.read(hexToBytes(FIXTURES.stepModeStepped42), 0);
      assert.deepEqual(value, { kind: "Stepped", fields: [42n] });
    });

    it("encodes Stepped(42) to the exact real Rust bytes", () => {
      const encoded = codec.encode({ kind: "Stepped", fields: [42n] });
      assert.equal(bytesToHex(encoded), FIXTURES.stepModeStepped42);
    });

    it("decodes real Rust bytes for Named{rate:7,mode:B} correctly (explicit discriminant=10, nested enum field)", () => {
      const [value] = codec.read(hexToBytes(FIXTURES.stepModeNamedRate7ModeB), 0);
      assert.deepEqual(value, { kind: "Named", rate: 7, mode: { kind: "B" } });
    });

    it("encodes Named{rate:7,mode:B} to the exact real Rust bytes", () => {
      const encoded = codec.encode({ kind: "Named", rate: 7, mode: { kind: "B" } });
      assert.equal(bytesToHex(encoded), FIXTURES.stepModeNamedRate7ModeB);
    });

    it("decodes real Rust bytes for Named{rate:u32::MAX,mode:A} correctly (max-value field boundary)", () => {
      const [value] = codec.read(hexToBytes(FIXTURES.stepModeNamedRateMaxModeA), 0);
      assert.deepEqual(value, { kind: "Named", rate: 4294967295, mode: { kind: "A" } });
    });

    it("encodes Named{rate:u32::MAX,mode:A} to the exact real Rust bytes", () => {
      const encoded = codec.encode({ kind: "Named", rate: 4294967295, mode: { kind: "A" } });
      assert.equal(bytesToHex(encoded), FIXTURES.stepModeNamedRateMaxModeA);
    });

    it("rejects a corrupted *nested* InnerMode discriminant, proving recursive validation actually inspects the inner byte (mirrors the on-chain nested_enum_invalid_discriminant_is_rejected Miri test)", () => {
      const bytes = hexToBytes(FIXTURES.stepModeNamedRate7ModeB);
      // Byte 5 is InnerMode's own tag within the Named payload (tag at 0,
      // `rate` at 1..5) — 5 is not a valid InnerMode discriminant (only 0/A
      // and 1/B exist).
      bytes[5] = 5;
      assert.throws(() => codec.read(bytes, 0));
    });

    it("every possible u8 tag value (0..255) is accepted iff it's a real discriminant (0, 1, or 10) — exhaustive, not sampled", () => {
      const validTags = new Set([0, 1, 10]);
      for (let tag = 0; tag < 256; tag++) {
        const buf = new Uint8Array(9);
        buf[0] = tag;
        let threw = false;
        try {
          codec.read(buf, 0);
        } catch {
          threw = true;
        }
        assert.equal(
          threw,
          !validTags.has(tag),
          `tag=${tag}: expected ${validTags.has(tag) ? "accept" : "reject"}, got ${threw ? "reject" : "accept"}`
        );
      }
    });
  });

  describe("WideMode (#[repr(u16)] — non-default discriminant width)", () => {
    const codec = getIdlCodec({ defined: "WideMode" }, definedTypes, true);

    it("decodes real Rust bytes for Alpha correctly", () => {
      const [value] = codec.read(hexToBytes(FIXTURES.wideModeAlpha), 0);
      assert.deepEqual(value, { kind: "Alpha" });
    });

    it("encodes Alpha to the exact real Rust bytes", () => {
      const encoded = codec.encode({ kind: "Alpha" });
      assert.equal(bytesToHex(encoded), FIXTURES.wideModeAlpha);
    });

    it("decodes real Rust bytes for Beta(99) correctly", () => {
      const [value] = codec.read(hexToBytes(FIXTURES.wideModeBeta99), 0);
      assert.deepEqual(value, { kind: "Beta", fields: [99] });
    });

    it("encodes Beta(99) to the exact real Rust bytes", () => {
      const encoded = codec.encode({ kind: "Beta", fields: [99] });
      assert.equal(bytesToHex(encoded), FIXTURES.wideModeBeta99);
    });

    it("decodes real Rust bytes for Beta(u32::MAX) correctly (max-value field boundary)", () => {
      const [value] = codec.read(hexToBytes(FIXTURES.wideModeBetaMax), 0);
      assert.deepEqual(value, { kind: "Beta", fields: [4294967295] });
    });

    it("encodes Beta(u32::MAX) to the exact real Rust bytes", () => {
      const encoded = codec.encode({ kind: "Beta", fields: [4294967295] });
      assert.equal(bytesToHex(encoded), FIXTURES.wideModeBetaMax);
    });

    it("every possible u16 tag value (0..65535) is accepted iff it's a real discriminant (0 or 1) — exhaustive, proves the u16-width tag read/compare itself is correct, not just u8", () => {
      const validTags = new Set([0, 1]);
      for (let tag = 0; tag < 65536; tag++) {
        const buf = new Uint8Array(6);
        buf[0] = tag & 0xff;
        buf[1] = (tag >> 8) & 0xff;
        let threw = false;
        try {
          codec.read(buf, 0);
        } catch {
          threw = true;
        }
        assert.equal(
          threw,
          !validTags.has(tag),
          `tag=${tag}: expected ${validTags.has(tag) ? "accept" : "reject"}, got ${threw ? "reject" : "accept"}`
        );
      }
    });
  });
});
