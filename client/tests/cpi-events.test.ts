/**
 * @naclac/client — self-CPI event decoding (`emit_cpi!`)
 *
 * Only a call to the event's own program, whose first account is that
 * program's `event_authority` PDA and whose data starts with the IDL's
 * `eventCpiTag`, may be decoded as an event.
 *
 * Run: pnpm test
 */

import { strict as assert } from "assert";
import { getBase58Codec } from "@solana/codecs";
import {
  eventFieldLayout,
  parseEventsFromInnerInstructions,
  parseEventsFromTransaction,
  resolveRpcInnerInstructions,
  type NaclacInnerInstruction,
} from "../src/events";
import type { NaclacIdl } from "../src/idl";

const PROGRAM = "11111111111111111111111111111112";
const OTHER_PROGRAM = "11111111111111111111111111111113";
const AUTHORITY = "11111111111111111111111111111114";
const STRANGER = "11111111111111111111111111111115";
const TAG = [1, 2, 3];
const DISCRIMINATOR = [9, 9];

const idl = {
  address: PROGRAM,
  metadata: { name: "t", version: "0", description: "" },
  instructions: [],
  accounts: [],
  events: [
    { name: "Counter", discriminator: DISCRIMINATOR, fields: [{ name: "value", type: "u64" }] },
    { name: "Other", discriminator: [7, 7], fields: [{ name: "value", type: "u64" }] },
  ],
  errors: [],
  constants: [],
  definedTypes: [],
  pdas: [],
  eventCpiTag: TAG,
} as unknown as NaclacIdl;

function u64(value: bigint): number[] {
  const bytes = new Uint8Array(8);
  new DataView(bytes.buffer).setBigUint64(0, value, true);
  return [...bytes];
}

function payload(tag: number[], value: bigint, discriminator = DISCRIMINATOR): Uint8Array {
  return Uint8Array.from([...tag, ...discriminator, ...u64(value)]);
}

function call(
  programId: string,
  firstAccount: string | undefined,
  data: Uint8Array,
): NaclacInnerInstruction {
  return {
    outerIndex: 0,
    stackHeight: 2,
    programId,
    accounts: firstAccount === undefined ? [] : [firstAccount],
    data,
  };
}

const values = (events: any[]) => events.map((event) => event.value);

describe("self-CPI events", () => {
  it("decodes a genuine self-CPI event", () => {
    const events = parseEventsFromInnerInstructions(
      idl,
      "Counter",
      [call(PROGRAM, AUTHORITY, payload(TAG, 41n))],
      AUTHORITY,
    );
    assert.deepEqual(values(events), [41n]);
  });

  it("ignores calls that don't match program, authority and tag", () => {
    const forged = [
      call(OTHER_PROGRAM, AUTHORITY, payload(TAG, 1n)),
      call(PROGRAM, STRANGER, payload(TAG, 2n)),
      call(PROGRAM, AUTHORITY, payload([1, 2, 4], 3n)),
      call(PROGRAM, AUTHORITY, payload([], 4n)),
      call(PROGRAM, undefined, payload(TAG, 5n)),
      call(PROGRAM, AUTHORITY, Uint8Array.from(TAG)),
    ];
    assert.deepEqual(parseEventsFromInnerInstructions(idl, "Counter", forged, AUTHORITY), []);
  });

  it("ignores a matching call carrying a different event's discriminator", () => {
    const calls = [call(PROGRAM, AUTHORITY, payload(TAG, 6n, [7, 7]))];
    assert.deepEqual(parseEventsFromInnerInstructions(idl, "Counter", calls, AUTHORITY), []);
    assert.deepEqual(values(parseEventsFromInnerInstructions(idl, "Other", calls, AUTHORITY)), [6n]);
  });

  it("decodes nothing for a program without an eventCpiTag", () => {
    const untagged = { ...idl, eventCpiTag: undefined } as unknown as NaclacIdl;
    const calls = [call(PROGRAM, AUTHORITY, payload(TAG, 7n))];
    assert.deepEqual(parseEventsFromInnerInstructions(untagged, "Counter", calls, AUTHORITY), []);
  });

  it("throws for an event that isn't in the IDL", () => {
    assert.throws(() => parseEventsFromInnerInstructions(idl, "Missing", [], AUTHORITY), /not found/);
  });

  it("returns log events first, then CPI events in execution order", () => {
    const b64 = (bytes: number[]) => Buffer.from(bytes).toString("base64");
    const log = `Program data: ${b64(DISCRIMINATOR)} ${b64(u64(10n))}`;
    const events = parseEventsFromTransaction(
      idl,
      "Counter",
      {
        logs: [log],
        innerInstructions: [
          call(PROGRAM, AUTHORITY, payload(TAG, 20n)),
          call(PROGRAM, AUTHORITY, payload(TAG, 30n)),
        ],
        eventAuthority: AUTHORITY,
      },
    );
    assert.deepEqual(values(events), [10n, 20n, 30n]);
  });

  it("skips inner instructions when unavailable or when no authority is supplied", () => {
    const b64 = (bytes: number[]) => Buffer.from(bytes).toString("base64");
    const log = `Program data: ${b64(DISCRIMINATOR)} ${b64(u64(10n))}`;
    const calls = [call(PROGRAM, AUTHORITY, payload(TAG, 20n))];
    assert.deepEqual(values(parseEventsFromTransaction(idl, "Counter", { logs: [log] })), [10n]);
    assert.deepEqual(
      values(parseEventsFromTransaction(idl, "Counter", { logs: [log], innerInstructions: calls })),
      [10n],
    );
  });
});

