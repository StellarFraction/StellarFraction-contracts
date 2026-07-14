#![no_std]
use soroban_sdk::{contract, contractimpl, contractmeta, token, Address, Env, Vec};

pub mod events;
pub mod math;
pub mod storage;
pub mod types;

#[cfg(test)]
mod test;

use crate::types::{Error, Pool, PoolId, Position};

/// On-chain contract version, surfaced both in embedded wasm metadata and via
/// the `version()` entrypoint so tooling and clients agree on a single source.
pub const CONTRACT_VERSION: &str = "0.1.0";

// Metadata embedded directly into the compiled wasm. Explorers and tooling can
// read these ledger entries without invoking the contract.
contractmeta!(key = "name", val = "StellarFraction Distribution");
contractmeta!(key = "version", val = "0.1.0");
contractmeta!(
    key = "description",
    val = "O(1) proportional rental-yield distribution for fractional real estate stakers"
);
contractmeta!(
    key = "repository",
    val = "github.com/StellarFraction/StellarFraction-contracts"
);

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

        // Refresh the lockup: each deposit restarts the lock window so a
        // fresh top-up can't be used to sidestep the configured lockup.
        let lockup = storage::get_lockup_duration(&env);
        if lockup > 0 {
            let unlock_at = env.ledger().timestamp() + lockup;
            storage::set_unlock_at(&env, &user, unlock_at);
        }

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

        let reward_client = token::Client::new(&env, &pool.reward_token);

        // 1. Pull the full amount from the sender into the contract.
        reward_client.transfer(&sender, &env.current_contract_address(), &amount);

        // 2. Skim the landlord management fee (if configured) off the top and
        //    forward it to the collector. Only the remainder is shared out to
        //    stakers. The fee is applied solely when a non-zero rate AND a
        //    collector are both set.
        let fee_bps = storage::get_management_fee_bps(&env);
        let mut fee_amount: i128 = 0;
        if fee_bps > 0 {
            match storage::get_fee_collector(&env) {
                Some(collector) => {
                    fee_amount = (amount * fee_bps as i128) / 10_000;
                    if fee_amount > 0 {
                        reward_client.transfer(
                            &env.current_contract_address(),
                            &collector,
                            &fee_amount,
                        );
                    }
                }
                None => return Err(Error::FeeCollectorNotSet),
            }
        }
        let distributable = amount - fee_amount;

        // 3. Accumulate the reward per share over the post-fee remainder.
        let increase = math::reward_increase(distributable, pool.total_shares)?;
        pool.acc_reward_per_share = pool
            .acc_reward_per_share
            .checked_add(increase)
            .ok_or(Error::ArithmeticOverflow)?;
        storage::set_pool(&env, pool_id, &pool);

        // Emit distribution event (reports the net distributed amount).
        events::distributed(&env, pool_id, &sender, distributable);

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

        // Enforce the staking lockup: the position cannot be withdrawn until
        // its unlock timestamp has passed.
        if env.ledger().timestamp() < storage::get_unlock_at(&env, &user) {
            return Err(Error::StillLocked);
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

    /// Read-only: The contract's semantic version string. Backed by the same
    /// CONTRACT_VERSION constant embedded in the wasm metadata.
    pub fn version(env: Env) -> soroban_sdk::String {
        soroban_sdk::String::from_str(&env, CONTRACT_VERSION)
    }

    /// Read-only: Structured contract identity (name, version, description) in a
    /// single call, mirroring the embedded wasm metadata.
    pub fn metadata(env: Env) -> crate::types::ContractMetadata {
        crate::types::ContractMetadata {
            name: soroban_sdk::String::from_str(&env, "StellarFraction Distribution"),
            version: soroban_sdk::String::from_str(&env, CONTRACT_VERSION),
            description: soroban_sdk::String::from_str(
                &env,
                "O(1) proportional rental-yield distribution for fractional real estate stakers",
            ),
        }
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

    /// Admin-only: Set the address that receives skimmed management fees
    /// (e.g. the landlord / property manager treasury).
    pub fn set_fee_collector(env: Env, collector: Address) -> Result<(), Error> {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        Self::check_initialized(&env)?;

        storage::set_fee_collector(&env, &collector);
        Ok(())
    }

    /// Read-only: The configured fee collector, if any has been set.
    pub fn get_fee_collector(env: Env) -> Option<Address> {
        storage::get_fee_collector(&env)
    }

    /// Read-only: The full management-fee configuration as (fee_bps, collector).
    /// `collector` is None until one is set. Convenience accessor so a client
    /// can fetch the whole fee policy in a single call.
    pub fn get_fee_config(env: Env) -> (u32, Option<Address>) {
        (
            storage::get_management_fee_bps(&env),
            storage::get_fee_collector(&env),
        )
    }

    /// Admin-only: Rescue tokens accidentally sent to the contract.
    ///
    /// Hard-guarded so it can NEVER move the staked share token or the reward
    /// token - those balances belong to stakers (share custody and owed
    /// dividends respectively). Only unrelated ("foreign") tokens that were
    /// mistakenly transferred in can be swept out, protecting user funds.
    pub fn recover_token(env: Env, token: Address, to: Address, amount: i128) -> Result<(), Error> {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        Self::check_initialized(&env)?;

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let pool_count = storage::get_next_pool_id(&env);
        for pool_id in 0..pool_count {
            if let Some(pool) = storage::get_pool(&env, pool_id) {
                if token == pool.share_token || token == pool.reward_token {
                    return Err(Error::CannotRecoverProtocolToken);
                }
            }
        }

        token::Client::new(&env, &token).transfer(&env.current_contract_address(), &to, &amount);

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
