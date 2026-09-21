//! Node.js (napi) bindings for naclac's litesvm test provider. A thin
//! translation layer over `naclac_client::NaclacProvider` (the same Rust
//! litesvm backend the Rust `NaclacProvider` already uses) — every method
//! here delegates to an already-existing, already-tested `NaclacProvider`
//! method rather than reimplementing litesvm logic. The surface is
//! RPC-shaped and SDK-agnostic (base58 addresses, raw wire-format
//! transaction bytes) on purpose: both `@solana/kit`- and
//! `@solana/web3.js`-based generated clients can front this same binding
//! without needing type-specific conversion code, since that's the same
//! boundary every real Solana SDK already targets.

use napi::bindgen_prelude::{BigInt, Buffer, Env, Error, FnArgs, FunctionRef, Result};
use napi_derive::napi;
use naclac_client::{NaclacClientError, NaclacProvider, NaclacTransactionMetadata};
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signature::Signature;
use solana_transaction::versioned::VersionedTransaction;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

/// `(programId, logs, signature, slot)` — the same raw shape a real
/// `onLogs`/`logsSubscribe` push notification already hands to
/// `addEventListener`'s existing decode step (`tryDecodeEventLog`), so that
/// TypeScript-side decoding logic stays completely unchanged whether it's
/// listening against a real cluster or this emulated litesvm source.
///
/// Wrapped in `FnArgs` deliberately — only `FnArgs<(...)>` implements
/// napi's tuple-spreading `JsValuesTupleIntoVec`; a bare tuple falls
/// through to the single-value blanket impl instead, which marshals the
/// whole tuple as one JS array argument rather than four separate
/// positional ones (confirmed by reading napi 3.12.2's own source —
/// `bindgen_runtime/js_values/function.rs`).
type LogsCallback = FunctionRef<FnArgs<(String, Vec<String>, String, u32)>, ()>;

fn to_napi_err<E: std::fmt::Display>(e: E) -> Error {
    Error::from_reason(e.to_string())
}

fn parse_address(s: &str) -> Result<Address> {
    Address::from_str(s).map_err(|e| to_napi_err(format!("Invalid address '{}': {}", s, e)))
}

fn parse_signature(s: &str) -> Result<Signature> {
    Signature::from_str(s).map_err(|e| to_napi_err(format!("Invalid signature '{}': {}", s, e)))
}

fn to_send_transaction_result(meta: NaclacTransactionMetadata) -> SendTransactionResult {
    SendTransactionResult {
        signature: meta.signature.to_string(),
        logs: meta.logs,
        compute_units_consumed: BigInt::from(meta.compute_units_consumed),
    }
}

#[napi(object)]
pub struct AccountInfoResult {
    pub lamports: BigInt,
    pub owner: String,
    pub data: Buffer,
    pub executable: bool,
    pub rent_epoch: BigInt,
}

#[napi(object)]
pub struct LatestBlockhashResult {
    pub blockhash: String,
}

#[napi(object)]
pub struct SendTransactionResult {
    pub signature: String,
    pub logs: Vec<String>,
    pub compute_units_consumed: BigInt,
}

#[napi(object)]
pub struct ProgramAccountResult {
    pub pubkey: String,
    pub data: Buffer,
    pub lamports: BigInt,
    pub owner: String,
    pub executable: bool,
    pub rent_epoch: BigInt,
}

#[napi(object)]
pub struct TokenBalanceResult {
    pub amount: BigInt,
    pub decimals: u8,
}

#[napi(js_name = "LiteSvm")]
pub struct LiteSvm {
    inner: NaclacProvider,
    listeners: Mutex<HashMap<u32, (String, LogsCallback)>>,
    next_listener_id: AtomicU32,
}

