#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Env, String,
};

#[contract]
pub struct MockToken;

#[contractimpl]
impl MockToken {
    pub fn transfer(_env: Env, _from: Address, _to: Address, _amount: i128) {}
}

const GRACE_PERIOD: u64 = 604_800; // 7 days in seconds

// ── Existing tests (updated for auto_refund_after parameter) ─────────────────

#[test]
fn test_create_job() {
    let env = Env::default();
    env.mock_all_auths();

    // Set initial timestamp
    env.ledger().with_mut(|l| l.timestamp = 1000);

    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);

    let user_client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let token = Address::generate(&env);

    let milestones = vec![
        &env,
        (String::from_str(&env, "Design mockups"), 500_i128, 2000_u64),
        (
            String::from_str(&env, "Frontend implementation"),
            1000_i128,
            3000_u64,
        ),
        (
            String::from_str(&env, "Backend integration"),
            1500_i128,
            4000_u64,
        ),
    ];

    let job_id = client.create_job(
        &user_client,
        &freelancer,
        &token,
        &milestones,
        &5000_u64,
        &GRACE_PERIOD,
    );
    assert_eq!(job_id, 1);

    let job = client.get_job(&job_id);
    assert_eq!(job.client, user_client);
    assert_eq!(job.freelancer, freelancer);
    assert_eq!(job.total_amount, 3000);
    assert_eq!(job.status, JobStatus::Created);
    assert_eq!(job.milestones.len(), 3);
    assert_eq!(job.job_deadline, 5000);
    assert_eq!(job.auto_refund_after, GRACE_PERIOD);
}

#[test]
fn test_job_count_increments() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = 1000);

    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let token = Address::generate(&env);

    let milestones = vec![&env, (String::from_str(&env, "Task 1"), 100_i128, 2000_u64)];

    let id1 = client.create_job(
        &user,
        &freelancer,
        &token,
        &milestones,
        &2500_u64,
        &GRACE_PERIOD,
    );
    let id2 = client.create_job(
        &user,
        &freelancer,
        &token,
        &milestones,
        &2500_u64,
        &GRACE_PERIOD,
    );

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(client.get_job_count(), 2);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #7)")] // InvalidDeadline
fn test_create_job_invalid_deadline() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = 1000);

    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let token = Address::generate(&env);

    let milestones = vec![
        &env,
        (String::from_str(&env, "Task 1"), 100_i128, 500_u64), // Invalid, < 1000
    ];

    client.create_job(
        &user,
        &freelancer,
        &token,
        &milestones,
        &2000_u64,
        &GRACE_PERIOD,
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #8)")] // MilestoneDeadlineExceeded
fn test_submit_milestone_past_deadline() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = 1000);

    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let token = env.register_contract(None, MockToken);

    let milestones = vec![&env, (String::from_str(&env, "Task 1"), 100_i128, 2000_u64)];

    let job_id = client.create_job(
        &user,
        &freelancer,
        &token,
        &milestones,
        &3000_u64,
        &GRACE_PERIOD,
    );
    client.fund_job(&job_id, &user);

    // fast forward past deadline
    env.ledger().with_mut(|l| l.timestamp = 2500);

    client.submit_milestone(&job_id, &0, &freelancer);
}

#[test]
fn test_is_milestone_overdue() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = 1000);

    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let token = Address::generate(&env);

    let milestones = vec![&env, (String::from_str(&env, "Task 1"), 100_i128, 2000_u64)];

    let job_id = client.create_job(
        &user,
        &freelancer,
        &token,
        &milestones,
        &3000_u64,
        &GRACE_PERIOD,
    );

    // not overdue initially
    assert_eq!(client.is_milestone_overdue(&job_id, &0), false);

    // fast forward past deadline
    env.ledger().with_mut(|l| l.timestamp = 2500);

    // overdue now
    assert_eq!(client.is_milestone_overdue(&job_id, &0), true);
}

#[test]
fn test_extend_deadline() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = 1000);

    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let token = Address::generate(&env);

    let milestones = vec![&env, (String::from_str(&env, "Task 1"), 100_i128, 2000_u64)];

    let job_id = client.create_job(
        &user,
        &freelancer,
        &token,
        &milestones,
        &3000_u64,
        &GRACE_PERIOD,
    );

    client.extend_deadline(&job_id, &0, &4000_u64);

    let job = client.get_job(&job_id);
    assert_eq!(job.milestones.get(0).unwrap().deadline, 4000);
}

