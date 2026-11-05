/**
 * Kit litesvm provider: the signer that signs must be the account litesvm
 * funds, so only signers with a recorded secret key are accepted.
 */

import { strict as assert } from "assert";
import { createKeyPairSignerFromBytes, generateKeyPairSigner } from "@solana/kit";

import { generateExtractableKeyPairSigner } from "../src/kit";
import { createLiteSvmProvider, registerSignerSecretKey } from "../src/kit/provider-litesvm";

const SIGNER_ERROR = /needs a signer created by loadNodeWallet\(\) or generateExtractableKeyPairSigner\(\)/;

function errorOf(fn: () => unknown): Error | undefined {
  try {
    fn();
  } catch (err) {
    return err as Error;
  }
  return undefined;
}

describe("kit createLiteSvmProvider signer requirement", () => {
  it("rejects a signer whose secret key was never recorded", async () => {
    const signer = await generateKeyPairSigner();
    assert.throws(() => createLiteSvmProvider(signer), SIGNER_ERROR);
  });

  it("generateExtractableKeyPairSigner records a verified secret key", async () => {
    const signer = await generateExtractableKeyPairSigner();
    const err = errorOf(() => createLiteSvmProvider(signer));
    assert.ok(!err || !SIGNER_ERROR.test(err.message), err?.message);
  });

  it("registerSignerSecretKey accepts a signer built from known bytes", async () => {
    const source = await generateExtractableKeyPairSigner();
    const exported = new Uint8Array(await globalThis.crypto.subtle.exportKey("pkcs8", source.keyPair.privateKey));
    const secretKey = new Uint8Array(64);
    secretKey.set(exported.subarray(16), 0);
    secretKey.set(new Uint8Array(await globalThis.crypto.subtle.exportKey("raw", source.keyPair.publicKey)), 32);

    const signer = await createKeyPairSignerFromBytes(secretKey);
    assert.equal(signer.address, source.address);
    registerSignerSecretKey(signer, secretKey);

    const err = errorOf(() => createLiteSvmProvider(signer));
    assert.ok(!err || !SIGNER_ERROR.test(err.message), err?.message);
  });
});
