/**
 * Transaction-version selection and V1 resource-limit defaults shared by the
 * `kit` and `legacy` clients.
 */

export type TxVersion = "v0" | "v1" | "legacy";

/**
 * Maps a transaction version to its wire form. `undefined` means the default
 * (V1); any value other than "v0", "v1" or "legacy" throws rather than
 * silently building a different transaction version than the caller asked for.
 */
export function normalizeTxVersion(version?: TxVersion): 0 | 1 | "legacy" {
  if (version === undefined || version === "v1") return 1;
  if (version === "v0") return 0;
  if (version === "legacy") return "legacy";
  throw new Error(
    `[Naclac] Unknown transaction version ${JSON.stringify(version)}; use "v0", "v1" or "legacy".`,
  );
}

/**
 * A V1 transaction that leaves either limit unset is budgeted zero and fails,
 * so every V1 transaction the clients build sets both.
 */
export const V1_DEFAULT_COMPUTE_UNIT_LIMIT = 1_400_000;
export const V1_DEFAULT_LOADED_ACCOUNTS_DATA_SIZE_LIMIT = 64 * 1024 * 1024;
