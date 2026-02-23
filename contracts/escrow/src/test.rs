#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, vec, Env, String};

#[contract]
pub struct MockToken;

#[contractimpl]
impl MockToken {
    pub fn transfer(_env: Env, _from: Address, _to: Address, _amount: i128) {}
}

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
        (String::from_str(&env, "Frontend implementation"), 1000_i128, 3000_u64),
        (String::from_str(&env, "Backend integration"), 1500_i128, 4000_u64),
    ];

    let job_id = client.create_job(&user_client, &freelancer, &token, &milestones, &5000_u64);
    assert_eq!(job_id, 1);

    let job = client.get_job(&job_id);
    assert_eq!(job.client, user_client);
    assert_eq!(job.freelancer, freelancer);
    assert_eq!(job.total_amount, 3000);
    assert_eq!(job.status, JobStatus::Created);
    assert_eq!(job.milestones.len(), 3);
    assert_eq!(job.job_deadline, 5000);
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

    let milestones = vec![
        &env,
        (String::from_str(&env, "Task 1"), 100_i128, 2000_u64),
    ];

    let id1 = client.create_job(&user, &freelancer, &token, &milestones, &2500_u64);
    let id2 = client.create_job(&user, &freelancer, &token, &milestones, &2500_u64);

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

    client.create_job(&user, &freelancer, &token, &milestones, &2000_u64);
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

    let milestones = vec![
        &env,
        (String::from_str(&env, "Task 1"), 100_i128, 2000_u64),
    ];

    let job_id = client.create_job(&user, &freelancer, &token, &milestones, &3000_u64);
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

    let milestones = vec![
        &env,
        (String::from_str(&env, "Task 1"), 100_i128, 2000_u64),
    ];

    let job_id = client.create_job(&user, &freelancer, &token, &milestones, &3000_u64);
    
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

    let milestones = vec![
        &env,
        (String::from_str(&env, "Task 1"), 100_i128, 2000_u64),
    ];

    let job_id = client.create_job(&user, &freelancer, &token, &milestones, &3000_u64);

    client.extend_deadline(&job_id, &0, &4000_u64);

    let job = client.get_job(&job_id);
    assert_eq!(job.milestones.get(0).unwrap().deadline, 4000);
}
