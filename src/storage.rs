use crate::types::{DataKey, Pool, PoolId, Position};
use soroban_sdk::{Address, Env};

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Initialized)
}

pub fn set_initialized(env: &Env) {
    env.storage().instance().set(&DataKey::Initialized, &true);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_share_token(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::ShareToken).unwrap()
}

pub fn set_share_token(env: &Env, share_token: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::ShareToken, share_token);
}

pub fn get_reward_token(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::RewardToken).unwrap()
}

pub fn set_reward_token(env: &Env, reward_token: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::RewardToken, reward_token);
}

pub fn get_acc_reward_per_share(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::AccRewardPerShare)
        .unwrap_or(0)
}

pub fn set_acc_reward_per_share(env: &Env, val: i128) {
    env.storage()
        .instance()
        .set(&DataKey::AccRewardPerShare, &val);
}

pub fn get_total_shares(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TotalShares)
        .unwrap_or(0)
}

pub fn set_total_shares(env: &Env, val: i128) {
    env.storage().instance().set(&DataKey::TotalShares, &val);
}

pub fn get_user_shares(env: &Env, user: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::UserShare(user.clone()))
        .unwrap_or(0)
}

pub fn set_user_shares(env: &Env, user: &Address, val: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::UserShare(user.clone()), &val);
}

pub fn remove_user_shares(env: &Env, user: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::UserShare(user.clone()));
}

pub fn get_user_debt(env: &Env, user: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::UserDebt(user.clone()))
        .unwrap_or(0)
}

pub fn set_user_debt(env: &Env, user: &Address, val: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::UserDebt(user.clone()), &val);
}

pub fn remove_user_debt(env: &Env, user: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::UserDebt(user.clone()));
}

pub fn get_next_pool_id(env: &Env) -> PoolId {
    env.storage()
        .instance()
        .get(&DataKey::NextPoolId)
        .unwrap_or(0)
}

pub fn set_next_pool_id(env: &Env, pool_id: PoolId) {
    env.storage().instance().set(&DataKey::NextPoolId, &pool_id);
}

pub fn get_pool(env: &Env, pool_id: PoolId) -> Option<Pool> {
    env.storage().persistent().get(&DataKey::Pool(pool_id))
}

pub fn set_pool(env: &Env, pool_id: PoolId, pool: &Pool) {
    env.storage()
        .persistent()
        .set(&DataKey::Pool(pool_id), pool);
}

pub fn get_position(env: &Env, pool_id: PoolId, user: &Address) -> Position {
    env.storage()
        .persistent()
        .get(&DataKey::Position(pool_id, user.clone()))
        .unwrap_or(Position {
            shares: 0,
            reward_debt: 0,
        })
}

pub fn set_position(env: &Env, pool_id: PoolId, user: &Address, position: &Position) {
    env.storage()
        .persistent()
        .set(&DataKey::Position(pool_id, user.clone()), position);
}

pub fn remove_position(env: &Env, pool_id: PoolId, user: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::Position(pool_id, user.clone()));
}
