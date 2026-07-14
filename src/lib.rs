#![no_std]
use soroban_sdk::{contract, contractimpl, token, Address, Env, Vec};

pub mod math;
pub mod storage;
pub mod types;

#[cfg(test)]
mod test;

use crate::types::{Error, Pool, PoolId, Position};

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
        storage::set_pool(
            &env,
            0,
            &Pool {
                manager: admin,
                share_token,
                reward_token,
                total_shares: 0,
                acc_reward_per_share: 0,
            },
        );
        storage::set_next_pool_id(&env, 1);
        storage::set_initialized(&env);

        Ok(())
    }

    /// Creates an isolated dividend pool for another tokenized property.
    pub fn create_pool(
        env: Env,
        manager: Address,
        share_token: Address,
        reward_token: Address,
    ) -> Result<PoolId, Error> {
        Self::check_initialized(&env)?;
        storage::get_admin(&env).require_auth();

        let pool_id = storage::get_next_pool_id(&env);
        let next_pool_id = pool_id.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        let pool = Pool {
            manager,
            share_token,
            reward_token,
            total_shares: 0,
            acc_reward_per_share: 0,
        };
        storage::set_pool(&env, pool_id, &pool);
        storage::set_next_pool_id(&env, next_pool_id);

        Ok(pool_id)
    }

    /// Read-only: Returns one property's pool configuration and accounting state.
    pub fn get_pool(env: Env, pool_id: PoolId) -> Result<Pool, Error> {
        Self::load_pool(&env, pool_id)
    }

    /// Read-only: Returns the number of pools created so far.
    pub fn get_pool_count(env: Env) -> PoolId {
        storage::get_next_pool_id(&env)
    }

    /// Read-only: Returns an investor's position in a property pool.
    pub fn get_position(env: Env, pool_id: PoolId, user: Address) -> Result<Position, Error> {
        Self::load_pool(&env, pool_id)?;
        Ok(storage::get_position(&env, pool_id, &user))
    }

    /// Deposits share tokens into a selected property pool.
    pub fn deposit_into(
        env: Env,
        pool_id: PoolId,
        user: Address,
        amount: i128,
    ) -> Result<(), Error> {
        user.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut pool = Self::load_pool(&env, pool_id)?;
        let mut position = storage::get_position(&env, pool_id, &user);
        let pending = Self::calculate_pool_pending(&pool, &position)?;
        if pending > 0 {
            token::Client::new(&env, &pool.reward_token).transfer(
                &env.current_contract_address(),
                &user,
                &pending,
            );
        }

        token::Client::new(&env, &pool.share_token).transfer(
            &user,
            &env.current_contract_address(),
            &amount,
        );
        position.shares = position
            .shares
            .checked_add(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        pool.total_shares = pool
            .total_shares
            .checked_add(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        position.reward_debt = math::accumulated(position.shares, pool.acc_reward_per_share)?;

        storage::set_position(&env, pool_id, &user, &position);
        storage::set_pool(&env, pool_id, &pool);
        Ok(())
    }

    /// Read-only: Returns claimable rewards in a selected property pool.
    pub fn get_pool_pending(env: Env, pool_id: PoolId, user: Address) -> Result<i128, Error> {
        let pool = Self::load_pool(&env, pool_id)?;
        let position = storage::get_position(&env, pool_id, &user);
        Self::calculate_pool_pending(&pool, &position)
    }

    /// Funds and distributes rewards within a selected property pool.
    pub fn distribute_to(
        env: Env,
        pool_id: PoolId,
        sender: Address,
        amount: i128,
    ) -> Result<(), Error> {
        sender.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut pool = Self::load_pool(&env, pool_id)?;
        if pool.total_shares == 0 {
            return Err(Error::NoSharesStaked);
        }
        token::Client::new(&env, &pool.reward_token).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );
        let increase = math::reward_increase(amount, pool.total_shares)?;
        pool.acc_reward_per_share = pool
            .acc_reward_per_share
            .checked_add(increase)
            .ok_or(Error::ArithmeticOverflow)?;
        storage::set_pool(&env, pool_id, &pool);
        Ok(())
    }

    /// Claims rewards from a selected property pool.
    pub fn claim_from(env: Env, pool_id: PoolId, user: Address) -> Result<i128, Error> {
        user.require_auth();
        let pool = Self::load_pool(&env, pool_id)?;
        let mut position = storage::get_position(&env, pool_id, &user);
        let pending = Self::calculate_pool_pending(&pool, &position)?;
        if pending <= 0 {
            return Ok(0);
        }

        position.reward_debt = math::accumulated(position.shares, pool.acc_reward_per_share)?;
        storage::set_position(&env, pool_id, &user, &position);
        token::Client::new(&env, &pool.reward_token).transfer(
            &env.current_contract_address(),
            &user,
            &pending,
        );
        Ok(pending)
    }

    /// Claims rewards from up to twenty property pools in one invocation.
    pub fn claim_many(env: Env, user: Address, pool_ids: Vec<PoolId>) -> Result<i128, Error> {
        user.require_auth();
        if pool_ids.len() > 20 {
            return Err(Error::TooManyPools);
        }

        let mut total_claimed = 0i128;
        for pool_id in pool_ids.iter() {
            let pool = Self::load_pool(&env, pool_id)?;
            let mut position = storage::get_position(&env, pool_id, &user);
            let pending = Self::calculate_pool_pending(&pool, &position)?;
            if pending > 0 {
                position.reward_debt =
                    math::accumulated(position.shares, pool.acc_reward_per_share)?;
                storage::set_position(&env, pool_id, &user, &position);
                token::Client::new(&env, &pool.reward_token).transfer(
                    &env.current_contract_address(),
                    &user,
                    &pending,
                );
                total_claimed = total_claimed
                    .checked_add(pending)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
        }
        Ok(total_claimed)
    }

    /// Withdraws share tokens from a selected property pool.
    pub fn withdraw_from(
        env: Env,
        pool_id: PoolId,
        user: Address,
        amount: i128,
    ) -> Result<(), Error> {
        user.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut pool = Self::load_pool(&env, pool_id)?;
        let mut position = storage::get_position(&env, pool_id, &user);
        if position.shares < amount {
            return Err(Error::InsufficientShares);
        }
        let pending = Self::calculate_pool_pending(&pool, &position)?;
        if pending > 0 {
            token::Client::new(&env, &pool.reward_token).transfer(
                &env.current_contract_address(),
                &user,
                &pending,
            );
        }
        token::Client::new(&env, &pool.share_token).transfer(
            &env.current_contract_address(),
            &user,
            &amount,
        );

        position.shares = position
            .shares
            .checked_sub(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        pool.total_shares = pool
            .total_shares
            .checked_sub(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        if position.shares == 0 {
            storage::remove_position(&env, pool_id, &user);
        } else {
            position.reward_debt = math::accumulated(position.shares, pool.acc_reward_per_share)?;
            storage::set_position(&env, pool_id, &user, &position);
        }
        storage::set_pool(&env, pool_id, &pool);
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

    fn load_pool(env: &Env, pool_id: PoolId) -> Result<Pool, Error> {
        Self::check_initialized(env)?;
        storage::get_pool(env, pool_id).ok_or(Error::PoolNotFound)
    }

    fn calculate_pool_pending(pool: &Pool, position: &Position) -> Result<i128, Error> {
        if position.shares == 0 {
            return Ok(0);
        }
        math::pending(
            position.shares,
            pool.acc_reward_per_share,
            position.reward_debt,
        )
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
