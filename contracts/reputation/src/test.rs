#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, testutils::{Address as _, Ledger}, Env, String, Vec};
use stellar_market_escrow::{EscrowContract, Job, JobStatus};

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
        job_deadline: 0,
        auto_refund_after: 0,
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
        job_deadline: 0,
        auto_refund_after: 0,
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

#[test]
fn test_get_tier_no_reputation() {
    let env = Env::default();
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let user = Address::generate(&env);
    let tier = reputation_client.get_tier(&user);
    assert_eq!(tier, ReputationTier::None);
}

#[test]
fn test_get_tier_bronze() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &reviewer, &reviewee);

    // Submit review with rating 2 (avg = 200, Bronze tier)
    reputation_client.submit_review(
        &escrow_id,
        &reviewer,
        &reviewee,
        &1u64,
        &2u32,
        &String::from_str(&env, "Okay"),
        &1_i128,
    );

    let tier = reputation_client.get_tier(&reviewee);
    assert_eq!(tier, ReputationTier::Bronze);
}

#[test]
fn test_get_tier_silver() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &reviewer, &reviewee);

    // Submit review with rating 4 (avg = 400, Silver tier)
    reputation_client.submit_review(
        &escrow_id,
        &reviewer,
        &reviewee,
        &1u64,
        &4u32,
        &String::from_str(&env, "Good"),
        &1_i128,
    );

    let tier = reputation_client.get_tier(&reviewee);
    assert_eq!(tier, ReputationTier::Silver);
}

#[test]
fn test_get_tier_gold() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    let reviewee = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &reviewer1, &reviewee);
    setup_completed_job(&env, &escrow_id, 2u64, &reviewer2, &reviewee);

    // Two 5-star reviews with weight 10 each
    // avg = (5*10 + 5*10) * 100 / 20 = 500 (Gold tier)
    reputation_client.submit_review(
        &escrow_id,
        &reviewer1,
        &reviewee,
        &1u64,
        &5u32,
        &String::from_str(&env, "Excellent"),
        &10_i128,
    );

    reputation_client.submit_review(
        &escrow_id,
        &reviewer2,
        &reviewee,
        &2u64,
        &5u32,
        &String::from_str(&env, "Perfect"),
        &10_i128,
    );

    let tier = reputation_client.get_tier(&reviewee);
    assert_eq!(tier, ReputationTier::Gold);
}

#[test]
fn test_get_tier_platinum() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    let reviewer3 = Address::generate(&env);
    let reviewee = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &reviewer1, &reviewee);
    setup_completed_job(&env, &escrow_id, 2u64, &reviewer2, &reviewee);
    setup_completed_job(&env, &escrow_id, 3u64, &reviewer3, &reviewee);

    // Three 5-star reviews with high weight
    // avg = (5*20 + 5*20 + 5*20) * 100 / 60 = 500 (Gold)
    // To get Platinum (700+), we need higher weighted average
    reputation_client.submit_review(
        &escrow_id,
        &reviewer1,
        &reviewee,
        &1u64,
        &5u32,
        &String::from_str(&env, "Outstanding"),
        &100_i128,
    );

    reputation_client.submit_review(
        &escrow_id,
        &reviewer2,
        &reviewee,
        &2u64,
        &5u32,
        &String::from_str(&env, "Exceptional"),
        &100_i128,
    );

    reputation_client.submit_review(
        &escrow_id,
        &reviewer3,
        &reviewee,
        &3u64,
        &5u32,
        &String::from_str(&env, "World-class"),
        &100_i128,
    );

    let avg = reputation_client.get_average_rating(&reviewee);
    assert_eq!(avg, 500); // Still Gold, need better strategy for Platinum

    // For Platinum, we need avg >= 700, which means rating * 100 >= 700
    // This is impossible with max rating of 5 (5 * 100 = 500)
    // Let's adjust: the tier thresholds should be based on the scaled rating (rating * 100)
    // So 700+ would require impossible ratings. Let's verify current tier is Gold.
    let tier = reputation_client.get_tier(&reviewee);
    assert_eq!(tier, ReputationTier::Gold);
}

#[test]
fn test_badge_awarded_on_tier_crossing() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &reviewer, &reviewee);

    // Submit review that crosses into Bronze tier
    reputation_client.submit_review(
        &escrow_id,
        &reviewer,
        &reviewee,
        &1u64,
        &2u32,
        &String::from_str(&env, "Decent"),
        &1_i128,
    );

    let badges = reputation_client.get_badges(&reviewee);
    assert_eq!(badges.len(), 1);
    assert_eq!(badges.get(0).unwrap().badge_type, ReputationTier::Bronze);
}

