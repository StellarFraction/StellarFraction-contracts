#[cfg(test)]
use super::*;
use soroban_sdk::{testutils::Address as _, token, vec, Address, Env};

fn register_token<'a>(env: &'a Env, admin: &Address) -> (Address, token::StellarAssetClient<'a>) {
    let contract = env.register_stellar_asset_contract_v2(admin.clone());
    let address = contract.address();
    let client = token::StellarAssetClient::new(env, &address);
    (address, client)
}

#[test]
fn test_full_dividend_distribution_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    // 1. Register Mock Share and Reward Tokens (using built-in token simulator in testutils)
    let (share_token_id, share_token_admin) = register_token(&env, &admin);
    let share_token = token::Client::new(&env, &share_token_id);

    let (reward_token_id, reward_token_admin) = register_token(&env, &admin);
    let reward_token = token::Client::new(&env, &reward_token_id);

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

    let (share_token_id, _) = register_token(&env, &admin);
    let (reward_token_id, _) = register_token(&env, &admin);

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

#[test]
fn test_deposit_after_distribution_does_not_receive_historical_rewards() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let early_user = Address::generate(&env);
    let late_user = Address::generate(&env);
    let (share_token_id, share_token_admin) = register_token(&env, &admin);
    let (reward_token_id, reward_token_admin) = register_token(&env, &admin);
    let contract_id = env.register(DistributionContract, ());
    let client = DistributionContractClient::new(&env, &contract_id);

    client.initialize(&admin, &share_token_id, &reward_token_id);
    share_token_admin.mint(&early_user, &100);
    share_token_admin.mint(&late_user, &100);
    reward_token_admin.mint(&admin, &200);

    client.deposit(&early_user, &100);
    client.distribute(&admin, &100);
    client.deposit(&late_user, &100);

    assert_eq!(client.get_pending(&early_user), 100);
    assert_eq!(client.get_pending(&late_user), 0);

    client.distribute(&admin, &100);
    assert_eq!(client.get_pending(&early_user), 150);
    assert_eq!(client.get_pending(&late_user), 50);
}

#[test]
fn test_multi_property_pool_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let manager = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);
    let (legacy_share, _) = register_token(&env, &admin);
    let (legacy_reward, _) = register_token(&env, &admin);
    let (share_token, share_admin) = register_token(&env, &admin);
    let (reward_token, reward_admin) = register_token(&env, &admin);
    let share_client = token::Client::new(&env, &share_token);
    let reward_client = token::Client::new(&env, &reward_token);
    let contract_id = env.register(DistributionContract, ());
    let client = DistributionContractClient::new(&env, &contract_id);

    client.initialize(&admin, &legacy_share, &legacy_reward);
    let pool_id = client.create_pool(&manager, &share_token, &reward_token);
    assert_eq!(pool_id, 1);
    assert_eq!(client.get_pool_count(), 2);
    let new_manager = Address::generate(&env);
    client.set_pool_manager(&pool_id, &new_manager);
    assert_eq!(client.get_pool(&pool_id).manager, new_manager);

    client.set_pool_paused(&pool_id, &true);
    assert!(client.try_deposit_into(&pool_id, &user_a, &100).is_err());
    client.set_pool_paused(&pool_id, &false);

    share_admin.mint(&user_a, &100);
    share_admin.mint(&user_b, &300);
    reward_admin.mint(&admin, &400);
    client.deposit_into(&pool_id, &user_a, &100);
    client.deposit_into(&pool_id, &user_b, &300);
    client.distribute_to(&pool_id, &admin, &400);

    assert_eq!(client.get_pool_pending(&pool_id, &user_a), 100);
    assert_eq!(client.get_pool_pending(&pool_id, &user_b), 300);
    assert_eq!(client.claim_from(&pool_id, &user_a), 100);
    assert_eq!(reward_client.balance(&user_a), 100);

    client.withdraw_from(&pool_id, &user_b, &100);
    assert_eq!(reward_client.balance(&user_b), 300);
    assert_eq!(share_client.balance(&user_b), 100);
    assert_eq!(client.get_position(&pool_id, &user_b).shares, 200);
    assert_eq!(client.get_pool(&pool_id).total_shares, 300);
}

#[test]
fn test_property_pool_accounting_is_isolated() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let (share_a, share_a_admin) = register_token(&env, &admin);
    let (reward_a, reward_a_admin) = register_token(&env, &admin);
    let (share_b, share_b_admin) = register_token(&env, &admin);
    let (reward_b, reward_b_admin) = register_token(&env, &admin);
    let contract_id = env.register(DistributionContract, ());
    let client = DistributionContractClient::new(&env, &contract_id);

    client.initialize(&admin, &share_a, &reward_a);
    let pool_a = 0;
    let pool_b = client.create_pool(&admin, &share_b, &reward_b);
    share_a_admin.mint(&user, &100);
    share_b_admin.mint(&user, &400);
    reward_a_admin.mint(&admin, &50);
    reward_b_admin.mint(&admin, &800);

    client.deposit_into(&pool_a, &user, &100);
    client.deposit_into(&pool_b, &user, &400);
    client.distribute_to(&pool_a, &admin, &50);
    assert_eq!(client.get_pool_pending(&pool_a, &user), 50);
    assert_eq!(client.get_pool_pending(&pool_b, &user), 0);

    client.distribute_to(&pool_b, &admin, &800);
    assert_eq!(client.get_pool_pending(&pool_a, &user), 50);
    assert_eq!(client.get_pool_pending(&pool_b, &user), 800);
    assert_eq!(client.get_pool(&pool_a).total_shares, 100);
    assert_eq!(client.get_pool(&pool_b).total_shares, 400);

    let claimed = client.claim_many(&user, &vec![&env, pool_a, pool_b]);
    assert_eq!(claimed, 850);
    assert_eq!(token::Client::new(&env, &reward_a).balance(&user), 50);
    assert_eq!(token::Client::new(&env, &reward_b).balance(&user), 800);
}