describe("eventFieldLayout", () => {
  it("is fixed-layout for a plain event and alloc for an alloc event in a zero-copy program", () => {
    assert.equal(eventFieldLayout(true, ["Sized"], "Counter"), true);
    assert.equal(eventFieldLayout(true, ["Sized"], "Sized"), "alloc");
    assert.equal(eventFieldLayout(false, [], "Counter"), false);
    assert.equal(eventFieldLayout(false, ["Sized"], "Sized"), false);
  });
});

describe("alloc event holding a defined struct", () => {
  const paddedIdl = {
    ...idl,
    events: [
      {
        name: "Nested",
        discriminator: DISCRIMINATOR,
        fields: [
          { name: "head", type: "u8" },
          { name: "inner", type: { defined: "Padded" } },
          { name: "many", type: { vec: { defined: "Padded" } } },
          { name: "maybe", type: { option: { defined: "Padded" } } },
          { name: "note", type: "string" },
        ],
      },
    ],
    definedTypes: [
      {
        name: "Padded",
        type: {
          kind: "struct",
          fields: [
            { name: "kind", type: "u8" },
            { name: "amount", type: "u64" },
          ],
        },
      },
    ],
  } as unknown as NaclacIdl;

  const packed = (kind: number, amount: bigint) => [kind, ...u64(amount)];
  const u32 = (value: number) => [value, 0, 0, 0];

  it("reads each nested value in its packed 9-byte layout, with no padding", () => {
    const bytes = Uint8Array.from([
      ...TAG,
      ...DISCRIMINATOR,
      5,
      ...packed(3, 0x1122334455667788n),
      ...u32(2),
      ...packed(9, 42n),
      ...packed(3, 7n),
      1,
      ...packed(4, 8n),
      ...u32(2),
      104,
      105,
    ]);
    const [event] = parseEventsFromInnerInstructions(
      paddedIdl,
      "Nested",
      [call(PROGRAM, AUTHORITY, bytes)],
      AUTHORITY,
      "alloc",
    );
    assert.equal(event.head, 5);
    assert.deepEqual(event.inner, { kind: 3, amount: 0x1122334455667788n });
    assert.deepEqual(event.many, [
      { kind: 9, amount: 42n },
      { kind: 3, amount: 7n },
    ]);
    assert.deepEqual(event.maybe, { __option: "Some", value: { kind: 4, amount: 8n } });
    assert.equal(event.note, "hi");
  });
});

describe("resolveRpcInnerInstructions", () => {
  const base58 = getBase58Codec();
  const keys = [STRANGER, PROGRAM, AUTHORITY];

  it("resolves compiled instructions against the account keys", () => {
    const data = payload(TAG, 5n);
    const resolved = resolveRpcInnerInstructions(keys, [
      {
        index: 0,
        instructions: [
          { programIdIndex: 1, accounts: [2], data: base58.decode(data), stackHeight: 2 },
        ],
      },
    ]);
    assert.ok(resolved);
    assert.equal(resolved.length, 1);
    assert.equal(resolved[0].programId, PROGRAM);
    assert.deepEqual(resolved[0].accounts, [AUTHORITY]);
    assert.equal(resolved[0].outerIndex, 0);
    assert.equal(resolved[0].stackHeight, 2);
    assert.deepEqual([...resolved[0].data], [...data]);

    const events = parseEventsFromInnerInstructions(idl, "Counter", resolved, AUTHORITY);
    assert.deepEqual(values(events), [5n]);
  });

  it("reports a missing stack height as 0", () => {
    const resolved = resolveRpcInnerInstructions(keys, [
      { index: 0, instructions: [{ programIdIndex: 1, accounts: [], data: "" }] },
    ]);
    assert.equal(resolved?.[0].stackHeight, 0);
  });

  it("distinguishes 'no CPIs' from 'unavailable'", () => {
    assert.deepEqual(resolveRpcInnerInstructions(keys, []), []);
    assert.equal(resolveRpcInnerInstructions(keys, undefined), undefined);
    assert.equal(resolveRpcInnerInstructions(keys, null), undefined);
  });

  it("is unavailable when a key index can't be resolved", () => {
    assert.equal(
      resolveRpcInnerInstructions(keys, [
        { index: 0, instructions: [{ programIdIndex: 9, accounts: [], data: "" }] },
      ]),
      undefined,
    );
    assert.equal(
      resolveRpcInnerInstructions(keys, [
        { index: 0, instructions: [{ programIdIndex: 1, accounts: [9], data: "" }] },
      ]),
      undefined,
    );
  });
});