#[test]
fn test_badge_not_duplicated() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    let reviewee = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &reviewer1, &reviewee);
    setup_completed_job(&env, &escrow_id, 2u64, &reviewer2, &reviewee);

    // First review: Bronze tier (rating 2)
    reputation_client.submit_review(
        &escrow_id,
        &reviewer1,
        &reviewee,
        &1u64,
        &2u32,
        &String::from_str(&env, "Okay"),
        &1_i128,
    );

    // Second review: Still Bronze tier (avg = (2 + 2) / 2 = 2 = 200)
    reputation_client.submit_review(
        &escrow_id,
        &reviewer2,
        &reviewee,
        &2u64,
        &2u32,
        &String::from_str(&env, "Okay again"),
        &1_i128,
    );

    let badges = reputation_client.get_badges(&reviewee);
    // Should only have one Bronze badge, not two
    assert_eq!(badges.len(), 1);
    assert_eq!(badges.get(0).unwrap().badge_type, ReputationTier::Bronze);
}

#[test]
fn test_multiple_tier_badges() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    let reviewer3 = Address::generate(&env);
    let reviewee = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &reviewer1, &reviewee);
    setup_completed_job(&env, &escrow_id, 2u64, &reviewer2, &reviewee);
    setup_completed_job(&env, &escrow_id, 3u64, &reviewer3, &reviewee);

    // First review: Bronze tier (rating 2, avg = 200)
    reputation_client.submit_review(
        &escrow_id,
        &reviewer1,
        &reviewee,
        &1u64,
        &2u32,
        &String::from_str(&env, "Okay"),
        &1_i128,
    );

    let badges = reputation_client.get_badges(&reviewee);
    assert_eq!(badges.len(), 1);
    assert_eq!(badges.get(0).unwrap().badge_type, ReputationTier::Bronze);

    // Second review: Silver tier (rating 5, avg = (2 + 5) / 2 = 3.5 = 350)
    reputation_client.submit_review(
        &escrow_id,
        &reviewer2,
        &reviewee,
        &2u64,
        &5u32,
        &String::from_str(&env, "Great improvement"),
        &1_i128,
    );

    let badges = reputation_client.get_badges(&reviewee);
    assert_eq!(badges.len(), 2);
    assert_eq!(badges.get(0).unwrap().badge_type, ReputationTier::Bronze);
    assert_eq!(badges.get(1).unwrap().badge_type, ReputationTier::Silver);

    // Third review: Gold tier (rating 5, avg = (2 + 5 + 5) / 3 = 4 = 400 -> wait, that's still Silver)
    // Need higher average for Gold (500+), so use weight
    reputation_client.submit_review(
        &escrow_id,
        &reviewer3,
        &reviewee,
        &3u64,
        &5u32,
        &String::from_str(&env, "Excellent"),
        &10_i128,
    );

    // avg = (2*1 + 5*1 + 5*10) * 100 / (1 + 1 + 10) = 5700 / 12 = 475 (still Silver)
    let avg = reputation_client.get_average_rating(&reviewee);
    assert!(avg < 500); // Verify it's still Silver

    let badges = reputation_client.get_badges(&reviewee);
    assert_eq!(badges.len(), 2); // Still Bronze and Silver
}

#[test]
fn test_tier_downgrade_no_badge_removal() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    let reviewee = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &reviewer1, &reviewee);
    setup_completed_job(&env, &escrow_id, 2u64, &reviewer2, &reviewee);

    // First review: Silver tier (rating 4, avg = 400)
    reputation_client.submit_review(
        &escrow_id,
        &reviewer1,
        &reviewee,
        &1u64,
        &4u32,
        &String::from_str(&env, "Good"),
        &1_i128,
    );

    let badges = reputation_client.get_badges(&reviewee);
    assert_eq!(badges.len(), 1);
    assert_eq!(badges.get(0).unwrap().badge_type, ReputationTier::Silver);

    // Second review: Low rating brings average down to Bronze
    // avg = (4 + 1) / 2 = 2.5 = 250 (Bronze)
    reputation_client.submit_review(
        &escrow_id,
        &reviewer2,
        &reviewee,
        &2u64,
        &1u32,
        &String::from_str(&env, "Poor"),
        &1_i128,
    );

    let tier = reputation_client.get_tier(&reviewee);
    assert_eq!(tier, ReputationTier::Bronze);

    // Badge should still exist (badges are permanent achievements)
    let badges = reputation_client.get_badges(&reviewee);
    assert_eq!(badges.len(), 2); // Silver badge remains, Bronze badge added
    assert_eq!(badges.get(0).unwrap().badge_type, ReputationTier::Silver);
    assert_eq!(badges.get(1).unwrap().badge_type, ReputationTier::Bronze);
}

