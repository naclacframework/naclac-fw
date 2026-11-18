/**
 * Tests against the real metadata accounts recorded from devnet: the derived
 * address is where the account actually lives, and each account resolves to a
 * naclac IDL equal to the local IDL `naclac build` wrote. Run with
 * `node --test src/real.test.ts`.
 */

import assert from "node:assert/strict";
import { test } from "node:test";
import { PROGRAM_METADATA_ID, idlMetadataAddress, parseHeader } from "./pmp.ts";
import { resolveIdl } from "./resolve.ts";
import { counterLocalIdls, plain, recordedMetadata } from "./test-support.ts";

test("all five counter programs have a metadata account at the derived address", async () => {
  const recorded = recordedMetadata();
  assert.equal(recorded.length, 5);
  for (const item of recorded) {
    assert.equal(await idlMetadataAddress(item.programId), item.metadataAddress, item.name);
    assert.ok(item.account, `${item.name} has no account on devnet`);
    assert.equal(item.account.owner, PROGRAM_METADATA_ID, item.name);
  }
});

test("each real account is a finalized, canonical metadata account for its program", () => {
  for (const item of recordedMetadata()) {
    assert.ok(item.account, item.name);
    const header = parseHeader(item.account.data);
    assert.ok(header, item.name);
    assert.equal(header.kind, "metadata", item.name);
    assert.equal(header.program, item.programId, item.name);
    assert.equal(header.canonical, true, item.name);
    assert.equal(new TextDecoder().decode(header.seed).replace(/\0+$/, ""), "idl", item.name);
  }
});

test("each real account resolves to a naclac IDL equal to the local IDL", async () => {
  const locals = counterLocalIdls();
  for (const item of recordedMetadata()) {
    const result = await resolveIdl(item.programId, item.account);
    assert.equal(result.kind, "naclac", `${item.name}: ${JSON.stringify(result)}`);
    if (result.kind !== "naclac") continue;
    assert.equal(result.metadataAddress, item.metadataAddress, item.name);
    assert.deepEqual(plain(result.document), locals[item.name], item.name);
  }
});
