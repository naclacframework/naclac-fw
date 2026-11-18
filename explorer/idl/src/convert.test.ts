/**
 * Tests of the on-chain to local IDL conversion: a round trip over the five
 * counter IDLs, the structural rules of the Rust converter, and untrusted
 * input. Run with `node --test src/convert.test.ts`.
 */

import assert from "node:assert/strict";
import { test } from "node:test";
import { isNaclacIdl, toLocalIdl } from "./convert.ts";
import { IdlError } from "./errors.ts";
import { counterLocalIdls, plain, toOnchain } from "./test-support.ts";

test("on-chain form is built, then converted back to exactly the local IDL, for all five counters", () => {
  const locals = counterLocalIdls();
  assert.equal(Object.keys(locals).length, 5);
  for (const [name, local] of Object.entries(locals)) {
    const onchain = toOnchain(local);
    assert.equal(onchain.metadata.spec, "0.1.0", name);
    assert.ok(onchain.accounts.every((a: any) => a.type === undefined && a.fields === undefined), name);
    assert.ok(onchain.events.every((e: any) => e.fields === undefined), name);
    assert.ok(Array.isArray(onchain.types) && onchain.definedTypes === undefined, name);
    assert.deepEqual(plain(toLocalIdl(onchain)), local, name);
  }
});

test("accounts and events take their fields from the types entry of the same name", () => {
  const local = counterLocalIdls().counter_borsh;
  const onchain = toOnchain(local);
  const types = onchain.types.map((t: any) => t.name);
  assert.deepEqual(types, ["Counter", "CounterDecremented", "CounterIncremented"]);
  const restored = plain(toLocalIdl(onchain)) as any;
  assert.deepEqual(restored.accounts[0].type, local.accounts[0].type);
  assert.deepEqual(restored.events[0].fields, local.events[0].fields);
  assert.deepEqual(restored.definedTypes, []);
  assert.equal(restored.metadata.spec, undefined);
  assert.equal(restored.metadata.serialization, "borsh");
});

test("isNaclacIdl is decided by metadata.serialization alone", () => {
  const local = counterLocalIdls().counter_zc;
  assert.equal(isNaclacIdl(toOnchain(local)), true);
  assert.equal(isNaclacIdl({ metadata: { spec: "0.1.0", name: "x" }, instructions: [] }), false);
  assert.equal(isNaclacIdl({ metadata: { serialization: null } }), true);
  for (const value of [null, 1, "x", [], {}, { metadata: 1 }]) assert.equal(isNaclacIdl(value), false);
});

test("a document that is not a naclac on-chain IDL is rejected, not converted", () => {
  const onchain = toOnchain(counterLocalIdls().counter_borsh);
  for (const bad of [null, [], "x", 3]) assert.throws(() => toLocalIdl(bad), IdlError);

  for (const key of ["types", "accounts", "events"]) {
    const copy = structuredClone(onchain);
    delete copy[key];
    assert.throws(() => toLocalIdl(copy), IdlError, key);
  }

  const noType = structuredClone(onchain);
  noType.types = noType.types.filter((t: any) => t.name !== "Counter");
  assert.throws(() => toLocalIdl(noType), /account `Counter` has no entry/);

  const noEventType = structuredClone(onchain);
  noEventType.types = noEventType.types.filter((t: any) => t.name !== "CounterIncremented");
  assert.throws(() => toLocalIdl(noEventType), /event `CounterIncremented` has no entry/);

  const unnamed = structuredClone(onchain);
  unnamed.accounts[0] = { discriminator: [1] };
  assert.throws(() => toLocalIdl(unnamed), /has no name/);
});

test("the result is a null-prototype object, so hostile keys cannot reach Object.prototype", () => {
  const onchain = JSON.parse(
    JSON.stringify(toOnchain(counterLocalIdls().counter_borsh)).replace(
      '"address"',
      '"__proto__":{"polluted":true},"address"',
    ),
  );
  const local = toLocalIdl(onchain);
  assert.equal(Object.getPrototypeOf(local), null);
  assert.equal(({} as any).polluted, undefined);
  assert.deepEqual(Object.getOwnPropertyDescriptor(local, "__proto__")?.value, { polluted: true });
});
