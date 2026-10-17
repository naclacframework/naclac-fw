/**
 * @naclac/client — The core shared primitives for the Naclac framework.
 */

export * from "./wrappers";

export type {
  NaclacIdl,
  IdlInstruction,
  IdlAccount,
  IdlAccountDef,
  IdlField,
  IdlPda,
  IdlPdaDef,
  IdlSeed,
  IdlEventDef,
  IdlEventField,
  IdlErrorDef,
  IdlConstant,
  IdlDefinedType,
} from "./idl";

export { resolveSlots } from "./slots";

export {
  encodeInstructionData,
  decodeAccountData,
  decodeDiscriminatedAccountData,
  getAccountCodec,
  getIdlCodec,
} from "./coder/index";
export type { AccountCodec, NaclacCodec } from "./coder/index";

export {
  parseEventsFromLogs,
  parseEventsFromInnerInstructions,
  parseEventsFromTransaction,
  resolveRpcInnerInstructions,
} from "./events";
export type {
  NaclacInnerInstruction,
  NaclacRpcResult,
  RpcInnerInstructions,
} from "./events";

export { 
  resolvePdas, 
  translateRpcError, 
  sleep, 
  logError, 
  padString, 
  toLeBytes, 
  withRetry,
  decodeBase64,
  stringifyJson,
  bigIntReplacer,
  mapKeysToValues,
} from "./utils/index";
export type { ResolvedPda } from "./utils/index";

export * from "./constants";