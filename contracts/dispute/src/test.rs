#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    token, Address, Env, String,
};

fn setup_test(env: &Env) -> (DisputeContractClient, Address, Address, Address, Address) {
    let contract_id = env.register_contract(None, DisputeContract);
    let client = DisputeContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &80);

    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(token_admin.clone()).address();

    let escrow_contract_id = env.register_contract(None, DummyEscrow);

    (client, admin, token_id, token_admin, escrow_contract_id)
}

#[contract]
pub struct DummyEscrow;

#[contractimpl]
impl DummyEscrow {
    pub fn resolve_dispute_callback(_env: Env, _job_id: u64, _resolved_for_client: bool) {}
}

fn setup_env() -> (Env, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, token_id, _token_admin, escrow_id) = setup_test(&env);
    let contract_id = client.address.clone();
    (env, contract_id, token_id, admin, escrow_id)
}

fn create_token(env: &Env) -> (Address, token::StellarAssetClient<'_>) {
    let admin = Address::generate(env);
    #[allow(deprecated)]
    let token_address = env.register_stellar_asset_contract(admin.clone());
    let token_admin = token::StellarAssetClient::new(env, &token_address);
    (token_address, token_admin)
}

// ---- Core dispute tests ----

#[test]
fn test_raise_dispute() {
    let (env, contract_id, token_id, _admin, _escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let token_asset_client = token::StellarAssetClient::new(&env, &token_id);
    token_asset_client.mint(&user_client, &1000);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Work not delivered"),
        &3u32,
        &100i128,
        &token_id,
        &0i128,
    );

    assert_eq!(dispute_id, 1);

    let dispute = client.get_dispute(&dispute_id);
    assert_eq!(dispute.job_id, 1);
    assert_eq!(dispute.status, DisputeStatus::Open);
    assert_eq!(dispute.min_votes, 3);
    assert_eq!(dispute.appeal_count, 0);
    assert_eq!(dispute.max_appeals, 2);
    assert_eq!(dispute.dispute_fee, 100);
    assert_eq!(dispute.malicious, false);

    // Verify fee was transferred to contract
    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&user_client), 900);
    assert_eq!(token_client.balance(&client.address), 100);
}

#[test]
fn test_vote_and_resolve() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let token_asset_client = token::StellarAssetClient::new(&env, &token_id);
    token_asset_client.mint(&freelancer, &1000);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &freelancer,
        &String::from_str(&env, "Payment not released"),
        &3u32,
        &100i128,
        &token_id,
        &0i128,
    );

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    client.cast_vote(
        &dispute_id,
        &voter1,
        &VoteChoice::Freelancer,
        &String::from_str(&env, "Work was done"),
    );
    client.cast_vote(
        &dispute_id,
        &voter2,
        &VoteChoice::Freelancer,
        &String::from_str(&env, "Agree with freelancer"),
    );
    client.cast_vote(
        &dispute_id,
        &voter3,
        &VoteChoice::Client,
        &String::from_str(&env, "Incomplete work"),
    );

    let result = client.resolve_dispute(&dispute_id, &escrow_id, &false);
    assert_eq!(result, DisputeStatus::ResolvedForFreelancer);

    // Fee should still be held in contract for voter rewards
    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&client.address), 100);
}

#[test]
fn test_malicious_dispute_penalty() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);
    let token = token::Client::new(&env, &token_id);
    let token_asset_client = token::StellarAssetClient::new(&env, &token_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let penalty_amount = 1000i128;

    // Mint tokens to freelancer (initiator)
    token_asset_client.mint(&freelancer, &penalty_amount);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &freelancer,
        &String::from_str(&env, "Malicious dispute"),
        &3u32,
        &0i128,
        &token_id,
        &penalty_amount,
    );

    assert_eq!(token.balance(&freelancer), 0);
    assert_eq!(token.balance(&client.address), penalty_amount);

    // 3 voters: all for client (100% against freelancer)
    for _ in 0..3 {
        client.cast_vote(
            &dispute_id,
            &Address::generate(&env),
            &VoteChoice::Client,
            &String::from_str(&env, "Frivolous"),
        );
    }

    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    assert!(client.is_malicious_dispute(&dispute_id));

    // Penalty should go to the winner (client)
    assert_eq!(token.balance(&user_client), penalty_amount);
    assert_eq!(token.balance(&freelancer), 0);
}

#[test]
#[should_panic]
fn test_resolve_without_enough_votes() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
        &0i128,
        &token_id,
        &0i128,
    );

    let voter = Address::generate(&env);
    client.cast_vote(
        &dispute_id,
        &voter,
        &VoteChoice::Client,
        &String::from_str(&env, "Reason"),
    );

    client.resolve_dispute(&dispute_id, &escrow_id, &false);
}

