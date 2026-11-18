/**
 * Saves the raw getAccountInfo result of each program's canonical IDL metadata
 * account on devnet, for the programs of the IDLs in fixtures/devnet/idl, to
 * explorer/fixtures/devnet/pmp/<program id>.account.json exactly as the node
 * returned it, with pmp/index.json listing each program, its metadata address
 * and whether the account exists. Needs no installed packages: run it with
 * Node 22.18 or later.
 *
 *   node src/record-idl.ts [idl-folder] [output-folder]
 *
 * Both folders are names under fixtures/devnet and default to "idl" and "pmp".
 *
 * Reads DEVNET_RPC_URL from explorer/.env or the environment. A program whose
 * account cannot be fetched is reported with the reason; nothing is retried or
 * substituted.
 */

import fs from "node:fs";
import path from "node:path";
import { idlMetadataAddress } from "../../../idl/src/resolve.ts";
import { devnetUrl, root, rpcCall } from "./common.ts";

interface IndexEntry {
  name: string;
  metadataAddress: string;
  exists: boolean;
  owner: string | null;
  dataBytes: number | null;
}

async function main(): Promise<void> {
  const url = await devnetUrl();
  const idlName = process.argv[2] ?? "idl";
  const outName = process.argv[3] ?? "pmp";
  for (const name of [idlName, outName]) {
    if (!/^[A-Za-z0-9_-]+$/.test(name)) {
      throw new Error("Naclac Error: folder names may hold only letters, digits, - and _.");
    }
  }
  const idlDir = path.join(root, "fixtures/devnet", idlName);
  const outDir = path.join(root, "fixtures/devnet", outName);
  fs.mkdirSync(outDir, { recursive: true });

  const programs = fs
    .readdirSync(idlDir)
    .filter((file) => file.endsWith(".json"))
    .map((file) => ({
      name: file.replace(/\.json$/, ""),
      programId: JSON.parse(fs.readFileSync(path.join(idlDir, file), "utf8")).address as string,
    }));

  const index: Record<string, IndexEntry> = {};
  const failures: string[] = [];
  for (const { name, programId } of programs) {
    const metadataAddress = await idlMetadataAddress(programId);
    let reply: { status: number; text: string };
    try {
      reply = await rpcCall(url, "getAccountInfo", [
        metadataAddress,
        { encoding: "base64", commitment: "finalized" },
      ]);
    } catch (error) {
      failures.push(`${name}: ${error instanceof Error ? error.message : String(error)}`);
      continue;
    }
    if (reply.status !== 200) {
      failures.push(`${name}: HTTP ${reply.status}: ${reply.text.slice(0, 200)}`);
      continue;
    }
    const body = JSON.parse(reply.text);
    if (body.error) {
      failures.push(`${name}: RPC error: ${JSON.stringify(body.error)}`);
      continue;
    }
    fs.writeFileSync(path.join(outDir, `${programId}.account.json`), reply.text);
    const value = body.result?.value ?? null;
    index[programId] = {
      name,
      metadataAddress,
      exists: value !== null,
      owner: value?.owner ?? null,
      dataBytes: value === null ? null : Buffer.from(value.data[0], "base64").length,
    };
    console.log(
      `${name} ${programId}\n    metadata ${metadataAddress}: ` +
        (value === null
          ? "no account"
          : `owner ${value.owner}, ${index[programId]?.dataBytes} bytes`),
    );
  }

  fs.writeFileSync(path.join(outDir, "index.json"), JSON.stringify(index, null, 2));
  for (const failure of failures) console.log(`  FAILED ${failure}`);
  console.log(`\n${Object.keys(index).length} of ${programs.length} recorded in ${outDir}`);
  if (failures.length > 0) process.exitCode = 1;
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