// ── Helpers for claim_refund tests ───────────────────────────────────────────

fn setup_refund_env(env: &Env) -> (EscrowContractClient, Address) {
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = 1000);

    let contract_id = env.register_contract(None, EscrowContract);
    let escrow = EscrowContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let token_addr = env.register_stellar_asset_contract(admin.clone());

    (escrow, token_addr)
}

fn mint_tokens(env: &Env, token: &Address, to: &Address, amount: i128) {
    let admin_client = StellarAssetClient::new(env, token);
    admin_client.mint(to, &amount);
}

fn default_milestones(env: &Env) -> Vec<(String, i128, u64)> {
    vec![
        env,
        (String::from_str(env, "Design"), 500_i128, 500_000_u64),
        (String::from_str(env, "Frontend"), 1000_i128, 700_000_u64),
        (String::from_str(env, "Backend"), 1500_i128, 900_000_u64),
    ]
}

const JOB_DEADLINE: u64 = 1_000_000;

// ── Full refund: no milestones approved, job funded and abandoned ─────────────

#[test]
fn test_claim_refund_full() {
    let env = Env::default();
    let (escrow, token) = setup_refund_env(&env);

    let client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let milestones = default_milestones(&env);

    let job_id = escrow.create_job(
        &client,
        &freelancer,
        &token,
        &milestones,
        &JOB_DEADLINE,
        &GRACE_PERIOD,
    );

    mint_tokens(&env, &token, &client, 3000);
    escrow.fund_job(&job_id, &client);

    // Advance time past job_deadline + grace period
    env.ledger()
        .with_mut(|l| l.timestamp = JOB_DEADLINE + GRACE_PERIOD + 1);

    escrow.claim_refund(&job_id, &client);

    let job = escrow.get_job(&job_id);
    assert_eq!(job.status, JobStatus::Cancelled);

    // Client should have received full refund (3000)
    let token_client = token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&client), 3000);
}

// ── Partial refund: one milestone approved, rest refunded ────────────────────

#[test]
fn test_claim_refund_partial() {
    let env = Env::default();
    let (escrow, token) = setup_refund_env(&env);

    let client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let milestones = default_milestones(&env);

    let job_id = escrow.create_job(
        &client,
        &freelancer,
        &token,
        &milestones,
        &JOB_DEADLINE,
        &GRACE_PERIOD,
    );

    mint_tokens(&env, &token, &client, 3000);
    escrow.fund_job(&job_id, &client);

    // Freelancer submits milestone 0, client approves it (500 released)
    escrow.submit_milestone(&job_id, &0, &freelancer);
    escrow.approve_milestone(&job_id, &0, &client);

    // Advance past job_deadline + grace
    env.ledger()
        .with_mut(|l| l.timestamp = JOB_DEADLINE + GRACE_PERIOD + 1);

    escrow.claim_refund(&job_id, &client);

    let job = escrow.get_job(&job_id);
    assert_eq!(job.status, JobStatus::Cancelled);

    // Client gets back 3000 - 500 = 2500
    let token_client = token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&client), 2500);

    // Freelancer received 500 from the approved milestone
    assert_eq!(token_client.balance(&freelancer), 500);
}

// ── Refund on InProgress job ─────────────────────────────────────────────────

#[test]
fn test_claim_refund_in_progress_status() {
    let env = Env::default();
    let (escrow, token) = setup_refund_env(&env);

    let client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let milestones = default_milestones(&env);

    let job_id = escrow.create_job(
        &client,
        &freelancer,
        &token,
        &milestones,
        &JOB_DEADLINE,
        &GRACE_PERIOD,
    );

    mint_tokens(&env, &token, &client, 3000);
    escrow.fund_job(&job_id, &client);

    // Submit and approve first milestone to move to InProgress
    escrow.submit_milestone(&job_id, &0, &freelancer);
    escrow.approve_milestone(&job_id, &0, &client);

    let job = escrow.get_job(&job_id);
    assert_eq!(job.status, JobStatus::InProgress);

    env.ledger()
        .with_mut(|l| l.timestamp = JOB_DEADLINE + GRACE_PERIOD + 1);

    escrow.claim_refund(&job_id, &client);
    let job = escrow.get_job(&job_id);
    assert_eq!(job.status, JobStatus::Cancelled);
}

