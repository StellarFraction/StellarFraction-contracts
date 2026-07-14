#[cfg(test)]
use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token, Address, Env,
};

#[test]
fn test_full_dividend_distribution_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    // 1. Register Mock Share and Reward Tokens (using built-in token simulator in testutils)
    let share_token_id = env.register_token_contract(&admin);
    let share_token = token::Client::new(&env, &share_token_id);
    
    let reward_token_id = env.register_token_contract(&admin);
    let reward_token = token::Client::new(&env, &reward_token_id);

    // 2. Register the Distribution Contract
    let contract_id = env.register_contract(None, DistributionContract);
    let client = DistributionContractClient::new(&env, &contract_id);

    // 3. Initialize Contract
    client.initialize(&admin, &share_token_id, &reward_token_id).unwrap();

    // 4. Mint tokens to stakers and admin
    share_token.mint(&user_a, &1000); // User A has 1000 deed tokens
    share_token.mint(&user_b, &3000); // User B has 3000 deed tokens
    reward_token.mint(&admin, &50000); // Admin has 50,000 USDC (reward tokens)

    // 5. Deposit Share Tokens (Staking)
    client.deposit(&user_a, &1000).unwrap();
    client.deposit(&user_b, &3000).unwrap();

    // Verify deposits
    assert_eq!(client.get_shares(&user_a), 1000);
    assert_eq!(client.get_shares(&user_b), 3000);
    assert_eq!(share_token.balance(&contract_id), 4000);
    assert_eq!(share_token.balance(&user_a), 0);
    assert_eq!(share_token.balance(&user_b), 0);

    // 6. First distribution of rewards (Admin distributes 4000 USDC)
    client.distribute(&admin, &4000).unwrap();

    // Verify pending rewards:
    // Total Shares = 4000
    // AccRewardPerShare = 4000 * 1e12 / 4000 = 1e12
    // User A pending: 1000 * 1e12 / 1e12 - 0 = 1000 USDC
    // User B pending: 3000 * 1e12 / 1e12 - 0 = 3000 USDC
    assert_eq!(client.get_pending(&user_a), 1000);
    assert_eq!(client.get_pending(&user_b), 3000);

    // 7. User A claims rewards
    let claimed_a = client.claim(&user_a).unwrap();
    assert_eq!(claimed_a, 1000);
    assert_eq!(reward_token.balance(&user_a), 1000);
    assert_eq!(client.get_pending(&user_a), 0);

    // 8. Second distribution (Admin distributes another 8000 USDC)
    client.distribute(&admin, &8000).unwrap();

    // Total Shares = 4000
    // AccRewardPerShare increases by 8000 * 1e12 / 4000 = 2e12.
    // Total AccRewardPerShare = 3e12
    // User A pending: 1000 * 3e12 / 1e12 - Debt(1000) = 3000 - 1000 = 2000 USDC
    // User B pending: 3000 * 3e12 / 1e12 - Debt(0) = 9000 USDC
    assert_eq!(client.get_pending(&user_a), 2000);
    assert_eq!(client.get_pending(&user_b), 9000);

    // 9. User B withdraws 1500 shares (unstakes half)
    // This should auto-claim their pending 9000 USDC and set shares to 1500
    client.withdraw(&user_b, &1500).unwrap();
    
    assert_eq!(client.get_shares(&user_b), 1500);
    assert_eq!(reward_token.balance(&user_b), 9000);
    assert_eq!(share_token.balance(&user_b), 1500);
    assert_eq!(client.get_pending(&user_b), 0);
    assert_eq!(share_token.balance(&contract_id), 2500);

    // 10. Third distribution (Admin distributes 5000 USDC)
    // Total Shares = 2500
    // AccRewardPerShare increases by 5000 * 1e12 / 2500 = 2e12
    // Total AccRewardPerShare = 5e12
    client.distribute(&admin, &5000).unwrap();

    // User A pending: shares = 1000, debt = 1000 (after first claim, wait, no, after first claim,
    // their debt was set to shares * acc_reward_per_share = 1000 * 1e12 / 1e12 = 1000.
    // When they did NOT interact, their debt remained 1000.
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
fn test_errors_and_boundaries() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let share_token_id = env.register_token_contract(&admin);
    let reward_token_id = env.register_token_contract(&admin);

    let contract_id = env.register_contract(None, DistributionContract);
    let client = DistributionContractClient::new(&env, &contract_id);

    // Try to deposit before initialization
    let err_pre_init = client.try_deposit(&user, &100);
    assert!(err_pre_init.is_err());

    // Initialize
    client.initialize(&admin, &share_token_id, &reward_token_id).unwrap();

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
