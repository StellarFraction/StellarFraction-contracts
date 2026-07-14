use soroban_sdk::{contracterror, contracttype, Address, String};

/// Structured, on-chain-readable identity for the contract, returned by the
/// `metadata()` entrypoint. Mirrors the embedded wasm `contractmeta!` values in
/// a form clients can fetch in a single call.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
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
    Paused,             // Contract pause status
    MinimumDeposit,     // Minimum deposit amount
    MaxStakePerUser(Address), // Maximum stake limit per user
    LockupDuration,     // Seconds a deposit is locked before it can be withdrawn
    UnlockAt(Address),  // Ledger timestamp at which a user's stake unlocks
    ManagementFeeBps,   // Landlord management fee in basis points (1 bps = 0.01%)
    FeeCollector,       // Address that receives skimmed management fees
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
    ContractPaused = 7,
    BelowMinimumDeposit = 8,
    ExceedsMaxStake = 9,
    CannotRecoverProtocolToken = 10,
    StillLocked = 11,
    InvalidFeeBps = 12,
    FeeCollectorNotSet = 13,
}
