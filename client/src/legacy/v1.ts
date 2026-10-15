/**
 * Transaction V1 (SIMD-0385) message serialization and compilation for `@solana/web3.js`.
 */

import {
  PublicKey,
  TransactionInstruction,
  VersionedTransaction,
  type Blockhash,
} from "@solana/web3.js";
import { MessageV1, type TransactionConfig } from "@solana/web3.js";

const CONFIG_MASK_PRIORITY_FEE_BITS = 0b00011;
const CONFIG_MASK_COMPUTE_UNIT_LIMIT_BIT = 0b00100;
const CONFIG_MASK_LOADED_ACCOUNTS_DATA_SIZE_LIMIT_BIT = 0b01000;
const CONFIG_MASK_HEAP_SIZE_BIT = 0b10000;

function encodeU32(val: number): number[] {
  return [
    val & 0xff,
    (val >> 8) & 0xff,
    (val >> 16) & 0xff,
    (val >> 24) & 0xff,
  ];
}

function encodeU64(val: bigint | number): number[] {
  let v = BigInt(val);
  const bytes: number[] = [];
  for (let i = 0; i < 8; i++) {
    bytes.push(Number(v & 0xffn));
    v >>= 8n;
  }
  return bytes;
}

/**
 * Serializes a `MessageV1` into the canonical Solana wire format matching `MessageV1.deserialize`.
 */
export function serializeMessageV1(message: MessageV1): Uint8Array {
  const bytes: number[] = [];

  // Version 1 prefix (0x80 | 1 = 0x81)
  bytes.push(0x81);

  // Header
  bytes.push(message.header.numRequiredSignatures);
  bytes.push(message.header.numReadonlySignedAccounts);
  bytes.push(message.header.numReadonlyUnsignedAccounts);

  // Config mask
  const cfg = message.transactionConfig;
  let configMask = 0;
  if (cfg.priorityFee !== null && cfg.priorityFee !== undefined) {
    configMask |= CONFIG_MASK_PRIORITY_FEE_BITS;
  }
  if (cfg.computeUnitLimit !== null && cfg.computeUnitLimit !== undefined) {
    configMask |= CONFIG_MASK_COMPUTE_UNIT_LIMIT_BIT;
  }
  if (cfg.loadedAccountsDataSizeLimit !== null && cfg.loadedAccountsDataSizeLimit !== undefined) {
    configMask |= CONFIG_MASK_LOADED_ACCOUNTS_DATA_SIZE_LIMIT_BIT;
  }
  if (cfg.heapSize !== null && cfg.heapSize !== undefined) {
    configMask |= CONFIG_MASK_HEAP_SIZE_BIT;
  }
  bytes.push(...encodeU32(configMask));

  // Recent blockhash (32 bytes)
  const blockhashBytes = new PublicKey(message.recentBlockhash).toBytes();
  bytes.push(...Array.from(blockhashBytes));

  // Instruction count & static account keys length
  bytes.push(message.compiledInstructions.length);
  bytes.push(message.staticAccountKeys.length);

  // Static account keys
  for (const key of message.staticAccountKeys) {
    bytes.push(...Array.from(key.toBytes()));
  }

  // Config values in order: priorityFee, computeUnitLimit, loadedAccountsDataSizeLimit, heapSize
  if (cfg.priorityFee !== null && cfg.priorityFee !== undefined) {
    bytes.push(...encodeU64(cfg.priorityFee));
  }
  if (cfg.computeUnitLimit !== null && cfg.computeUnitLimit !== undefined) {
    bytes.push(...encodeU32(cfg.computeUnitLimit));
  }
  if (cfg.loadedAccountsDataSizeLimit !== null && cfg.loadedAccountsDataSizeLimit !== undefined) {
    bytes.push(...encodeU32(cfg.loadedAccountsDataSizeLimit));
  }
  if (cfg.heapSize !== null && cfg.heapSize !== undefined) {
    bytes.push(...encodeU32(cfg.heapSize));
  }

  // Instruction headers
  for (const ix of message.compiledInstructions) {
    bytes.push(ix.programIdIndex);
    bytes.push(ix.accountKeyIndexes.length);
    const dataLen = ix.data.length;
    bytes.push(dataLen & 0xff);
    bytes.push((dataLen >> 8) & 0xff);
  }

  // Instruction payloads
  for (const ix of message.compiledInstructions) {
    bytes.push(...ix.accountKeyIndexes);
    bytes.push(...Array.from(ix.data));
  }

  return new Uint8Array(bytes);
}

/**
 * Monkey-patches `MessageV1.prototype.serialize` if not yet implemented by `@solana/web3.js`.
 */
export function ensureMessageV1Serialization(): void {
  try {
    const dummy = new MessageV1({
      header: { numRequiredSignatures: 0, numReadonlySignedAccounts: 0, numReadonlyUnsignedAccounts: 0 },
      staticAccountKeys: [],
      recentBlockhash: "11111111111111111111111111111111",
      compiledInstructions: [],
    });
    dummy.serialize();
  } catch {
    MessageV1.prototype.serialize = function (this: MessageV1): Uint8Array {
      return serializeMessageV1(this);
    };
  }
}