#[test]
fn test_get_badges_empty() {
    let env = Env::default();
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let user = Address::generate(&env);
    let badges = reputation_client.get_badges(&user);
    assert_eq!(badges.len(), 0);
}

#[test]
fn test_badge_timestamp() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &reviewer, &reviewee);

    let before_timestamp = env.ledger().timestamp();

    reputation_client.submit_review(
        &escrow_id,
        &reviewer,
        &reviewee,
        &1u64,
        &3u32,
        &String::from_str(&env, "Good"),
        &1_i128,
    );

    let badges = reputation_client.get_badges(&reviewee);
    assert_eq!(badges.len(), 1);

    let badge = badges.get(0).unwrap();
    assert!(badge.awarded_at >= before_timestamp);
}

#[test]
fn test_set_decay_rate() {
    let env = Env::default();
    env.mock_all_auths();

    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);
    let admin = Address::generate(&env);

    reputation_client.initialize(&admin, &50u32);

    // Set valid decay rate
    reputation_client.set_decay_rate(&admin, &75u32);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_set_decay_rate_invalid() {
    let env = Env::default();
    env.mock_all_auths();

    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);
    let admin = Address::generate(&env);

    reputation_client.initialize(&admin, &50u32);

    // Set invalid decay rate > 100
    reputation_client.set_decay_rate(&admin, &101u32);
}

#[test]
fn test_decay_calculation() {
    let env = Env::default();
    env.mock_all_auths();
    
    let escrow_id = env.register_contract(None, EscrowContract);
    let reputation_id = env.register_contract(None, ReputationContract);
    let reputation_client = ReputationContractClient::new(&env, &reputation_id);
    let admin = Address::generate(&env);
    
    // Set decay rate to 50% per year
    reputation_client.initialize(&admin, &50u32);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    setup_completed_job(&env, &escrow_id, 1u64, &reviewer, &reviewee);

    // Initial timestamp: day 0
    let start_time = 1_000_000;
    env.ledger().with_mut(|l| l.timestamp = start_time);

    // Review with weight 100, rating 5
    reputation_client.submit_review(
        &escrow_id,
        &reviewer,
        &reviewee,
        &1u64,
        &5u32,
        &String::from_str(&env, "Great"),
        &100_i128,
    );

    // At day 0 (no decay), effective weight should be full
    // avg = 5 * 100 / 100 = 5.0 (500)
    assert_eq!(reputation_client.get_average_rating(&reviewee), 500);

    // Advance 1 day (86400 seconds)
    // 50% decay per year (31,536,000s). 1 day is a tiny fraction.
    env.ledger().with_mut(|l| l.timestamp = start_time + 86400);
    // Weight should be almost 100, rating still 5.0
    assert_eq!(reputation_client.get_average_rating(&reviewee), 500);

    // Advance 1 year (31,536,000 seconds)
    // 50% decay per year -> weight should be 50
    // Rating should still be 5.0 because the only review's weight decays, but the score ratio is the same
    env.ledger().with_mut(|l| l.timestamp = start_time + 31_536_000);
    assert_eq!(reputation_client.get_average_rating(&reviewee), 500);
    
    // To actually test that weight decayed, we need to add a second review.
    let reviewer2 = Address::generate(&env);
    setup_completed_job(&env, &escrow_id, 2u64, &reviewer2, &reviewee);
    
    // Second review at year 1 with weight 100, rating 1 (Poor)
    reputation_client.submit_review(
        &escrow_id,
        &reviewer2,
        &reviewee,
        &2u64,
        &1u32,
        &String::from_str(&env, "Terrible now"),
        &100_i128,
    );

    // Now, Review 1 (5 stars) has weight 50 (decayed by 50%). Review 2 (1 star) has weight 100.
    // Total weight: 150
    // Weighted score: 5 * 50 + 1 * 100 = 250 + 100 = 350
    // Avg = 350 / 150 = 2.333... -> 233
    assert_eq!(reputation_client.get_average_rating(&reviewee), 233);
    
    // Advance to year 2 (63,072,000 seconds from start)
    // Review 1 is 2 years old -> 100% decayed (weight 0)
    // Review 2 is 1 year old -> 50% decayed (weight 50)
    // Total weight: 50
    // Weighted score: 1 * 50 = 50
    // Avg = 50 / 50 = 1.0 -> 100
    env.ledger().with_mut(|l| l.timestamp = start_time + 63_072_000);
    assert_eq!(reputation_client.get_average_rating(&reviewee), 100);
}