// ── Fail: grace period not met ───────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #11)")] // GracePeriodNotMet
fn test_claim_refund_fails_before_grace_period() {
    let env = Env::default();
    let (escrow, token) = setup_refund_env(&env);

    let client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let milestones = default_milestones(&env);

    let job_id = escrow.create_job(
        &client,
        &freelancer,
        &token,
        &milestones,
        &JOB_DEADLINE,
        &GRACE_PERIOD,
    );

    mint_tokens(&env, &token, &client, 3000);
    escrow.fund_job(&job_id, &client);

    // Time is before job_deadline + grace (only at deadline)
    env.ledger().with_mut(|l| l.timestamp = JOB_DEADLINE);

    escrow.claim_refund(&job_id, &client);
}

// ── Fail: pending milestone submission ───────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #9)")] // HasPendingMilestone
fn test_claim_refund_fails_with_pending_milestone() {
    let env = Env::default();
    let (escrow, token) = setup_refund_env(&env);

    let client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let milestones = default_milestones(&env);

    let job_id = escrow.create_job(
        &client,
        &freelancer,
        &token,
        &milestones,
        &JOB_DEADLINE,
        &GRACE_PERIOD,
    );

    mint_tokens(&env, &token, &client, 3000);
    escrow.fund_job(&job_id, &client);

    // Freelancer submits a milestone (status = Submitted, not yet approved)
    escrow.submit_milestone(&job_id, &0, &freelancer);

    env.ledger()
        .with_mut(|l| l.timestamp = JOB_DEADLINE + GRACE_PERIOD + 1);

    // Should fail because there's a submitted milestone awaiting review
    escrow.claim_refund(&job_id, &client);
}

// ── Fail: wrong caller (not the client) ──────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #2)")] // Unauthorized
fn test_claim_refund_fails_unauthorized() {
    let env = Env::default();
    let (escrow, token) = setup_refund_env(&env);

    let client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let milestones = default_milestones(&env);

    let job_id = escrow.create_job(
        &client,
        &freelancer,
        &token,
        &milestones,
        &JOB_DEADLINE,
        &GRACE_PERIOD,
    );

    mint_tokens(&env, &token, &client, 3000);
    escrow.fund_job(&job_id, &client);

    env.ledger()
        .with_mut(|l| l.timestamp = JOB_DEADLINE + GRACE_PERIOD + 1);

    // Freelancer tries to claim refund — should fail
    escrow.claim_refund(&job_id, &freelancer);
}

// ── Fail: job already completed ──────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // InvalidStatus
fn test_claim_refund_fails_on_completed_job() {
    let env = Env::default();
    let (escrow, token) = setup_refund_env(&env);

    let client = Address::generate(&env);
    let freelancer = Address::generate(&env);

    // Single milestone so we can complete the whole job
    let milestones = vec![
        &env,
        (String::from_str(&env, "Only task"), 1000_i128, 500_000_u64),
    ];

    let job_id = escrow.create_job(
        &client,
        &freelancer,
        &token,
        &milestones,
        &JOB_DEADLINE,
        &GRACE_PERIOD,
    );

    mint_tokens(&env, &token, &client, 1000);
    escrow.fund_job(&job_id, &client);

    // Complete the job
    escrow.submit_milestone(&job_id, &0, &freelancer);
    escrow.approve_milestone(&job_id, &0, &client);

    let job = escrow.get_job(&job_id);
    assert_eq!(job.status, JobStatus::Completed);

    env.ledger()
        .with_mut(|l| l.timestamp = JOB_DEADLINE + GRACE_PERIOD + 1);

    // Should fail — job is already completed
    escrow.claim_refund(&job_id, &client);
}

// ── Fail: job already cancelled ──────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // InvalidStatus
fn test_claim_refund_fails_on_cancelled_job() {
    let env = Env::default();
    let (escrow, token) = setup_refund_env(&env);

    let client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let milestones = default_milestones(&env);

    let job_id = escrow.create_job(
        &client,
        &freelancer,
        &token,
        &milestones,
        &JOB_DEADLINE,
        &GRACE_PERIOD,
    );

    mint_tokens(&env, &token, &client, 3000);
    escrow.fund_job(&job_id, &client);

    // Cancel the job first via existing cancel_job
    escrow.cancel_job(&job_id, &client);

    env.ledger()
        .with_mut(|l| l.timestamp = JOB_DEADLINE + GRACE_PERIOD + 1);

    // Should fail — job is already cancelled
    escrow.claim_refund(&job_id, &client);
}
