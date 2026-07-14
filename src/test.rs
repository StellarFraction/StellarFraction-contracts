#![cfg(test)]
use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

/// Register a Stellar Asset Contract to act as a mock token.
/// Returns (token address, standard client, admin client for minting).
fn create_token<'a>(
    env: &'a Env,
    admin: &Address,
) -> (Address, TokenClient<'a>, StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let address = sac.address();
    (
        address.clone(),
        TokenClient::new(env, &address),
        StellarAssetClient::new(env, &address),
    )
}

#[test]
fn test_full_dividend_distribution_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    // 1. Register mock share and reward tokens (Stellar Asset Contracts)
    let (share_token_id, share_token, share_token_admin) = create_token(&env, &admin);
    let (reward_token_id, reward_token, reward_token_admin) = create_token(&env, &admin);

    // 2. Register the Distribution Contract
    let contract_id = env.register(DistributionContract, ());
    let client = DistributionContractClient::new(&env, &contract_id);

    // 3. Initialize Contract
    client.initialize(&admin, &share_token_id, &reward_token_id);

    // 4. Mint tokens to stakers and admin
    share_token_admin.mint(&user_a, &1000); // User A has 1000 deed tokens
    share_token_admin.mint(&user_b, &3000); // User B has 3000 deed tokens
    reward_token_admin.mint(&admin, &50000); // Admin has 50,000 USDC (reward tokens)

    // 5. Deposit Share Tokens (Staking)
    client.deposit(&user_a, &1000);
    client.deposit(&user_b, &3000);

    // Verify deposits
    assert_eq!(client.get_shares(&user_a), 1000);
    assert_eq!(client.get_shares(&user_b), 3000);
    assert_eq!(share_token.balance(&contract_id), 4000);
    assert_eq!(share_token.balance(&user_a), 0);
    assert_eq!(share_token.balance(&user_b), 0);

    // 6. First distribution of rewards (Admin distributes 4000 USDC)
    client.distribute(&admin, &4000);

    // Verify pending rewards:
    // Total Shares = 4000
    // AccRewardPerShare = 4000 * 1e12 / 4000 = 1e12
    // User A pending: 1000 * 1e12 / 1e12 - 0 = 1000 USDC
    // User B pending: 3000 * 1e12 / 1e12 - 0 = 3000 USDC
    assert_eq!(client.get_pending(&user_a), 1000);
    assert_eq!(client.get_pending(&user_b), 3000);

    // 7. User A claims rewards
    let claimed_a = client.claim(&user_a);
    assert_eq!(claimed_a, 1000);
    assert_eq!(reward_token.balance(&user_a), 1000);
    assert_eq!(client.get_pending(&user_a), 0);

    // 8. Second distribution (Admin distributes another 8000 USDC)
    client.distribute(&admin, &8000);

    // Total Shares = 4000
    // AccRewardPerShare increases by 8000 * 1e12 / 4000 = 2e12.
    // Total AccRewardPerShare = 3e12
    // User A pending: 1000 * 3e12 / 1e12 - Debt(1000) = 3000 - 1000 = 2000 USDC
    // User B pending: 3000 * 3e12 / 1e12 - Debt(0) = 9000 USDC
    assert_eq!(client.get_pending(&user_a), 2000);
    assert_eq!(client.get_pending(&user_b), 9000);

    // 9. User B withdraws 1500 shares (unstakes half)
    // This should auto-claim their pending 9000 USDC and set shares to 1500
    client.withdraw(&user_b, &1500);

    assert_eq!(client.get_shares(&user_b), 1500);
    assert_eq!(reward_token.balance(&user_b), 9000);
    assert_eq!(share_token.balance(&user_b), 1500);
    assert_eq!(client.get_pending(&user_b), 0);
    assert_eq!(share_token.balance(&contract_id), 2500);

    // 10. Third distribution (Admin distributes 5000 USDC)
    // Total Shares = 2500
    // AccRewardPerShare increases by 5000 * 1e12 / 2500 = 2e12
    // Total AccRewardPerShare = 5e12
    client.distribute(&admin, &5000);

    // User A pending: shares = 1000, debt = 1000 (set when they claimed).
    // So pending now: (1000 * 5e12) / 1e12 - 1000 = 5000 - 1000 = 4000 USDC.
    // (Which is 2000 from second dist + 2000 from third dist)
    assert_eq!(client.get_pending(&user_a), 4000);

    // User B pending: shares = 1500, debt was updated during withdraw to:
    // (1500 * 3e12) / 1e12 = 4500.
    // So pending now: (1500 * 5e12) / 1e12 - 4500 = 7500 - 4500 = 3000 USDC.
    // (Which is 1500 shares * 2 USDC per share = 3000 USDC)
    assert_eq!(client.get_pending(&user_b), 3000);
}

