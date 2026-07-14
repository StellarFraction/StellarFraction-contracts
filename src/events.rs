use crate::types::PoolId;
use soroban_sdk::{symbol_short, Address, Env, Symbol};

// ── Event topic constants ─────────────────────────────────────────
const POOL_NEW: Symbol = symbol_short!("pool_new");
const MANAGER: Symbol = symbol_short!("manager");
const PAUSED: Symbol = symbol_short!("paused");      // per-pool
const PAUSE_ALL: Symbol = symbol_short!("pause");   // global — renamed for clarity
const DEPOSIT: Symbol = symbol_short!("deposit");
const DISTRIBUTE: Symbol = symbol_short!("distrib");
const CLAIM: Symbol = symbol_short!("claim");
const WITHDRAW: Symbol = symbol_short!("withdraw");

// ── Internal helper ─────────────────────────────────────────────
fn emit<T: soroban_sdk::IntoVal<Env, T> + soroban_sdk::TryIntoVal<Env, T>>(
    env: &Env,
    topic0: Symbol,
    topics: impl IntoIterator<Item = impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>>,
    data: T,
) {
    env.events().publish((topic0,).into_iter().chain(topics), data);
}

// ── Pool lifecycle ────────────────────────────────────────────────
pub fn pool_created(env: &Env, pool_id: PoolId, manager: &Address) {
    emit(env, POOL_NEW, [pool_id], manager.clone());
}

pub fn manager_changed(env: &Env, pool_id: PoolId, manager: &Address) {
    emit(env, MANAGER, [pool_id], manager.clone());
}

// ── Pause events ────────────────────────────────────────────────
pub fn pool_pause_changed(env: &Env, pool_id: PoolId, paused: bool) {
    emit(env, PAUSED, [pool_id], paused);
}

pub fn global_pause_changed(env: &Env, paused: bool) {
    // single-topic event: just the event name, no extra topics
    env.events().publish((PAUSE_ALL,), paused);
}

// ── User actions ──────────────────────────────────────────────────
pub fn deposited(env: &Env, pool_id: PoolId, user: &Address, amount: i128) {
    emit(env, DEPOSIT, [pool_id, user.clone()], amount);
}


pub fn distributed(env: &Env, pool_id: PoolId, sender: &Address, amount: i128) {
    emit(env, DISTRIBUTE, [pool_id, sender.clone()], amount);
}
