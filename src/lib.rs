#![no_std]
use soroban_sdk::{contract, contractimpl, token, Address, Env};

pub mod storage;
pub mod types;

#[cfg(test)]
mod test;

use crate::types::Error;

const SCALE_FACTOR: i128 = 1_000_000_000_000; // 1e12 for precision

#[contract]
pub struct DistributionContract;

#[contractimpl]
impl DistributionContract {
    /// Initialize the contract with the admin, the property share token, and the rental yield (USDC) token.
    ///
    /// Requires authorization from `admin` so that a third party cannot
    /// front-run deployment and appoint themselves admin.
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
        storage::set_initialized(&env);

        Ok(())
    }

    /// Deposits real estate share tokens to stake them and earn dividends.
    pub fn deposit(env: Env, user: Address, amount: i128) -> Result<(), Error> {
        user.require_auth();
        Self::check_initialized(&env)?;
        Self::check_not_paused(&env)?;

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let min_deposit = storage::get_minimum_deposit(&env);
        if amount < min_deposit {
            return Err(Error::BelowMinimumDeposit);
        }

        let current_shares = storage::get_user_shares(&env, &user);
        let new_shares = current_shares + amount;
        let max_stake = storage::get_max_stake_per_user(&env, &user);
        if new_shares > max_stake {
            return Err(Error::ExceedsMaxStake);
        }

        let share_token_addr = storage::get_share_token(&env);
        let reward_token_addr = storage::get_reward_token(&env);

        // 1. Claim any pending rewards first to prevent overriding them
        let pending = Self::calculate_pending(&env, &user);
        if pending > 0 {
            let reward_client = token::Client::new(&env, &reward_token_addr);
            reward_client.transfer(&env.current_contract_address(), &user, &pending);
        }

        // 2. Transfer the share tokens from the user to the contract
        let share_client = token::Client::new(&env, &share_token_addr);
        share_client.transfer(&user, &env.current_contract_address(), &amount);

        // 3. Update user and global share records
        storage::set_user_shares(&env, &user, new_shares);

        let total_shares = storage::get_total_shares(&env);
        let new_total_shares = total_shares + amount;
        storage::set_total_shares(&env, new_total_shares);

        // 4. Update the user's reward debt
        let acc_reward_per_share = storage::get_acc_reward_per_share(&env);
        let new_debt = (new_shares * acc_reward_per_share) / SCALE_FACTOR;
        storage::set_user_debt(&env, &user, new_debt);

        // 5. Refresh the lockup: each deposit restarts the lock window so a
        // fresh top-up can't be used to sidestep the configured lockup.
        let lockup = storage::get_lockup_duration(&env);
        if lockup > 0 {
            let unlock_at = env.ledger().timestamp() + lockup;
            storage::set_unlock_at(&env, &user, unlock_at);
        }

        // 6. Emit deposit event
        env.events().publish(("deposit", user.clone()), (amount, new_shares));

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

        // Enforce the staking lockup: the position cannot be withdrawn until
        // its unlock timestamp has passed.
        if env.ledger().timestamp() < storage::get_unlock_at(&env, &user) {
            return Err(Error::StillLocked);
        }

        let share_token_addr = storage::get_share_token(&env);
        let reward_token_addr = storage::get_reward_token(&env);

        // 1. Claim any pending rewards
        let pending = Self::calculate_pending(&env, &user);
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
            let new_debt = (new_shares * acc_reward_per_share) / SCALE_FACTOR;
            storage::set_user_debt(&env, &user, new_debt);
        }

        let total_shares = storage::get_total_shares(&env);
        let new_total_shares = total_shares - amount;
        storage::set_total_shares(&env, new_total_shares);

        // 4. Emit withdraw event
        env.events().publish(("withdraw", user.clone()), (amount, new_shares));

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
        let reward_increase = (amount * SCALE_FACTOR) / total_shares;
        let new_acc_reward_per_share = acc_reward_per_share + reward_increase;
        storage::set_acc_reward_per_share(&env, new_acc_reward_per_share);

        // 3. Emit distribution event
        env.events().publish(("distribute",), (amount, total_shares, new_acc_reward_per_share));

        Ok(())
    }

    /// Claims accumulated USDC dividends for a user.
    pub fn claim(env: Env, user: Address) -> Result<i128, Error> {
        user.require_auth();
        Self::check_initialized(&env)?;

        let pending = Self::calculate_pending(&env, &user);
        if pending <= 0 {
            return Ok(0);
        }

        let reward_token_addr = storage::get_reward_token(&env);

        // 1. Reset user debt
        let current_shares = storage::get_user_shares(&env, &user);
        let acc_reward_per_share = storage::get_acc_reward_per_share(&env);
        let new_debt = (current_shares * acc_reward_per_share) / SCALE_FACTOR;
        storage::set_user_debt(&env, &user, new_debt);

        // 2. Transfer rewards
        let reward_client = token::Client::new(&env, &reward_token_addr);
        reward_client.transfer(&env.current_contract_address(), &user, &pending);

        // 3. Emit claim event
        env.events().publish(("claim", user.clone()), pending);

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
    pub fn get_pending(env: Env, user: Address) -> i128 {
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

    /// Admin-only: Pause contract operations to prevent new deposits during emergencies.
    pub fn pause(env: Env) -> Result<(), Error> {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        Self::check_initialized(&env)?;

        storage::set_paused(&env, true);
        env.events().publish(("pause",), true);

        Ok(())
    }

    /// Admin-only: Unpause contract to resume normal operations.
    pub fn unpause(env: Env) -> Result<(), Error> {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        Self::check_initialized(&env)?;

        storage::set_paused(&env, false);
        env.events().publish(("pause",), false);

        Ok(())
    }

    /// Admin-only: Set minimum deposit amount.
    pub fn set_minimum_deposit(env: Env, amount: i128) -> Result<(), Error> {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        Self::check_initialized(&env)?;

        if amount < 0 {
            return Err(Error::InvalidAmount);
        }

        storage::set_minimum_deposit(&env, amount);
        Ok(())
    }

    /// Admin-only: Set maximum stake limit for a user.
    pub fn set_max_stake_per_user(env: Env, user: Address, limit: i128) -> Result<(), Error> {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        Self::check_initialized(&env)?;

        if limit < 0 {
            return Err(Error::InvalidAmount);
        }

        storage::set_max_stake_per_user(&env, &user, limit);
        Ok(())
    }

    /// Admin-only: Transfer admin privileges to a new address.
    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        Self::check_initialized(&env)?;

        storage::set_admin(&env, &new_admin);
        Ok(())
    }

    /// Read-only: Check if contract is paused.
    pub fn is_paused(env: Env) -> bool {
        storage::is_paused(&env)
    }

    /// Admin-only: Set the staking lockup duration (in seconds). New deposits
    /// lock the depositor's stake for this long before it can be withdrawn.
    /// A duration of 0 disables lockups entirely.
    pub fn set_lockup_duration(env: Env, seconds: u64) -> Result<(), Error> {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        Self::check_initialized(&env)?;

        storage::set_lockup_duration(&env, seconds);
        Ok(())
    }

    /// Read-only: Current staking lockup duration in seconds (0 = disabled).
    pub fn get_lockup_duration(env: Env) -> u64 {
        storage::get_lockup_duration(&env)
    }

    /// Read-only: Ledger timestamp at which the user's stake unlocks. Returns 0
    /// when the user has no locked position (never deposited under a lockup).
    pub fn get_unlock_time(env: Env, user: Address) -> u64 {
        storage::get_unlock_at(&env, &user)
    }

    /// Admin-only: Set the landlord management fee in basis points (1 bps =
    /// 0.01%, so 10000 bps = 100%). Rejected above 10000. A fee of 0 disables
    /// the skim. The fee is only applied on distribute once a collector is set.
    pub fn set_management_fee(env: Env, bps: u32) -> Result<(), Error> {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        Self::check_initialized(&env)?;

        if bps > 10_000 {
            return Err(Error::InvalidFeeBps);
        }

        storage::set_management_fee_bps(&env, bps);
        Ok(())
    }

    /// Read-only: Current management fee in basis points (0 = disabled).
    pub fn get_management_fee(env: Env) -> u32 {
        storage::get_management_fee_bps(&env)
    }

    /// Admin-only: Rescue tokens accidentally sent to the contract.
    ///
    /// Hard-guarded so it can NEVER move the staked share token or the reward
    /// token - those balances belong to stakers (share custody and owed
    /// dividends respectively). Only unrelated ("foreign") tokens that were
    /// mistakenly transferred in can be swept out, protecting user funds.
    pub fn recover_token(
        env: Env,
        token: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), Error> {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        Self::check_initialized(&env)?;

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        // Refuse to touch either protocol token; those are staker funds.
        if token == storage::get_share_token(&env) || token == storage::get_reward_token(&env) {
            return Err(Error::CannotRecoverProtocolToken);
        }

        let client = token::Client::new(&env, &token);
        client.transfer(&env.current_contract_address(), &to, &amount);

        Ok(())
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

    fn calculate_pending(env: &Env, user: &Address) -> i128 {
        let shares = storage::get_user_shares(env, user);
        if shares == 0 {
            return 0;
        }
        let acc_reward_per_share = storage::get_acc_reward_per_share(env);
        let debt = storage::get_user_debt(env, user);

        let accumulated = (shares * acc_reward_per_share) / SCALE_FACTOR;
        accumulated - debt
    }
}