#[test]
fn test_initialize_requires_admin_auth() {
    // No mock_all_auths here: initialize must fail without the admin's
    // authorization, otherwise anyone could front-run deployment and
    // appoint themselves admin.
    let env = Env::default();

    let admin = Address::generate(&env);
    let (share_token_id, _, _) = create_token(&env, &admin);
    let (reward_token_id, _, _) = create_token(&env, &admin);

    let contract_id = env.register(DistributionContract, ());
    let client = DistributionContractClient::new(&env, &contract_id);

    let unauthenticated = client.try_initialize(&admin, &share_token_id, &reward_token_id);
    assert!(unauthenticated.is_err());
}

#[test]
fn test_errors_and_boundaries() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let (share_token_id, _, _) = create_token(&env, &admin);
    let (reward_token_id, _, _) = create_token(&env, &admin);

    let contract_id = env.register(DistributionContract, ());
    let client = DistributionContractClient::new(&env, &contract_id);

    // Try to deposit before initialization
    let err_pre_init = client.try_deposit(&user, &100);
    assert!(err_pre_init.is_err());

    // Initialize
    client.initialize(&admin, &share_token_id, &reward_token_id);

    // Try to initialize again
    let err_re_init = client.try_initialize(&admin, &share_token_id, &reward_token_id);
    assert!(err_re_init.is_err());

    // Try to distribute with 0 shares
    let err_dist_zero = client.try_distribute(&admin, &1000);
    assert!(err_dist_zero.is_err());

    // Try to withdraw more than staked
    let err_withdraw_insufficient = client.try_withdraw(&user, &100);
    assert!(err_withdraw_insufficient.is_err());
}

/// Shared harness: owns the env and token/contract addresses. Clients are
/// built on demand so the struct itself doesn't self-reference the env.
struct Harness {
    env: Env,
    admin: Address,
    contract_id: Address,
    share_id: Address,
    reward_id: Address,
}

impl Harness {
    fn client(&self) -> DistributionContractClient<'_> {
        DistributionContractClient::new(&self.env, &self.contract_id)
    }
    fn share_token(&self) -> TokenClient<'_> {
        TokenClient::new(&self.env, &self.share_id)
    }
    fn reward_token(&self) -> TokenClient<'_> {
        TokenClient::new(&self.env, &self.reward_id)
    }
    fn share_admin(&self) -> StellarAssetClient<'_> {
        StellarAssetClient::new(&self.env, &self.share_id)
    }
    fn reward_admin(&self) -> StellarAssetClient<'_> {
        StellarAssetClient::new(&self.env, &self.reward_id)
    }
}

fn setup() -> Harness {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (share_id, _, _) = create_token(&env, &admin);
    let (reward_id, _, _) = create_token(&env, &admin);

    let contract_id = env.register(DistributionContract, ());
    let client = DistributionContractClient::new(&env, &contract_id);
    client.initialize(&admin, &share_id, &reward_id);

    Harness {
        env,
        admin,
        contract_id,
        share_id,
        reward_id,
    }
}

/// Issue #21: distribute must reject when there are no shares staked,
/// guarding the `amount * SCALE / total_shares` division against a zero
/// denominator — both before anyone stakes and after everyone withdraws.
#[test]
fn test_distribute_division_by_zero_safeguard() {
    let h = setup();
    let user = Address::generate(&h.env);
    h.share_admin().mint(&user, &1000);
    h.reward_admin().mint(&h.admin, &10_000);

    // No stakers yet -> distribute must error, never divide by zero.
    assert!(h.client().try_distribute(&h.admin, &1000).is_err());

    // Stake, then fully withdraw so total_shares returns to zero.
    h.client().deposit(&user, &1000);
    assert!(h.client().try_distribute(&h.admin, &1000).is_ok());
    h.client().withdraw(&user, &1000);

    // Denominator is zero again -> must still be safely rejected.
    assert!(h.client().try_distribute(&h.admin, &1000).is_err());
}