#[napi]
impl LiteSvm {
    /// `payer_bytes` is the standard 64-byte `[secret(32) || public(32)]`
    /// ed25519-dalek "keypair bytes" layout — the same format Solana
    /// keypair JSON files already use, so callers can just
    /// `Uint8Array.from(JSON.parse(fs.readFileSync(...)))` and pass that
    /// straight through, matching `loadNodeWallet`'s existing shape.
    #[napi(constructor)]
    pub fn new(payer_bytes: Buffer) -> Result<Self> {
        let bytes: &[u8] = payer_bytes.as_ref();
        let keypair = Keypair::try_from(bytes)
            .map_err(|e| to_napi_err(format!("Invalid payer keypair bytes: {}", e)))?;
        let inner = NaclacProvider::new_litesvm(keypair).map_err(to_napi_err)?;
        Ok(Self {
            inner,
            listeners: Mutex::new(HashMap::new()),
            next_listener_id: AtomicU32::new(1),
        })
    }

    /// Emulates a live `onLogs`/`logsSubscribe` push notification: registers
    /// `callback` to fire synchronously, in-process, the instant a
    /// transaction mentioning `program_id` executes via `sendTransaction` —
    /// there is no real background validator here to subscribe to. Fires
    /// from inside the very call that executed the transaction, so unlike a
    /// real websocket subscription there is no connection-race window where
    /// a notification could be missed. Returns a listener id for
    /// `removeLogsListener`.
    #[napi]
    pub fn add_logs_listener(&self, program_id: String, callback: LogsCallback) -> Result<u32> {
        let id = self.next_listener_id.fetch_add(1, Ordering::SeqCst);
        self.listeners
            .lock()
            .map_err(|e| to_napi_err(format!("Failed to lock listener registry: {}", e)))?
            .insert(id, (program_id, callback));
        Ok(id)
    }

    #[napi]
    pub fn remove_logs_listener(&self, listener_id: u32) -> Result<()> {
        self.listeners
            .lock()
            .map_err(|e| to_napi_err(format!("Failed to lock listener registry: {}", e)))?
            .remove(&listener_id);
        Ok(())
    }

    /// A transaction "mentions" `program_id` if it appears as an
    /// invoke target anywhere in its logs — matching real
    /// `logsSubscribe({ mentions: [...] })` semantics (top-level or via
    /// CPI), not just the top-level instruction.
    fn notify_logs_listeners(&self, env: &Env, meta: &NaclacTransactionMetadata) -> Result<()> {
        let listeners = self
            .listeners
            .lock()
            .map_err(|e| to_napi_err(format!("Failed to lock listener registry: {}", e)))?;
        if listeners.is_empty() {
            return Ok(());
        }
        let signature = meta.signature.to_string();
        for (program_id, callback) in listeners.values() {
            let invoke_prefix = format!("Program {} invoke", program_id);
            let mentioned = meta.logs.iter().any(|line| line.starts_with(&invoke_prefix));
            if !mentioned {
                continue;
            }
            let function = callback.borrow_back(env)?;
            function.call(FnArgs::from((program_id.clone(), meta.logs.clone(), signature.clone(), 0u32)))?;
        }
        Ok(())
    }

    #[napi]
    pub fn get_account_info(&self, address: String) -> Result<Option<AccountInfoResult>> {
        let addr = parse_address(&address)?;
        match self.inner.get_account(&addr) {
            Ok(account) => Ok(Some(AccountInfoResult {
                lamports: BigInt::from(account.lamports),
                owner: account.owner.to_string(),
                data: Buffer::from(account.data),
                executable: account.executable,
                rent_epoch: BigInt::from(account.rent_epoch),
            })),
            Err(NaclacClientError::AccountNotFound(_)) => Ok(None),
            Err(e) => Err(to_napi_err(e)),
        }
    }

    #[napi]
    pub fn get_balance(&self, address: String) -> Result<BigInt> {
        let addr = parse_address(&address)?;
        let balance = self.inner.get_balance(&addr).map_err(to_napi_err)?;
        Ok(BigInt::from(balance))
    }

