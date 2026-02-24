#![cfg(test)]

use super::*;
use soroban_sdk::{contract, contractimpl, testutils::{Address as _, Ledger}, Env, String};

#[contract]
pub struct DummyEscrow;

#[contractimpl]
impl DummyEscrow {
    pub fn resolve_dispute_callback(_env: Env, _job_id: u64, _resolved_for_client: bool) {}
}

#[test]
fn test_raise_dispute() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, DisputeContract);
    let client = DisputeContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Work not delivered"),
        &3u32,
    );

    assert_eq!(dispute_id, 1);

    let dispute = client.get_dispute(&dispute_id);
    assert_eq!(dispute.job_id, 1);
    assert_eq!(dispute.status, DisputeStatus::Open);
    assert_eq!(dispute.min_votes, 3);
    assert_eq!(dispute.appeal_count, 0);
    assert_eq!(dispute.max_appeals, 2);
}

#[test]
fn test_vote_and_resolve() {
    let env = Env::default();
    env.mock_all_auths();

    let dispute_contract_id = env.register_contract(None, DisputeContract);
    let client = DisputeContractClient::new(&env, &dispute_contract_id);

    let escrow_contract_id = env.register_contract(None, DummyEscrow);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &freelancer,
        &String::from_str(&env, "Payment not released"),
        &3u32,
    );

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

    let result = client.resolve_dispute(&dispute_id, &escrow_contract_id);
    assert_eq!(result, DisputeStatus::ResolvedForFreelancer);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_resolve_without_enough_votes() {
    let env = Env::default();
    env.mock_all_auths();

    let dispute_contract_id = env.register_contract(None, DisputeContract);
    let client = DisputeContractClient::new(&env, &dispute_contract_id);

    let escrow_contract_id = env.register_contract(None, DummyEscrow);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
    );

    let voter = Address::generate(&env);
    client.cast_vote(
        &dispute_id,
        &voter,
        &VoteChoice::Client,
        &String::from_str(&env, "Reason"),
    );

    client.resolve_dispute(&dispute_id, &escrow_contract_id);
}

#[test]
fn test_appeal_by_losing_party() {
    let env = Env::default();
    env.mock_all_auths();

    let dispute_contract_id = env.register_contract(None, DisputeContract);
    let client = DisputeContractClient::new(&env, &dispute_contract_id);

    let escrow_contract_id = env.register_contract(None, DummyEscrow);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    // Raise dispute
    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Quality issue"),
        &3u32,
    );

    // Vote in favor of freelancer
    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Freelancer, &String::from_str(&env, "Good work"));
    client.cast_vote(&dispute_id, &voter2, &VoteChoice::Freelancer, &String::from_str(&env, "Agree"));
    client.cast_vote(&dispute_id, &voter3, &VoteChoice::Client, &String::from_str(&env, "Disagree"));

    // Resolve - client loses
    let result = client.resolve_dispute(&dispute_id, &escrow_contract_id);
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
    let env = Env::default();
    env.mock_all_auths();

    let dispute_contract_id = env.register_contract(None, DisputeContract);
    let client = DisputeContractClient::new(&env, &dispute_contract_id);

    let escrow_contract_id = env.register_contract(None, DummyEscrow);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    // Raise dispute with min_votes = 3
    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
    );

    // First round: 3 votes needed
    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Freelancer, &String::from_str(&env, "Vote 1"));
    client.cast_vote(&dispute_id, &voter2, &VoteChoice::Freelancer, &String::from_str(&env, "Vote 2"));
    client.cast_vote(&dispute_id, &voter3, &VoteChoice::Client, &String::from_str(&env, "Vote 3"));

    client.resolve_dispute(&dispute_id, &escrow_contract_id);

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
    let result = client.resolve_dispute(&dispute_id, &escrow_contract_id);
    assert_eq!(result, DisputeStatus::ResolvedForClient);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_appeal_by_winning_party_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let dispute_contract_id = env.register_contract(None, DisputeContract);
    let client = DisputeContractClient::new(&env, &dispute_contract_id);

    let escrow_contract_id = env.register_contract(None, DummyEscrow);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
    );

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);

    client.cast_vote(&dispute_id, &voter1, &VoteChoice::Freelancer, &String::from_str(&env, "Vote 1"));
    client.cast_vote(&dispute_id, &voter2, &VoteChoice::Freelancer, &String::from_str(&env, "Vote 2"));
    client.cast_vote(&dispute_id, &voter3, &VoteChoice::Client, &String::from_str(&env, "Vote 3"));

    client.resolve_dispute(&dispute_id, &escrow_contract_id);

    // Freelancer (winning party) tries to appeal - should fail
    client.raise_appeal(&dispute_id, &freelancer);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_max_appeals_reached() {
    let env = Env::default();
    env.mock_all_auths();

    let dispute_contract_id = env.register_contract(None, DisputeContract);
    let client = DisputeContractClient::new(&env, &dispute_contract_id);

    let escrow_contract_id = env.register_contract(None, DummyEscrow);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
    );

    // First resolution
    for _i in 0..3 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Freelancer, &String::from_str(&env, "V1"));
    }
    client.resolve_dispute(&dispute_id, &escrow_contract_id);

    // First appeal
    client.raise_appeal(&dispute_id, &user_client);
    for _i in 0..6 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Freelancer, &String::from_str(&env, "Vote"));
    }
    client.resolve_dispute(&dispute_id, &escrow_contract_id);

    // Second appeal
    client.raise_appeal(&dispute_id, &user_client);
    for _i in 0..12 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Freelancer, &String::from_str(&env, "Vote"));
    }
    client.resolve_dispute(&dispute_id, &escrow_contract_id);

    // Third appeal should fail (max_appeals = 2)
    client.raise_appeal(&dispute_id, &user_client);
}