// ---- Voter reward tests ----

#[test]
fn test_claim_voter_reward_proportional() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let token_asset_client = token::StellarAssetClient::new(&env, &token_id);
    token_asset_client.mint(&user_client, &1000);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Bad work"),
        &3u32,
        &100i128,
        &token_id,
        &0i128,
    );

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    // 2 vote for client, 1 for freelancer
    client.cast_vote(
        &dispute_id,
        &voter1,
        &VoteChoice::Client,
        &String::from_str(&env, "r1"),
    );
    client.cast_vote(
        &dispute_id,
        &voter2,
        &VoteChoice::Client,
        &String::from_str(&env, "r2"),
    );
    client.cast_vote(
        &dispute_id,
        &voter3,
        &VoteChoice::Freelancer,
        &String::from_str(&env, "r3"),
    );

    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // Each winning voter (voter1, voter2) should get 100/2 = 50
    let reward1 = client.claim_voter_reward(&dispute_id, &voter1);
    assert_eq!(reward1, 50);

    let reward2 = client.claim_voter_reward(&dispute_id, &voter2);
    assert_eq!(reward2, 50);

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&voter1), 50);
    assert_eq!(token_client.balance(&voter2), 50);
    assert_eq!(token_client.balance(&client.address), 0);
}

#[test]
fn test_get_claimable_reward() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let token_asset_client = token::StellarAssetClient::new(&env, &token_id);
    token_asset_client.mint(&user_client, &1000);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Bad work"),
        &3u32,
        &90i128,
        &token_id,
        &0i128,
    );

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);
    let non_voter = Address::generate(&env);

    client.cast_vote(
        &dispute_id,
        &voter1,
        &VoteChoice::Client,
        &String::from_str(&env, "r"),
    );
    client.cast_vote(
        &dispute_id,
        &voter2,
        &VoteChoice::Client,
        &String::from_str(&env, "r"),
    );
    client.cast_vote(
        &dispute_id,
        &voter3,
        &VoteChoice::Freelancer,
        &String::from_str(&env, "r"),
    );

    // Before resolution, should return 0
    assert_eq!(client.get_claimable_reward(&dispute_id, &voter1), 0);

    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // Winning voter gets 90/2 = 45
    assert_eq!(client.get_claimable_reward(&dispute_id, &voter1), 45);
    assert_eq!(client.get_claimable_reward(&dispute_id, &voter2), 45);
    // Losing voter gets 0
    assert_eq!(client.get_claimable_reward(&dispute_id, &voter3), 0);
    // Non-voter gets 0
    assert_eq!(client.get_claimable_reward(&dispute_id, &non_voter), 0);

    // After claiming, should return 0
    client.claim_voter_reward(&dispute_id, &voter1);
    assert_eq!(client.get_claimable_reward(&dispute_id, &voter1), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_double_claim_prevented() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let token_asset_client = token::StellarAssetClient::new(&env, &token_id);
    token_asset_client.mint(&user_client, &1000);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Bad work"),
        &3u32,
        &90i128,
        &token_id,
        &0i128,
    );

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Client, &String::from_str(&env, "r"));
    client.cast_vote(&dispute_id, &voter2, &VoteChoice::Client, &String::from_str(&env, "r"));
    client.cast_vote(&dispute_id, &voter3, &VoteChoice::Freelancer, &String::from_str(&env, "r"));

    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    client.claim_voter_reward(&dispute_id, &voter1); // First claim OK
    client.claim_voter_reward(&dispute_id, &voter1); // Double claim panics
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_losing_voter_cannot_claim() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let token_asset_client = token::StellarAssetClient::new(&env, &token_id);
    token_asset_client.mint(&user_client, &1000);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Bad work"),
        &3u32,
        &90i128,
        &token_id,
        &0i128,
    );

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Client, &String::from_str(&env, "r"));
    client.cast_vote(&dispute_id, &voter2, &VoteChoice::Client, &String::from_str(&env, "r"));
    client.cast_vote(&dispute_id, &voter3, &VoteChoice::Freelancer, &String::from_str(&env, "r"));

    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // voter3 voted for freelancer but client won — should fail
    client.claim_voter_reward(&dispute_id, &voter3);
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_claim_before_resolution_fails() {
    let (env, contract_id, token_id, _admin, _escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let token_asset_client = token::StellarAssetClient::new(&env, &token_id);
    token_asset_client.mint(&user_client, &1000);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Bad work"),
        &3u32,
        &90i128,
        &token_id,
        &0i128,
    );

    let voter1 = Address::generate(&env);
    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Client, &String::from_str(&env, "r"));

    // Try to claim before resolution
    client.claim_voter_reward(&dispute_id, &voter1);
}