    #[napi]
    pub fn get_latest_blockhash(&self) -> Result<LatestBlockhashResult> {
        let hash = self.inner.get_latest_blockhash().map_err(to_napi_err)?;
        Ok(LatestBlockhashResult {
            blockhash: hash.to_string(),
        })
    }

    #[napi]
    pub fn get_minimum_balance_for_rent_exemption(&self, size: BigInt) -> Result<BigInt> {
        let size = size.get_u64().1 as usize;
        let lamports = self
            .inner
            .get_minimum_balance_for_rent_exemption(size)
            .map_err(to_napi_err)?;
        Ok(BigInt::from(lamports))
    }

    /// `wire_bytes` is a fully-signed transaction already serialized to
    /// Solana's standard wire format — exactly what
    /// `getBase64EncodedWireTransaction` (kit) or `.serialize()` (web3.js)
    /// already produce, so callers on either SDK can pass the same bytes
    /// straight through with no naclac-specific conversion. litesvm
    /// executes and finalizes synchronously — there is no separate
    /// "confirm" step to wait for once this returns.
    #[napi]
    pub fn send_transaction(&self, env: Env, wire_bytes: Buffer) -> Result<SendTransactionResult> {
        let bytes: &[u8] = wire_bytes.as_ref();
        let vtx: VersionedTransaction = wincode::deserialize_exact(bytes)
            .map_err(|e| to_napi_err(format!("Failed to deserialize transaction: {}", e)))?;
        let meta = self.inner.send_transaction(&vtx, None).map_err(to_napi_err)?;
        self.notify_logs_listeners(&env, &meta)?;
        Ok(to_send_transaction_result(meta))
    }

    /// Forces the next `getLatestBlockhash()` to return a fresh blockhash.
    /// litesvm's blockhash doesn't advance on its own the way a real
    /// cluster's does every ~400ms — sending two otherwise-identical
    /// transactions back-to-back under the same blockhash produces the
    /// same signature, which litesvm correctly rejects as already-processed
    /// (real replay protection, not a bug). Call this between them.
    #[napi]
    pub fn expire_blockhash(&self) -> Result<()> {
        self.inner.expire_blockhash().map_err(to_napi_err)
    }

    /// Sets an account's lamport balance directly, bypassing normal transfer
    /// execution (and its rent-exemption rule) — `airdrop` goes through a
    /// real System Program transfer, which the runtime rejects if it would
    /// leave a new account underfunded.
    #[napi]
    pub fn set_account_lamports(&self, address: String, lamports: BigInt) -> Result<()> {
        let addr = parse_address(&address)?;
        let amount = lamports.get_u64().1;
        self.inner
            .set_account_lamports(&addr, amount)
            .map_err(to_napi_err)
    }

    #[napi]
    pub fn set_clock_unix_timestamp(&self, unix_timestamp: i64) -> Result<()> {
        self.inner
            .set_clock_unix_timestamp(unix_timestamp)
            .map_err(to_napi_err)
    }

    /// Directly injects an account into ledger state — data, owner, and
    /// lamports all explicitly controlled — bypassing normal account
    /// creation entirely.
    #[napi]
    pub fn set_account(
        &self,
        address: String,
        data: Buffer,
        owner: String,
        lamports: BigInt,
    ) -> Result<()> {
        let addr = parse_address(&address)?;
        let owner_addr = parse_address(&owner)?;
        let amount = lamports.get_u64().1;
        self.inner
            .set_account(&addr, data.to_vec(), &owner_addr, amount)
            .map_err(to_napi_err)
    }

    /// Runs a transaction without committing any state changes.
    #[napi]
    pub fn simulate_transaction(&self, wire_bytes: Buffer) -> Result<SendTransactionResult> {
        let bytes: &[u8] = wire_bytes.as_ref();
        let vtx: VersionedTransaction = wincode::deserialize_exact(bytes)
            .map_err(|e| to_napi_err(format!("Failed to deserialize transaction: {}", e)))?;
        let meta = self
            .inner
            .simulate_transaction(&vtx, None)
            .map_err(to_napi_err)?;
        Ok(to_send_transaction_result(meta))
    }

