#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Vec,
};
use stellar_market_escrow::{EscrowContractClient, JobStatus};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReputationError {
    InvalidRating = 1,
    AlreadyReviewed = 2,
    SelfReview = 3,
    UserNotFound = 4,
    JobNotCompleted = 5,
    NotJobParticipant = 6,
    JobNotFound = 7,
    Unauthorized = 8,
    NotInitialized = 9,
    InvalidDecayRate = 10,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Review {
    pub reviewer: Address,
    pub reviewee: Address,
    pub job_id: u64,
    pub rating: u32,
    pub comment: String,
    pub stake_weight: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserReputation {
    pub user: Address,
    pub total_score: u64,
    pub total_weight: u64,
    pub review_count: u32,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReputationTier {
    None = 0,
    Bronze = 1,
    Silver = 2,
    Gold = 3,
    Platinum = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Badge {
    pub badge_type: ReputationTier,
    pub awarded_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum DataKey {
    Reputation(Address),
    Reviews(Address),
    ReviewExists(Address, Address, u64),
    Badges(Address),
    Admin,
    DecayRate,
}

const MIN_TTL_THRESHOLD: u32 = 1_000;
const MIN_TTL_EXTEND_TO: u32 = 10_000;

fn bump_reputation_ttl(env: &Env, user: &Address) {
    env.storage().persistent().extend_ttl(
        &DataKey::Reputation(user.clone()),
        MIN_TTL_THRESHOLD,
        MIN_TTL_EXTEND_TO,
    );
}

fn bump_reviews_ttl(env: &Env, user: &Address) {
    env.storage().persistent().extend_ttl(
        &DataKey::Reviews(user.clone()),
        MIN_TTL_THRESHOLD,
        MIN_TTL_EXTEND_TO,
    );
}

fn bump_review_exists_ttl(env: &Env, reviewer: &Address, reviewee: &Address, job_id: u64) {
    env.storage().persistent().extend_ttl(
        &DataKey::ReviewExists(reviewer.clone(), reviewee.clone(), job_id),
        MIN_TTL_THRESHOLD,
        MIN_TTL_EXTEND_TO,
    );
}

fn bump_badges_ttl(env: &Env, user: &Address) {
    env.storage().persistent().extend_ttl(
        &DataKey::Badges(user.clone()),
        MIN_TTL_THRESHOLD,
        MIN_TTL_EXTEND_TO,
    );
}

fn bump_instance_ttl(env: &Env) {
    env.storage().instance().extend_ttl(MIN_TTL_THRESHOLD, MIN_TTL_EXTEND_TO);
}

/// Calculate the reputation tier based on average rating score.
/// Score thresholds:
/// - 0-99: None
/// - 100-299: Bronze
/// - 300-499: Silver
/// - 500-699: Gold
/// - 700+: Platinum
fn calculate_tier(average_rating: u64) -> ReputationTier {
    if average_rating >= 700 {
        ReputationTier::Platinum
    } else if average_rating >= 500 {
        ReputationTier::Gold
    } else if average_rating >= 300 {
        ReputationTier::Silver
    } else if average_rating >= 100 {
        ReputationTier::Bronze
    } else {
        ReputationTier::None
    }
}

#[contract]
pub struct ReputationContract;

#[contractimpl]
impl ReputationContract {
    /// Submit a review for a user after completing a job.
    /// Rating must be between 1 and 5. Stake weight affects the review's influence.
    /// The escrow_contract_id is used to verify the job exists, is completed,
    /// and that reviewer/reviewee are the actual participants of the job.
    pub fn submit_review(
        env: Env,
        escrow_contract_id: Address,
        reviewer: Address,
        reviewee: Address,
        job_id: u64,
        rating: u32,
        comment: String,
        stake_weight: i128,
    ) -> Result<(), ReputationError> {
        reviewer.require_auth();

        if !(1..=5).contains(&rating) {
            return Err(ReputationError::InvalidRating);
        }
        if reviewer == reviewee {
            return Err(ReputationError::SelfReview);
        }

        // Check if this reviewer already reviewed this user for this job
        let review_key = DataKey::ReviewExists(reviewer.clone(), reviewee.clone(), job_id);
        if env.storage().persistent().has(&review_key) {
            return Err(ReputationError::AlreadyReviewed);
        }

        // Cross-contract call: verify the job exists, is completed, and the
        // reviewer/reviewee are the actual client and freelancer of the job.
        let escrow_client = EscrowContractClient::new(&env, &escrow_contract_id);
        let job = match escrow_client.try_get_job(&job_id) {
            Ok(Ok(j)) => j,
            Ok(Err(_)) | Err(_) => return Err(ReputationError::JobNotFound),
        };

        if job.status != JobStatus::Completed {
            return Err(ReputationError::JobNotCompleted);
        }

        let valid_participants = (reviewer == job.client && reviewee == job.freelancer)
            || (reviewer == job.freelancer && reviewee == job.client);

        if !valid_participants {
            return Err(ReputationError::NotJobParticipant);
        }

        let weight = if stake_weight > 0 {
            stake_weight as u64
        } else {
            1u64
        };

        // Update user reputation
        let rep_key = DataKey::Reputation(reviewee.clone());
        let mut reputation: UserReputation =
            env.storage()
                .persistent()
                .get(&rep_key)
                .unwrap_or(UserReputation {
                    user: reviewee.clone(),
                    total_score: 0,
                    total_weight: 0,
                    review_count: 0,
                });

        reputation.total_score += (rating as u64) * weight;
        reputation.total_weight += weight;
        reputation.review_count += 1;

        env.storage().persistent().set(&rep_key, &reputation);
        bump_reputation_ttl(&env, &reviewee);

        // Store review
        let review = Review {
            reviewer: reviewer.clone(),
            reviewee: reviewee.clone(),
            job_id,
            rating,
            comment,
            stake_weight,
            timestamp: env.ledger().timestamp(),
        };

        let reviews_key = DataKey::Reviews(reviewee.clone());
        let mut reviews: Vec<Review> = env
            .storage()
            .persistent()
            .get(&reviews_key)
            .unwrap_or(Vec::new(&env));
        reviews.push_back(review);
        env.storage().persistent().set(&reviews_key, &reviews);
        bump_reviews_ttl(&env, &reviewee);

        // Mark as reviewed
        env.storage().persistent().set(&review_key, &true);
        bump_review_exists_ttl(&env, &reviewer, &reviewee, job_id);

        // Check for tier upgrade and award badge if necessary
        let new_avg_rating = Self::get_average_rating(env.clone(), reviewee.clone()).unwrap_or(0);
        let new_tier = calculate_tier(new_avg_rating);

        // Get existing badges to check if this tier badge already exists
        let badges_key = DataKey::Badges(reviewee.clone());
        let mut badges: Vec<Badge> = env
            .storage()
            .persistent()
            .get(&badges_key)
            .unwrap_or(Vec::new(&env));

        // Check if user already has this tier badge
        let has_tier_badge = badges.iter().any(|b| b.badge_type == new_tier);

        if !has_tier_badge && new_tier != ReputationTier::None {
            let badge = Badge {
                badge_type: new_tier,
                awarded_at: env.ledger().timestamp(),
            };
            badges.push_back(badge);
            env.storage().persistent().set(&badges_key, &badges);
            bump_badges_ttl(&env, &reviewee);

            // Emit badge awarded event
            env.events().publish(
                (symbol_short!("reput"), symbol_short!("badge")),
                (reviewee.clone(), new_tier),
            );
        }

        // Emit event
        env.events().publish(
            (symbol_short!("reput"), symbol_short!("reviewed")),
            (reviewer, reviewee, job_id, rating),
        );

        Ok(())
    }

    /// Get the reputation data for a user.
    pub fn get_reputation(env: Env, user: Address) -> Result<UserReputation, ReputationError> {
        let rep_key = DataKey::Reputation(user);
        let reputation: UserReputation = env
            .storage()
            .persistent()
            .get(&rep_key)
            .ok_or(ReputationError::UserNotFound)?;
        bump_reputation_ttl(&env, &reputation.user);
        Ok(reputation)
    }

    /// Initialize the reputation contract with an admin.
    pub fn initialize(env: Env, admin: Address, decay_rate: u32) -> Result<(), ReputationError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ReputationError::Unauthorized); // already initialized
        }
        if decay_rate > 100 {
            return Err(ReputationError::InvalidDecayRate);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::DecayRate, &decay_rate);
        bump_instance_ttl(&env);
        Ok(())
    }

    /// Set the decay rate for reviews (0-100 percentage per year).
    pub fn set_decay_rate(env: Env, admin: Address, rate: u32) -> Result<(), ReputationError> {
        admin.require_auth();
        
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).ok_or(ReputationError::NotInitialized)?;
        if admin != stored_admin {
            return Err(ReputationError::Unauthorized);
        }
        
        if rate > 100 {
            return Err(ReputationError::InvalidDecayRate);
        }
        
        env.storage().instance().set(&DataKey::DecayRate, &rate);
        bump_instance_ttl(&env);
        
        // Emit event
        env.events().publish(
            (symbol_short!("reput"), symbol_short!("decay_rt")),
            (admin, rate),
        );
        Ok(())
    }

    /// Calculate effective weight of a review, applying time decay.
    /// Formula: effective_weight = stake_weight * max(0, 100 - decay_rate * age_in_seconds / ONE_YEAR) / 100
    pub fn get_effective_weight(env: Env, review: Review, current_time: u64) -> i128 {
        let decay_rate: u32 = env.storage().instance().get(&DataKey::DecayRate).unwrap_or(0);
        
        let initial_weight = if review.stake_weight > 0 {
            review.stake_weight
        } else {
            1_i128
        };
        
        if decay_rate == 0 {
            return initial_weight;
        }

        let age_in_seconds = current_time.saturating_sub(review.timestamp);
        let one_year_in_seconds = 31_536_000_u64;
        
        let decay_amount = (decay_rate as u64).saturating_mul(age_in_seconds) / one_year_in_seconds;
        let decay_factor = 100_u64.saturating_sub(decay_amount);
        
        if decay_factor == 0 {
            return 0;
        }
        
        (initial_weight.saturating_mul(decay_factor as i128)) / 100
    }

    pub fn get_average_rating(env: Env, user: Address) -> Result<u64, ReputationError> {
        let reviews = Self::get_reviews(env.clone(), user.clone());
        if reviews.is_empty() {
            return Ok(0);
        }
        
        let current_time = env.ledger().timestamp();
        let mut total_score: u64 = 0;
        let mut total_weight: u64 = 0;
        
        for review in reviews.iter() {
            let effective_weight = Self::get_effective_weight(env.clone(), review.clone(), current_time);
            let weight = if effective_weight > 0 {
                effective_weight as u64
            } else {
                0
            };
            total_score += (review.rating as u64) * weight;
            total_weight += weight;
        }
        
        if total_weight == 0 {
            return Ok(0); // If completely decayed, acts as no rep
        }
        
        Ok((total_score * 100) / total_weight)
    }

    /// Get the total number of reviews for a user.
    pub fn get_review_count(env: Env, user: Address) -> u32 {
        let rep_key = DataKey::Reputation(user);
        let reputation: Option<UserReputation> = env.storage().persistent().get(&rep_key);
        match reputation {
            Some(rep) => {
                bump_reputation_ttl(&env, &rep.user);
                rep.review_count
            }
            None => 0,
        }
    }

    /// Get all reviews for a user.
    pub fn get_reviews(env: Env, user: Address) -> Vec<Review> {
        let reviews_key = DataKey::Reviews(user);
        let reviews: Option<Vec<Review>> = env.storage().persistent().get(&reviews_key);
        match reviews {
            Some(list) => {
                env.storage().persistent().extend_ttl(
                    &reviews_key,
                    MIN_TTL_THRESHOLD,
                    MIN_TTL_EXTEND_TO,
                );
                list
            }
            None => Vec::new(&env),
        }
    }

    /// Get the reputation tier for a user based on their average rating.
    pub fn get_tier(env: Env, user: Address) -> ReputationTier {
        match Self::get_average_rating(env, user) {
            Ok(avg_rating) => calculate_tier(avg_rating),
            Err(_) => ReputationTier::None,
        }
    }

    /// Get all badges awarded to a user.
    pub fn get_badges(env: Env, user: Address) -> Vec<Badge> {
        let badges_key = DataKey::Badges(user);
        let badges: Option<Vec<Badge>> = env.storage().persistent().get(&badges_key);
        match badges {
            Some(list) => {
                env.storage().persistent().extend_ttl(
                    &badges_key,
                    MIN_TTL_THRESHOLD,
                    MIN_TTL_EXTEND_TO,
                );
                list
            }
            None => Vec::new(&env),
        }
    }
}

#[cfg(test)]
mod test;
