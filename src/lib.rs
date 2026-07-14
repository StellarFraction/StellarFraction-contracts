#![no_std]
use soroban_sdk::{contract, contractimpl, token, Address, Env, Vec};

pub mod events;
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
        admin.require_auth();

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
                paused: false,
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
        Self::check_not_paused(&env)?;
        storage::get_admin(&env).require_auth();

        let pool_id = storage::get_next_pool_id(&env);
        let next_pool_id = pool_id.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        let pool = Pool {
            manager,
            share_token,
            reward_token,
            total_shares: 0,
            acc_reward_per_share: 0,
            paused: false,
        };
        storage::set_pool(&env, pool_id, &pool);
        storage::set_next_pool_id(&env, next_pool_id);
        events::pool_created(&env, pool_id, &pool.manager);

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

    /// Rotates the manager authorized to administer a property pool.
    pub fn set_pool_manager(env: Env, pool_id: PoolId, new_manager: Address) -> Result<(), Error> {
        let mut pool = Self::load_pool(&env, pool_id)?;
        pool.manager.require_auth();
        pool.manager = new_manager;
        storage::set_pool(&env, pool_id, &pool);
        events::manager_changed(&env, pool_id, &pool.manager);
        Ok(())
    }

    /// Pauses or resumes new deposits and distributions for a property pool.
    pub fn set_pool_paused(env: Env, pool_id: PoolId, paused: bool) -> Result<(), Error> {
        let mut pool = Self::load_pool(&env, pool_id)?;
        pool.manager.require_auth();
        pool.paused = paused;
        storage::set_pool(&env, pool_id, &pool);
        events::pause_changed(&env, pool_id, paused);
        Ok(())
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
        Self::check_not_paused(&env)?;
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let minimum_deposit = storage::get_minimum_deposit(&env);
        if amount < minimum_deposit {
            return Err(Error::BelowMinimumDeposit);
        }

        let mut pool = Self::load_pool(&env, pool_id)?;
        Self::ensure_pool_active(&pool)?;
        let mut position = storage::get_position(&env, pool_id, &user);
        let new_shares = position
            .shares
            .checked_add(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        if new_shares > storage::get_max_stake_per_user(&env, &user) {
            return Err(Error::ExceedsMaxStake);
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
            &user,
            &env.current_contract_address(),
            &amount,
        );
        position.shares = new_shares;
        pool.total_shares = pool
            .total_shares
            .checked_add(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        position.reward_debt = math::accumulated(position.shares, pool.acc_reward_per_share)?;

        storage::set_position(&env, pool_id, &user, &position);
        storage::set_pool(&env, pool_id, &pool);
        events::deposited(&env, pool_id, &user, amount);
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
        Self::check_not_paused(&env)?;
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut pool = Self::load_pool(&env, pool_id)?;
        Self::ensure_pool_active(&pool)?;
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
        events::distributed(&env, pool_id, &sender, amount);
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
        events::claimed(&env, pool_id, &user, pending);
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
                events::claimed(&env, pool_id, &user, pending);
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
        events::withdrawn(&env, pool_id, &user, amount);
        Ok(())
    }

    /// Deposits real estate share tokens to stake them and earn dividends.
    pub fn deposit(env: Env, user: Address, amount: i128) -> Result<(), Error> {
        Self::deposit_into(env, 0, user, amount)
    }

    /// Withdraws real estate share tokens, unstaking them.
    pub fn withdraw(env: Env, user: Address, amount: i128) -> Result<(), Error> {
        Self::withdraw_from(env, 0, user, amount)
    }

    /// Deposits a lump sum of rental yield (USDC) and distributes it proportionally to stakers.
    pub fn distribute(env: Env, sender: Address, amount: i128) -> Result<(), Error> {
        Self::distribute_to(env, 0, sender, amount)
    }

    /// Claims accumulated USDC dividends for a user.
    pub fn claim(env: Env, user: Address) -> Result<i128, Error> {
        Self::claim_from(env, 0, user)
    }

    /// Read-only: Gets the amount of deed tokens a user has staked.
    pub fn get_shares(env: Env, user: Address) -> i128 {
        storage::get_position(&env, 0, &user).shares
    }

    /// Read-only: Gets the reward debt of a user.
    pub fn get_debt(env: Env, user: Address) -> i128 {
        storage::get_position(&env, 0, &user).reward_debt
    }

    /// Read-only: Returns the claimable USDC dividends for a user.
    pub fn get_pending(env: Env, user: Address) -> Result<i128, Error> {
        Self::get_pool_pending(env, 0, user)
    }

    /// Read-only: Returns contract configuration and global state.
    /// (admin, share_token, reward_token, total_shares, acc_reward_per_share)
    pub fn get_contract_info(env: Env) -> (Address, Address, Address, i128, i128) {
        let admin = storage::get_admin(&env);
        let pool = Self::load_pool(&env, 0).unwrap();
        (
            admin,
            pool.share_token,
            pool.reward_token,
            pool.total_shares,
            pool.acc_reward_per_share,
        )
    }

    /// Admin-only: Pauses pool creation, deposits, and distributions globally.
    pub fn pause(env: Env) -> Result<(), Error> {
        Self::check_initialized(&env)?;
        storage::get_admin(&env).require_auth();
        storage::set_paused(&env, true);
        events::contract_paused(&env, true);
        Ok(())
    }

    /// Admin-only: Resumes globally paused operations.
    pub fn unpause(env: Env) -> Result<(), Error> {
        Self::check_initialized(&env)?;
        storage::get_admin(&env).require_auth();
        storage::set_paused(&env, false);
        events::contract_paused(&env, false);
        Ok(())
    }

    /// Admin-only: Sets the minimum amount accepted by a deposit operation.
    pub fn set_minimum_deposit(env: Env, amount: i128) -> Result<(), Error> {
        Self::check_initialized(&env)?;
        storage::get_admin(&env).require_auth();
        if amount < 0 {
            return Err(Error::InvalidAmount);
        }
        storage::set_minimum_deposit(&env, amount);
        Ok(())
    }

    /// Admin-only: Sets the per-pool stake ceiling for an investor.
    pub fn set_max_stake_per_user(env: Env, user: Address, limit: i128) -> Result<(), Error> {
        Self::check_initialized(&env)?;
        storage::get_admin(&env).require_auth();
        if limit < 0 {
            return Err(Error::InvalidAmount);
        }
        storage::set_max_stake_per_user(&env, &user, limit);
        Ok(())
    }

    /// Admin-only: Transfers contract administration to a new address.
    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        Self::check_initialized(&env)?;
        storage::get_admin(&env).require_auth();
        storage::set_admin(&env, &new_admin);
        Ok(())
    }

    /// Read-only: Returns whether global emergency pause is active.
    pub fn is_paused(env: Env) -> bool {
        storage::is_paused(&env)
    }

    // Helper functions

    fn check_initialized(env: &Env) -> Result<(), Error> {
        if !storage::is_initialized(env) {
            return Err(Error::NotInitialized);
        }
        Ok(())
    }

    fn check_not_paused(env: &Env) -> Result<(), Error> {
        if storage::is_paused(env) {
            return Err(Error::ContractPaused);
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

    fn ensure_pool_active(pool: &Pool) -> Result<(), Error> {
        if pool.paused {
            return Err(Error::PoolPaused);
        }
        Ok(())
    }
}

// StellarFraction distribution contract - handles real estate dividend payouts

// Scale factor configured to 1e12 for high-precision arithmetic representation

// Initialize method sets up the primary admin and contract state configuration

// Check if contract has already been initialized to prevent re-initialization

// Store property share token address in persistent storage for stakers check

// Store reward token address in persistent storage for dividend distributions

// Set accumulated reward per share to zero initially for global index tracker

// Set total staked shares to zero initially to represent empty pool state

// Set initialized flag to lock initial setup parameters from future updates

// Deposit function allows users to stake property shares and earn USDC

// Enforce user authorization signature check to secure staking deposits

// Verify that initialization has run successfully before permitting deposits

// Enforce minimum threshold that deposited share amount must be positive

// Fetch share token address from contract storage to identify target SAC client

// Fetch reward token address from contract storage to handle pending claims

// Calculate any pending rewards to be automatically claimed during deposit

// Transfer accumulated dividends to user if they have claimable balances

// Transfer staking shares from user account to contract address securely

// Retrieve current shares staked by this user to update balance database

// Calculate new shares total by adding current amount to the deposit value

// Save updated user shares count to persistent contract storage index

// Fetch global total shares to add newly deposited shares to pool sum

// Save updated global total shares value to track distribution ratio

// Fetch global reward accumulator index to compute new user reward debt

// Calculate new user reward debt based on updated shares and global index

// Save user reward debt to storage to mark previous dividends as claimed

// Withdraw function allows users to unstake their property share tokens

// Enforce user auth check to prevent unauthorized withdrawals from account

// Validate that the withdraw amount is greater than zero before processing

// Verify user has sufficient shares to fulfill the withdrawal request

// Calculate and auto-claim pending dividends before executing unstake

// Transfer share tokens back to user's wallet address from contract balance

// Deduct withdrawn shares from user record to compute new share total

// Remove user share and debt keys from storage if balance reaches zero

// Save updated user shares to storage if remaining balance is positive

// Deduct withdrawn shares from global total shares to adjust pool size

// Distribute function allows admin or users to deposit USDC rent to stakers

// Enforce authorization requirement on sender for rent distribution