    /// Looks up an already-processed transaction by its base58 signature —
    /// used to fetch a confirmed transaction's logs after the fact.
    #[napi]
    pub fn get_transaction(&self, signature: String) -> Result<SendTransactionResult> {
        let sig = parse_signature(&signature)?;
        let meta = self.inner.get_transaction(&sig).map_err(to_napi_err)?;
        Ok(to_send_transaction_result(meta))
    }

    /// `(amount, decimals)` for an SPL Token or Token-2022 account.
    #[napi]
    pub fn get_token_account_balance(&self, address: String) -> Result<TokenBalanceResult> {
        let addr = parse_address(&address)?;
        let (amount, decimals) = self.inner.get_token_balance(&addr).map_err(to_napi_err)?;
        Ok(TokenBalanceResult {
            amount: BigInt::from(amount),
            decimals,
        })
    }

    #[napi]
    pub fn airdrop(&self, address: String, lamports: BigInt) -> Result<()> {
        let addr = parse_address(&address)?;
        let amount = lamports.get_u64().1;
        self.inner.airdrop(&addr, amount).map_err(to_napi_err)?;
        Ok(())
    }

    /// Loads a program's compiled `.so` (already built, e.g. via
    /// `naclac build`) into this litesvm instance at `program_id`. For
    /// programs outside this workspace (a CPI target built locally but not
    /// part of this project) — the workspace's own program(s) are already
    /// auto-loaded when this provider was constructed.
    #[napi]
    pub fn add_program(&self, program_id: String, so_path: String) -> Result<()> {
        let addr = parse_address(&program_id)?;
        self.inner.add_program(&addr, &so_path).map_err(to_napi_err)?;
        Ok(())
    }

    /// Fetches a deployed program's executable bytes from a live cluster
    /// (e.g. mainnet) and loads them into this litesvm instance — for CPI
    /// targets that live on a real network rather than in this workspace.
    /// Cached process-wide and retried on transient failure, same as the
    /// Rust `NaclacProvider`.
    #[napi]
    pub fn add_program_from_cluster(&self, program_id: String, cluster_url: String) -> Result<()> {
        let addr = parse_address(&program_id)?;
        self.inner
            .add_program_from_cluster(&addr, &cluster_url)
            .map_err(to_napi_err)?;
        Ok(())
    }

    /// Reaches past `NaclacProvider::get_program_accounts` (which trims each
    /// result down to just `(Address, Vec<u8>)` for its own Rust-side use
    /// case) to litesvm's own `get_program_accounts`, so every result
    /// carries full account info (lamports/owner/executable), matching what
    /// a real RPC's `getProgramAccounts` returns — needed by
    /// `fetchAllAccountsByProgram` (kit), which reads `lamports`/`owner`/
    /// `executable` off each result, not just its data.
    #[napi]
    pub fn get_program_accounts(&self, program_id: String) -> Result<Vec<ProgramAccountResult>> {
        use naclac_client::ClientBackend;

        let addr = parse_address(&program_id)?;

        match &self.inner.backend {
            ClientBackend::LiteSVM(svm_lock) => {
                let svm = svm_lock.lock().map_err(|e| {
                    to_napi_err(format!("Failed to lock LiteSVM: {}", e))
                })?;
                Ok(svm
                    .get_program_accounts(&addr)
                    .into_iter()
                    .map(|(pk, account)| ProgramAccountResult {
                        pubkey: pk.to_string(),
                        data: Buffer::from(account.data),
                        lamports: BigInt::from(account.lamports),
                        owner: account.owner.to_string(),
                        executable: account.executable,
                        rent_epoch: BigInt::from(account.rent_epoch),
                    })
                    .collect())
            }
            ClientBackend::Rpc(_) => Err(to_napi_err(
                "get_program_accounts on the RPC backend is not supported by this binding",
            )),
        }
    }
}
