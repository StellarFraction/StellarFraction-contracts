#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, panic_with_error, token, Address, Env, IntoVal, Val,
};

mod test;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    ShareToken,         // The fractional real estate asset (deed token)
    RewardToken,        // The yield token (e.g. USDC)
    AccRewardPerShare,  // Accumulator for dividends per share, scaled by 1e12
    TotalShares,        // Total deed tokens staked in the contract
    UserShare(Address), // Amount of deed tokens staked by a user
    UserDebt(Address),  // Reward debt for a user
    Initialized,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InsufficientShares = 3,
    NoSharesStaked = 4,
    InvalidAmount = 5,
    NotAdmin = 6,
}

const SCALE_FACTOR: i128 = 1_000_000_000_000; // 1e12 for precision

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
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::ShareToken, &share_token);
        env.storage().instance().set(&DataKey::RewardToken, &reward_token);
        env.storage().instance().set(&DataKey::AccRewardPerShare, &0i128);
        env.storage().instance().set(&DataKey::TotalShares, &0i128);
        env.storage().instance().set(&DataKey::Initialized, &true);

        Ok(())
    }

    /// Deposits real estate share tokens to stake them and earn dividends.
    pub fn deposit(env: Env, user: Address, amount: i128) -> Result<(), Error> {
        user.require_auth();
        Self::check_initialized(&env)?;

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let share_token_addr: Address = env.storage().instance().get(&DataKey::ShareToken).unwrap();
        let reward_token_addr: Address = env.storage().instance().get(&DataKey::RewardToken).unwrap();

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
        let current_shares = Self::get_shares(env.clone(), user.clone());
        let new_shares = current_shares + amount;
        env.storage().persistent().set(&DataKey::UserShare(user.clone()), &new_shares);

        let total_shares: i128 = env.storage().instance().get(&DataKey::TotalShares).unwrap();
        let new_total_shares = total_shares + amount;
        env.storage().instance().set(&DataKey::TotalShares, &new_total_shares);

        // 4. Update the user's reward debt
        let acc_reward_per_share: i128 = env.storage().instance().get(&DataKey::AccRewardPerShare).unwrap();
        let new_debt = (new_shares * acc_reward_per_share) / SCALE_FACTOR;
        env.storage().persistent().set(&DataKey::UserDebt(user), &new_debt);

        Ok(())
    }

    /// Withdraws real estate share tokens, unstaking them.
    pub fn withdraw(env: Env, user: Address, amount: i128) -> Result<(), Error> {
        user.require_auth();
        Self::check_initialized(&env)?;

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let current_shares = Self::get_shares(env.clone(), user.clone());
        if current_shares < amount {
            return Err(Error::InsufficientShares);
        }

        let share_token_addr: Address = env.storage().instance().get(&DataKey::ShareToken).unwrap();
        let reward_token_addr: Address = env.storage().instance().get(&DataKey::RewardToken).unwrap();

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
            env.storage().persistent().remove(&DataKey::UserShare(user.clone()));
            env.storage().persistent().remove(&DataKey::UserDebt(user.clone()));
        } else {
            env.storage().persistent().set(&DataKey::UserShare(user.clone()), &new_shares);
            let acc_reward_per_share: i128 = env.storage().instance().get(&DataKey::AccRewardPerShare).unwrap();
            let new_debt = (new_shares * acc_reward_per_share) / SCALE_FACTOR;
            env.storage().persistent().set(&DataKey::UserDebt(user.clone()), &new_debt);
        }

        let total_shares: i128 = env.storage().instance().get(&DataKey::TotalShares).unwrap();
        let new_total_shares = total_shares - amount;
        env.storage().instance().set(&DataKey::TotalShares, &new_total_shares);

        Ok(())
    }

    /// Deposits a lump sum of rental yield (USDC) and distributes it proportionally to stakers.
    pub fn distribute(env: Env, sender: Address, amount: i128) -> Result<(), Error> {
        sender.require_auth();
        Self::check_initialized(&env)?;

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let total_shares: i128 = env.storage().instance().get(&DataKey::TotalShares).unwrap();
        if total_shares == 0 {
            return Err(Error::NoSharesStaked);
        }

        let reward_token_addr: Address = env.storage().instance().get(&DataKey::RewardToken).unwrap();

        // 1. Transfer reward tokens to contract
        let reward_client = token::Client::new(&env, &reward_token_addr);
        reward_client.transfer(&sender, &env.current_contract_address(), &amount);

        // 2. Accumulate the reward per share
        let acc_reward_per_share: i128 = env.storage().instance().get(&DataKey::AccRewardPerShare).unwrap();
        let reward_increase = (amount * SCALE_FACTOR) / total_shares;
        let new_acc_reward_per_share = acc_reward_per_share + reward_increase;
        env.storage().instance().set(&DataKey::AccRewardPerShare, &new_acc_reward_per_share);

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

        let reward_token_addr: Address = env.storage().instance().get(&DataKey::RewardToken).unwrap();

        // 1. Reset user debt
        let current_shares = Self::get_shares(env.clone(), user.clone());
        let acc_reward_per_share: i128 = env.storage().instance().get(&DataKey::AccRewardPerShare).unwrap();
        let new_debt = (current_shares * acc_reward_per_share) / SCALE_FACTOR;
        env.storage().persistent().set(&DataKey::UserDebt(user.clone()), &new_debt);

        // 2. Transfer rewards
        let reward_client = token::Client::new(&env, &reward_token_addr);
        reward_client.transfer(&env.current_contract_address(), &user, &pending);

        Ok(pending)
    }

    /// Read-only: Gets the amount of deed tokens a user has staked.
    pub fn get_shares(env: Env, user: Address) -> i128 {
        env.storage().persistent().get(&DataKey::UserShare(user)).unwrap_or(0i128)
    }

    /// Read-only: Gets the reward debt of a user.
    pub fn get_debt(env: Env, user: Address) -> i128 {
        env.storage().persistent().get(&DataKey::UserDebt(user)).unwrap_or(0i128)
    }

    /// Read-only: Returns the claimable USDC dividends for a user.
    pub fn get_pending(env: Env, user: Address) -> i128 {
        Self::calculate_pending(&env, &user)
    }

    /// Read-only: Returns contract configuration and global state.
    /// (admin, share_token, reward_token, total_shares, acc_reward_per_share)
    pub fn get_contract_info(env: Env) -> (Address, Address, Address, i128, i128) {
        let admin = env.storage().instance().get(&DataKey::Admin).unwrap();
        let share = env.storage().instance().get(&DataKey::ShareToken).unwrap();
        let reward = env.storage().instance().get(&DataKey::RewardToken).unwrap();
        let total_shares = env.storage().instance().get(&DataKey::TotalShares).unwrap_or(0i128);
        let acc_reward = env.storage().instance().get(&DataKey::AccRewardPerShare).unwrap_or(0i128);
        (admin, share, reward, total_shares, acc_reward)
    }

    // Helper functions

    fn check_initialized(env: &Env) -> Result<(), Error> {
        if !env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::NotInitialized);
        }
        Ok(())
    }

    fn calculate_pending(env: &Env, user: &Address) -> i128 {
        let shares = env.storage().persistent().get(&DataKey::UserShare(user.clone())).unwrap_or(0i128);
        if shares == 0 {
            return 0;
        }
        let acc_reward_per_share = env.storage().instance().get(&DataKey::AccRewardPerShare).unwrap_or(0i128);
        let debt = env.storage().persistent().get(&DataKey::UserDebt(user.clone())).unwrap_or(0i128);
        
        let accumulated = (shares * acc_reward_per_share) / SCALE_FACTOR;
        accumulated - debt
    }
}