#[test]
fn test_final_resolution_after_max_appeals() {
    let env = Env::default();
    env.mock_all_auths();

    let dispute_contract_id = env.register_contract(None, DisputeContract);
    let client = DisputeContractClient::new(&env, &dispute_contract_id);

    let escrow_contract_id = env.register_contract(None, DummyEscrow);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
    );

    // First resolution
    for _i in 0..3 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Freelancer, &String::from_str(&env, "Vote"));
    }
    client.resolve_dispute(&dispute_id, &escrow_contract_id);

    // First appeal
    client.raise_appeal(&dispute_id, &user_client);
    for _i in 0..6 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Freelancer, &String::from_str(&env, "Vote"));
    }
    client.resolve_dispute(&dispute_id, &escrow_contract_id);

    // Second appeal (last one allowed)
    client.raise_appeal(&dispute_id, &user_client);
    for _i in 0..12 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Client, &String::from_str(&env, "Vote"));
    }
    
    // This should be final resolution
    let result = client.resolve_dispute(&dispute_id, &escrow_contract_id);
    assert_eq!(result, DisputeStatus::FinalResolution);

    let dispute = client.get_dispute(&dispute_id);
    assert_eq!(dispute.appeal_count, 2);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_appeal_before_resolution_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let dispute_contract_id = env.register_contract(None, DisputeContract);
    let client = DisputeContractClient::new(&env, &dispute_contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
    );

    // Try to appeal before any resolution - should fail
    client.raise_appeal(&dispute_id, &user_client);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_appeal_after_deadline_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let dispute_contract_id = env.register_contract(None, DisputeContract);
    let client = DisputeContractClient::new(&env, &dispute_contract_id);

    let escrow_contract_id = env.register_contract(None, DummyEscrow);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    let dispute_id = client.raise_dispute(
        &1u64,
        &user_client,
        &freelancer,
        &user_client,
        &String::from_str(&env, "Issue"),
        &3u32,
    );

    // Vote and resolve
    for _i in 0..3 {
        let voter = Address::generate(&env);
        client.cast_vote(&dispute_id, &voter, &VoteChoice::Freelancer, &String::from_str(&env, "Vote"));
    }
    client.resolve_dispute(&dispute_id, &escrow_contract_id);

    // Jump past appeal deadline (100 ledgers)
    env.ledger().with_mut(|li| {
        li.sequence_number += 101;
    });

    // Try to appeal after deadline - should fail
    client.raise_appeal(&dispute_id, &user_client);
}
