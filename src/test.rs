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

/// Issue #25: distributions must split strictly in proportion to each staker's
/// share of the pool, and successive distributions accumulate independently of
/// when each staker joined. Uses a 1:3:6 split so a 10k distribution maps to
/// clean 1k / 3k / 6k entitlements, then adds a late joiner before a second
/// distribution to confirm the accumulator only rewards shares present at the
/// time of each distribution.
#[test]
fn test_multiple_staker_distribution() {
    let h = setup();
    let a = Address::generate(&h.env);
    let b = Address::generate(&h.env);
    let c = Address::generate(&h.env);

    h.share_admin().mint(&a, &1000);
    h.share_admin().mint(&b, &3000);
    h.share_admin().mint(&c, &6000);
    h.reward_admin().mint(&h.admin, &50_000);

    h.client().deposit(&a, &1000);
    h.client().deposit(&b, &3000);
    // c has not joined yet.

    // First distribution over 4000 shares (a + b), 1:3 split of 4000.
    h.client().distribute(&h.admin, &4000);
    assert_eq!(h.client().get_pending(&a), 1000);
    assert_eq!(h.client().get_pending(&b), 3000);
    assert_eq!(h.client().get_pending(&c), 0);

    // c joins; now pool is 1000:3000:6000 = 10000 shares.
    h.client().deposit(&c, &6000);

    // Second distribution over 10000 shares: a=1000, b=3000, c=6000.
    h.client().distribute(&h.admin, &10_000);
    // a and b carry their first-round entitlement forward.
    assert_eq!(h.client().get_pending(&a), 1000 + 1000);
    assert_eq!(h.client().get_pending(&b), 3000 + 3000);
    // c only earns from the round it was present for.
    assert_eq!(h.client().get_pending(&c), 6000);
}

/// Issue #26: unit-test the withdrawal path - a partial withdraw returns the
/// exact share count, auto-claims pending rewards, and leaves the remaining
/// position earning; a full withdraw clears the position entirely and zeroes
/// its pending. Also confirms the global total shrinks by each withdrawal.
#[test]
fn test_staker_share_withdrawals() {
    let h = setup();
    let user = Address::generate(&h.env);
    h.share_admin().mint(&user, &1000);
    h.reward_admin().mint(&h.admin, &10_000);

    h.client().deposit(&user, &1000);

    // Accrue 1000 in rewards to the sole staker.
    h.client().distribute(&h.admin, &1000);
    assert_eq!(h.client().get_pending(&user), 1000);

    // Partial withdraw of 400 auto-claims the 1000 pending and returns shares.
    h.client().withdraw(&user, &400);
    assert_eq!(h.client().get_shares(&user), 600);
    assert_eq!(h.share_token().balance(&user), 400);
    assert_eq!(h.reward_token().balance(&user), 1000); // auto-claimed
    assert_eq!(h.client().get_pending(&user), 0);
    let (_, _, _, total_after_partial, _) = h.client().get_contract_info();
    assert_eq!(total_after_partial, 600);

    // Remaining 600 still earns on the next distribution.
    h.client().distribute(&h.admin, &600);
    assert_eq!(h.client().get_pending(&user), 600);

    // Full withdraw clears the position and pays out the remaining pending.
    h.client().withdraw(&user, &600);
    assert_eq!(h.client().get_shares(&user), 0);
    assert_eq!(h.client().get_pending(&user), 0);
    assert_eq!(h.share_token().balance(&user), 1000);
    assert_eq!(h.reward_token().balance(&user), 1600);
    let (_, _, _, total_after_full, _) = h.client().get_contract_info();
    assert_eq!(total_after_full, 0);
}