#[test]
fn test_malicious_dispute_refunds_winning_party() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let token_asset_client = token::StellarAssetClient::new(&env, &token_id);
    token_asset_client.mint(&user_client, &1000);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Malicious claim"),
        &3u32,
        &100i128,
        &token_id,
        &0i128,
    );

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    // Freelancer wins (dispute was malicious)
    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Freelancer, &String::from_str(&env, "r"));
    client.cast_vote(&dispute_id, &voter2, &VoteChoice::Freelancer, &String::from_str(&env, "r"));
    client.cast_vote(&dispute_id, &voter3, &VoteChoice::Client, &String::from_str(&env, "r"));

    let result = client.resolve_dispute(&dispute_id, &escrow_id, &true);
    assert_eq!(result, DisputeStatus::ResolvedForFreelancer);

    // Fee refunded to freelancer (winning/victim party)
    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&freelancer), 100);
    assert_eq!(token_client.balance(&client.address), 0);

    // Dispute marked as malicious
    let dispute = client.get_dispute(&dispute_id);
    assert_eq!(dispute.malicious, true);
}

#[test]
#[should_panic(expected = "Error(Contract, #15)")]
fn test_claim_reward_on_malicious_dispute_fails() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let token_asset_client = token::StellarAssetClient::new(&env, &token_id);
    token_asset_client.mint(&user_client, &1000);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Malicious"),
        &3u32,
        &100i128,
        &token_id,
        &0i128,
    );

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Freelancer, &String::from_str(&env, "r"));
    client.cast_vote(&dispute_id, &voter2, &VoteChoice::Freelancer, &String::from_str(&env, "r"));
    client.cast_vote(&dispute_id, &voter3, &VoteChoice::Client, &String::from_str(&env, "r"));

    client.resolve_dispute(&dispute_id, &escrow_id, &true);

    // Winning voter tries to claim on malicious dispute — no reward available
    client.claim_voter_reward(&dispute_id, &voter1);
}

#[test]
fn test_zero_fee_dispute() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "No fee dispute"),
        &3u32,
        &0i128,
        &token_id,
        &0i128,
    );

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Client, &String::from_str(&env, "r"));
    client.cast_vote(&dispute_id, &voter2, &VoteChoice::Client, &String::from_str(&env, "r"));
    client.cast_vote(&dispute_id, &voter3, &VoteChoice::Freelancer, &String::from_str(&env, "r"));

    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // Claimable reward should be 0
    assert_eq!(client.get_claimable_reward(&dispute_id, &voter1), 0);
}

#[test]
fn test_single_winning_voter_gets_full_fee() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let token_asset_client = token::StellarAssetClient::new(&env, &token_id);
    token_asset_client.mint(&user_client, &1000);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
        &100i128,
        &token_id,
        &0i128,
    );

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    // All 3 vote for client -> client wins, all 3 share equally
    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Client, &String::from_str(&env, "r"));
    client.cast_vote(&dispute_id, &voter2, &VoteChoice::Client, &String::from_str(&env, "r"));
    client.cast_vote(&dispute_id, &voter3, &VoteChoice::Client, &String::from_str(&env, "r"));

    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // All 3 are winners: 100/3 = 33 each (integer division)
    let reward = client.claim_voter_reward(&dispute_id, &voter1);
    assert_eq!(reward, 33);

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&voter1), 33);
}

// ---- Appeal system tests ----

#[test]
fn test_appeal_by_losing_party() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Quality issue"),
        &3u32,
        &0i128,
        &token_id,
        &0i128,
    );

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Freelancer, &String::from_str(&env, "Good work"));
    client.cast_vote(&dispute_id, &voter2, &VoteChoice::Freelancer, &String::from_str(&env, "Agree"));
    client.cast_vote(&dispute_id, &voter3, &VoteChoice::Client, &String::from_str(&env, "Disagree"));

    let result = client.resolve_dispute(&dispute_id, &escrow_id, &false);
    assert_eq!(result, DisputeStatus::ResolvedForFreelancer);

    // Client (losing party) appeals
    client.raise_appeal(&dispute_id, &user_client);

    let dispute = client.get_dispute(&dispute_id);
    assert_eq!(dispute.status, DisputeStatus::Appealed);
    assert_eq!(dispute.appeal_count, 1);
    assert_eq!(dispute.votes_for_client, 0);
    assert_eq!(dispute.votes_for_freelancer, 0);
}