// Apply the patch immediately upon module load.
ensureMessageV1Serialization();

let v1TransactionSerializePatched = false;

export function ensureVersionedTransactionV1Serialization(): void {
  if (v1TransactionSerializePatched) return;
  v1TransactionSerializePatched = true;
  const originalSerialize = VersionedTransaction.prototype.serialize;
  VersionedTransaction.prototype.serialize = function (this: VersionedTransaction): Uint8Array {
    if (this.version === 1 || this.message instanceof MessageV1 || (this.message as any)?.version === 1) {
      const serializedMessage = this.message.serialize();
      const numRequiredSignatures = this.message.header.numRequiredSignatures;
      const signatureByteLength = 64;
      const signaturesLength = numRequiredSignatures * signatureByteLength;
      const out = new Uint8Array(serializedMessage.length + signaturesLength);
      out.set(serializedMessage, 0);
      for (let i = 0; i < numRequiredSignatures; i++) {
        const sig = this.signatures[i];
        if (sig && sig.length === signatureByteLength) {
          out.set(sig, serializedMessage.length + i * signatureByteLength);
        }
      }
      return out;
    }
    return originalSerialize.call(this);
  };
}

ensureVersionedTransactionV1Serialization();

export interface CompileV1Args {
  payerKey: PublicKey;
  recentBlockhash: Blockhash;
  instructions: TransactionInstruction[];
  transactionConfig?: TransactionConfig | null;
}

/**
 * Compiles a list of `TransactionInstruction`s into a `MessageV1`.
 */
export function compileToV1Message(args: CompileV1Args): MessageV1 {
  const { payerKey, recentBlockhash, instructions, transactionConfig } = args;

  type AccountMetaResolved = {
    pubkey: PublicKey;
    isSigner: boolean;
    isWritable: boolean;
  };

  const accountMap = new Map<string, AccountMetaResolved>();

  accountMap.set(payerKey.toBase58(), {
    pubkey: payerKey,
    isSigner: true,
    isWritable: true,
  });

  for (const ix of instructions) {
    for (const key of ix.keys) {
      const b58 = key.pubkey.toBase58();
      const existing = accountMap.get(b58);
      if (existing) {
        existing.isSigner = existing.isSigner || key.isSigner;
        existing.isWritable = existing.isWritable || key.isWritable;
      } else {
        accountMap.set(b58, {
          pubkey: key.pubkey,
          isSigner: key.isSigner,
          isWritable: key.isWritable,
        });
      }
    }

    const progB58 = ix.programId.toBase58();
    if (!accountMap.has(progB58)) {
      accountMap.set(progB58, {
        pubkey: ix.programId,
        isSigner: false,
        isWritable: false,
      });
    }
  }

  const allAccounts = Array.from(accountMap.values());

  const writableSigners = allAccounts.filter(
    (a) => a.isSigner && a.isWritable && !a.pubkey.equals(payerKey),
  );
  const readonlySigners = allAccounts.filter(
    (a) => a.isSigner && !a.isWritable && !a.pubkey.equals(payerKey),
  );
  const writableNonSigners = allAccounts.filter(
    (a) => !a.isSigner && a.isWritable,
  );
  const readonlyNonSigners = allAccounts.filter(
    (a) => !a.isSigner && !a.isWritable,
  );

  const staticAccounts: AccountMetaResolved[] = [
    accountMap.get(payerKey.toBase58())!,
    ...writableSigners,
    ...readonlySigners,
    ...writableNonSigners,
    ...readonlyNonSigners,
  ];

  const staticAccountKeys = staticAccounts.map((a) => a.pubkey);
  const keyIndexMap = new Map<string, number>();
  staticAccountKeys.forEach((k, idx) => keyIndexMap.set(k.toBase58(), idx));

  const numRequiredSignatures = 1 + writableSigners.length + readonlySigners.length;
  const numReadonlySignedAccounts = readonlySigners.length;
  const numReadonlyUnsignedAccounts = readonlyNonSigners.length;

  const compiledInstructions = instructions.map((ix) => {
    const programIdIndex = keyIndexMap.get(ix.programId.toBase58());
    if (programIdIndex === undefined) {
      throw new Error(`[Naclac] Program ID ${ix.programId.toBase58()} not in static keys`);
    }
    const accountKeyIndexes = ix.keys.map((k) => {
      const idx = keyIndexMap.get(k.pubkey.toBase58());
      if (idx === undefined) {
        throw new Error(`[Naclac] Account ${k.pubkey.toBase58()} not in static keys`);
      }
      return idx;
    });
    return {
      programIdIndex,
      accountKeyIndexes,
      data: new Uint8Array(ix.data),
    };
  });

  return new MessageV1({
    header: {
      numRequiredSignatures,
      numReadonlySignedAccounts,
      numReadonlyUnsignedAccounts,
    },
    staticAccountKeys,
    recentBlockhash,
    compiledInstructions,
    transactionConfig: transactionConfig ?? {
      computeUnitLimit: 1_400_000,
      heapSize: null,
      loadedAccountsDataSizeLimit: 64 * 1024 * 1024,
      priorityFee: null,
    },
  });
}