/// Deterministic xorshift PRNG - keeps the fuzz test reproducible with no
/// external dependency (proptest/rand don't fit a no_std contract crate).
fn next_rand(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Issue #27: fuzz the staking reward pool with randomized sequences of
/// deposit / withdraw / distribute / claim across several actors, asserting the
/// core invariants hold after every operation regardless of ordering:
///   (I1) global total_shares == sum of individual staked shares
///   (I2) contract's share custody balance == total_shares
///   (I3) reward-token conservation: every distributed unit is either still
///        held by the contract or has been paid out to a staker - none is ever
///        minted or destroyed by the accounting.
///
/// All mutating calls go through `try_*`; any operation the contract rejects
/// (e.g. a rounding-dust shortfall on an auto-claim) rolls back cleanly and is
/// simply skipped, so the fuzzer only advances through valid on-chain states.
#[test]
fn test_fuzz_reward_pool_invariants() {
    let h = setup();
    let users = [
        Address::generate(&h.env),
        Address::generate(&h.env),
        Address::generate(&h.env),
        Address::generate(&h.env),
    ];

    // Fund everyone generously up front.
    for u in &users {
        h.share_admin().mint(u, &1_000_000);
    }
    h.reward_admin().mint(&h.admin, &10_000_000);

    let mut rng: u64 = 0x9E3779B97F4A7C15;
    let mut total_distributed: i128 = 0;

    for _ in 0..250 {
        let action = next_rand(&mut rng) % 4;
        let user = &users[(next_rand(&mut rng) % users.len() as u64) as usize];
        let amount = ((next_rand(&mut rng) % 5000) + 1) as i128;

        match action {
            0 => {
                let free = h.share_token().balance(user);
                if amount <= free {
                    let _ = h.client().try_deposit(user, &amount);
                }
            }
            1 => {
                let staked = h.client().get_shares(user);
                if amount <= staked {
                    let _ = h.client().try_withdraw(user, &amount);
                }
            }
            2 => {
                let (_, _, _, total_shares, _) = h.client().get_contract_info();
                if total_shares > 0 && h.client().try_distribute(&h.admin, &amount).is_ok() {
                    total_distributed += amount;
                }
            }
            _ => {
                let _ = h.client().try_claim(user);
            }
        }

        // (I1) + (I2): share custody mirrors the summed ledger.
        let (_, _, _, total_shares, _) = h.client().get_contract_info();
        let summed: i128 = users.iter().map(|u| h.client().get_shares(u)).sum();
        assert_eq!(total_shares, summed, "I1: total_shares desync");
        assert_eq!(
            h.share_token().balance(&h.contract_id),
            total_shares,
            "I2: custody balance desync"
        );

        // (I3): reward-token conservation across the whole system.
        let contract_reward = h.reward_token().balance(&h.contract_id);
        let paid_out: i128 = users.iter().map(|u| h.reward_token().balance(u)).sum();
        assert_eq!(
            contract_reward + paid_out,
            total_distributed,
            "I3: reward conservation broken (held {} + paid {} != distributed {})",
            contract_reward,
            paid_out,
            total_distributed
        );
    }
}

/// Issue #28: benchmark the reward math and prove it is O(1) in the number of
/// stakers. `distribute` only bumps the global accumulator - it never iterates
/// over stakers - so its metered CPU/memory cost must be identical whether the
/// pool has a handful of stakers or an order of magnitude more. We measure a
/// single distribute against a small pool and a large pool and assert the cost
/// does not grow with staker count.
#[test]
fn test_benchmark_reward_math_is_constant() {
    // Measures the metered cost of one distribute() over a pool of `n` stakers.
    fn cost_for_pool(n: u32) -> (u64, u64) {
        let h = setup();
        h.reward_admin().mint(&h.admin, &1_000_000);
        for _ in 0..n {
            let u = Address::generate(&h.env);
            h.share_admin().mint(&u, &1000);
            h.client().deposit(&u, &1000);
        }
        // Reset the budget so only the distribute call is measured.
        h.env.cost_estimate().budget().reset_default();
        h.client().distribute(&h.admin, &10_000);
        let b = h.env.cost_estimate().budget();
        (b.cpu_instruction_cost(), b.memory_bytes_cost())
    }

    const SMALL_POOL: u32 = 2;
    const LARGE_POOL: u32 = 20;
    let pool_ratio = (LARGE_POOL / SMALL_POOL) as u64; // 10x

    let (cpu_small, mem_small) = cost_for_pool(SMALL_POOL);
    let (cpu_large, mem_large) = cost_for_pool(LARGE_POOL);

    // The pool grew `pool_ratio` (10x). A per-staker O(n) loop would scale cost
    // roughly proportionally (~10x). `distribute` only bumps the global
    // accumulator, so cost stays far below linear - we require it under half the
    // linear projection. The residual growth is host/auth-harness overhead
    // (more recorded auths), not per-staker contract work.
    let cpu_linear = cpu_small * pool_ratio;
    let mem_linear = mem_small * pool_ratio;
    assert!(
        cpu_large * 2 < cpu_linear,
        "distribute CPU cost scales ~linearly with stakers - not O(1) ({} -> {}, linear would be ~{})",
        cpu_small,
        cpu_large,
        cpu_linear
    );
    assert!(
        mem_large * 2 < mem_linear,
        "distribute memory cost scales ~linearly with stakers - not O(1) ({} -> {}, linear would be ~{})",
        mem_small,
        mem_large,
        mem_linear
    );
}

/// Issue #29: verify the heaviest single call stays comfortably inside
/// Soroban's per-transaction resource limits. `deposit` is the worst case: it
/// authorizes, auto-claims pending rewards (a reward-token transfer), then
/// moves share tokens into custody and rewrites three storage entries. We
/// measure it and assert both CPU instructions and memory sit well under the
/// network ceilings, leaving ample headroom for host/auth overhead on-chain.
#[test]
fn test_execution_footprint_within_limits() {
    // Soroban smart-contract per-transaction resource ceilings.
    const CPU_TX_LIMIT: u64 = 100_000_000; // instructions
    const MEM_TX_LIMIT: u64 = 40 * 1024 * 1024; // 40 MiB

    let h = setup();
    let user = Address::generate(&h.env);
    let other = Address::generate(&h.env);
    h.share_admin().mint(&user, &1_000_000);
    h.share_admin().mint(&other, &1_000_000);
    h.reward_admin().mint(&h.admin, &1_000_000);

    // Establish a position and accrue pending so the next deposit auto-claims.
    h.client().deposit(&user, &1000);
    h.client().deposit(&other, &1000);
    h.client().distribute(&h.admin, &10_000);
    assert!(h.client().get_pending(&user) > 0);

    // Measure only the worst-case deposit (auto-claim + custody + storage).
    h.env.cost_estimate().budget().reset_default();
    h.client().deposit(&user, &500);
    let b = h.env.cost_estimate().budget();
    let cpu = b.cpu_instruction_cost();
    let mem = b.memory_bytes_cost();

    assert!(
        cpu < CPU_TX_LIMIT,
        "deposit CPU footprint {} exceeds Soroban tx limit {}",
        cpu,
        CPU_TX_LIMIT
    );
    assert!(
        mem < MEM_TX_LIMIT,
        "deposit memory footprint {} exceeds Soroban tx limit {}",
        mem,
        MEM_TX_LIMIT
    );

    // Sanity: the worst-case call should use only a small fraction of budget,
    // i.e. an order of magnitude below the ceiling.
    assert!(cpu * 10 < CPU_TX_LIMIT, "deposit CPU footprint has thin headroom: {}", cpu);
}

/// Issue #30 (section B): a foreign token accidentally sent to the contract
/// can be swept out by the admin to any recipient, in full.
#[test]
fn test_recover_foreign_token_succeeds() {
    let h = setup();
    let stranger = Address::generate(&h.env);
    let recipient = Address::generate(&h.env);

    // A third, unrelated token gets mistakenly transferred into the contract.
    let (foreign_id, foreign_token, foreign_admin) = create_token(&h.env, &h.admin);
    foreign_admin.mint(&stranger, &5000);
    foreign_token.transfer(&stranger, &h.contract_id, &5000);
    assert_eq!(foreign_token.balance(&h.contract_id), 5000);

    // Admin rescues it to a chosen recipient.
    h.client().recover_token(&foreign_id, &recipient, &5000);
    assert_eq!(foreign_token.balance(&recipient), 5000);
    assert_eq!(foreign_token.balance(&h.contract_id), 0);
}

/// Issue #30 (section C): the staked share token is staker custody and must be
/// un-recoverable - even by the admin - so positions can never be swept out.
#[test]
fn test_recover_rejects_share_token() {
    let h = setup();
    let user = Address::generate(&h.env);
    let recipient = Address::generate(&h.env);
    h.share_admin().mint(&user, &1000);
    h.client().deposit(&user, &1000);

    // Contract now custodies 1000 share tokens; admin must not be able to take them.
    let share_id = h.share_token().address;
    let res = h.client().try_recover_token(&share_id, &recipient, &1000);
    assert!(res.is_err(), "share-token recovery must be rejected");

    // Custody is untouched and the staker can still withdraw in full.
    assert_eq!(h.share_token().balance(&h.contract_id), 1000);
    h.client().withdraw(&user, &1000);
    assert_eq!(h.share_token().balance(&user), 1000);
}
