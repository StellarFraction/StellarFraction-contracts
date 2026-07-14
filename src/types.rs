use soroban_sdk::{contracterror, contracttype, Address};

pub type PoolId = u32;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pool {
    pub manager: Address,
    pub share_token: Address,
    pub reward_token: Address,
    pub total_shares: i128,
    pub acc_reward_per_share: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Position {
    pub shares: i128,
    pub reward_debt: i128,
}

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
    NextPoolId,
    Pool(PoolId),
    Position(PoolId, Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InsufficientShares = 3,
    NoSharesStaked = 4,
    InvalidAmount = 5,
    NotAdmin = 6,
    ArithmeticOverflow = 7,
    PoolNotFound = 8,
    PoolPaused = 9,
    PoolNotEmpty = 10,
}
