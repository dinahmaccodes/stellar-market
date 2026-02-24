#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short, Address, Env, String, Vec,
    Symbol, vec, IntoVal,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DisputeError {
    DisputeNotFound = 1,
    Unauthorized = 2,
    AlreadyVoted = 3,
    VotingClosed = 4,
    NotEnoughVotes = 5,
    InvalidParty = 6,
    AlreadyResolved = 7,
    AppealWindowExpired = 8,
    MaxAppealsReached = 9,
    NotLosingParty = 10,
    CannotAppealBeforeResolution = 11,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    Open,
    Voting,
    ResolvedForClient,
    ResolvedForFreelancer,
    Appealed,
    FinalResolution,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoteChoice {
    Client,
    Freelancer,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vote {
    pub voter: Address,
    pub choice: VoteChoice,
    pub reason: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dispute {
    pub id: u64,
    pub job_id: u64,
    pub client: Address,
    pub freelancer: Address,
    pub initiator: Address,
    pub reason: String,
    pub status: DisputeStatus,
    pub votes_for_client: u32,
    pub votes_for_freelancer: u32,
    pub min_votes: u32,
    pub created_at: u64,
    pub appeal_count: u32,
    pub max_appeals: u32,
    pub appeal_deadline: u64,
    pub resolution_timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum DataKey {
    Dispute(u64),
    DisputeCount,
    Votes(u64),
    HasVoted(u64, Address),
}

const MIN_TTL_THRESHOLD: u32 = 1_000;
const MIN_TTL_EXTEND_TO: u32 = 10_000;

fn bump_dispute_ttl(env: &Env, dispute_id: u64) {
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::Dispute(dispute_id), MIN_TTL_THRESHOLD, MIN_TTL_EXTEND_TO);
}

fn bump_votes_ttl(env: &Env, dispute_id: u64) {
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::Votes(dispute_id), MIN_TTL_THRESHOLD, MIN_TTL_EXTEND_TO);
}

fn bump_has_voted_ttl(env: &Env, dispute_id: u64, voter: &Address) {
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::HasVoted(dispute_id, voter.clone()), MIN_TTL_THRESHOLD, MIN_TTL_EXTEND_TO);
}

fn bump_dispute_count_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(MIN_TTL_THRESHOLD, MIN_TTL_EXTEND_TO);
}

#[contract]
pub struct DisputeContract;

#[contractimpl]
impl DisputeContract {
    /// Raise a dispute on a job. Either the client or freelancer can initiate.
    pub fn raise_dispute(
        env: Env,
        job_id: u64,
        client: Address,
        freelancer: Address,
        initiator: Address,
        reason: String,
        min_votes: u32,
    ) -> Result<u64, DisputeError> {
        initiator.require_auth();

        if initiator != client && initiator != freelancer {
            return Err(DisputeError::InvalidParty);
        }

        let mut count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::DisputeCount)
            .unwrap_or(0);
        count += 1;

        let dispute = Dispute {
            id: count,
            job_id,
            client,
            freelancer,
            initiator: initiator.clone(),
            reason,
            status: DisputeStatus::Open,
            votes_for_client: 0,
            votes_for_freelancer: 0,
            min_votes: if min_votes < 3 { 3 } else { min_votes },
            created_at: env.ledger().timestamp(),
            appeal_count: 0,
            max_appeals: 2,
            appeal_deadline: 0,
            resolution_timestamp: 0,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Dispute(count), &dispute);
        env.storage()
            .instance()
            .set(&DataKey::DisputeCount, &count);
        bump_dispute_ttl(&env, count);
        bump_dispute_count_ttl(&env);
        env.storage()
            .persistent()
            .set(&DataKey::Votes(count), &Vec::<Vote>::new(&env));
        bump_votes_ttl(&env, count);

        // Emit event
        env.events().publish(
            (symbol_short!("dispute"), symbol_short!("raised")),
            (count, job_id, initiator),
        );

