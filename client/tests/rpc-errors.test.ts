/**
 * @naclac/client — `translateRpcError` decoding of naclac framework errors
 * (`10000 + accountIndex * 100 + type`).
 *
 * Run: pnpm test
 */

import { strict as assert } from "assert";
import { translateRpcError } from "../src/utils/rpc";

function translated(code: number, ixAccounts?: { name: string }[]): string {
  const err = new Error(
    `custom program error: 0x${code.toString(16)}`,
  );
  try {
    translateRpcError(err, [], ixAccounts);
  } catch (e) {
    return (e as Error).message;
  }
  throw new Error("translateRpcError must always throw");
}

describe("translateRpcError framework errors", () => {
  it("names the account for index 0", () => {
    const msg = translated(10002, [{ name: "authority" }]);
    assert.match(msg, /Missing required signature/);
    assert.match(msg, /"authority"/);
  });

  it("decodes account indices of 10 and above", () => {
    const accounts = Array.from({ length: 13 }, (_, i) => ({ name: `acc${i}` }));
    const msg = translated(10000 + 12 * 100 + 1, accounts);
    assert.match(msg, /Mutability mismatch/);
    assert.match(msg, /"acc12"/);
  });

  it("decodes account indices of 30 and above", () => {
    const accounts = Array.from({ length: 41 }, (_, i) => ({ name: `acc${i}` }));
    const msg = translated(10000 + 40 * 100 + 4, accounts);
    assert.match(msg, /Owner mismatch/);
    assert.match(msg, /"acc40"/);
  });

  it("decodes the highest account index", () => {
    const msg = translated(10000 + 255 * 100 + 3);
    assert.match(msg, /Address mismatch/);
    assert.match(msg, /Index 255/);
  });

  it("reports 10007 as not-executable at index 0, not as an initialization error", () => {
    const msg = translated(10007);
    assert.match(msg, /Account is not executable/);
    assert.doesNotMatch(msg, /Initialized/i);
  });

  it("decodes the account-entry cap error", () => {
    const msg = translated(10031);
    assert.match(msg, /more than 64 account entries/);
  });

  it("leaves user error codes (6000 to 9999) to the IDL", () => {
    assert.doesNotMatch(translated(6000), /Framework Error/);
    assert.doesNotMatch(translated(9999), /Framework Error/);
  });

  it("leaves codes past the highest account index undecoded", () => {
    assert.doesNotMatch(translated(10000 + 256 * 100), /Framework Error/);
  });
});
