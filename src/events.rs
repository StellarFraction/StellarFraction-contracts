use crate::types::PoolId;
use soroban_sdk::{symbol_short, Address, Env, Symbol};

// ── Event topic constants ─────────────────────────────────────────
const POOL_NEW: Symbol = symbol_short!("pool_new");
const MANAGER: Symbol = symbol_short!("manager");
const PAUSED: Symbol = symbol_short!("paused");
const PAUSE_ALL: Symbol = symbol_short!("pause");
const DEPOSIT: Symbol = symbol_short!("deposit");
const DISTRIBUTE: Symbol = symbol_short!("distrib");
const CLAIM: Symbol = symbol_short!("claim");
const WITHDRAW: Symbol = symbol_short!("withdraw");

// ── Pool lifecycle ────────────────────────────────────────────────
pub fn pool_created(env: &Env, pool_id: PoolId, manager: &Address) {
    env.events().publish((POOL_NEW, pool_id), manager.clone());
}

pub fn manager_changed(env: &Env, pool_id: PoolId, manager: &Address) {
    env.events().publish((MANAGER, pool_id), manager.clone());
}

// ── Pause events ────────────────────────────────────────────────
pub fn pause_changed(env: &Env, pool_id: PoolId, paused: bool) {
    env.events().publish((PAUSED, pool_id), paused);
}

pub fn contract_paused(env: &Env, paused: bool) {
    env.events().publish((PAUSE_ALL,), paused);
}

// ── User actions ──────────────────────────────────────────────────
pub fn deposited(env: &Env, pool_id: PoolId, user: &Address, amount: i128) {
    env.events()
        .publish((DEPOSIT, pool_id, user.clone()), amount);
}

pub fn distributed(env: &Env, pool_id: PoolId, sender: &Address, amount: i128) {
    env.events()
        .publish((DISTRIBUTE, pool_id, sender.clone()), amount);
}

pub fn claimed(env: &Env, pool_id: PoolId, user: &Address, amount: i128) {
    env.events().publish((CLAIM, pool_id, user.clone()), amount);
}

pub fn withdrawn(env: &Env, pool_id: PoolId, user: &Address, amount: i128) {
    env.events()
        .publish((WITHDRAW, pool_id, user.clone()), amount);
}
