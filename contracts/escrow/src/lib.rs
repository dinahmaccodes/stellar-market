#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short, token, Address, Env, String,
    Vec, Symbol,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscrowError {
    JobNotFound = 1,
    Unauthorized = 2,
    InvalidStatus = 3,
    MilestoneNotFound = 4,
    InsufficientFunds = 5,
    AlreadyFunded = 6,
    InvalidDeadline = 7,
    MilestoneDeadlineExceeded = 8,
    HasPendingMilestone = 9,
    NoRefundDue = 10,
    GracePeriodNotMet = 11,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobStatus {
    Created,
    Funded,
    InProgress,
    Completed,
    Disputed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MilestoneStatus {
    Pending,
    InProgress,
    Submitted,
    Approved,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Milestone {
    pub id: u32,
    pub description: String,
    pub amount: i128,
    pub status: MilestoneStatus,
    pub deadline: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Job {
    pub id: u64,
    pub client: Address,
    pub freelancer: Address,
    pub token: Address,
    pub total_amount: i128,
    pub status: JobStatus,
    pub milestones: Vec<Milestone>,
    pub job_deadline: u64,
    pub auto_refund_after: u64,
}

const JOB_COUNT: &str = "JOB_COUNT";

fn get_job_key(job_id: u64) -> (Symbol, u64) {
    (symbol_short!("JOB"), job_id)
}

const MIN_TTL_THRESHOLD: u32 = 1_000;
const MIN_TTL_EXTEND_TO: u32 = 10_000;

fn bump_job_ttl(env: &Env, job_id: u64) {
    env.storage()
        .persistent()
        .extend_ttl(&get_job_key(job_id), MIN_TTL_THRESHOLD, MIN_TTL_EXTEND_TO);
}

fn bump_job_count_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(MIN_TTL_THRESHOLD, MIN_TTL_EXTEND_TO);
}

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    /// Creates a new job with milestones. Client specifies the freelancer and token for payment.
    pub fn create_job(
        env: Env,
        client: Address,
        freelancer: Address,
        token: Address,
        milestones: Vec<(String, i128, u64)>,
        job_deadline: u64,
        auto_refund_after: u64,
    ) -> Result<u64, EscrowError> {
        client.require_auth();

        if job_deadline <= env.ledger().timestamp() {
            return Err(EscrowError::InvalidDeadline);
        }

        let mut job_count: u64 = env.storage().instance().get(&symbol_short!("JOB_CNT")).unwrap_or(0);
        job_count += 1;

        let mut total: i128 = 0;
        let mut milestone_vec: Vec<Milestone> = Vec::new(&env);

        for (i, m) in milestones.iter().enumerate() {
            let (desc, amount, deadline) = m;
            if deadline <= env.ledger().timestamp() {
                return Err(EscrowError::InvalidDeadline);
            }
            if deadline > job_deadline {
                return Err(EscrowError::InvalidDeadline);
            }
            total += amount;
            milestone_vec.push_back(Milestone {
                id: i as u32,
                description: desc,
                amount,
                status: MilestoneStatus::Pending,
                deadline,
            });
        }

        let job = Job {
            id: job_count,
            client: client.clone(),
            freelancer: freelancer.clone(),
            token,
            total_amount: total,
            status: JobStatus::Created,
            milestones: milestone_vec,
            job_deadline,
            auto_refund_after,
        };

        env.storage().persistent().set(&get_job_key(job_count), &job);
        bump_job_ttl(&env, job_count);
        env.storage().instance().set(&symbol_short!("JOB_CNT"), &job_count);
        bump_job_count_ttl(&env);

        // Emit event
        env.events().publish(
            (symbol_short!("escrow"), symbol_short!("created")),
            (job_count, client, freelancer),
        );

        Ok(job_count)
    }

    /// Fund the escrow for a job. The client transfers the total amount to this contract.
    pub fn fund_job(env: Env, job_id: u64, client: Address) -> Result<(), EscrowError> {
        client.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&get_job_key(job_id))
            .ok_or(EscrowError::JobNotFound)?;
        bump_job_ttl(&env, job_id);

        if job.client != client {
            return Err(EscrowError::Unauthorized);
        }
        if job.status != JobStatus::Created {
            return Err(EscrowError::AlreadyFunded);
        }

        let token_client = token::Client::new(&env, &job.token);
        token_client.transfer(&client, &env.current_contract_address(), &job.total_amount);

        job.status = JobStatus::Funded;
        env.storage().persistent().set(&get_job_key(job_id), &job);
        bump_job_ttl(&env, job_id);

        // Emit event
        env.events().publish(
            (symbol_short!("escrow"), symbol_short!("funded")),
            (job_id, client),
        );

        Ok(())
    }

    pub fn resolve_dispute_callback(
        env: Env,
        job_id: u64,
        resolved_for_client: bool,
    ) -> Result<(), EscrowError> {
        let mut job: Job = env
            .storage()
            .persistent()
            .get(&get_job_key(job_id))
            .ok_or(EscrowError::JobNotFound)?;

        if job.status == JobStatus::Created
            || job.status == JobStatus::Completed
            || job.status == JobStatus::Cancelled
        {
            return Err(EscrowError::InvalidStatus);
        }

        let approved_amount: i128 = job
            .milestones
            .iter()
            .filter(|m| m.status == MilestoneStatus::Approved)
            .map(|m| m.amount)
            .sum();

        let remaining = job.total_amount - approved_amount;

        if remaining > 0 {
            let token_client = token::Client::new(&env, &job.token);
            if resolved_for_client {
                token_client.transfer(&env.current_contract_address(), &job.client, &remaining);
                job.status = JobStatus::Cancelled;
            } else {
                token_client.transfer(
                    &env.current_contract_address(),
                    &job.freelancer,
                    &remaining,
                );
                job.status = JobStatus::Completed;
            }
        } else {
            if resolved_for_client {
                job.status = JobStatus::Cancelled;
            } else {
                job.status = JobStatus::Completed;
            }
        }

        env.storage().persistent().set(&get_job_key(job_id), &job);

        env.events().publish(
            (symbol_short!("escrow"), symbol_short!("dispute")),
            (job_id, resolved_for_client),
        );

        Ok(())
    }

    /// Freelancer submits a milestone as completed.
    pub fn submit_milestone(
        env: Env,
        job_id: u64,
        milestone_id: u32,
        freelancer: Address,
    ) -> Result<(), EscrowError> {
        freelancer.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&get_job_key(job_id))
            .ok_or(EscrowError::JobNotFound)?;
        bump_job_ttl(&env, job_id);

        if job.freelancer != freelancer {
            return Err(EscrowError::Unauthorized);
        }
        if job.status != JobStatus::Funded && job.status != JobStatus::InProgress {
            return Err(EscrowError::InvalidStatus);
        }

        let mut milestones = job.milestones.clone();
        let milestone = milestones.get(milestone_id).ok_or(EscrowError::MilestoneNotFound)?;

        if milestone.status != MilestoneStatus::Pending && milestone.status != MilestoneStatus::InProgress {
            return Err(EscrowError::InvalidStatus);
        }

        if env.ledger().timestamp() > milestone.deadline {
            return Err(EscrowError::MilestoneDeadlineExceeded);
        }

        let updated = Milestone {
            id: milestone.id,
            description: milestone.description.clone(),
            amount: milestone.amount,
            status: MilestoneStatus::Submitted,
            deadline: milestone.deadline,
        };
        milestones.set(milestone_id, updated);

        job.milestones = milestones;
        job.status = JobStatus::InProgress;
        env.storage().persistent().set(&get_job_key(job_id), &job);
        bump_job_ttl(&env, job_id);

        Ok(())
    }

    /// Client approves a milestone and releases payment to the freelancer.
    pub fn approve_milestone(
        env: Env,
        job_id: u64,
        milestone_id: u32,
        client: Address,
    ) -> Result<(), EscrowError> {
        client.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&get_job_key(job_id))
            .ok_or(EscrowError::JobNotFound)?;
        bump_job_ttl(&env, job_id);

        if job.client != client {
            return Err(EscrowError::Unauthorized);
        }

        let mut milestones = job.milestones.clone();
        let milestone = milestones.get(milestone_id).ok_or(EscrowError::MilestoneNotFound)?;

        if milestone.status != MilestoneStatus::Submitted {
            return Err(EscrowError::InvalidStatus);
        }

        // Release payment for this milestone
        let token_client = token::Client::new(&env, &job.token);
        token_client.transfer(
            &env.current_contract_address(),
            &job.freelancer,
            &milestone.amount,
        );

        let updated = Milestone {
            id: milestone.id,
            description: milestone.description.clone(),
            amount: milestone.amount,
            status: MilestoneStatus::Approved,
            deadline: milestone.deadline,
        };
        milestones.set(milestone_id, updated);
        job.milestones = milestones.clone();

        // Check if all milestones are approved
        let all_approved = milestones.iter().all(|m| m.status == MilestoneStatus::Approved);
        if all_approved {
            job.status = JobStatus::Completed;
        }

        env.storage().persistent().set(&get_job_key(job_id), &job);
        bump_job_ttl(&env, job_id);

        // Emit event
        env.events().publish(
            (symbol_short!("escrow"), symbol_short!("milestone")),
            (job_id, milestone_id, client),
        );

        Ok(())
    }

    /// Cancel the job and refund remaining funds to the client.
    pub fn cancel_job(env: Env, job_id: u64, client: Address) -> Result<(), EscrowError> {
        client.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&get_job_key(job_id))
            .ok_or(EscrowError::JobNotFound)?;
        bump_job_ttl(&env, job_id);

        if job.client != client {
            return Err(EscrowError::Unauthorized);
        }
        if job.status == JobStatus::Completed || job.status == JobStatus::Cancelled {
            return Err(EscrowError::InvalidStatus);
        }

        // Calculate remaining funds (total minus already approved milestones)
        let approved_amount: i128 = job
            .milestones
            .iter()
            .filter(|m| m.status == MilestoneStatus::Approved)
            .map(|m| m.amount)
            .sum();

        let refund = job.total_amount - approved_amount;

        if refund > 0 && (job.status == JobStatus::Funded || job.status == JobStatus::InProgress) {
            let token_client = token::Client::new(&env, &job.token);
            token_client.transfer(&env.current_contract_address(), &client, &refund);
        }

        job.status = JobStatus::Cancelled;
        env.storage().persistent().set(&get_job_key(job_id), &job);
        bump_job_ttl(&env, job_id);

        // Emit event
        env.events().publish(
            (symbol_short!("escrow"), symbol_short!("cancelled")),
            (job_id, client),
        );

        Ok(())
    }

    /// Claim a refund for an abandoned job past the deadline + grace period.
    /// Only the client can call this. Refund excludes amounts for already-approved milestones.
    /// Fails if the freelancer has a pending (submitted) milestone awaiting approval.
    pub fn claim_refund(env: Env, job_id: u64, client: Address) -> Result<(), EscrowError> {
        client.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&get_job_key(job_id))
            .ok_or(EscrowError::JobNotFound)?;
        bump_job_ttl(&env, job_id);

        if job.client != client {
            return Err(EscrowError::Unauthorized);
        }

        // Only allow refund for Funded or InProgress jobs
        if job.status != JobStatus::Funded && job.status != JobStatus::InProgress {
            return Err(EscrowError::InvalidStatus);
        }

        // Ensure the grace period after deadline has elapsed
        let refund_eligible_at = job.job_deadline + job.auto_refund_after;
        if env.ledger().timestamp() < refund_eligible_at {
            return Err(EscrowError::GracePeriodNotMet);
        }

        // Prevent refund if freelancer has an active pending milestone submission
        let has_pending = job
            .milestones
            .iter()
            .any(|m| m.status == MilestoneStatus::Submitted);
        if has_pending {
            return Err(EscrowError::HasPendingMilestone);
        }

        // Calculate refund: total minus already-approved milestone amounts
        let approved_amount: i128 = job
            .milestones
            .iter()
            .filter(|m| m.status == MilestoneStatus::Approved)
            .map(|m| m.amount)
            .sum();

        let refund = job.total_amount - approved_amount;
        if refund <= 0 {
            return Err(EscrowError::NoRefundDue);
        }

        // Transfer refund to client
        let token_client = token::Client::new(&env, &job.token);
        token_client.transfer(&env.current_contract_address(), &client, &refund);

        job.status = JobStatus::Cancelled;
        env.storage().persistent().set(&get_job_key(job_id), &job);
        bump_job_ttl(&env, job_id);

        // Emit event
        env.events().publish(
            (symbol_short!("escrow"), symbol_short!("refund")),
            (job_id, refund, client),
        );

        Ok(())
    }

    /// Get job details by ID.
    pub fn get_job(env: Env, job_id: u64) -> Result<Job, EscrowError> {
        let job: Job = env
            .storage()
            .persistent()
            .get(&get_job_key(job_id))
            .ok_or(EscrowError::JobNotFound)?;
        bump_job_ttl(&env, job_id);
        Ok(job)
    }

    /// Get total number of jobs.
    pub fn get_job_count(env: Env) -> u64 {
        let count: u64 = env
            .storage()
            .instance()
            .get(&symbol_short!("JOB_CNT"))
            .unwrap_or(0);
        bump_job_count_ttl(&env);
        count
    }

    /// Check if a milestone is overdue.
    pub fn is_milestone_overdue(env: Env, job_id: u64, milestone_id: u32) -> bool {
        if let Some(job) = env.storage().persistent().get::<_, Job>(&get_job_key(job_id)) {
            if let Some(milestone) = job.milestones.get(milestone_id) {
                return env.ledger().timestamp() > milestone.deadline;
            }
        }
        false
    }

    /// Extend the deadline for a milestone (requires mutual agreement).
    pub fn extend_deadline(
        env: Env,
        job_id: u64,
        milestone_id: u32,
        new_deadline: u64,
    ) -> Result<(), EscrowError> {
        let mut job: Job = env
            .storage()
            .persistent()
            .get(&get_job_key(job_id))
            .ok_or(EscrowError::JobNotFound)?;

        job.client.require_auth();
        job.freelancer.require_auth();

        if new_deadline <= env.ledger().timestamp() {
            return Err(EscrowError::InvalidDeadline);
        }

        let mut milestones = job.milestones.clone();
        let mut milestone = milestones.get(milestone_id).ok_or(EscrowError::MilestoneNotFound)?;

        milestone.deadline = new_deadline;
        milestones.set(milestone_id, milestone);

        job.milestones = milestones;
        env.storage().persistent().set(&get_job_key(job_id), &job);

        Ok(())
    }
}

#[cfg(test)]
mod test;
