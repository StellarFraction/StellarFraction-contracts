use soroban_sdk::{contracterror, contracttype, Address};

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
}