        Ok(count)
    }

    /// Cast a vote on a dispute. Voters cannot be the client or freelancer.
    pub fn cast_vote(
        env: Env,
        dispute_id: u64,
        voter: Address,
        choice: VoteChoice,
        reason: String,
    ) -> Result<(), DisputeError> {
        voter.require_auth();

        let mut dispute: Dispute = env
            .storage()
            .persistent()
            .get(&DataKey::Dispute(dispute_id))
            .ok_or(DisputeError::DisputeNotFound)?;
        bump_dispute_ttl(&env, dispute_id);

        if dispute.status != DisputeStatus::Open 
            && dispute.status != DisputeStatus::Voting 
            && dispute.status != DisputeStatus::Appealed {
            return Err(DisputeError::VotingClosed);
        }

        // Parties involved cannot vote
        if voter == dispute.client || voter == dispute.freelancer {
            return Err(DisputeError::InvalidParty);
        }

        // Check if already voted
        let voted_key = DataKey::HasVoted(dispute_id, voter.clone());
        if env.storage().persistent().has(&voted_key) {
            return Err(DisputeError::AlreadyVoted);
        }

        // Record vote
        let vote = Vote {
            voter: voter.clone(),
            choice: choice.clone(),
            reason,
            timestamp: env.ledger().timestamp(),
        };

        let mut votes: Vec<Vote> = env
            .storage()
            .persistent()
            .get(&DataKey::Votes(dispute_id))
            .unwrap_or(Vec::new(&env));
        votes.push_back(vote);
        env.storage()
            .persistent()
            .set(&DataKey::Votes(dispute_id), &votes);
        bump_votes_ttl(&env, dispute_id);

        match choice {
            VoteChoice::Client => dispute.votes_for_client += 1,
            VoteChoice::Freelancer => dispute.votes_for_freelancer += 1,
        }

        dispute.status = DisputeStatus::Voting;
        env.storage()
            .persistent()
            .set(&DataKey::Dispute(dispute_id), &dispute);
        env.storage().persistent().set(&voted_key, &true);
        bump_dispute_ttl(&env, dispute_id);
        bump_has_voted_ttl(&env, dispute_id, &voter);

        // Emit event
        env.events().publish(
            (symbol_short!("dispute"), symbol_short!("voted")),
            (dispute_id, voter, choice),
        );

        Ok(())
    }

    pub fn resolve_dispute(
        env: Env,
        dispute_id: u64,
        escrow: Address,
    ) -> Result<DisputeStatus, DisputeError> {
        let mut dispute: Dispute = env
            .storage()
            .persistent()
            .get(&DataKey::Dispute(dispute_id))
            .ok_or(DisputeError::DisputeNotFound)?;
        bump_dispute_ttl(&env, dispute_id);

        if dispute.status == DisputeStatus::FinalResolution {
            return Err(DisputeError::AlreadyResolved);
        }

        let total_votes = dispute.votes_for_client + dispute.votes_for_freelancer;
        
        // Calculate required votes based on appeal count (doubles each round)
        let required_votes = dispute.min_votes * (2_u32.pow(dispute.appeal_count));
        
        if total_votes < required_votes {
            return Err(DisputeError::NotEnoughVotes);
        }

        let resolved_for_client = dispute.votes_for_client >= dispute.votes_for_freelancer;
        
        // Check if this is the final resolution
        let is_final = dispute.appeal_count >= dispute.max_appeals;
        
        if is_final {
            dispute.status = DisputeStatus::FinalResolution;
        } else {
            dispute.status = if resolved_for_client {
                DisputeStatus::ResolvedForClient
            } else {
                DisputeStatus::ResolvedForFreelancer
            };
        }

        // Set resolution timestamp and appeal deadline (e.g., 100 ledgers = ~8.3 minutes)
        dispute.resolution_timestamp = env.ledger().timestamp();
        dispute.appeal_deadline = env.ledger().sequence() as u64 + 100;

        let resolved_status = dispute.status.clone();

        // Only invoke escrow callback for final resolution
        if is_final {
            let _ = env.invoke_contract::<()>(
                &escrow,
                &Symbol::new(&env, "resolve_dispute_callback"),
                vec![
                    &env,
                    dispute.job_id.into_val(&env),
                    resolved_for_client.into_val(&env),
                ],
            );
        }

        env.storage()
            .persistent()
            .set(&DataKey::Dispute(dispute_id), &dispute);
        bump_dispute_ttl(&env, dispute_id);

        // Emit different events for first resolution vs final resolution
        if is_final {
            env.events().publish(
                (symbol_short!("dispute"), symbol_short!("final")),
                (dispute_id, resolved_status.clone()),
            );
        } else {
            env.events().publish(
                (symbol_short!("dispute"), symbol_short!("resolved")),
                (dispute_id, resolved_status.clone()),
            );
        }

        Ok(resolved_status)
    }

    /// Raise an appeal on a resolved dispute. Only the losing party can appeal.
    pub fn raise_appeal(
        env: Env,
        dispute_id: u64,
        appellant: Address,
    ) -> Result<(), DisputeError> {
        appellant.require_auth();

        let mut dispute: Dispute = env
            .storage()
            .persistent()
            .get(&DataKey::Dispute(dispute_id))
            .ok_or(DisputeError::DisputeNotFound)?;
        bump_dispute_ttl(&env, dispute_id);

        // Check if max appeals reached first
        if dispute.appeal_count >= dispute.max_appeals {
            return Err(DisputeError::MaxAppealsReached);
        }

        // Check if dispute has been resolved (but not final)
        if dispute.status != DisputeStatus::ResolvedForClient 
            && dispute.status != DisputeStatus::ResolvedForFreelancer 
            && dispute.status != DisputeStatus::FinalResolution {
            return Err(DisputeError::CannotAppealBeforeResolution);
        }

        // Cannot appeal final resolution
        if dispute.status == DisputeStatus::FinalResolution {
            return Err(DisputeError::MaxAppealsReached);
        }

        // Check if appeal window has expired
        if env.ledger().sequence() as u64 > dispute.appeal_deadline {
            return Err(DisputeError::AppealWindowExpired);
        }

        // Verify appellant is the losing party
        let is_losing_party = match dispute.status {
            DisputeStatus::ResolvedForClient => appellant == dispute.freelancer,
            DisputeStatus::ResolvedForFreelancer => appellant == dispute.client,
            _ => false,
        };

        if !is_losing_party {
            return Err(DisputeError::NotLosingParty);
        }

        // Increment appeal count and reset voting
        dispute.appeal_count += 1;
        dispute.status = DisputeStatus::Appealed;
        dispute.votes_for_client = 0;
        dispute.votes_for_freelancer = 0;

        // Clear previous votes
        env.storage()
            .persistent()
            .set(&DataKey::Votes(dispute_id), &Vec::<Vote>::new(&env));
        bump_votes_ttl(&env, dispute_id);

        env.storage()
            .persistent()
            .set(&DataKey::Dispute(dispute_id), &dispute);
        bump_dispute_ttl(&env, dispute_id);

        // Emit event
        env.events().publish(
            (symbol_short!("dispute"), symbol_short!("appealed")),
            (dispute_id, appellant, dispute.appeal_count),
        );

        Ok(())
    }

    /// Get dispute details.
    pub fn get_dispute(env: Env, dispute_id: u64) -> Result<Dispute, DisputeError> {
        let dispute: Dispute = env
            .storage()
            .persistent()
            .get(&DataKey::Dispute(dispute_id))
            .ok_or(DisputeError::DisputeNotFound)?;
        bump_dispute_ttl(&env, dispute_id);
        Ok(dispute)
    }

    /// Get all votes for a dispute.
    pub fn get_votes(env: Env, dispute_id: u64) -> Vec<Vote> {
        env.storage()
            .persistent()
            .get(&DataKey::Votes(dispute_id))
            .unwrap_or(Vec::new(&env))
    }

    /// Get total dispute count.
    pub fn get_dispute_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::DisputeCount)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
