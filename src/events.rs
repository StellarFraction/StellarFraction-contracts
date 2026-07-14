use crate::types::PoolId;
use soroban_sdk::{symbol_short, Address, Env};

pub fn pool_created(env: &Env, pool_id: PoolId, manager: &Address) {
    env.events()
        .publish((symbol_short!("pool_new"), pool_id), manager.clone());
}

pub fn manager_changed(env: &Env, pool_id: PoolId, manager: &Address) {
    env.events()
        .publish((symbol_short!("manager"), pool_id), manager.clone());
}

pub fn pause_changed(env: &Env, pool_id: PoolId, paused: bool) {
    env.events()
        .publish((symbol_short!("paused"), pool_id), paused);
}

pub fn contract_paused(env: &Env, paused: bool) {
    env.events().publish((symbol_short!("pause"),), paused);
}

pub fn deposited(env: &Env, pool_id: PoolId, user: &Address, amount: i128) {
    env.events()
        .publish((symbol_short!("deposit"), pool_id, user.clone()), amount);
}

pub fn distributed(env: &Env, pool_id: PoolId, sender: &Address, amount: i128) {
    env.events()
        .publish((symbol_short!("distrib"), pool_id, sender.clone()), amount);
}

pub fn claimed(env: &Env, pool_id: PoolId, user: &Address, amount: i128) {
    env.events()
        .publish((symbol_short!("claim"), pool_id, user.clone()), amount);
}

pub fn withdrawn(env: &Env, pool_id: PoolId, user: &Address, amount: i128) {
    env.events()
        .publish((symbol_short!("withdraw"), pool_id, user.clone()), amount);
}
