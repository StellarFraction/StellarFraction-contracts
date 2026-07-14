#![no_std]
use soroban_sdk::{contract, contractimpl, token, Address, Env};

pub mod math;
pub mod storage;
pub mod types;

#[cfg(test)]
mod test;

use crate::types::Error;

#[contract]
pub struct DistributionContract;

#[contractimpl]
impl DistributionContract {
    /// Initialize the contract with the admin, the property share token, and the rental yield (USDC) token.
    pub fn initialize(
        env: Env,
        admin: Address,
        share_token: Address,
        reward_token: Address,
    ) -> Result<(), Error> {
        if storage::is_initialized(&env) {
            return Err(Error::AlreadyInitialized);
        }

        storage::set_admin(&env, &admin);
        storage::set_share_token(&env, &share_token);
        storage::set_reward_token(&env, &reward_token);
        storage::set_acc_reward_per_share(&env, 0);
        storage::set_total_shares(&env, 0);
        storage::set_initialized(&env);

        Ok(())
    }

    /// Deposits real estate share tokens to stake them and earn dividends.
    pub fn deposit(env: Env, user: Address, amount: i128) -> Result<(), Error> {
        user.require_auth();
        Self::check_initialized(&env)?;

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let share_token_addr = storage::get_share_token(&env);
        let reward_token_addr = storage::get_reward_token(&env);

        // 1. Claim any pending rewards first to prevent overriding them
        let pending = Self::calculate_pending(&env, &user)?;
        if pending > 0 {
            let reward_client = token::Client::new(&env, &reward_token_addr);
            reward_client.transfer(&env.current_contract_address(), &user, &pending);
        }

        // 2. Transfer the share tokens from the user to the contract
        let share_client = token::Client::new(&env, &share_token_addr);
        share_client.transfer(&user, &env.current_contract_address(), &amount);

        // 3. Update user and global share records
        let current_shares = storage::get_user_shares(&env, &user);
        let new_shares = current_shares
            .checked_add(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        storage::set_user_shares(&env, &user, new_shares);

        let total_shares = storage::get_total_shares(&env);
        let new_total_shares = total_shares
            .checked_add(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        storage::set_total_shares(&env, new_total_shares);

        // 4. Update the user's reward debt
        let acc_reward_per_share = storage::get_acc_reward_per_share(&env);
        let new_debt = math::accumulated(new_shares, acc_reward_per_share)?;
        storage::set_user_debt(&env, &user, new_debt);

        Ok(())
    }

    /// Withdraws real estate share tokens, unstaking them.
    pub fn withdraw(env: Env, user: Address, amount: i128) -> Result<(), Error> {
        user.require_auth();
        Self::check_initialized(&env)?;

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let current_shares = storage::get_user_shares(&env, &user);
        if current_shares < amount {
            return Err(Error::InsufficientShares);
        }

        let share_token_addr = storage::get_share_token(&env);
        let reward_token_addr = storage::get_reward_token(&env);

        // 1. Claim any pending rewards
        let pending = Self::calculate_pending(&env, &user)?;
        if pending > 0 {
            let reward_client = token::Client::new(&env, &reward_token_addr);
            reward_client.transfer(&env.current_contract_address(), &user, &pending);
        }

        // 2. Transfer share tokens back to user
        let share_client = token::Client::new(&env, &share_token_addr);
        share_client.transfer(&env.current_contract_address(), &user, &amount);

        // 3. Update user and global share records
        let new_shares = current_shares - amount;
        if new_shares == 0 {
            storage::remove_user_shares(&env, &user);
            storage::remove_user_debt(&env, &user);
        } else {
            storage::set_user_shares(&env, &user, new_shares);
            let acc_reward_per_share = storage::get_acc_reward_per_share(&env);
            let new_debt = math::accumulated(new_shares, acc_reward_per_share)?;
            storage::set_user_debt(&env, &user, new_debt);
        }

        let total_shares = storage::get_total_shares(&env);
        let new_total_shares = total_shares
            .checked_sub(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        storage::set_total_shares(&env, new_total_shares);

        Ok(())
    }

    /// Deposits a lump sum of rental yield (USDC) and distributes it proportionally to stakers.
    pub fn distribute(env: Env, sender: Address, amount: i128) -> Result<(), Error> {
        sender.require_auth();
        Self::check_initialized(&env)?;

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let total_shares = storage::get_total_shares(&env);
        if total_shares == 0 {
            return Err(Error::NoSharesStaked);
        }

        let reward_token_addr = storage::get_reward_token(&env);

        // 1. Transfer reward tokens to contract
        let reward_client = token::Client::new(&env, &reward_token_addr);
        reward_client.transfer(&sender, &env.current_contract_address(), &amount);

        // 2. Accumulate the reward per share
        let acc_reward_per_share = storage::get_acc_reward_per_share(&env);
        let reward_increase = math::reward_increase(amount, total_shares)?;
        let new_acc_reward_per_share = acc_reward_per_share
            .checked_add(reward_increase)
            .ok_or(Error::ArithmeticOverflow)?;
        storage::set_acc_reward_per_share(&env, new_acc_reward_per_share);

        Ok(())
    }

    /// Claims accumulated USDC dividends for a user.
    pub fn claim(env: Env, user: Address) -> Result<i128, Error> {
        user.require_auth();
        Self::check_initialized(&env)?;

        let pending = Self::calculate_pending(&env, &user)?;
        if pending <= 0 {
            return Ok(0);
        }

        let reward_token_addr = storage::get_reward_token(&env);

        // 1. Reset user debt
        let current_shares = storage::get_user_shares(&env, &user);
        let acc_reward_per_share = storage::get_acc_reward_per_share(&env);
        let new_debt = math::accumulated(current_shares, acc_reward_per_share)?;
        storage::set_user_debt(&env, &user, new_debt);

        // 2. Transfer rewards
        let reward_client = token::Client::new(&env, &reward_token_addr);
        reward_client.transfer(&env.current_contract_address(), &user, &pending);

        Ok(pending)
    }

    /// Read-only: Gets the amount of deed tokens a user has staked.
    pub fn get_shares(env: Env, user: Address) -> i128 {
        storage::get_user_shares(&env, &user)
    }

    /// Read-only: Gets the reward debt of a user.
    pub fn get_debt(env: Env, user: Address) -> i128 {
        storage::get_user_debt(&env, &user)
    }

    /// Read-only: Returns the claimable USDC dividends for a user.
    pub fn get_pending(env: Env, user: Address) -> Result<i128, Error> {
        Self::check_initialized(&env)?;
        Self::calculate_pending(&env, &user)
    }

    /// Read-only: Returns contract configuration and global state.
    /// (admin, share_token, reward_token, total_shares, acc_reward_per_share)
    pub fn get_contract_info(env: Env) -> (Address, Address, Address, i128, i128) {
        let admin = storage::get_admin(&env);
        let share = storage::get_share_token(&env);
        let reward = storage::get_reward_token(&env);
        let total_shares = storage::get_total_shares(&env);
        let acc_reward = storage::get_acc_reward_per_share(&env);
        (admin, share, reward, total_shares, acc_reward)
    }

    // Helper functions

    fn check_initialized(env: &Env) -> Result<(), Error> {
        if !storage::is_initialized(env) {
            return Err(Error::NotInitialized);
        }
        Ok(())
    }

    fn calculate_pending(env: &Env, user: &Address) -> Result<i128, Error> {
        let shares = storage::get_user_shares(env, user);
        if shares == 0 {
            return Ok(0);
        }
        let acc_reward_per_share = storage::get_acc_reward_per_share(env);
        let debt = storage::get_user_debt(env, user);

        math::pending(shares, acc_reward_per_share, debt)
    }
}
