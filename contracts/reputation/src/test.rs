#![cfg(test)]

use super::*;
use stellar_market_escrow::{EscrowContract, Job, JobStatus};
use soroban_sdk::{testutils::Address as _, symbol_short, Env, String, Vec};

/// Helper: directly write a completed job into the escrow contract's storage,
/// bypassing the full escrow flow (token transfers, milestone approvals, etc.).
fn setup_completed_job(
    env: &Env,
    escrow_id: &Address,
    job_id: u64,
    client: &Address,
    freelancer: &Address,
) {
    let token = Address::generate(env);
    let job = Job {
        id: job_id,
        client: client.clone(),
        freelancer: freelancer.clone(),
        token,
        total_amount: 1000,
        status: JobStatus::Completed,
        milestones: Vec::new(env),
    };
    env.as_contract(escrow_id, || {
        env.storage()
            .persistent()
            .set(&(symbol_short!("JOB"), job_id), &job);
    });
}

/// Helper: write an in-progress (non-completed) job into escrow storage.
fn setup_in_progress_job(
    env: &Env,
    escrow_id: &Address,
    job_id: u64,
    client: &Address,
    freelancer: &Address,
) {
    let token = Address::generate(env);
    let job = Job {
        id: job_id,
        client: client.clone(),
        freelancer: freelancer.clone(),
        token,
        total_amount: 1000,
        status: JobStatus::InProgress,
        milestones: Vec::new(env),
    };
    env.as_contract(escrow_id, || {
        env.storage()
            .persistent()
            .set(&(symbol_short!("JOB"), job_id), &job);
    });
}

#[test]
fn test_submit_review_client_reviews_freelancer() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &client_addr, &freelancer_addr);

    reputation_client.submit_review(
        &escrow_id,
        &client_addr,
        &freelancer_addr,
        &1u64,
        &4u32,
        &String::from_str(&env, "Great work!"),
        &10_i128,
    );

    let rep = reputation_client.get_reputation(&freelancer_addr);
    assert_eq!(rep.review_count, 1);
    assert_eq!(rep.total_score, 40); // 4 * 10 weight
    assert_eq!(rep.total_weight, 10);
}

#[test]
fn test_submit_review_freelancer_reviews_client() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &client_addr, &freelancer_addr);

    // Freelancer reviews the client (reverse direction is also valid)
    reputation_client.submit_review(
        &escrow_id,
        &freelancer_addr,
        &client_addr,
        &1u64,
        &5u32,
        &String::from_str(&env, "Easy to work with"),
        &5_i128,
    );

    let rep = reputation_client.get_reputation(&client_addr);
    assert_eq!(rep.review_count, 1);
    assert_eq!(rep.total_score, 25); // 5 * 5 weight
    assert_eq!(rep.total_weight, 5);
}

#[test]
fn test_average_rating() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    let reviewee = Address::generate(&env);

    // Two separate completed jobs with different reviewers
    setup_completed_job(&env, &escrow_id, 1u64, &reviewer1, &reviewee);
    setup_completed_job(&env, &escrow_id, 2u64, &reviewer2, &reviewee);

    // Review 1: 5 stars, weight 10
    reputation_client.submit_review(
        &escrow_id,
        &reviewer1,
        &reviewee,
        &1u64,
        &5u32,
        &String::from_str(&env, "Excellent"),
        &10_i128,
    );

    // Review 2: 3 stars, weight 10
    reputation_client.submit_review(
        &escrow_id,
        &reviewer2,
        &reviewee,
        &2u64,
        &3u32,
        &String::from_str(&env, "Average"),
        &10_i128,
    );

    let avg = reputation_client.get_average_rating(&reviewee);
    // (5*10 + 3*10) * 100 / (10 + 10) = 8000 / 20 = 400 (4.00 stars)
    assert_eq!(avg, 400);
    assert_eq!(reputation_client.get_review_count(&reviewee), 2);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_invalid_rating() {
    let env = Env::default();
    env.mock_all_auths();

    // Rating is validated before the escrow call, so any address suffices here
    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    reputation_client.submit_review(
        &escrow_id,
        &reviewer,
        &reviewee,
        &1u64,
        &6u32, // Invalid: max is 5
        &String::from_str(&env, "Too high"),
        &1_i128,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_self_review() {
    let env = Env::default();
    env.mock_all_auths();

    // Self-review is validated before the escrow call
    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let user = Address::generate(&env);

    reputation_client.submit_review(
        &escrow_id,
        &user,
        &user, // Self review
        &1u64,
        &5u32,
        &String::from_str(&env, "I'm great"),
        &1_i128,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_job_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    // No job set up in escrow — should fail with JobNotFound
    reputation_client.submit_review(
        &escrow_id,
        &reviewer,
        &reviewee,
        &99u64,
        &5u32,
        &String::from_str(&env, "Does not exist"),
        &1_i128,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_job_not_completed() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);

    // Job is InProgress, not Completed
    setup_in_progress_job(&env, &escrow_id, 1u64, &client_addr, &freelancer_addr);

    reputation_client.submit_review(
        &escrow_id,
        &client_addr,
        &freelancer_addr,
        &1u64,
        &5u32,
        &String::from_str(&env, "Too early"),
        &1_i128,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_not_job_participant() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let outsider = Address::generate(&env);
    let another = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &client_addr, &freelancer_addr);

    // outsider and another were not part of job 1
    reputation_client.submit_review(
        &escrow_id,
        &outsider,
        &another,
        &1u64,
        &5u32,
        &String::from_str(&env, "Fraudulent review"),
        &1_i128,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_reviewer_not_participant_but_reviewee_is() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let outsider = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &client_addr, &freelancer_addr);

    // outsider tries to review the freelancer — reviewer is not a participant
    reputation_client.submit_review(
        &escrow_id,
        &outsider,
        &freelancer_addr,
        &1u64,
        &5u32,
        &String::from_str(&env, "I wasn't there"),
        &1_i128,
    );
}