#[test]
fn test_appeal_requires_double_votes() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
        &0i128,
        &token_id,
        &0i128,
    );

    // First round: 3 votes needed
    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Freelancer, &String::from_str(&env, "Vote 1"));
    client.cast_vote(&dispute_id, &voter2, &VoteChoice::Freelancer, &String::from_str(&env, "Vote 2"));
    client.cast_vote(&dispute_id, &voter3, &VoteChoice::Client, &String::from_str(&env, "Vote 3"));

    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // Client appeals
    client.raise_appeal(&dispute_id, &user_client);

    // Second round: 6 votes needed (3 * 2^1)
    let voter4 = Address::generate(&env);
    let voter5 = Address::generate(&env);
    let voter6 = Address::generate(&env);
    let voter7 = Address::generate(&env);
    let voter8 = Address::generate(&env);
    let voter9 = Address::generate(&env);

    client.cast_vote(&dispute_id, &voter4, &VoteChoice::Client, &String::from_str(&env, "Appeal vote 1"));
    client.cast_vote(&dispute_id, &voter5, &VoteChoice::Client, &String::from_str(&env, "Appeal vote 2"));
    client.cast_vote(&dispute_id, &voter6, &VoteChoice::Client, &String::from_str(&env, "Appeal vote 3"));
    client.cast_vote(&dispute_id, &voter7, &VoteChoice::Client, &String::from_str(&env, "Appeal vote 4"));
    client.cast_vote(&dispute_id, &voter8, &VoteChoice::Freelancer, &String::from_str(&env, "Appeal vote 5"));
    client.cast_vote(&dispute_id, &voter9, &VoteChoice::Freelancer, &String::from_str(&env, "Appeal vote 6"));

    // Should succeed with 6 votes
    let result = client.resolve_dispute(&dispute_id, &escrow_id, &false);
    assert_eq!(result, DisputeStatus::ResolvedForClient);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_appeal_by_winning_party_fails() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
        &0i128,
        &token_id,
        &0i128,
    );

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Freelancer, &String::from_str(&env, "Vote 1"));
    client.cast_vote(&dispute_id, &voter2, &VoteChoice::Freelancer, &String::from_str(&env, "Vote 2"));
    client.cast_vote(&dispute_id, &voter3, &VoteChoice::Client, &String::from_str(&env, "Vote 3"));

    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // Freelancer (winning party) tries to appeal - should fail
    client.raise_appeal(&dispute_id, &freelancer);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_max_appeals_reached() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
        &0i128,
        &token_id,
        &0i128,
    );

    // First resolution
    for _i in 0..3 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Freelancer, &String::from_str(&env, "V1"));
    }
    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // First appeal
    client.raise_appeal(&dispute_id, &user_client);
    for _i in 0..6 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Freelancer, &String::from_str(&env, "Vote"));
    }
    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // Second appeal
    client.raise_appeal(&dispute_id, &user_client);
    for _i in 0..12 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Freelancer, &String::from_str(&env, "Vote"));
    }
    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // Third appeal should fail (max_appeals = 2)
    client.raise_appeal(&dispute_id, &user_client);
}

#[test]
fn test_final_resolution_after_max_appeals() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
        &0i128,
        &token_id,
        &0i128,
    );

    // First resolution
    for _i in 0..3 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Freelancer, &String::from_str(&env, "Vote"));
    }
    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // First appeal
    client.raise_appeal(&dispute_id, &user_client);
    for _i in 0..6 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Freelancer, &String::from_str(&env, "Vote"));
    }
    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // Second appeal (last one allowed)
    client.raise_appeal(&dispute_id, &user_client);
    for _i in 0..12 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Client, &String::from_str(&env, "Vote"));
    }

    // This should be final resolution
    let result = client.resolve_dispute(&dispute_id, &escrow_id, &false);
    assert_eq!(result, DisputeStatus::FinalResolution);

    let dispute = client.get_dispute(&dispute_id);
    assert_eq!(dispute.appeal_count, 2);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_appeal_before_resolution_fails() {
    let (env, contract_id, token_id, _admin, _escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
        &0i128,
        &token_id,
        &0i128,
    );

    // Try to appeal before any resolution - should fail
    client.raise_appeal(&dispute_id, &user_client);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_appeal_after_deadline_fails() {
    let (env, contract_id, token_id, _admin, escrow_id) = setup_env();
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
        &0i128,
        &token_id,
        &0i128,
    );

    // Vote and resolve
    for _i in 0..3 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Freelancer, &String::from_str(&env, "Vote"));
    }
    client.resolve_dispute(&dispute_id, &escrow_id, &false);

    // Jump past appeal deadline (100 ledgers)
    env.ledger().with_mut(|li| {
        li.sequence_number += 101;
    });

    // Try to appeal after deadline - should fail
    client.raise_appeal(&dispute_id, &user_client);
}