/// Issue #22: reward math must stay correct at institutional scale without
/// overflowing i128. Uses a billion tokens at 7-decimal precision (1e16) and
/// a matching distribution, exercising the `amount * SCALE_FACTOR` (1e28) and
/// `shares * acc_reward_per_share` intermediates while staying within i128's
/// ~1.7e38 ceiling. cargo test runs with overflow-checks on, so any overflow
/// would panic rather than silently wrap.
#[test]
fn test_reward_math_no_overflow_at_scale() {
    let h = setup();
    let whale = Address::generate(&h.env);

    // 1 billion deed tokens at 7 decimals.
    let big_stake: i128 = 1_000_000_000 * 10_000_000; // 1e16
    h.share_admin().mint(&whale, &big_stake);
    h.reward_admin().mint(&h.admin, &big_stake);

    h.client().deposit(&whale, &big_stake);

    // Distribute a large lump sum; internally computes big_stake * 1e12 (~1e28).
    h.client().distribute(&h.admin, &big_stake);

    // Sole staker is owed the entire distribution, exactly, with no overflow.
    assert_eq!(h.client().get_pending(&whale), big_stake);

    // A second identical distribution should double the entitlement.
    h.reward_admin().mint(&h.admin, &big_stake);
    h.client().distribute(&h.admin, &big_stake);
    assert_eq!(h.client().get_pending(&whale), big_stake * 2);
}

/// Issue #23: with integer division the contract must always round *down*
/// so it can never distribute more reward than was deposited. Three stakers
/// each hold 1 share; a 2-unit distribution can't be split evenly, so the
/// per-share increment truncates and every micro-entitlement rounds to zero
/// (the 2 units of dust stay in the contract). A later 3-unit distribution
/// divides cleanly and each staker becomes owed exactly 1.
#[test]
fn test_precision_rounding_micro_investments() {
    let h = setup();
    let a = Address::generate(&h.env);
    let b = Address::generate(&h.env);
    let c = Address::generate(&h.env);
    for u in [&a, &b, &c] {
        h.share_admin().mint(u, &1);
        h.client().deposit(u, &1);
    }
    h.reward_admin().mint(&h.admin, &100);

    // 2 units across 3 shares -> per-share increment truncates, all pending 0.
    h.client().distribute(&h.admin, &2);
    let pending_sum = h.client().get_pending(&a)
        + h.client().get_pending(&b)
        + h.client().get_pending(&c);
    assert_eq!(pending_sum, 0, "rounding must never over-pay stakers");

    // 3 units across 3 shares divides cleanly -> each owed exactly 1.
    h.client().distribute(&h.admin, &3);
    assert_eq!(h.client().get_pending(&a), 1);
    assert_eq!(h.client().get_pending(&b), 1);
    assert_eq!(h.client().get_pending(&c), 1);
}

/// Issue #24: unit-test the single-staker deposit path in isolation, asserting
/// every piece of state moves consistently - user share ledger, global total,
/// token custody transfer, and the reward-debt baseline. Also covers a second
/// top-up deposit accumulating onto the existing position.
#[test]
fn test_single_staker_deposit() {
    let h = setup();
    let user = Address::generate(&h.env);
    h.share_admin().mint(&user, &1000);

    // First deposit.
    h.client().deposit(&user, &400);
    assert_eq!(h.client().get_shares(&user), 400);
    assert_eq!(h.share_token().balance(&user), 600);
    assert_eq!(h.share_token().balance(&h.contract_id), 400);
    // No rewards yet, so debt baseline is zero.
    assert_eq!(h.client().get_debt(&user), 0);
    assert_eq!(h.client().get_pending(&user), 0);

    // Top-up deposit accumulates onto the same position.
    h.client().deposit(&user, &600);
    assert_eq!(h.client().get_shares(&user), 1000);
    assert_eq!(h.share_token().balance(&user), 0);
    assert_eq!(h.share_token().balance(&h.contract_id), 1000);

    // Global total mirrors the single staker's holdings.
    let (_, _, _, total_shares, _) = h.client().get_contract_info();
    assert_eq!(total_shares, 1000);
}
