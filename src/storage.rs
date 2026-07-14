use crate::types::DataKey;
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

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

pub fn get_minimum_deposit(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::MinimumDeposit)
        .unwrap_or(1)
}

pub fn set_minimum_deposit(env: &Env, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::MinimumDeposit, &amount);
}

pub fn get_max_stake_per_user(env: &Env, user: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::MaxStakePerUser(user.clone()))
        .unwrap_or(i128::MAX)
}

pub fn set_max_stake_per_user(env: &Env, user: &Address, limit: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::MaxStakePerUser(user.clone()), &limit);
}

pub fn get_lockup_duration(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::LockupDuration)
        .unwrap_or(0)
}

pub fn set_lockup_duration(env: &Env, seconds: u64) {
    env.storage()
        .instance()
        .set(&DataKey::LockupDuration, &seconds);
}

pub fn get_unlock_at(env: &Env, user: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::UnlockAt(user.clone()))
        .unwrap_or(0)
}

pub fn set_unlock_at(env: &Env, user: &Address, ts: u64) {
    env.storage()
        .persistent()
        .set(&DataKey::UnlockAt(user.clone()), &ts);
}
