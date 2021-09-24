// This file is part of Substrate.

// Copyright (C) 2017-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Staking Pallet
//!
//! The Staking pallet is used to manage funds at stake by network maintainers.
//!
//! - [`Config`]
//! - [`Call`]
//! - [`Pallet`]
//!
//! ## Overview
//!
//! The Staking pallet is the means by which a set of network maintainers (known as _authorities_ in
//! some contexts and _validators_ in others) are chosen based upon those who voluntarily place
//! funds under deposit. Under deposit, those funds are rewarded under normal operation but are held
//! at pain of _slash_ (expropriation) should the staked maintainer be found not to be discharging
//! its duties properly.
//!
//! ### Terminology
//! <!-- Original author of paragraph: @gavofyork -->
//!
//! - Staking: The process of locking up funds for some time, placing them at risk of slashing
//!   (loss) in order to become a rewarded maintainer of the network.
//! - Validating: The process of running a node to actively maintain the network, either by
//!   producing blocks or nominationsing finality of the chain.
//! - Nominating: The process of placing staked funds behind one or more validators in order to
//!   share in any reward, and punishment, they take.
//! - Stash account: The account holding an owner's funds used for staking.
//! - Controller account: The account that controls an owner's funds for staking.
//! - Era: A (whole) number of sessions, which is the period that the validator set (and each
//!   validator's active nominator set) is recalculated and where rewards are paid out.
//! - Slash: The punishment of a staker by reducing its funds.
//!
//! ### Goals
//! <!-- Original author of paragraph: @gavofyork -->
//!
//! The staking system in Substrate NPoS is designed to make the following possible:
//!
//! - Stake funds that are controlled by a cold wallet.
//! - Withdraw some, or deposit more, funds without interrupting the role of an entity.
//! - Switch between roles (nominator, validator, idle) with minimal overhead.
//!
//! ### Scenarios
//!
//! #### Staking
//!
//! Almost any interaction with the Staking pallet requires a process of _**bonding**_ (also known
//! as being a _staker_). To become *bonded*, a fund-holding account known as the _stash account_,
//! which holds some or all of the funds that become frozen in place as part of the staking process,
//! is paired with an active **controller** account, which issues instructions on how they shall be
//! used.
//!
//! An account pair can become bonded using the [`bond`](Call::bond) call.
//!
//! Stash accounts can change their associated controller using the
//! [`set_controller`](Call::set_controller) call.
//!
//! There are three possible roles that any staked account pair can be in: `Validator`, `Nominator`
//! and `Idle` (defined in [`StakerStatus`]). There are three
//! corresponding instructions to change between roles, namely:
//! [`validate`](Call::validate),
//! [`nominate`](Call::nominate), and [`chill`](Call::chill).
//!
//! #### Validating
//!
//! A **validator** takes the role of either validating blocks or ensuring their finality,
//! maintaining the veracity of the network. A validator should avoid both any sort of malicious
//! misbehavior and going offline. Bonded accounts that state interest in being a validator do NOT
//! get immediately chosen as a validator. Instead, they are declared as a _candidate_ and they
//! _might_ get elected at the _next era_ as a validator. The result of the election is determined
//! by nominators and their votes.
//!
//! An account can become a validator candidate via the
//! [`validate`](Call::validate) call.
//!
//! #### Nomination
//!
//! A **nominator** does not take any _direct_ role in maintaining the network, instead, it votes on
//! a set of validators  to be elected. Once interest in nomination is stated by an account, it
//! takes effect at the next election round. The funds in the nominator's stash account indicate the
//! _weight_ of its vote. Both the rewards and any punishment that a validator earns are shared
//! between the validator and its nominators. This rule incentivizes the nominators to NOT vote for
//! the misbehaving/offline validators as much as possible, simply because the nominators will also
//! lose funds if they vote poorly.
//!
//! An account can become a nominator via the [`nominate`](Call::nominate) call.
//!
//! #### Rewards and Slash
//!
//! The **reward and slashing** procedure is the core of the Staking pallet, attempting to _embrace
//! valid behavior_ while _punishing any misbehavior or lack of availability_.
//!
//! Rewards must be claimed for each era before it gets too old by `$HISTORY_DEPTH` using the
//! `payout_stakers` call. Any account can call `payout_stakers`, which pays the reward to the
//! validator as well as its nominators. Only the [`Config::MaxNominatorRewardedPerValidator`]
//! biggest stakers can claim their reward. This is to limit the i/o cost to mutate storage for each
//! nominator's account.
//!
//! Slashing can occur at any point in time, once misbehavior is reported. Once slashing is
//! determined, a value is deducted from the balance of the validator and all the nominators who
//! voted for this validator (values are deducted from the _stash_ account of the slashed entity).
//!
//! Slashing logic is further described in the documentation of the `slashing` pallet.
//!
//! Similar to slashing, rewards are also shared among a validator and its associated nominators.
//! Yet, the reward funds are not always transferred to the stash account and can be configured. See
//! [Reward Calculation](#reward-calculation) for more details.
//!
//! #### Chilling
//!
//! Finally, any of the roles above can choose to step back temporarily and just chill for a while.
//! This means that if they are a nominator, they will not be considered as voters anymore and if
//! they are validators, they will no longer be a candidate for the next election.
//!
//! An account can step back via the [`chill`](Call::chill) call.
//!
//! ### Session managing
//!
//! The pallet implement the trait `SessionManager`. Which is the only API to query new validator
//! set and allowing these validator set to be rewarded once their era is ended.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! The dispatchable functions of the Staking pallet enable the steps needed for entities to accept
//! and change their role, alongside some helper functions to get/set the metadata of the pallet.
//!
//! ### Public Functions
//!
//! The Staking pallet contains many public storage items and (im)mutable functions.
//!
//! ## Usage
//!
//! ### Example: Rewarding a validator by id.
//!
//! ```
//! use frame_support::{decl_module, dispatch};
//! use frame_system::ensure_signed;
//! use pallet_staking::{self as staking};
//!
//! pub trait Config: staking::Config {}
//!
//! decl_module! {
//!     pub struct Module<T: Config> for enum Call where origin: T::Origin {
//!         /// Reward a validator.
//!         #[weight = 0]
//!         pub fn reward_myself(origin) -> dispatch::DispatchResult {
//!             let reported = ensure_signed(origin)?;
//!             <staking::Pallet<T>>::reward_by_ids(vec![(reported, 10)]);
//!             Ok(())
//!         }
//!     }
//! }
//! # fn main() { }
//! ```
//!
//! ## Implementation Details
//!
//! ### Era payout
//!
//! The era payout is computed using yearly inflation curve defined at
//! [`Config::EraPayout`] as such:
//!
//! ```nocompile
//! staker_payout = yearly_inflation(npos_token_staked / total_tokens) * total_tokens / era_per_year
//! ```
//! This payout is used to reward stakers as defined in next section
//!
//! ```nocompile
//! remaining_payout = max_yearly_inflation * total_tokens / era_per_year - staker_payout
//! ```
//! The remaining reward is send to the configurable end-point
//! [`Config::RewardRemainder`].
//!
//! ### Reward Calculation
//!
//! Validators and nominators are rewarded at the end of each era. The total reward of an era is
//! calculated using the era duration and the staking rate (the total amount of tokens staked by
//! nominators and validators, divided by the total token supply). It aims to incentivize toward a
//! defined staking rate. The full specification can be found
//! [here](https://research.web3.foundation/en/latest/polkadot/Token%20Economics.html#inflation-model).
//!
//! Total reward is split among validators and their nominators depending on the number of points
//! they received during the era. Points are added to a validator using
//! [`reward_by_ids`](Pallet::reward_by_ids).
//!
//! [`Pallet`] implements
//! [`pallet_authorship::EventHandler`] to add reward
//! points to block producer and block producer of referenced uncles.
//!
//! The validator and its nominator split their reward as following:
//!
//! The validator can declare an amount, named
//! [`commission`](ValidatorPrefs::commission), that does not get shared
//! with the nominators at each reward payout through its
//! [`ValidatorPrefs`]. This value gets deducted from the total reward
//! that is paid to the validator and its nominators. The remaining portion is split among the
//! validator and all of the nominators that nominated the validator, proportional to the value
//! staked behind this validator (_i.e._ dividing the
//! [`own`](Exposure::own) or
//! [`others`](Exposure::others) by
//! [`total`](Exposure::total) in [`Exposure`]).
//!
//! All entities who receive a reward have the option to choose their reward destination through the
//! [`Payee`] storage item (see
//! [`set_payee`](Call::set_payee)), to be one of the following:
//!
//! - Controller account, (obviously) not increasing the staked value.
//! - Stash account, not increasing the staked value.
//! - Stash account, also increasing the staked value.
//!
//! ### Additional Fund Management Operations
//!
//! Any funds already placed into stash can be the target of the following operations:
//!
//! The controller account can free a portion (or all) of the funds using the
//! [`unbond`](Call::unbond) call. Note that the funds are not immediately
//! accessible. Instead, a duration denoted by
//! [`Config::BondingDuration`] (in number of eras) must
//! pass until the funds can actually be removed. Once the `BondingDuration` is over, the
//! [`withdraw_unbonded`](Call::withdraw_unbonded) call can be used to actually
//! withdraw the funds.
//!
//! Note that there is a limitation to the number of fund-chunks that can be scheduled to be
//! unlocked in the future via [`unbond`](Call::unbond). In case this maximum
//! (`MAX_UNLOCKING_CHUNKS`) is reached, the bonded account _must_ first wait until a successful
//! call to `withdraw_unbonded` to remove some of the chunks.
//!
//! ### Election Algorithm
//!
//! The current election algorithm is implemented based on Phragmén. The reference implementation
//! can be found [here](https://github.com/w3f/consensus/tree/master/NPoS).
//!
//! The election algorithm, aside from electing the validators with the most stake value and votes,
//! tries to divide the nominator votes among candidates in an equal manner. To further assure this,
//! an optional post-processing can be applied that iteratively normalizes the nominator staked
//! values until the total difference among votes of a particular nominator are less than a
//! threshold.
//!
//! ## GenesisConfig
//!
//! The Staking pallet depends on the [`GenesisConfig`]. The
//! `GenesisConfig` is optional and allow to set some initial stakers.
//!
//! ## Related Modules
//!
//! - [Balances](../pallet_balances/index.html): Used to manage values at stake.
//! - [Session](../pallet_session/index.html): Used to manage sessions. Also, a list of new
//!   validators is stored in the Session pallet's `Validators` at the end of each era.

#![recursion_limit = "128"]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
#[cfg(any(feature = "runtime-benchmarks", test))]
pub mod testing_utils;
#[cfg(any(feature = "runtime-benchmarks", test))]
pub mod benchmarking;

pub mod slashing;
pub mod inflation;
pub mod weights;

use sp_std::{
    prelude::*,
    collections::btree_map::BTreeMap,
    convert::*,
};
use codec::{HasCompact, Encode, Decode};
use frame_support::{
    pallet_prelude::*,
    weights::{
        Weight,
        constants::{WEIGHT_PER_MICROS, WEIGHT_PER_NANOS},
    },
    traits::{
        Currency, LockIdentifier, LockableCurrency, WithdrawReasons, OnUnbalanced, Imbalance, Get,
        UnixTime, EstimateNextNewSession, EnsureOrigin,
    },
};
use pallet_session::historical;
use sp_runtime::{
    Percent, Perbill, Permill, RuntimeDebug,
    traits::{
        Convert, Zero, StaticLookup, CheckedSub, Saturating, SaturatedConversion,
        AtLeast32BitUnsigned, One, CheckedAdd,
    },
};
use sp_staking::{
    SessionIndex,
    offence::{OnOffenceHandler, OffenceDetails, Offence, ReportOffence, OffenceError},
};
use frame_system::{
    ensure_signed, ensure_root, pallet_prelude::*,
    offchain::SendTransactionTypes,
};
pub use weights::WeightInfo;
pub use pallet::*;
use primitives::{
    constants::{
        time::*,
        currency::*,
        staking::*,
    },
    p_benefit::BenefitInterface,
};
use primitives::p_payment::PaymentInterface;

pub mod total_stake_limit_ratio;

use total_stake_limit_ratio::total_stake_limit_ratio;

const STAKING_ID: LockIdentifier = *b"staking ";
const MAX_NOMINATIONS: usize = 16;

pub(crate) const LOG_TARGET: &'static str = "runtime::staking";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
	($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		log::$level!(
			target: crate::LOG_TARGET,
			concat!("[{:?}] 💸 ", $patter), <frame_system::Pallet<T>>::block_number() $(, $values)*
		)
	};
}

pub const MAX_UNLOCKING_CHUNKS: usize = 32;

/// Counter for the number of eras that have passed.
pub type EraIndex = u32;

/// Counter for the number of "reward" points earned by a given validator.
pub type RewardPoint = u32;

/// The balance type of this pallet.
pub type BalanceOf<T> =
<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

type PositiveImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::PositiveImbalance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

/// Information regarding the active era (era in used in session).
#[derive(Encode, Decode, RuntimeDebug)]
pub struct ActiveEraInfo {
    /// Index of era.
    pub index: EraIndex,
    /// Moment of start expressed as millisecond from `$UNIX_EPOCH`.
    ///
    /// Start can be none if start hasn't been set for the era yet,
    /// Start is set on the first on_finalize of the era to nominations usage of `Time`.
    start: Option<u64>,
}

/// Reward points of an era. Used to split era total payout between validators.
///
/// This points will be used to reward validators and their respective nominators.
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug)]
pub struct EraRewardPoints<AccountId: Ord> {
    /// Total number of points. Equals the sum of reward points for each validator.
    total: RewardPoint,
    /// The reward points earned by a given validator.
    individual: BTreeMap<AccountId, RewardPoint>,
}

/// Indicates the initial status of the staker.
#[derive(RuntimeDebug)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub enum StakerStatus<AccountId, Balance: HasCompact> {
    /// Chilling.
    Idle,
    /// Declared desire in validating or already participating in it.
    Validator,
    /// Nominating for a group of other stakers.
    Nominator(Vec<(AccountId, Balance)>),
}

/// A destination account for payment.
#[derive(PartialEq, Eq, Copy, Clone, Encode, Decode, RuntimeDebug)]
pub enum RewardDestination<AccountId> {
    /// Pay into the stash account, increasing the amount at stake accordingly.
    Staked,
    /// Pay into the stash account, not increasing the amount at stake.
    Stash,
    /// Pay into the controller account.
    Controller,
    /// Pay into a specified account.
    Account(AccountId),
    /// Receive no reward.
    None,
}

impl<AccountId> Default for RewardDestination<AccountId> {
    fn default() -> Self {
        RewardDestination::Staked
    }
}

/// Preference of what happens regarding validation.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct ValidatorPrefs {
    /// Reward that validator takes up-front; only the rest is split between themselves and
    /// nominators.
    #[codec(compact)]
    pub commission: Perbill,
    /// Whether or not this validator is accepting more nominations. If `true`, then no nominator
    /// who is not already nominating this validator may nominate them. By default, validators
    /// are accepting nominations.
    pub blocked: bool,
}

impl Default for ValidatorPrefs {
    fn default() -> Self {
        ValidatorPrefs {
            commission: Default::default(),
            blocked: false,
        }
    }
}

/// Just a Balance/BlockNumber tuple to encode when a chunk of funds will be unlocked.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct UnlockChunk<Balance: HasCompact> {
    /// Amount of funds to be unlocked.
    #[codec(compact)]
    value: Balance,
    /// Era number at which point it'll be unlocked.
    #[codec(compact)]
    era: EraIndex,
}

/// The ledger of a (bonded) stash.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct StakingLedger<AccountId, Balance: HasCompact> {
    /// The stash account whose balance is actually locked and at stake.
    pub stash: AccountId,
    /// The total amount of the stash's balance that we are currently accounting for.
    /// It's just `active` plus all the `unlocking` balances.
    #[codec(compact)]
    pub total: Balance,
    /// The total amount of the stash's balance that will be at stake in any forthcoming
    /// rounds.
    #[codec(compact)]
    pub active: Balance,
    /// Any balance that is becoming free, which may eventually be transferred out
    /// of the stash (assuming it doesn't get slashed first).
    pub unlocking: Vec<UnlockChunk<Balance>>,
    /// List of eras for which the stakers behind a validator have claimed rewards. Only updated
    /// for validators.
    pub claimed_rewards: Vec<EraIndex>,
}

impl<
    AccountId,
    Balance: HasCompact + Copy + Saturating + AtLeast32BitUnsigned,
> StakingLedger<AccountId, Balance> {
    /// Remove entries from `unlocking` that are sufficiently old and reduce the
    /// total by the sum of their balances.
    fn consolidate_unlocked(self, current_era: EraIndex) -> Self {
        let mut total = self.total;
        let unlocking = self.unlocking.into_iter()
            .filter(|chunk| if chunk.era > current_era {
                true
            } else {
                total = total.saturating_sub(chunk.value);
                false
            })
            .collect();

        Self {
            stash: self.stash,
            total,
            active: self.active,
            unlocking,
            claimed_rewards: self.claimed_rewards,
        }
    }

    /// Re-bond funds that were scheduled for unlocking.
    fn rebond(mut self, value: Balance) -> Self {
        let mut unlocking_balance: Balance = Zero::zero();

        while let Some(last) = self.unlocking.last_mut() {
            if unlocking_balance + last.value <= value {
                unlocking_balance += last.value;
                self.active += last.value;
                self.unlocking.pop();
            } else {
                let diff = value - unlocking_balance;

                unlocking_balance += diff;
                unlocking_balance += diff;
                self.active += diff;
                last.value -= diff;
            }

            if unlocking_balance >= value {
                break;
            }
        }

        self
    }
}

impl<AccountId, Balance> StakingLedger<AccountId, Balance> where
    Balance: AtLeast32BitUnsigned + Saturating + Copy,
{
    /// Slash the validator for a given amount of balance. This can grow the value
    /// of the slash in the case that the validator has less than `minimum_balance`
    /// active funds. Returns the amount of funds actually slashed.
    ///
    /// Slashes from `active` funds first, and then `unlocking`, starting with the
    /// chunks that are closest to unlocking.
    fn slash(
        &mut self,
        mut value: Balance,
        minimum_balance: Balance,
    ) -> Balance {
        let pre_total = self.total;
        let total = &mut self.total;
        let active = &mut self.active;

        let slash_out_of = |total_remaining: &mut Balance,
                            target: &mut Balance,
                            value: &mut Balance,
        | {
            let mut slash_from_target = (*value).min(*target);

            if !slash_from_target.is_zero() {
                *target -= slash_from_target;

                // Don't leave a dust balance in the staking system.
                if *target <= minimum_balance {
                    slash_from_target += *target;
                    *value += sp_std::mem::replace(target, Zero::zero());
                }

                *total_remaining = total_remaining.saturating_sub(slash_from_target);
                *value -= slash_from_target;
            }
        };

        slash_out_of(total, active, &mut value);

        let i = self.unlocking.iter_mut()
            .map(|chunk| {
                slash_out_of(total, &mut chunk.value, &mut value);
                chunk.value
            })
            .take_while(|value| value.is_zero()) // Take all fully-consumed chunks out.
            .count();

        // Kill all drained chunks.
        let _ = self.unlocking.drain(..i);

        pre_total.saturating_sub(*total)
    }
}

/// A record of the nominations made by a specific account.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct Nominations<AccountId, Balance: HasCompact> {
    /// The targets of nomination.
    pub targets: Vec<IndividualExposure<AccountId, Balance>>,
    /// The total votes of nominations.
    pub total: Balance,
    /// The era the nominations were submitted.
    ///
    /// Except for initial nominations which are considered submitted at era 0.
    pub submitted_in: EraIndex,
    /// Whether the nominations have been suppressed. This can happen due to slashing of the
    /// validators, or other events that might invalidate the nomination.
    ///
    /// NOTE: this for future proofing and is thus far not used.
    pub suppressed: bool,
}

/// The amount of exposure (to slashing) than an individual nominator has.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, RuntimeDebug)]
pub struct IndividualExposure<AccountId, Balance: HasCompact> {
    /// The stash account of the nominator in question.
    pub who: AccountId,
    /// Amount of funds exposed.
    pub value: Balance,
}

/// A snapshot of the stake backing a single validator in the system.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct Exposure<AccountId, Balance: HasCompact> {
    /// The total balance backing this validator.
    #[codec(compact)]
    pub total: Balance,
    /// The validator's own stash that is exposed.
    #[codec(compact)]
    pub own: Balance,
    /// The portions of nominators stashes that are exposed.
    pub others: Vec<IndividualExposure<AccountId, Balance>>,
}

/// A pending slash record. The value of the slash has been computed but not applied yet,
/// rather deferred for several eras.
#[derive(Encode, Decode, Default, RuntimeDebug)]
pub struct UnappliedSlash<AccountId, Balance: HasCompact> {
    /// The stash ID of the offending validator.
    validator: AccountId,
    /// The validator's own slash.
    own: Balance,
    /// All other slashed stakers and amounts.
    others: Vec<(AccountId, Balance)>,
    /// Reporters of the offence; bounty payout recipients.
    reporters: Vec<AccountId>,
    /// The amount of payout.
    payout: Balance,
}

/// Means for interacting with a specialized version of the `session` trait.
///
/// This is needed because `Staking` sets the `ValidatorIdOf` of the `pallet_session::Config`
pub trait SessionInterface<AccountId>: frame_system::Config {
    /// Disable a given validator by stash ID.
    ///
    /// Returns `true` if new era should be forced at the end of this session.
    /// This allows preventing a situation where there is too many validators
    /// disabled and block production stalls.
    fn disable_validator(validator: &AccountId) -> Result<bool, ()>;
    /// Get the validators from session.
    fn validators() -> Vec<AccountId>;
    /// Prune historical session tries up to but not including the given index.
    fn prune_historical_up_to(up_to: SessionIndex);
}

impl<T: Config> SessionInterface<<T as frame_system::Config>::AccountId> for T where
    T: pallet_session::Config<ValidatorId=<T as frame_system::Config>::AccountId>,
    T: pallet_session::historical::Config<
        FullIdentification=Exposure<<T as frame_system::Config>::AccountId, BalanceOf<T>>,
        FullIdentificationOf=ExposureOf<T>,
    >,
    T::SessionHandler: pallet_session::SessionHandler<<T as frame_system::Config>::AccountId>,
    T::SessionManager: pallet_session::SessionManager<<T as frame_system::Config>::AccountId>,
    T::ValidatorIdOf:
    Convert<<T as frame_system::Config>::AccountId, Option<<T as frame_system::Config>::AccountId>>,
{
    fn disable_validator(validator: &<T as frame_system::Config>::AccountId) -> Result<bool, ()> {
        <pallet_session::Pallet<T>>::disable(validator)
    }

    fn validators() -> Vec<<T as frame_system::Config>::AccountId> {
        <pallet_session::Pallet<T>>::validators()
    }

    fn prune_historical_up_to(up_to: SessionIndex) {
        <pallet_session::historical::Pallet<T>>::prune_up_to(up_to);
    }
}


/// Mode of era-forcing.
#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub enum Forcing {
    /// Not forcing anything - just let whatever happen.
    NotForcing,
    /// Force a new era, then reset to `NotForcing` as soon as it is done.
    /// Note that this will force to trigger an election until a new era is triggered, if the
    /// election failed, the next session end will trigger a new election again, until success.
    ForceNew,
    /// Avoid a new era indefinitely.
    ForceNone,
    /// Force a new era at the end of all sessions indefinitely.
    ForceAlways,
}

impl Default for Forcing {
    fn default() -> Self {
        Forcing::NotForcing
    }
}

// A value placed in storage that represents the current version of the Staking storage. This value
// is used by the `on_runtime_upgrade` logic to determine whether we run storage migration logic.
// This should match directly with the semantic versions of the Rust crate.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug)]
enum Releases {
    V1_0_0Ancient,
    V2_0_0,
    V3_0_0,
    V4_0_0,
    V5_0_0,
    // blockable validators.
    V6_0_0,
    // removal of all storage associated with offchain phragmen.
    V7_0_0, // keep track of number of nominators / validators in map
}

impl Default for Releases {
    fn default() -> Self {
        Releases::V7_0_0
    }
}

pub mod migrations {
    use super::*;

    pub mod v7 {
        use super::*;

        pub fn pre_migrate<T: Config>() -> Result<(), &'static str> {
            assert!(CounterForValidators::<T>::get().is_zero(), "CounterForValidators already set.");
            assert!(CounterForNominators::<T>::get().is_zero(), "CounterForNominators already set.");
            assert!(StorageVersion::<T>::get() == Releases::V6_0_0);
            Ok(())
        }

        pub fn migrate<T: Config>() -> Weight {
            log!(info, "Migrating staking to Releases::V7_0_0");
            let validator_count = Validators::<T>::iter().count() as u32;
            let nominator_count = Nominators::<T>::iter().count() as u32;

            CounterForValidators::<T>::put(validator_count);
            CounterForNominators::<T>::put(nominator_count);

            StorageVersion::<T>::put(Releases::V7_0_0);
            log!(info, "Completed staking migration to Releases::V7_0_0");

            T::DbWeight::get().reads_writes(
                validator_count.saturating_add(nominator_count).into(),
                2,
            )
        }
    }

    pub mod v6 {
        use super::*;
        use frame_support::{traits::Get, weights::Weight, generate_storage_alias};

        // NOTE: value type doesn't matter, we just set it to () here.
        generate_storage_alias!(Staking, SnapshotValidators => Value<()>);
        generate_storage_alias!(Staking, SnapshotNominators => Value<()>);
        generate_storage_alias!(Staking, QueuedElected => Value<()>);
        generate_storage_alias!(Staking, QueuedScore => Value<()>);
        generate_storage_alias!(Staking, EraElectionStatus => Value<()>);
        generate_storage_alias!(Staking, IsCurrentSessionFinal => Value<()>);

        /// check to execute prior to migration.
        pub fn pre_migrate<T: Config>() -> Result<(), &'static str> {
            // these may or may not exist.
            log!(info, "SnapshotValidators.exits()? {:?}", SnapshotValidators::exists());
            log!(info, "SnapshotNominators.exits()? {:?}", SnapshotNominators::exists());
            log!(info, "QueuedElected.exits()? {:?}", QueuedElected::exists());
            log!(info, "QueuedScore.exits()? {:?}", QueuedScore::exists());
            // these must exist.
            assert!(IsCurrentSessionFinal::exists(), "IsCurrentSessionFinal storage item not found!");
            assert!(EraElectionStatus::exists(), "EraElectionStatus storage item not found!");
            Ok(())
        }

        /// Migrate storage to v6.
        pub fn migrate<T: Config>() -> Weight {
            log!(info, "Migrating staking to Releases::V6_0_0");

            SnapshotValidators::kill();
            SnapshotNominators::kill();
            QueuedElected::kill();
            QueuedScore::kill();
            EraElectionStatus::kill();
            IsCurrentSessionFinal::kill();

            StorageVersion::<T>::put(Releases::V6_0_0);
            log!(info, "Done.");
            T::DbWeight::get().writes(6 + 1)
        }
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + SendTransactionTypes<Call<Self>> {
        /// The staking balance.
        type Currency: LockableCurrency<Self::AccountId, Moment=Self::BlockNumber>;

        /// Time used for computing era duration.
        ///
        /// It is nominationsd to start being called from the first `on_finalize`. Thus value at genesis
        /// is not used.
        type UnixTime: UnixTime;

        /// Convert a balance into a number used for election calculation. This must fit into a `u64`
        /// but is allowed to be sensibly lossy. The `u64` is used to communicate with the
        /// [`sp_npos_elections`] crate which accepts u64 numbers and does operations in 128.
        /// Consequently, the backward convert is used convert the u128s from sp-elections back to a
        /// [`BalanceOf`].
        type CurrencyToVote: Convert<u128, BalanceOf<Self>> + Convert<BalanceOf<Self>, u128> + Convert<BalanceOf<Self>, u64>;

        /// Maximum number of nominations per nominator.
        const MAX_NOMINATIONS: u32;

        /// Tokens have been minted and are unused for validator-reward.
        /// See [Era payout](./index.html#era-payout).
        type RewardRemainder: OnUnbalanced<NegativeImbalanceOf<Self>>;

        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Handler for the unbalanced reduction when slashing a staker.
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;

        /// Handler for the unbalanced increment when rewarding a staker.
        type Reward: OnUnbalanced<PositiveImbalanceOf<Self>>;

        /// Number of sessions per era.
        #[pallet::constant]
        type SessionsPerEra: Get<SessionIndex>;

        /// Number of eras that staked funds must remain bonded for.
        #[pallet::constant]
        type BondingDuration: Get<EraIndex>;

        /// Number of eras that slashes are deferred by, after computation.
        ///
        /// This should be less than the bonding duration. Set to 0 if slashes
        /// should be applied immediately, without opportunity for intervention.
        #[pallet::constant]
        type SlashDeferDuration: Get<EraIndex>;

        /// The origin which can cancel a deferred slash. Root can always do this.
        type SlashCancelOrigin: EnsureOrigin<Self::Origin>;

        /// Interface for interacting with a session pallet.
        type SessionInterface: self::SessionInterface<Self::AccountId>;

        /// Something that can estimate the next session change, accurately or as a best effort guess.
        type NextNewSession: EstimateNextNewSession<Self::BlockNumber>;

        /// The maximum number of nominators rewarded for each validator.
        ///
        /// For each validator only the `$MaxNominatorRewardedPerValidator` biggest stakers can claim
        /// their reward. This used to limit the i/o cost for the nominator payout.
        #[pallet::constant]
        type MaxNominatorRewardedPerValidator: Get<u32>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;

        /// Storage power ratio for crust network phase 1
        type SPowerRatio: Get<u128>;

        /// Reference to Market staking pot.
        type PaymentInterface: PaymentInterface<AccountId=Self::AccountId, BlockNumber=Self::BlockNumber, Balance=BalanceOf<Self>>;

        /// Market Staking Pot Duration. Count of EraIndex
        type MarketStakingPotDuration: Get<u32>;

        /// Fee reduction interface
        type BenefitInterface: BenefitInterface<Self::AccountId, BalanceOf<Self>, NegativeImbalanceOf<Self>>;
    }

    #[pallet::extra_constants]
    impl<T: Config> Pallet<T> {
        //TODO: rename to snake case after https://github.com/paritytech/substrate/issues/8826 fixed.
        #[allow(non_snake_case)]
        fn MaxNominations() -> u32 {
            T::MAX_NOMINATIONS
        }
    }

    #[pallet::type_value]
    pub(crate) fn HistoryDepthOnEmpty() -> u32 { 84u32 }


    #[pallet::type_value]
    pub fn DefaultForStartRewardEra() -> EraIndex { 1 }
    //todo:: 测试网 主网上线后修改
    //pub fn DefaultForStartRewardEra() -> EraIndex { 10000 }


    /// Number of eras to keep in history.
    ///
    /// Information is kept for eras in `[current_era - history_depth; current_era]`.
    ///
    /// Must be more than the number of eras delayed by session otherwise. I.e. active era must
    /// always be in history. I.e. `active_era > current_era - history_depth` must be
    /// nominationsd.
    #[pallet::storage]
    #[pallet::getter(fn history_depth)]
    pub(crate) type HistoryDepth<T> = StorageValue<_, u32, ValueQuery, HistoryDepthOnEmpty>;

    /// Start era for reward curve
    #[pallet::storage]
    #[pallet::getter(fn start_reward_era)]
    pub type StartRewardEra<T> = StorageValue<_, EraIndex, ValueQuery, DefaultForStartRewardEra>;


    /// The ideal number of staking participants.
    #[pallet::storage]
    #[pallet::getter(fn validator_count)]
    pub type ValidatorCount<T> = StorageValue<_, u32, ValueQuery>;

    /// Minimum number of staking participants before emergency conditions are imposed.
    #[pallet::storage]
    #[pallet::getter(fn minimum_validator_count)]
    pub type MinimumValidatorCount<T> = StorageValue<_, u32, ValueQuery>;

    /// Any validators that may never be slashed or forcibly kicked. It's a Vec since they're
    /// easy to initialize and the performance hit is minimal (we expect no more than four
    /// invulnerables) and restricted to testnets.
    #[pallet::storage]
    #[pallet::getter(fn invulnerables)]
    pub type Invulnerables<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    /// Map from all locked "stash" accounts to the controller account.
    #[pallet::storage]
    #[pallet::getter(fn bonded)]
    pub type Bonded<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, T::AccountId>;

    /// The minimum active bond to become and maintain the role of a nominator.
    #[pallet::storage]
    pub type MinNominatorBond<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// The minimum active bond to become and maintain the role of a validator.
    #[pallet::storage]
    pub type MinValidatorBond<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// Map from all (unlocked) "controller" accounts to the info regarding the staking.
    #[pallet::storage]
    #[pallet::getter(fn ledger)]
    pub type Ledger<T: Config> = StorageMap<
        _,
        Blake2_128Concat, T::AccountId,
        StakingLedger<T::AccountId, BalanceOf<T>>,
    >;

    /// Where the reward payment should be made. Keyed by stash.
    #[pallet::storage]
    #[pallet::getter(fn payee)]
    pub type Payee<T: Config> = StorageMap<
        _,
        Twox64Concat, T::AccountId,
        RewardDestination<T::AccountId>,
        ValueQuery,
    >;

    /// The map from (wannabe) validator stash key to the preferences of that validator.
    ///
    /// When updating this storage item, you must also update the `CounterForValidators`.
    #[pallet::storage]
    #[pallet::getter(fn validators)]
    pub type Validators<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, ValidatorPrefs, ValueQuery>;

    /// A tracker to keep count of the number of items in the `Validators` map.
    #[pallet::storage]
    pub type CounterForValidators<T> = StorageValue<_, u32, ValueQuery>;

    /// The maximum validator count before we stop allowing new validators to join.
    ///
    /// When this value is not set, no limits are enforced.
    #[pallet::storage]
    pub type MaxValidatorsCount<T> = StorageValue<_, u32, OptionQuery>;

    /// The map from nominator stash key to the set of stash keys of all validators to nominate.
    ///
    /// When updating this storage item, you must also update the `CounterForNominators`.
    #[pallet::storage]
    #[pallet::getter(fn nominators)]
    pub type Nominators<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, Nominations<T::AccountId, BalanceOf<T>>>;

    /// A tracker to keep count of the number of items in the `Nominators` map.
    #[pallet::storage]
    pub type CounterForNominators<T> = StorageValue<_, u32, ValueQuery>;

    /// The maximum nominator count before we stop allowing new validators to join.
    ///
    /// When this value is not set, no limits are enforced.
    #[pallet::storage]
    pub type MaxNominatorsCount<T> = StorageValue<_, u32, OptionQuery>;

    /// The current era index.
    ///
    /// This is the latest planned era, depending on how the Session pallet queues the validator
    /// set, it might be active or not.
    #[pallet::storage]
    #[pallet::getter(fn current_era)]
    pub type CurrentEra<T> = StorageValue<_, EraIndex>;

    /// The active era information, it holds index and start.
    ///
    /// The active era is the era being currently rewarded. Validator set of this era must be
    /// equal to [`SessionInterface::validators`].
    #[pallet::storage]
    #[pallet::getter(fn active_era)]
    pub type ActiveEra<T> = StorageValue<_, ActiveEraInfo>;

    /// The session index at which the era start for the last `HISTORY_DEPTH` eras.
    ///
    /// Note: This tracks the starting session (i.e. session index when era start being active)
    /// for the eras in `[CurrentEra - HISTORY_DEPTH, CurrentEra]`.
    #[pallet::storage]
    #[pallet::getter(fn eras_start_session_index)]
    pub type ErasStartSessionIndex<T> = StorageMap<_, Twox64Concat, EraIndex, SessionIndex>;

    /// Exposure of validator at era.
    ///
    /// This is keyed first by the era index to allow bulk deletion and then the stash account.
    ///
    /// Is it removed after `HISTORY_DEPTH` eras.
    /// If stakers hasn't been set or has been removed then empty exposure is returned.
    #[pallet::storage]
    #[pallet::getter(fn eras_stakers)]
    pub type ErasStakers<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat, EraIndex,
        Twox64Concat, T::AccountId,
        Exposure<T::AccountId, BalanceOf<T>>,
        ValueQuery,
    >;

    /// Clipped Exposure of validator at era.
    ///
    /// This is similar to [`ErasStakers`] but number of nominators exposed is reduced to the
    /// `T::MaxNominatorRewardedPerValidator` biggest stakers.
    /// (Note: the field `total` and `own` of the exposure remains unchanged).
    /// This is used to limit the i/o cost for the nominator payout.
    ///
    /// This is keyed fist by the era index to allow bulk deletion and then the stash account.
    ///
    /// Is it removed after `HISTORY_DEPTH` eras.
    /// If stakers hasn't been set or has been removed then empty exposure is returned.
    #[pallet::storage]
    #[pallet::getter(fn eras_stakers_clipped)]
    pub type ErasStakersClipped<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat, EraIndex,
        Twox64Concat, T::AccountId,
        Exposure<T::AccountId, BalanceOf<T>>,
        ValueQuery,
    >;

    /// Similar to `ErasStakers`, this holds the preferences of validators.
    ///
    /// This is keyed first by the era index to allow bulk deletion and then the stash account.
    ///
    /// Is it removed after `HISTORY_DEPTH` eras.
    // If prefs hasn't been set or has been removed then 0 commission is returned.
    #[pallet::storage]
    #[pallet::getter(fn eras_validator_prefs)]
    pub type ErasValidatorPrefs<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat, EraIndex,
        Twox64Concat, T::AccountId,
        ValidatorPrefs,
        ValueQuery,
    >;

    /// The total validator era payout for the last `HISTORY_DEPTH` eras.
    ///
    /// Eras that haven't finished yet or has been removed doesn't have reward.
    #[pallet::storage]
    #[pallet::getter(fn eras_validator_reward)]
    pub type ErasValidatorReward<T: Config> = StorageMap<_, Twox64Concat, EraIndex, BalanceOf<T>>;

    /// Rewards for the last `HISTORY_DEPTH` eras.
    /// If reward hasn't been set or has been removed then 0 reward is returned.
    #[pallet::storage]
    #[pallet::getter(fn eras_reward_points)]
    pub type ErasRewardPoints<T: Config> = StorageMap<
        _,
        Twox64Concat, EraIndex,
        EraRewardPoints<T::AccountId>,
        ValueQuery,
    >;

    /// The total amount staked for the last `HISTORY_DEPTH` eras.
    /// If total hasn't been set or has been removed then 0 stake is returned.
    #[pallet::storage]
    #[pallet::getter(fn eras_total_stake)]
    pub type ErasTotalStake<T: Config> = StorageMap<_, Twox64Concat, EraIndex, BalanceOf<T>, ValueQuery>;

    /// Mode of era forcing.
    #[pallet::storage]
    #[pallet::getter(fn force_era)]
    pub type ForceEra<T> = StorageValue<_, Forcing, ValueQuery>;

    /// The percentage of the slash that is distributed to reporters.
    ///
    /// The rest of the slashed value is handled by the `Slash`.
    #[pallet::storage]
    #[pallet::getter(fn slash_reward_fraction)]
    pub type SlashRewardFraction<T> = StorageValue<_, Perbill, ValueQuery>;

    /// The amount of currency given to reporters of a slash event which was
    /// canceled by extraordinary circumstances (e.g. governance).
    #[pallet::storage]
    #[pallet::getter(fn canceled_payout)]
    pub type CanceledSlashPayout<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// All unapplied slashes that are queued for later.
    #[pallet::storage]
    pub type UnappliedSlashes<T: Config> = StorageMap<
        _,
        Twox64Concat, EraIndex,
        Vec<UnappliedSlash<T::AccountId, BalanceOf<T>>>,
        ValueQuery,
    >;

    /// A mapping from still-bonded eras to the first session index of that era.
    ///
    /// Must contains information for eras for the range:
    /// `[active_era - bounding_duration; active_era]`
    #[pallet::storage]
    pub(crate) type BondedEras<T: Config> = StorageValue<_, Vec<(EraIndex, SessionIndex)>, ValueQuery>;

    /// All slashing events on validators, mapped by era to the highest slash proportion
    /// and slash value of the era.
    #[pallet::storage]
    pub(crate) type ValidatorSlashInEra<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat, EraIndex,
        Twox64Concat, T::AccountId,
        (Perbill, BalanceOf<T>),
    >;

    /// All slashing events on nominators, mapped by era to the highest slash value of the era.
    #[pallet::storage]
    pub(crate) type NominatorSlashInEra<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat, EraIndex,
        Twox64Concat, T::AccountId,
        BalanceOf<T>,
    >;

    /// Slashing spans for stash accounts.
    #[pallet::storage]
    pub(crate) type SlashingSpans<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, slashing::SlashingSpans>;

    /// Records information about the maximum slash of a stash within a slashing span,
    /// as well as how much reward has been paid out.
    #[pallet::storage]
    pub(crate) type SpanSlash<T: Config> = StorageMap<
        _,
        Twox64Concat, (T::AccountId, slashing::SpanIndex),
        slashing::SpanRecord<BalanceOf<T>>,
        ValueQuery,
    >;

    /// The earliest era for which we have a pending, unapplied slash.
    #[pallet::storage]
    pub(crate) type EarliestUnappliedSlash<T> = StorageValue<_, EraIndex>;

    /// The last planned session scheduled by the session pallet.
    ///
    /// This is basically in sync with the call to [`SessionManager::new_session`].
    #[pallet::storage]
    #[pallet::getter(fn current_planned_session)]
    pub type CurrentPlannedSession<T> = StorageValue<_, SessionIndex, ValueQuery>;

    /// True if network has been upgraded to this version.
    /// Storage version of the pallet.
    ///
    /// This is set to v6.0.0 for new networks.
    #[pallet::storage]
    pub(crate) type StorageVersion<T: Config> = StorageValue<_, Releases, ValueQuery>;

    /// The threshold for when users can start calling `chill_other` for other validators / nominators.
    /// The threshold is compared to the actual number of validators / nominators (`CountFor*`) in
    /// the system compared to the configured max (`Max*Count`).
    #[pallet::storage]
    pub(crate) type ChillThreshold<T: Config> = StorageValue<_, Percent, OptionQuery>;

    ////
    /// The stake limit, determined all the staking operations
    /// This is keyed by the stash account.
    #[pallet::storage]
    #[pallet::getter(fn stake_limit)]
    pub type StakeLimit<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, BalanceOf<T>, OptionQuery>;

    /// Total staking payout at era.
    #[pallet::storage]
    #[pallet::getter(fn eras_staking_payout)]
    pub type ErasStakingPayout<T: Config> = StorageMap<_, Twox64Concat, EraIndex, BalanceOf<T>, OptionQuery>;

    /// Market staking payout of validator at era.
    #[pallet::storage]
    #[pallet::getter(fn eras_market_payout)]
    pub type ErasMarketPayout<T: Config> = StorageMap<_, Twox64Concat, EraIndex, BalanceOf<T>, OptionQuery>;

    /// Authoring payout of validator at era.
    #[pallet::storage]
    #[pallet::getter(fn eras_authoring_payout)]
    pub type ErasAuthoringPayout<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat, EraIndex,
        Twox64Concat, T::AccountId,
        BalanceOf<T>,
        OptionQuery
    >;

    /// The amount of balance actively at stake for each validator slot, currently.
    ///
    /// This is used to derive rewards and punishments.
    #[pallet::storage]
    #[pallet::getter(fn eras_total_stakes)]
    pub type ErasTotalStakes<T: Config> = StorageMap<_, Twox64Concat, EraIndex, BalanceOf<T>, ValueQuery>;

    /// The currently elected validator set keyed by stash account ID.
    #[pallet::storage]
    #[pallet::getter(fn current_elected)]
    pub type CurrentElected<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;


    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub history_depth: u32,
        pub validator_count: u32,
        pub minimum_validator_count: u32,
        pub invulnerables: Vec<T::AccountId>,
        pub force_era: Forcing,
        pub slash_reward_fraction: Perbill,
        pub canceled_payout: BalanceOf<T>,
        pub stakers: Vec<(T::AccountId, T::AccountId, BalanceOf<T>, StakerStatus<T::AccountId, BalanceOf<T>>)>,
        pub min_nominator_bond: BalanceOf<T>,
        pub min_validator_bond: BalanceOf<T>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            GenesisConfig {
                history_depth: 84u32,
                validator_count: Default::default(),
                minimum_validator_count: Default::default(),
                invulnerables: Default::default(),
                force_era: Default::default(),
                slash_reward_fraction: Default::default(),
                canceled_payout: Default::default(),
                stakers: Default::default(),
                min_nominator_bond: Default::default(),
                min_validator_bond: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            HistoryDepth::<T>::put(self.history_depth);
            ValidatorCount::<T>::put(self.validator_count);
            MinimumValidatorCount::<T>::put(self.minimum_validator_count);
            Invulnerables::<T>::put(&self.invulnerables);
            ForceEra::<T>::put(self.force_era);
            CanceledSlashPayout::<T>::put(self.canceled_payout);
            SlashRewardFraction::<T>::put(self.slash_reward_fraction);
            StorageVersion::<T>::put(Releases::V6_0_0);
            MinNominatorBond::<T>::put(self.min_nominator_bond);
            MinValidatorBond::<T>::put(self.min_validator_bond);

            for &(ref stash, ref controller, balance, ref status) in &self.stakers {
                assert!(
                    T::Currency::free_balance(&stash) >= balance,
                    "Stash does not have enough balance to bond."
                );
                let _ = <Pallet<T>>::bond(
                    T::Origin::from(Some(stash.clone()).into()),
                    T::Lookup::unlookup(controller.clone()),
                    balance,
                    RewardDestination::Staked,
                );
                let _ = match status {
                    StakerStatus::Validator => {
                        <Pallet<T>>::validate(
                            T::Origin::from(Some(controller.clone()).into()),
                            Default::default(),
                        )
                    }
                    StakerStatus::Nominator(votes) => {
                        for (target, vote) in votes {
                            <Pallet<T>>::nominate(
                                T::Origin::from(Some(controller.clone()).into()),
                                (T::Lookup::unlookup(target.clone()), vote.clone()),
                            ).ok();
                        }
                        Ok(())
                    }
                    _ => Ok(())
                };
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf < T > = "Balance")]
    pub enum Event<T: Config> {
        /// 质押人获得奖励  \[stash, amount\]
        Reward(T::AccountId, BalanceOf<T>),
        /// 一名验证者（及其提名者）被惩罚了给定的金额
        /// \[validator, amount\]
        Slash(T::AccountId, BalanceOf<T>),
        /// 前一个时代的旧惩罚报告因无法处理而被丢弃 \[session_index\]
        OldSlashingReportDiscarded(SessionIndex),
        /// 账户被绑定了指定的金额 \[stash, amount\]
        ///
        /// NOTE: This event is only emitted when funds are bonded via a dispatchable. Notably,
        /// it will not be emitted for staking rewards when they are added to stake.
        Bonded(T::AccountId, BalanceOf<T>),
        /// 一个帐户已解除绑定了此金额 \[stash, amount\]
        Unbonded(T::AccountId, BalanceOf<T>),
        /// 一个账户调用了 `withdraw_unbonded` 并从未解锁的块中，解绑了指定金额. \[stash, amount\]
        Withdrawn(T::AccountId, BalanceOf<T>),
        // /// A nominator has been kicked from a validator. \[nominator, stash\]
        // Kicked(T::AccountId, T::AccountId),
        /// 每个era的总奖励
        EraReward(EraIndex, BalanceOf<T>, BalanceOf<T>),
        /// 一个账户调用了“validate”申请成为验证人，并设置了保证金
        ValidateSuccess(T::AccountId, ValidatorPrefs),
        /// 一个账户称为“nominations”并投票给一个验证者
        NominationsSuccess(T::AccountId, T::AccountId, BalanceOf<T>),
        /// 一个账户调用了 `cut_nominations` 并减少为一个验证者投票
        CutNominationsSuccess(T::AccountId, T::AccountId, BalanceOf<T>),
        /// 帐户冻结成功
        ChillSuccess(T::AccountId, T::AccountId),
        /// 更新质押上限成功
        UpdateStakeLimitSuccess(u32),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Not a controller account.
        NotController,
        /// Not a stash account.
        NotStash,
        /// Stash is already bonded.
        AlreadyBonded,
        /// Controller is already paired.
        AlreadyPaired,
        /// Targets cannot be empty.
        EmptyTargets,
        /// Duplicate index.
        DuplicateIndex,
        /// Slash record index out of bounds.
        InvalidSlashIndex,
        /// Can not bond with value less than minimum required.
        InsufficientBond,
        /// Can not schedule more unlock chunks.
        NoMoreChunks,
        /// Can not rebond without unlocking chunks.
        NoUnlockChunk,
        /// Attempting to target a stash that still has funds.
        FundedTarget,
        /// Invalid era to reward.
        InvalidEraToReward,
        /// Invalid number of nominations.
        InvalidNumberOfNominations,
        /// Items are not sorted and unique.
        NotSortedAndUnique,
        /// Rewards for this era have already been claimed for this validator.
        AlreadyClaimed,
        /// Incorrect previous history depth input provided.
        IncorrectHistoryDepth,
        /// Incorrect number of slashing spans provided.
        IncorrectSlashingSpans,
        /// Internal state has become somehow corrupted and the operation cannot continue.
        BadState,
        /// Too many nomination targets supplied.
        TooManyTargets,
        /// A nomination target was supplied that was blocked or otherwise not a validator.
        BadTarget,
        /// The user has enough bond and thus cannot be chilled forcefully by an external person.
        CannotChillOther,
        /// There are too many nominators in the system. Governance needs to adjust the staking settings
        /// to keep things safe for the runtime.
        TooManyNominators,
        /// There are too many validators in the system. Governance needs to adjust the staking settings
        /// to keep things safe for the runtime.
        TooManyValidators,
        /// Target is invalid.
        InvalidTarget,
        /// Can not bond with value less than minimum balance.
        InsufficientValue,
        /// Can not bond with more than limit
        ExceedNominationsLimit,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> Weight {
            if StorageVersion::<T>::get() == Releases::V6_0_0 {
                migrations::v7::migrate::<T>()
            } else {
                T::DbWeight::get().reads(1)
            }
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<(), &'static str> {
            if StorageVersion::<T>::get() == Releases::V6_0_0 {
                migrations::v7::pre_migrate::<T>()
            } else {
                Ok(())
            }
        }

        fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
            // just return the weight of the on_finalize.
            T::DbWeight::get().reads(1)
        }

        fn on_finalize(_n: BlockNumberFor<T>) {
            // Set the start of the first era.
            if let Some(mut active_era) = Self::active_era() {
                if active_era.start.is_none() {
                    let now_as_millis_u64 = T::UnixTime::now().as_millis().saturated_into::<u64>();
                    active_era.start = Some(now_as_millis_u64);
                    // This write only ever happens once, we don't include it in the weight in general
                    ActiveEra::<T>::put(active_era);
                }
            }
            // `on_finalize` weight is tracked in `on_initialize`
        }

        fn integrity_test() {
            sp_std::if_std! {
				sp_io::TestExternalities::new_empty().execute_with(||
					assert!(
						T::SlashDeferDuration::get() < T::BondingDuration::get() || T::BondingDuration::get() == 0,
						"As per documentation, slash defer duration ({}) should be less than bonding duration ({}).",
						T::SlashDeferDuration::get(),
						T::BondingDuration::get(),
					)
				);
			}
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 将原始帐户作为stash账户并锁定其金额. `controller` 将是控制它的帐户
        ///
        /// `value` must be more than the `minimum_balance` specified by `T::Currency`.
        ///
        /// The dispatch origin for this call must be _Signed_ by the stash account.
        ///
        /// Emits `Bonded`.
        ///
        /// # <weight>
        /// - Independent of the arguments. Moderate complexity.
        /// - O(1).
        /// - Three extra DB entries.
        ///
        /// NOTE: Two of the storage writes (`Self::bonded`, `Self::payee`) are _never_ cleaned
        /// unless the `origin` falls below _existential deposit_ and gets removed as dust.
        /// ------------------
        /// Weight: O(1)
        /// DB Weight:
        /// - Read: Bonded, Ledger, [Origin Account], Current Era, History Depth, Locks
        /// - Write: Bonded, Payee, [Origin Account], Locks, Ledger
        /// # </weight>
        #[pallet::weight(T::WeightInfo::bond())]
        pub fn bond(
            origin: OriginFor<T>,
            controller: <T::Lookup as StaticLookup>::Source,
            #[pallet::compact] value: BalanceOf<T>,
            payee: RewardDestination<T::AccountId>,
        ) -> DispatchResult {
            let stash = ensure_signed(origin)?;

            if <Bonded<T>>::contains_key(&stash) {
                Err(Error::<T>::AlreadyBonded)?
            }

            let controller = T::Lookup::lookup(controller)?;

            if <Ledger<T>>::contains_key(&controller) {
                Err(Error::<T>::AlreadyPaired)?
            }

            // Reject a bond which is considered to be _dust_.
            if value < T::Currency::minimum_balance() {
                Err(Error::<T>::InsufficientBond)?
            }

            frame_system::Pallet::<T>::inc_consumers(&stash).map_err(|_| Error::<T>::BadState)?;

            // You're auto-bonded forever, here. We might improve this by only bonding when
            // you actually validate/nominate and remove once you unbond __everything__.
            <Bonded<T>>::insert(&stash, &controller);
            <Payee<T>>::insert(&stash, payee);

            let current_era = CurrentEra::<T>::get().unwrap_or(0);
            let history_depth = Self::history_depth();
            let last_reward_era = current_era.saturating_sub(history_depth);

            let stash_balance = T::Currency::free_balance(&stash);
            let value = value.min(stash_balance);
            Self::deposit_event(Event::<T>::Bonded(stash.clone(), value));
            let item = StakingLedger {
                stash,
                total: value,
                active: value,
                unlocking: vec![],
                claimed_rewards: (last_reward_era..current_era).collect(),
            };
            Self::update_ledger(&controller, &item);
            Ok(())
        }

        /// 将一些已出现在 `free_balance` 中的额外金额添加到用于质押的金额中
        ///
        /// Use this if there are additional funds in your stash account that you wish to bond.
        /// Unlike [`bond`] or [`unbond`] this function does not impose any limitation on the amount
        /// that can be added.
        ///
        /// The dispatch origin for this call must be _Signed_ by the stash, not the controller and
        /// it can be only called when [`EraElectionStatus`] is `Closed`.
        ///
        /// Emits `Bonded`.
        ///
        /// # <weight>
        /// - Independent of the arguments. Insignificant complexity.
        /// - O(1).
        /// - One DB entry.
        /// ------------
        /// DB Weight:
        /// - Read: Era Election Status, Bonded, Ledger, [Origin Account], Locks
        /// - Write: [Origin Account], Locks, Ledger
        /// # </weight>
        #[pallet::weight(T::WeightInfo::bond_extra())]
        pub fn bond_extra(
            origin: OriginFor<T>,
            #[pallet::compact] max_additional: BalanceOf<T>,
        ) -> DispatchResult {
            let stash = ensure_signed(origin)?;

            let controller = Self::bonded(&stash).ok_or(Error::<T>::NotStash)?;
            let mut ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;

            let stash_balance = T::Currency::free_balance(&stash);
            if let Some(extra) = stash_balance.checked_sub(&ledger.total) {
                let extra = extra.min(max_additional);
                ledger.total += extra;
                ledger.active += extra;
                // Last check: the new active amount of ledger must be more than ED.
                ensure!(ledger.active >= T::Currency::minimum_balance(), Error::<T>::InsufficientBond);

                Self::deposit_event(Event::<T>::Bonded(stash, extra));
                Self::update_ledger(&controller, &ledger);
            }
            Ok(())
        }

        /// 安排一部分stash解锁准备在绑定期结束后转出，如果这使主动绑定的金额少于 T::Currency::minimum_balance()，则将其增加到全额
        /// Schedule a portion of the stash to be unlocked ready for transfer out after the bond
        /// period ends. If this leaves an amount actively bonded less than
        /// T::Currency::minimum_balance(), then it is increased to the full amount.
        ///
        /// Once the unlock period is done, you can call `withdraw_unbonded` to actually move
        /// the funds out of management ready for transfer.
        ///
        /// No more than a limited number of unlocking chunks (see `MAX_UNLOCKING_CHUNKS`)
        /// can co-exists at the same time. In that case, [`Call::withdraw_unbonded`] need
        /// to be called first to remove some of the chunks (if possible).
        ///
        /// If a user encounters the `InsufficientBond` error when calling this extrinsic,
        /// they should call `chill` first in order to free up their bonded funds.
        ///
        /// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
        /// And, it can be only called when [`EraElectionStatus`] is `Closed`.
        ///
        /// Emits `Unbonded`.
        ///
        /// See also [`Call::withdraw_unbonded`].
        ///
        /// # <weight>
        /// - Independent of the arguments. Limited but potentially exploitable complexity.
        /// - Contains a limited number of reads.
        /// - Each call (requires the remainder of the bonded balance to be above `minimum_balance`)
        ///   will cause a new entry to be inserted into a vector (`Ledger.unlocking`) kept in storage.
        ///   The only way to clean the aforementioned storage item is also user-controlled via
        ///   `withdraw_unbonded`.
        /// - One DB entry.
        /// ----------
        /// Weight: O(1)
        /// DB Weight:
        /// - Read: EraElectionStatus, Ledger, CurrentEra, Locks, BalanceOf Stash,
        /// - Write: Locks, Ledger, BalanceOf Stash,
        /// </weight>
        #[pallet::weight(T::WeightInfo::unbond())]
        pub fn unbond(origin: OriginFor<T>, #[pallet::compact] value: BalanceOf<T>) -> DispatchResult {
            let controller = ensure_signed(origin)?;
            let mut ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
            ensure!(
				ledger.unlocking.len() < MAX_UNLOCKING_CHUNKS,
				Error::<T>::NoMoreChunks,
			);

            let mut value = value.min(ledger.active);

            if !value.is_zero() {
                ledger.active -= value;

                // Avoid there being a dust balance left in the staking system.
                if ledger.active < T::Currency::minimum_balance() {
                    value += ledger.active;
                    ledger.active = Zero::zero();
                }

                let min_active_bond = if Nominators::<T>::contains_key(&ledger.stash) {
                    MinNominatorBond::<T>::get()
                } else if Validators::<T>::contains_key(&ledger.stash) {
                    MinValidatorBond::<T>::get()
                } else {
                    Zero::zero()
                };

                // Make sure that the user maintains enough active bond for their role.
                // If a user runs into this error, they should chill first.
                ensure!(ledger.active >= min_active_bond, Error::<T>::InsufficientBond);

                // Note: in case there is no current era it is fine to bond one era more.
                let era = Self::current_era().unwrap_or(0) + T::BondingDuration::get();
                ledger.unlocking.push(UnlockChunk { value, era });
                Self::update_ledger(&controller, &ledger);
                Self::deposit_event(Event::<T>::Unbonded(ledger.stash, value));
            }
            Ok(())
        }

        /// 重新绑定计划解锁的stash部分
        /// Rebond a portion of the stash scheduled to be unlocked.
        ///
        /// The dispatch origin must be signed by the controller, and it can be only called when
        /// [`EraElectionStatus`] is `Closed`.
        ///
        /// # <weight>
        /// - Time complexity: O(L), where L is unlocking chunks
        /// - Bounded by `MAX_UNLOCKING_CHUNKS`.
        /// - Storage changes: Can't increase storage, only decrease it.
        /// ---------------
        /// - DB Weight:
        ///     - Reads: EraElectionStatus, Ledger, Locks, [Origin Account]
        ///     - Writes: [Origin Account], Locks, Ledger
        /// # </weight>
        #[pallet::weight(T::WeightInfo::rebond(MAX_UNLOCKING_CHUNKS as u32))]
        pub fn rebond(
            origin: OriginFor<T>,
            #[pallet::compact] value: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
            ensure!(!ledger.unlocking.is_empty(), Error::<T>::NoUnlockChunk);

            let ledger = ledger.rebond(value);
            // Last check: the new active amount of ledger must be more than ED.
            ensure!(ledger.active >= T::Currency::minimum_balance(), Error::<T>::InsufficientBond);

            Self::deposit_event(Event::<T>::Bonded(ledger.stash.clone(), value));
            Self::update_ledger(&controller, &ledger);
            Ok(Some(
                35 * WEIGHT_PER_MICROS
                    + 50 * WEIGHT_PER_NANOS * (ledger.unlocking.len() as Weight)
                    + T::DbWeight::get().reads_writes(3, 2)
            ).into())
        }

        /// 从我们的管理队列 `unlocking` 中删除任何未锁定的块
        /// Remove any unlocked chunks from the `unlocking` queue from our management.
        ///
        /// This essentially frees up that balance to be used by the stash account to do
        /// whatever it wants.
        ///
        /// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
        /// And, it can be only called when [`EraElectionStatus`] is `Closed`.
        ///
        /// Emits `Withdrawn`.
        ///
        /// See also [`Call::unbond`].
        ///
        /// # <weight>
        /// - Could be dependent on the `origin` argument and how much `unlocking` chunks exist.
        ///  It implies `consolidate_unlocked` which loops over `Ledger.unlocking`, which is
        ///  indirectly user-controlled. See [`unbond`] for more detail.
        /// - Contains a limited number of reads, yet the size of which could be large based on `ledger`.
        /// - Writes are limited to the `origin` account key.
        /// ---------------
        /// Complexity O(S) where S is the number of slashing spans to remove
        /// Update:
        /// - Reads: EraElectionStatus, Ledger, Current Era, Locks, [Origin Account]
        /// - Writes: [Origin Account], Locks, Ledger
        /// Kill:
        /// - Reads: EraElectionStatus, Ledger, Current Era, Bonded, Slashing Spans, [Origin
        ///   Account], Locks, BalanceOf stash
        /// - Writes: Bonded, Slashing Spans (if S > 0), Ledger, Payee, Validators, Nominators,
        ///   [Origin Account], Locks, BalanceOf stash.
        /// - Writes Each: SpanSlash * S
        /// NOTE: Weight annotation is the kill scenario, we refund otherwise.
        /// # </weight>
        #[pallet::weight(T::WeightInfo::withdraw_unbonded_kill(* num_slashing_spans))]
        pub fn withdraw_unbonded(
            origin: OriginFor<T>,
            num_slashing_spans: u32,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let mut ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
            let (stash, old_total) = (ledger.stash.clone(), ledger.total);
            if let Some(current_era) = Self::current_era() {
                ledger = ledger.consolidate_unlocked(current_era)
            }

            //? minimum_balance < 500
            let post_info_weight = if ledger.unlocking.is_empty() && ledger.active < T::Currency::minimum_balance() {
                // This account must have called `unbond()` with some value that caused the active
                // portion to fall below existential deposit + will have no more unlocking chunks
                // left. We can now safely remove all staking-related information.
                Self::kill_stash(&stash, num_slashing_spans)?;
                // Remove the lock.
                T::Currency::remove_lock(STAKING_ID, &stash);
                // This is worst case scenario, so we use the full weight and return None
                None
            } else {
                // This was the consequence of a partial unbond. just update the ledger and move on.
                Self::update_ledger(&controller, &ledger);

                // This is only an update, so we use less overall weight.
                Some(T::WeightInfo::withdraw_unbonded_update(num_slashing_spans))
            };

            // `old_total` should never be less than the new total because
            // `consolidate_unlocked` strictly subtracts balance.
            if ledger.total < old_total {
                // Already checked that this won't overflow by entry condition.
                let value = old_total - ledger.total;
                Self::deposit_event(Event::<T>::Withdrawn(stash, value));
            }

            Ok(post_info_weight.into())
        }

        /// 由controller账户声明想成为验证人
        /// Declare the desire to validate for the origin controller.
        ///
        /// Effects will be felt at the beginning of the next era.
        ///
        /// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
        /// And, it can be only called when [`EraElectionStatus`] is `Closed`.
        ///
        /// # <weight>
        /// - Independent of the arguments. Insignificant complexity.
        /// - Contains a limited number of reads.
        /// - Writes are limited to the `origin` account key.
        /// -----------
        /// Weight: O(1)
        /// DB Weight:
        /// - Read: Era Election Status, Ledger
        /// - Write: Nominators, Validators
        /// # </weight>
        #[pallet::weight(T::WeightInfo::validate())]
        pub fn validate(origin: OriginFor<T>, prefs: ValidatorPrefs) -> DispatchResult {
            let controller = ensure_signed(origin)?;
            let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
            //ensure!(ledger.active >= MinValidatorBond::<T>::get(), Error::<T>::InsufficientBond);
            let stash = &ledger.stash;

            // Only check limits if they are not already a validator.
            if !Validators::<T>::contains_key(stash) {
                // If this error is reached, we need to adjust the `MinValidatorBond` and start calling `chill_other`.
                // Until then, we explicitly block new validators to protect the runtime.
                if let Some(max_validators) = MaxValidatorsCount::<T>::get() {
                    ensure!(CounterForValidators::<T>::get() < max_validators, Error::<T>::TooManyValidators);
                }
            }

            let prefs_commission = prefs.commission;

            Self::do_remove_nominator(stash);
            Self::do_add_validator(stash, prefs.clone());

            if let Some(active_era) = Self::active_era() {
                if <ErasValidatorPrefs<T>>::get(&active_era.index, &stash).commission > prefs_commission {
                    <ErasValidatorPrefs<T>>::insert(&active_era.index, &stash, ValidatorPrefs { commission: Perbill::one(), blocked: false });
                }
            }
            Self::deposit_event(Event::<T>::ValidateSuccess(controller, prefs));
            Ok(())
        }

        /// 由controller账户声明想成为提名人
        /// Declare the desire to nominate `targets` for the origin controller.
        ///
        /// Effects will be felt at the beginning of the next era. This can only be called when
        /// [`EraElectionStatus`] is `Closed`.
        ///
        /// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
        /// And, it can be only called when [`EraElectionStatus`] is `Closed`.
        ///
        /// # <weight>
        /// - The transaction's complexity is proportional to the size of `targets` (N)
        /// which is capped at CompactAssignments::LIMIT (MAX_NOMINATIONS).
        /// - Both the reads and writes follow a similar pattern.
        /// ---------
        /// Weight: O(N)
        /// where N is the number of targets
        /// DB Weight:
        /// - Reads: Era Election Status, Ledger, Current Era
        /// - Writes: Validators, Nominators
        /// # </weight>
        #[pallet::weight(T::WeightInfo::nominate(0))]
        pub fn nominate(
            origin: OriginFor<T>,
            targets: (<T::Lookup as StaticLookup>::Source, BalanceOf<T>),
        ) -> DispatchResult {
            // 1. Get ledger
            let controller = ensure_signed(origin)?;
            let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
            let g_stash = &ledger.stash;
            let (target, votes) = targets;

            // 2. Target should be legal
            let v_stash = T::Lookup::lookup(target)?;
            ensure!(<Validators<T>>::contains_key(&v_stash), Error::<T>::InvalidTarget);

            // 3. Votes value should greater than the dust
            ensure!(votes > T::Currency::minimum_balance(), Error::<T>::InsufficientValue);

            // 4. Upsert (increased) nominations
            let nominations = Self::increase_nominations(&v_stash, g_stash, ledger.active.clone(), votes.clone());

            // 5. `None` means exceed the nominations limit(`MAX_NOMINATIONS`)
            ensure!(nominations.is_some(), Error::<T>::ExceedNominationsLimit);
            let nominations = nominations.unwrap();

            Self::do_remove_validator(g_stash);
            Self::do_add_nominator(g_stash, nominations);
            Self::deposit_event(Event::<T>::NominationsSuccess(controller, v_stash, votes));

            Ok(())
        }

        /// 由controller账户声明想减少像提名的人
        /// Declare the desire to cut nominations for the origin controller.
        ///
        /// Effects will be felt at the beginning of the next era.
        ///
        /// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
        ///
        /// # <weight>
        /// - The transaction's complexity is proportional to the size of `validators` (N),
        /// `nominators`, `nominations_rel`
        /// - Both the reads and writes follow a similar pattern.
        /// ---------
        /// DB Weight:
        /// - Reads: Nominators, Ledger, Current Era
        /// - Writes: Validators, Nominators
        /// # </weight>
        #[pallet::weight(0)]
        pub fn cut_nominations(origin: OriginFor<T>, target: (<T::Lookup as StaticLookup>::Source, BalanceOf<T>)) -> DispatchResult {
            // 1. Get ledger
            let controller = ensure_signed(origin)?;
            let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
            let g_stash = &ledger.stash;
            let (target, votes) = target;

            // 2. Target should be legal
            let v_stash = T::Lookup::lookup(target)?;

            // 3. Votes value should greater than the dust
            ensure!(votes > T::Currency::minimum_balance(), Error::<T>::InsufficientValue);

            // 4. Upsert (decreased) nominations
            let nominations = Self::decrease_nominations(&v_stash, &g_stash, votes.clone());

            // 5. `None` means the target is invalid(cut a void)
            ensure!(nominations.is_some(), Error::<T>::InvalidTarget);
            let nominations = nominations.unwrap();

            Self::do_add_nominator(g_stash, nominations);
            Self::deposit_event(Event::<T>::CutNominationsSuccess(controller, v_stash, votes));

            Ok(())
        }

        /// 声明不想成为验证人与提名人
        /// Declare no desire to either validate or nominate.
        ///
        /// Effects will be felt at the beginning of the next era.
        ///
        /// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
        /// And, it can be only called when [`EraElectionStatus`] is `Closed`.
        ///
        /// # <weight>
        /// - Independent of the arguments. Insignificant complexity.
        /// - Contains one read.
        /// - Writes are limited to the `origin` account key.
        /// --------
        /// Weight: O(1)
        /// DB Weight:
        /// - Read: EraElectionStatus, Ledger
        /// - Write: Validators, Nominators
        /// # </weight>
        #[pallet::weight(T::WeightInfo::chill())]
        pub fn chill(origin: OriginFor<T>) -> DispatchResult {
            let controller = ensure_signed(origin)?;
            let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
            Self::chill_stash(&ledger.stash);
            Self::deposit_event(Event::<T>::ChillSuccess(controller, ledger.stash));

            Ok(())
        }

        /// （重新）重新设置获取收益的账户
        /// (Re-)set the payment target for a controller
        ///
        /// Effects will be felt at the beginning of the next era.
        ///
        /// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
        ///
        /// # <weight>
        /// - Independent of the arguments. Insignificant complexity.
        /// - Contains a limited number of reads.
        /// - Writes are limited to the `origin` account key.
        /// ---------
        /// - Weight: O(1)
        /// - DB Weight:
        ///     - Read: Ledger
        ///     - Write: Payee
        /// # </weight>
        #[pallet::weight(0)]
        pub fn set_payee(
            origin: OriginFor<T>,
            payee: RewardDestination<T::AccountId>,
        ) -> DispatchResult {
            let controller = ensure_signed(origin)?;
            let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
            let stash = &ledger.stash;
            <Payee<T>>::insert(stash, payee);
            Ok(())
        }

        ///（重新）设置stash账户的controller账户
        /// (Re-)set the controller of a stash.
        ///
        /// Effects will be felt at the beginning of the next era.
        ///
        /// The dispatch origin for this call must be _Signed_ by the stash, not the controller.
        ///
        /// # <weight>
        /// - Independent of the arguments. Insignificant complexity.
        /// - Contains a limited number of reads.
        /// - Writes are limited to the `origin` account key.
        /// ----------
        /// Weight: O(1)
        /// DB Weight:
        /// - Read: Bonded, Ledger New Controller, Ledger Old Controller
        /// - Write: Bonded, Ledger New Controller, Ledger Old Controller
        /// # </weight>
        #[pallet::weight(T::WeightInfo::set_controller())]
        pub fn set_controller(
            origin: OriginFor<T>,
            controller: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResult {
            let stash = ensure_signed(origin)?;
            let old_controller = Self::bonded(&stash).ok_or(Error::<T>::NotStash)?;
            let controller = T::Lookup::lookup(controller)?;
            if <Ledger<T>>::contains_key(&controller) {
                Err(Error::<T>::AlreadyPaired)?
            }
            if controller != old_controller {
                <Bonded<T>>::insert(&stash, &controller);
                if let Some(l) = <Ledger<T>>::take(&old_controller) {
                    <Ledger<T>>::insert(&controller, l);
                }
            }
            Ok(())
        }

        /// 设置最大验证人数量
        ///
        /// The dispatch origin must be Root.
        ///
        /// # <weight>
        /// Weight: O(1)
        /// Write: Validator Count
        /// # </weight>
        #[pallet::weight(T::WeightInfo::set_validator_count())]
        pub fn set_validator_count(
            origin: OriginFor<T>,
            #[pallet::compact] new: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ValidatorCount::<T>::put(new);
            Ok(())
        }

        /// 增加最大验证人数量
        ///
        /// The dispatch origin must be Root.
        ///
        /// # <weight>
        /// Same as [`set_validator_count`].
        /// # </weight>
        #[pallet::weight(T::WeightInfo::set_validator_count())]
        pub fn increase_validator_count(
            origin: OriginFor<T>,
            #[pallet::compact] additional: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ValidatorCount::<T>::mutate(|n| *n += additional);
            Ok(())
        }

        /// 将最大的验证人数量扩大一个因子
        /// Scale up the ideal number of validators by a factor.
        ///
        /// The dispatch origin must be Root.
        ///
        /// # <weight>
        /// Same as [`set_validator_count`].
        /// # </weight>
        #[pallet::weight(T::WeightInfo::set_validator_count())]
        pub fn scale_validator_count(origin: OriginFor<T>, factor: Percent) -> DispatchResult {
            ensure_root(origin)?;
            ValidatorCount::<T>::mutate(|n| *n += factor * *n);
            Ok(())
        }

        /// 强制永远不产生新era
        /// Force there to be no new eras indefinitely.
        ///
        /// The dispatch origin must be Root.
        ///
        /// # Warning
        ///
        /// The election process starts multiple blocks before the end of the era.
        /// Thus the election process may be ongoing when this is called. In this case the
        /// election will continue until the next era is triggered.
        ///
        /// # <weight>
        /// - No arguments.
        /// - Weight: O(1)
        /// - Write: ForceEra
        /// # </weight>
        #[pallet::weight(T::WeightInfo::force_no_eras())]
        pub fn force_no_eras(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;
            ForceEra::<T>::put(Forcing::ForceNone);
            Ok(())
        }

        /// 在下个session结束时进入一个新era。在此之后，它将被重置为正常行为
        /// Force there to be a new era at the end of the next session. After this, it will be
        /// reset to normal (non-forced) behaviour.
        ///
        /// The dispatch origin must be Root.
        ///
        /// # Warning
        ///
        /// The election process starts multiple blocks before the end of the era.
        /// If this is called just before a new era is triggered, the election process may not
        /// have enough blocks to get a result.
        ///
        /// # <weight>
        /// - No arguments.
        /// - Weight: O(1)
        /// - Write ForceEra
        /// # </weight>
        #[pallet::weight(T::WeightInfo::force_new_era())]
        pub fn force_new_era(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;
            ForceEra::<T>::put(Forcing::ForceNew);
            Ok(())
        }

        /// 设置不能被惩罚的验证人
        ///
        /// The dispatch origin must be Root.
        ///
        /// # <weight>
        /// - O(V)
        /// - Write: Invulnerables
        /// # </weight>
        #[pallet::weight(T::WeightInfo::set_invulnerables(invulnerables.len() as u32))]
        pub fn set_invulnerables(
            origin: OriginFor<T>,
            invulnerables: Vec<T::AccountId>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            <Invulnerables<T>>::put(invulnerables);
            Ok(())
        }

        /// 立即强制当前的质押者完全取消质押
        /// Force a current staker to become completely unstaked, immediately.
        ///
        /// The dispatch origin must be Root.
        ///
        /// # <weight>
        /// O(S) where S is the number of slashing spans to be removed
        /// Reads: Bonded, Slashing Spans, Account, Locks
        /// Writes: Bonded, Slashing Spans (if S > 0), Ledger, Payee, Validators, Nominators, Account, Locks
        /// Writes Each: SpanSlash * S
        /// # </weight>
        #[pallet::weight(T::WeightInfo::force_unstake(* num_slashing_spans))]
        pub fn force_unstake(
            origin: OriginFor<T>,
            stash: T::AccountId,
            num_slashing_spans: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Remove all staking-related information.
            Self::kill_stash(&stash, num_slashing_spans)?;

            // Remove the lock.
            T::Currency::remove_lock(STAKING_ID, &stash);
            Ok(())
        }

        /// 永远在下个session结束时进入一个新era
        ///
        /// The dispatch origin must be Root.
        ///
        /// # Warning
        ///
        /// The election process starts multiple blocks before the end of the era.
        /// If this is called just before a new era is triggered, the election process may not
        /// have enough blocks to get a result.
        ///
        /// # <weight>
        /// - Weight: O(1)
        /// - Write: ForceEra
        /// # </weight>
        #[pallet::weight(T::WeightInfo::force_new_era_always())]
        pub fn force_new_era_always(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;
            ForceEra::<T>::put(Forcing::ForceAlways);
            Ok(())
        }

        /// 取消延迟的惩罚
        /// Cancel enactment of a deferred slash.
        ///
        /// Can be called by the `T::SlashCancelOrigin`.
        ///
        /// Parameters: era and indices of the slashes for that era to kill.
        ///
        /// # <weight>
        /// Complexity: O(U + S)
        /// with U unapplied slashes weighted with U=1000
        /// and S is the number of slash indices to be canceled.
        /// - Read: Unapplied Slashes
        /// - Write: Unapplied Slashes
        /// # </weight>
        #[pallet::weight(T::WeightInfo::cancel_deferred_slash(slash_indices.len() as u32))]
        pub fn cancel_deferred_slash(
            origin: OriginFor<T>,
            era: EraIndex,
            slash_indices: Vec<u32>,
        ) -> DispatchResult {
            T::SlashCancelOrigin::ensure_origin(origin)?;

            ensure!(!slash_indices.is_empty(), Error::<T>::EmptyTargets);
            ensure!(is_sorted_and_unique(&slash_indices), Error::<T>::NotSortedAndUnique);

            let mut unapplied = <Self as Store>::UnappliedSlashes::get(&era);
            let last_item = slash_indices[slash_indices.len() - 1];
            ensure!((last_item as usize) < unapplied.len(), Error::<T>::InvalidSlashIndex);

            for (removed, index) in slash_indices.into_iter().enumerate() {
                let index = (index as usize) - removed;
                unapplied.remove(index);
            }

            <Self as Store>::UnappliedSlashes::insert(&era, &unapplied);
            Ok(())
        }

        /// 在一个单独的era，支付一个验证人和他的所有提名人的奖励
        /// Pay out all the stakers behind a single validator for a single era.
        ///
        /// - `validator_stash` is the stash account of the validator. Their nominators, up to
        ///   `T::MaxNominatorRewardedPerValidator`, will also receive their rewards.
        /// - `era` may be any era between `[current_era - history_depth; current_era]`.
        ///
        /// The origin of this call must be _Signed_. Any account can call this function, even if
        /// it is not one of the stakers.
        ///
        /// This can only be called when [`EraElectionStatus`] is `Closed`.
        ///
        /// # <weight>
        /// - Time complexity: at most O(MaxNominatorRewardedPerValidator).
        /// - Contains a limited number of reads and writes.
        /// -----------
        /// N is the Number of payouts for the validator (including the validator)
        /// Weight:
        /// - Reward Destination Staked: O(N)
        /// - Reward Destination Controller (Creating): O(N)
        /// DB Weight:
        /// - Read: EraElectionStatus, CurrentEra, HistoryDepth, ErasValidatorReward,
        ///         ErasStakersClipped, ErasRewardPoints, ErasValidatorPrefs (8 items)
        /// - Read Each: Bonded, Ledger, Payee, Locks, System Account (5 items)
        /// - Write Each: System Account, Locks, Ledger (3 items)
        ///
        ///   NOTE: weights are assuming that payouts are made to alive stash account (Staked).
        ///   Paying even a dead controller is cheaper weight-wise. We don't do any refunds here.
        /// # </weight>
        #[pallet::weight(T::WeightInfo::payout_stakers_alive_staked(T::MaxNominatorRewardedPerValidator::get()))]
        pub fn reward_stakers(
            origin: OriginFor<T>,
            validator_stash: T::AccountId,
            era: EraIndex,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            Self::do_reward_stakers(validator_stash, era)
        }


        /// 设置“HistoryDe​​pth”值。当`HistoryDe​​pth`减少时，该函数将删除任何历史信息
        /// Set `HistoryDepth` value. This function will delete any history information
        /// when `HistoryDepth` is reduced.
        ///
        /// Parameters:
        /// - `new_history_depth`: The new history depth you would like to set.
        /// - `era_items_deleted`: The number of items that will be deleted by this dispatch.
        ///    This should report all the storage items that will be deleted by clearing old
        ///    era history. Needed to report an accurate weight for the dispatch. Trusted by
        ///    `Root` to report an accurate number.
        ///
        /// Origin must be root.
        ///
        /// # <weight>
        /// - E: Number of history depths removed, i.e. 10 -> 7 = 3
        /// - Weight: O(E)
        /// - DB Weight:
        ///     - Reads: Current Era, History Depth
        ///     - Writes: History Depth
        ///     - Clear Prefix Each: Era Stakers, EraStakersClipped, ErasValidatorPrefs
        ///     - Writes Each: ErasValidatorReward, ErasRewardPoints, ErasTotalStake, ErasStartSessionIndex
        /// # </weight>
        #[pallet::weight(T::WeightInfo::set_history_depth(* _era_items_deleted))]
        pub fn set_history_depth(origin: OriginFor<T>,
                                 #[pallet::compact] new_history_depth: EraIndex,
                                 #[pallet::compact] _era_items_deleted: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            if let Some(current_era) = Self::current_era() {
                HistoryDepth::<T>::mutate(|history_depth| {
                    let last_kept = current_era.checked_sub(*history_depth).unwrap_or(0);
                    let new_last_kept = current_era.checked_sub(new_history_depth).unwrap_or(0);
                    for era_index in last_kept..new_last_kept {
                        Self::clear_era_information(era_index);
                    }
                    *history_depth = new_history_depth
                })
            }
            Ok(())
        }

        /// 一旦其余额达到最低限度，就删除所有与该质押人有关的数据
        /// Remove all data structure concerning a staker/stash once its balance is at the minimum.
        /// This is essentially equivalent to `withdraw_unbonded` except it can be called by anyone
        /// and the target `stash` must have no funds left beyond the ED.
        ///
        /// This can be called from any origin.
        ///
        /// - `stash`: The stash account to reap. Its balance must be zero.
        ///
        /// # <weight>
        /// Complexity: O(S) where S is the number of slashing spans on the account.
        /// DB Weight:
        /// - Reads: Stash Account, Bonded, Slashing Spans, Locks
        /// - Writes: Bonded, Slashing Spans (if S > 0), Ledger, Payee, Validators, Nominators, Stash Account, Locks
        /// - Writes Each: SpanSlash * S
        /// # </weight>
        #[pallet::weight(T::WeightInfo::reap_stash(* num_slashing_spans))]
        pub fn reap_stash(
            _origin: OriginFor<T>,
            stash: T::AccountId,
            num_slashing_spans: u32,
        ) -> DispatchResult {
            let at_minimum = T::Currency::total_balance(&stash) == T::Currency::minimum_balance();
            ensure!(at_minimum, Error::<T>::FundedTarget);
            Self::kill_stash(&stash, num_slashing_spans)?;
            T::Currency::remove_lock(STAKING_ID, &stash);
            Ok(())
        }

        /// 开启奖励的纪元
        // TODO: 主网奖励开始后移除
        #[pallet::weight(0)]
        pub fn set_start_reward_era(origin: OriginFor<T>, start_reward_era: EraIndex) -> DispatchResult {
            ensure_root(origin)?;
            StartRewardEra::<T>::put(start_reward_era);
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The total balance that can be slashed from a stash account as of right now.
    pub fn slashable_balance_of(stash: &T::AccountId) -> BalanceOf<T> {
        // Weight note: consider making the stake accessible through stash.
        Self::bonded(stash).and_then(Self::ledger).map(|l| l.active).unwrap_or_default()
    }

    /// Get the updated (increased) nominations relationship
    /// Basically, this function construct an updated edge or insert a new edge,
    /// then returns the updated `Nominations`
    ///
    /// # <weight>
    /// - Independent of the arguments. Insignificant complexity.
    /// - O(1).
    /// - 1 DB entry.
    /// # </weight>
    fn increase_nominations(
        v_stash: &T::AccountId,
        g_stash: &T::AccountId,
        bonded: BalanceOf<T>,
        votes: BalanceOf<T>,
    ) -> Option<Nominations<T::AccountId, BalanceOf<T>>> {
        // 1. Already nominationsd
        if let Some(nominations) = Self::nominators(g_stash) {
            //剩余 = 所有活跃的绑定金额-所有担保的金额
            //真实的担保 = 最小值（剩余，担保）
            //新的所有的担保金额 = 原来的 + 真实的担保
            let remains = bonded.saturating_sub(nominations.total);
            let real_votes = remains.min(votes);
            let new_total = nominations.total.saturating_add(real_votes);
            let mut new_targets: Vec<IndividualExposure<T::AccountId, BalanceOf<T>>> = vec![];
            let mut update = false;

            if real_votes <= Zero::zero() {
                log!(
                    debug,
                    "💸 Staking limit of validator {:?} is zero.",
                    v_stash
                );
                return None;
            }

            // Fill in `new_targets`, always LOOP the `targets`
            // However, the TC is O(1) due to the `MAX_NOMINATIONS` restriction 🤪
            for mut target in nominations.targets {
                // a. Update an edge
                if &target.who == v_stash {
                    target.value += real_votes;
                    update = true;
                }
                new_targets.push(target.clone());
            }

            if !update {
                if new_targets.len() >= MAX_NOMINATIONS {
                    return None;
                } else {
                    // b. New an edge
                    new_targets.push(IndividualExposure {
                        who: v_stash.clone(),
                        value: real_votes,
                    });
                }
            }

            Some(Nominations {
                targets: new_targets.clone(),
                total: new_total,
                submitted_in: Self::current_era().unwrap_or(0),
                suppressed: false,
            })

            // 2. New nominations
        } else {
            let real_votes = bonded.min(votes);
            let new_total = real_votes;

            // No need check with this case, votes and bonded all greater than 0

            let mut new_targets: Vec<IndividualExposure<T::AccountId, BalanceOf<T>>> = vec![];
            new_targets.push(IndividualExposure {
                who: v_stash.clone(),
                value: real_votes,
            });

            Some(Nominations {
                targets: new_targets.clone(),
                total: new_total,
                submitted_in: Self::current_era().unwrap_or(0),
                suppressed: false,
            })
        }
    }


    /// Get the updated (decreased) nominations relationship
    /// Basically, this function construct an updated edge,
    /// then returns the updated `Nominations`
    ///
    /// # <weight>
    /// - Independent of the arguments. Insignificant complexity.
    /// - O(1).
    /// - 1 DB entry.
    /// # </weight>
    fn decrease_nominations(
        v_stash: &T::AccountId,
        g_stash: &T::AccountId,
        votes: BalanceOf<T>,
    ) -> Option<Nominations<T::AccountId, BalanceOf<T>>> {
        if let Some(nominations) = Self::nominators(g_stash) {
            // `decreased_votes` = min(votes, target.value)
            // `new_targets` means the targets after decreased
            // `exists` means the targets contains `v_stash`
            let mut decreased_votes = Zero::zero();
            let mut new_targets: Vec<IndividualExposure<T::AccountId, BalanceOf<T>>> = vec![];
            let mut exists = false;

            // Always LOOP the targets
            // However, the TC is O(1), due to the `MAX_NOMINATIONS` restriction 🤪
            for target in nominations.targets {
                if &target.who == v_stash {
                    // 1. Mark it really exists (BRAVO), and update the decreased votes
                    exists = true;
                    decreased_votes = target.value.min(votes);

                    if target.value <= votes {
                        // 2. Remove this target
                    } else {
                        // 3. Decrease the value
                        let new_target = IndividualExposure {
                            who: v_stash.clone(),
                            value: target.value - votes,
                        };
                        new_targets.push(new_target);
                    }
                } else {
                    // 4. Push target with no change
                    new_targets.push(target.clone());
                }
            }

            if exists {
                // 5. Update `new_total` with saturating sub the decreased_votes
                let new_total = nominations.total.saturating_sub(decreased_votes);

                // TODO: `submitted_in` and `suppressed` should not be change?
                return Some(Nominations {
                    targets: new_targets.clone(),
                    total: new_total,
                    submitted_in: nominations.submitted_in,
                    suppressed: nominations.suppressed,
                });
            }
        }

        None
    }


    /// Update the ledger for a controller.
    ///
    /// This will also update the stash lock.
    fn update_ledger(
        controller: &T::AccountId,
        ledger: &StakingLedger<T::AccountId, BalanceOf<T>>,
    ) {
        T::Currency::set_lock(
            STAKING_ID,
            &ledger.stash,
            ledger.total,
            WithdrawReasons::all(),
        );
        <Ledger<T>>::insert(controller, ledger);
    }

    /// Chill a stash account.
    fn chill_stash(stash: &T::AccountId) {
        Self::do_remove_validator(stash);
        Self::do_remove_nominator(stash);
    }

    /// Actually make a payment to a staker. This uses the currency's reward function
    /// to pay the right payee for the given staker account.
    fn make_payout(stash: &T::AccountId, amount: BalanceOf<T>) -> Option<PositiveImbalanceOf<T>> {
        let dest = Self::payee(stash);
        match dest {
            RewardDestination::Controller => Self::bonded(stash)
                .and_then(|controller|
                    Some(T::Currency::deposit_creating(&controller, amount))
                ),
            RewardDestination::Stash =>
                T::Currency::deposit_into_existing(stash, amount).ok(),
            RewardDestination::Staked => Self::bonded(stash)
                .and_then(|c| Self::ledger(&c).map(|l| (c, l)))
                .and_then(|(controller, mut l)| {
                    l.active += amount;
                    l.total += amount;
                    let r = T::Currency::deposit_into_existing(stash, amount).ok();
                    Self::update_ledger(&controller, &l);
                    r
                }),
            RewardDestination::Account(dest_account) => {
                Some(T::Currency::deposit_creating(&dest_account, amount))
            }
            RewardDestination::None => None,
        }
    }

    /// Pay reward to stakers. Two kinds of reward.
    /// One is authoring reward which is paid to validator who are elected.
    /// Another one is staking reward.
    fn do_reward_stakers(
        validator_stash: T::AccountId,
        era: EraIndex,
    ) -> DispatchResult {
        // 1. Validate input data
        let current_era = CurrentEra::<T>::get().ok_or(Error::<T>::InvalidEraToReward)?;
        ensure!(era <= current_era, Error::<T>::InvalidEraToReward);
        let history_depth = Self::history_depth();
        ensure!(era >= current_era.saturating_sub(history_depth), Error::<T>::InvalidEraToReward);

        // 注意：如果时代没有奖励可领取，时代可能是未来的。在这种情况下最好不要更新“ledger.claimed Reward”。
        let total_era_staking_payout = <ErasStakingPayout<T>>::get(&era)
            .ok_or_else(|| Error::<T>::InvalidEraToReward)?;

        // 找到验证人stash的controller账号
        let controller = Self::bonded(&validator_stash).ok_or(Error::<T>::NotStash)?;
        // 查找账单
        let mut ledger = <Ledger<T>>::get(&controller).ok_or_else(|| Error::<T>::NotController)?;

        // 保留可以领取奖励的era，用二分法查找目标era。找到则说明已经领取，
        // 找不到则返回目标位置，并且插入数值
        ledger.claimed_rewards.retain(|&x| x >= current_era.saturating_sub(history_depth));
        match ledger.claimed_rewards.binary_search(&era) {
            Ok(_) => Err(Error::<T>::AlreadyClaimed)?,
            Err(pos) => ledger.claimed_rewards.insert(pos, era),
        }
        /* Input data seems good, no errors allowed after this point */
        // 插入新的账单
        let exposure = <ErasStakersClipped<T>>::get(&era, &ledger.stash);
        <Ledger<T>>::insert(&controller, &ledger);

        // 2. 总奖励 += 验证人的出块奖励
        let mut validator_imbalance = <PositiveImbalanceOf<T>>::zero();
        let mut total_reward: BalanceOf<T> = Zero::zero();
        if let Some(authoring_reward) = <ErasAuthoringPayout<T>>::get(&era, &validator_stash) {
            total_reward = total_reward.saturating_add(authoring_reward);
        }

        // to_num : 将balance转变成u128类型
        let to_num =
            |b: BalanceOf<T>| <T::CurrencyToVote as Convert<BalanceOf<T>, u128>>::convert(b);

        // 3. 获得总质押和总质押奖励
        // 获得此era的总质押
        let era_total_stakes = <ErasTotalStakes<T>>::get(&era);
        // 计算质押奖励： （账户的总质押/此era的总质押）* 此era的质押总奖励
        let staking_reward = Perbill::from_rational(to_num(exposure.total), to_num(era_total_stakes)) * total_era_staking_payout;
        // 总奖励 = 验证奖励+质押奖励
        total_reward = total_reward.saturating_add(staking_reward);
        let total = exposure.total.max(One::one());
        // 4. 计算所有担保人的总奖励 = 验证人设置的担保费率*总奖励
        let estimated_nominations_rewards = <ErasValidatorPrefs<T>>::get(&era, &ledger.stash).commission * total_reward;
        let mut nominations_rewards = Zero::zero();
        // 5. 支付给所有担保人
        for i in &exposure.others {
            // 奖励的比例 = 担保人质押的金额/此验证人的总质押
            let reward_ratio = Perbill::from_rational(i.value, total);
            // 所有担保人的奖励总数 = 奖励的比例 * 担保人的奖励金额
            nominations_rewards += reward_ratio * estimated_nominations_rewards;
            // 依次付款
            if let Some(imbalance) = Self::make_payout(
                &i.who,
                reward_ratio * estimated_nominations_rewards,
            ) {
                Self::deposit_event(Event::<T>::Reward(i.who.clone(), imbalance.peek()));
            };
        }
        // 6. 支付奖励给验证人
        validator_imbalance.maybe_subsume(Self::make_payout(&ledger.stash, total_reward - nominations_rewards));
        Self::deposit_event(Event::<T>::Reward(ledger.stash, validator_imbalance.peek()));
        Ok(())
    }


    /// Session has just ended. Provide the validator set for the next session if it's an era-end, along
    /// with the exposure of the prior validator set.
    fn new_session(
        session_index: SessionIndex,
    ) -> Option<Vec<T::AccountId>> {
        if let Some(current_era) = Self::current_era() {
            // Initial era has been set.
            let current_era_start_session_index = Self::eras_start_session_index(current_era)
                .unwrap_or_else(|| {
                    frame_support::print("Error: start_session_index must be set for current_era");
                    0
                });

            let era_length = session_index.checked_sub(current_era_start_session_index)
                .unwrap_or(0); // Must never happen.
            match ForceEra::<T>::get() {
                Forcing::ForceNew => ForceEra::<T>::kill(),
                Forcing::ForceAlways => (),
                Forcing::NotForcing if era_length >= T::SessionsPerEra::get() => (),
                _ => return None,
            }

            // New era
            Self::new_era(session_index)
        } else {
            // Set initial era
            Self::new_era(session_index)
        }
    }

    /// Start a session potentially starting an era.
    fn start_session(start_session: SessionIndex) {
        // 下一个活跃era+1
        let next_active_era = Self::active_era().map(|e| e.index + 1).unwrap_or(0);
        // This is only `Some` when current era has already progressed to the next era, while the
        // active era is one behind (i.e. in the *last session of the active era*, or *first session
        // of the new current era*, depending on how you look at it).
        // 当当前时代已经进入下一个时代时，这只是`Some`，而活跃时代落后一个（即在活跃时代的最后一个会话，或新当前时代的第一个会话，取决于你如何看待它）
        if let Some(next_active_era_start_session_index) =
        Self::eras_start_session_index(next_active_era)
        {
            if next_active_era_start_session_index == start_session {
                Self::start_era(start_session);
            } else if next_active_era_start_session_index < start_session {
                // This arm should never happen, but better handle it than to stall the staking
                // pallet.
                frame_support::print("Warning: A session appears to have been skipped.");
                Self::start_era(start_session);
            }
        }
    }

    /// End a session potentially ending an era.
    fn end_session(session_index: SessionIndex) {
        if let Some(active_era) = Self::active_era() {
            if let Some(next_active_era_start_session_index) =
            Self::eras_start_session_index(active_era.index + 1)
            {
                if next_active_era_start_session_index == session_index + 1 {
                    Self::end_era(active_era, session_index);
                }
            }
        }
    }

    /// * Increment `active_era.index`,
    /// * reset `active_era.start`,
    /// * update `BondedEras` and apply slashes.
    fn start_era(start_session: SessionIndex) {
        let active_era = ActiveEra::<T>::mutate(|active_era| {
            let new_index = active_era.as_ref().map(|info| info.index + 1).unwrap_or(0);
            *active_era = Some(ActiveEraInfo {
                index: new_index,
                // Set new active era start in next `on_finalize`. To nominations usage of `Time`
                start: None,
            });
            new_index
        });
        log!(
            trace,
            "💸 Start the era {:?}",
            active_era,
        );
        let bonding_duration = T::BondingDuration::get();

        BondedEras::<T>::mutate(|bonded| {
            bonded.push((active_era, start_session));

            if active_era > bonding_duration {
                let first_kept = active_era - bonding_duration;

                // Prune out everything that's from before the first-kept index.
                let n_to_prune = bonded.iter()
                    .take_while(|&&(era_idx, _)| era_idx < first_kept)
                    .count();

                // Kill slashing metadata.
                for (pruned_era, _) in bonded.drain(..n_to_prune) {
                    slashing::clear_era_metadata::<T>(pruned_era);
                }

                if let Some(&(_, first_session)) = bonded.first() {
                    T::SessionInterface::prune_historical_up_to(first_session);
                }
            }
        });

        Self::apply_unapplied_slashes(active_era);
    }


    /// Compute payout for era.
    fn end_era(active_era: ActiveEraInfo, _session_index: SessionIndex) {
        // Note:如果是创世配置，active_era_start可以为NONE
        log!(
            trace,
            "💸 End the era {:?}",
            active_era.index,
        );
        if let Some(active_era_start) = active_era.start {
            let now_as_millis_u64 = T::UnixTime::now().as_millis().saturated_into::<u64>();

            let era_duration = now_as_millis_u64 - active_era_start;
            if !era_duration.is_zero() {
                // active_era_index：活跃era索引
                let active_era_index = active_era.index.clone();
                // 获取此活跃era的总积分
                let points = <ErasRewardPoints<T>>::get(&active_era_index);
                // 获取这个era的gpos总支出
                let gpos_total_payout = Self::total_rewards_in_era(active_era_index);

                // 1. 计算市场支出
                let market_total_payout = Self::calculate_market_payout(active_era_index);
                // 总支出 = gpos支出 + 市场支出
                let mut total_payout = market_total_payout.saturating_add(gpos_total_payout);

                // 2. 减少上一次减费并更新下一次总减费
                let used_fee = T::BenefitInterface::update_era_benefit(active_era_index + 1, total_payout);
                total_payout = total_payout.saturating_sub(used_fee);

                // 3. 拆分质押和验证的奖励
                // 获取当前验证人的人数
                let num_of_validators = Self::current_elected().len();
                // 计算验证奖励和质押奖励
                let total_authoring_payout = Self::get_authoring_and_staking_reward_ratio(num_of_validators as u32) * total_payout;
                let total_staking_payout = total_payout.saturating_sub(total_authoring_payout);

                // 4. 将验证奖励放入<ErasAuthoringPayout>中
                for (v, p) in points.individual.iter() {
                    if *p != 0u32 {
                        let authoring_reward =
                            // 验证人奖励 = (积分/总积分)*总奖励
                            Perbill::from_rational(*p, points.total) * total_authoring_payout;
                        <ErasAuthoringPayout<T>>::insert(&active_era_index, v, authoring_reward);
                    }
                }

                // 5. 将质押总奖励放入<ErasStakingPayout>中
                <ErasStakingPayout<T>>::insert(active_era_index, total_staking_payout);

                // 6. Deposit era reward event
                Self::deposit_event(Event::<T>::EraReward(active_era_index, total_authoring_payout, total_staking_payout));

                // TODO: enable treasury and might bring this back
                // T::Reward::on_unbalanced(total_imbalance);
                // This is not been used
                // T::RewardRemainder::on_unbalanced(T::Currency::issue(rest));
            }
        }
    }

    fn total_rewards_in_era(active_era: EraIndex) -> BalanceOf<T> {
        // 1. 判断是否开始奖励
        if active_era < Self::start_reward_era() { return Zero::zero(); }
        // 今年要发放的总钱数
        let mut maybe_rewards_this_year = FIRST_YEAR_REWARDS;
        // 系统中的总发行量
        let total_issuance = TryInto::<u128>::try_into(T::Currency::total_issuance())
            .ok()
            .unwrap();
        // 每年的毫秒数 (365.25 天).
        const MILLISECONDS_PER_YEAR: u64 = 1000 * 3600 * 24 * 36525 / 100;
        // 1 Julian year = (365.25d * 24h * 3600s * 1000ms) / (millisecs_in_era = block_time * blocks_num_in_era)
        // 每年的毫秒数 / 每块的毫秒数 = 一年有多少块
        // 一个session的持续时间 * 每个era有多少个session = 一个era的持续时间
        // 一年的总块数 / 一个era的时间 = 一年的era数目
        let year_in_eras = MILLISECONDS_PER_YEAR / MILLISECS_PER_BLOCK / (EPOCH_DURATION_IN_BLOCKS * T::SessionsPerEra::get()) as u64;
        // （活跃的era - 开始奖励的era） / 一年的era数 = 现在是第几年
        let year_num = active_era.saturating_sub(Self::start_reward_era()) as u64 / year_in_eras;
        for _ in 0..year_num {
            // 每年通胀88%
            maybe_rewards_this_year = maybe_rewards_this_year * REWARD_DECREASE_RATIO.0 / REWARD_DECREASE_RATIO.1;

            // 如果奖励通胀<= 2.8%，停止减少
            let min_rewards_this_year = total_issuance / MIN_REWARD_RATIO.1 * MIN_REWARD_RATIO.0;
            if maybe_rewards_this_year <= min_rewards_this_year {
                maybe_rewards_this_year = min_rewards_this_year;
                break;
            }
        }

        // 过了区块奖励期(4年)后，全网有效 Staking 比率低于 30%，会进行通胀补偿
        if year_num >= EXTRA_REWARD_START_YEAR {
            maybe_rewards_this_year = maybe_rewards_this_year.saturating_add(Self::supply_extra_rewards_due_to_low_effective_staking_ratio(total_issuance));
        }

        // 给这个era的奖励 = 今年的奖励 / 一年的era数目
        let reward_this_era = maybe_rewards_this_year / year_in_eras as u128;

        reward_this_era.try_into().ok().unwrap()
    }

    fn supply_extra_rewards_due_to_low_effective_staking_ratio(total_issuance: u128) -> u128 {
        // 获得有效质押比例
        let maybe_effective_staking_ratio = Self::maybe_get_effective_staking_ratio(BalanceOf::<T>::saturated_from(total_issuance));
        // 计算补偿的值
        if let Some(effective_staking_ratio) = maybe_effective_staking_ratio {
            // 如果有效质押小于30%，则给予一个补偿
            if effective_staking_ratio < Permill::from_percent(30) {
                // (1 - sr / 0.3) * 0.08 * total_issuance = total_issuance * 8 / 100 - sr * total_issuance * 8 / 30
                return (total_issuance / 100 * 8).saturating_sub(effective_staking_ratio * total_issuance / 30 * 8);
            }
        }
        return 0;
    }

    fn maybe_get_effective_staking_ratio(total_issuance: BalanceOf<T>) -> Option<Permill> {
        let to_num =
            |b: BalanceOf<T>| <T::CurrencyToVote as Convert<BalanceOf<T>, u128>>::convert(b);
        if let Some(active_era) = Self::active_era() {
            //获得此era的总有效质押
            let total_effective_stake = <ErasTotalStakes<T>>::get(&active_era.index);
            //返回 有效质押比例 = 总有效质押 / 总发行量
            return Some(Permill::from_rational(to_num(total_effective_stake), to_num(total_issuance)));
        }
        None
    }

    fn calculate_market_payout(active_era: EraIndex) -> BalanceOf<T> {
        //市场需要withdraw_staking_pool接口
        let total_dsm_staking_payout: BalanceOf<T> = T::PaymentInterface::withdraw_staking_pool();

        let duration = T::MarketStakingPotDuration::get();
        let dsm_staking_payout_per_era = Perbill::from_rational(1, duration) * total_dsm_staking_payout;
        // Reward starts from this era.
        for i in 0..duration {
            <ErasMarketPayout<T>>::mutate(active_era + i, |payout| match *payout {
                Some(amount) => *payout = Some(amount.saturating_add(dsm_staking_payout_per_era.clone())),
                None => *payout = Some(dsm_staking_payout_per_era.clone())
            });
        }
        Self::eras_market_payout(active_era).unwrap()
    }

    pub fn get_authoring_and_staking_reward_ratio(num_of_validators: u32) -> Perbill {
        match num_of_validators {
            0..=500 => Perbill::from_percent(20),
            501..=1000 => Perbill::from_percent(25),
            1001..=2500 => Perbill::from_percent(30),
            2501..=5000 => Perbill::from_percent(40),
            5001..=u32::MAX => Perbill::from_percent(50),
        }
    }

    /// The era has changed - enact new staking set.
    ///
    /// NOTE: This always happens immediately before a session change to ensure that new validators
    /// get a chance to set their session keys.
    /// This also checks stake limitation based on work reports
    fn new_era(start_session_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        // 增加或设置CurrentEra(当前纪元号)
        let current_era = CurrentEra::<T>::mutate(|s| {
            *s = Some(s.map(|s| s + 1).unwrap_or(0));
            s.unwrap()
        });
        log!(
            trace,
            "💸 Plan a new era {:?}",
            current_era,
        );
        ErasStartSessionIndex::<T>::insert(&current_era, &start_session_index);

        // 清理旧的era信息
        if let Some(old_era) = current_era.checked_sub(Self::history_depth() + 1) {
            Self::clear_era_information(old_era);
        }

        // 给新era设置信息，并返回验证人集合
        let maybe_new_validators = Self::select_and_update_validators(current_era);

        maybe_new_validators
    }

    /// Select the new validator set at the end of the era.
    ///
    /// Returns the a set of newly selected _stash_ IDs.
    ///
    /// This should only be called at the end of an era.
    fn select_and_update_validators(current_era: EraIndex) -> Option<Vec<T::AccountId>> {
        // I. 获取验证人数量和最小验证人数量
        let validator_count = <Validators<T>>::iter().count();
        let minimum_validator_count = Self::minimum_validator_count().max(1) as usize;

        if validator_count < minimum_validator_count {
            // There were not enough validators for even our minimal level of functionality.
            // This is bad🥺.
            // We should probably disable all functionality except for block production
            // and let the chain keep producing blocks until we can decide on a sufficiently
            // substantial set.
            // TODO: [Substrate]substrate#2494
            return None;
        }

        //设置to_votes 将balance转为u128
        let to_votes =
            |b: BalanceOf<T>| <T::CurrencyToVote as Convert<BalanceOf<T>, u128>>::convert(b);
        //设置to_votes 将u128转为balance
        let to_balance = |e: u128| <T::CurrencyToVote as Convert<u128, BalanceOf<T>>>::convert(e);

        // II. 构建并添加 V/G graph
        // TC is O(V + G*1), V means validator's number, G means nominator's number
        // DB try is 2

        log!(
            debug,
            "💸 Construct and fill in the V/G graph for the era {:?}.",
            current_era,
        );
        //vg_graphre 验证人和他们的担保人集合
        let mut vg_graph: BTreeMap<T::AccountId, Vec<IndividualExposure<T::AccountId, BalanceOf<T>>>> =
            <Validators<T>>::iter().map(|(v_stash, _)|
                (v_stash, Vec::<IndividualExposure<T::AccountId, BalanceOf<T>>>::new())
            ).collect();

        //遍历所有担保人
        for (nominator, nominations) in <Nominators<T>>::iter() {
            let Nominations { total: _, submitted_in, mut targets, suppressed: _ } = nominations;
            //过滤掉担保人在最后一次惩罚发生前的担保目标
            targets.retain(|ie| {
                <Self as Store>::SlashingSpans::get(&ie.who).map_or(
                    true,
                    |spans| submitted_in >= spans.last_nonzero_slash(),
                )
            });

            //遍历每个担保人的担保目标，并添加到vg_graph中
            for target in targets {
                if let Some(g) = vg_graph.get_mut(&target.who) {
                    g.push(IndividualExposure {
                        who: nominator.clone(),
                        value: target.value,
                    });
                }
            }
        }

        // III. 这个部分会包括以下几个部分
        // 1. 获得 `ErasStakers` with `stake_limit` and `vg_graph`
        // 2. 获得 `ErasValidatorPrefs`
        // 3. 获得 `total_valid_stakes`
        // 4. 填入 `validator_stakes`
        log!(
            debug,
            "💸 Build the eras Stakers for the era {:?}.",
            current_era,
        );
        let mut eras_total_stakes: BalanceOf<T> = Zero::zero();
        let mut validators_stakes: Vec<(T::AccountId, u128)> = vec![];
        //遍历验证人和担保人集合
        for (v_stash, voters) in vg_graph.iter() {
            //获得controller账号
            let v_controller = Self::bonded(v_stash).unwrap();
            //获得验证人的账单
            let v_ledger: StakingLedger<T::AccountId, BalanceOf<T>> =
                Self::ledger(&v_controller).unwrap();
            //获得质押上限，如果没有则为0
            let stake_limit = Self::stake_limit(v_stash).unwrap_or(Zero::zero());
            // 0. 如果质押上限为0，添加 `validator_stakes` 但是跳过添加 `eras_stakers`
            if stake_limit == Zero::zero() {
                validators_stakes.push((v_stash.clone(), 0));
                continue;
            }

            // 1. 计算比率
            //总质押额 = 验证人的活跃质押金 + 所有担保人的担保金额
            let total_stakes = v_ledger.active.saturating_add(
                voters.iter().fold(
                    Zero::zero(),
                    |acc, ie| acc.saturating_add(ie.value),
                ));
            //有效比率 = 质押上限金额/总质押额    最大值为1
            let valid_votes_ratio = Perbill::from_rational(stake_limit, total_stakes).min(Perbill::one());

            // 2. 计算验证人的有效质押
            let own_stake = valid_votes_ratio * v_ledger.active;

            // 3. 构建 exposure
            let mut new_exposure = Exposure {
                total: own_stake,
                own: own_stake,
                others: vec![],
            };
            for voter in voters {
                //担保人有效质押金额
                let g_valid_stake = valid_votes_ratio * voter.value;
                //总量质押 = 担保人有效质押 + 验证人有效质押
                new_exposure.total = new_exposure.total.saturating_add(g_valid_stake);
                //插入验证人的担保人集合
                new_exposure.others.push(IndividualExposure {
                    who: voter.who.clone(),
                    value: g_valid_stake,
                });
            }

            // 4. Update snapshots
            //插入验证人
            <ErasStakers<T>>::insert(&current_era, &v_stash, new_exposure.clone());
            //exposure_total：当前验证人的所有质押金额
            let exposure_total = new_exposure.total;
            let mut exposure_clipped = new_exposure;
            //验证人最大的担保人数量
            let clipped_max_len = T::MaxNominatorRewardedPerValidator::get() as usize;
            //如果超过最大担保人数量，则取金额最大的前clipped_max_len个
            if exposure_clipped.others.len() > clipped_max_len {
                exposure_clipped.others.sort_by(|a, b| a.value.cmp(&b.value).reverse());
                exposure_clipped.others.truncate(clipped_max_len);
            }
            <ErasStakersClipped<T>>::insert(&current_era, &v_stash, exposure_clipped);

            //插入验证人到ErasValidatorPrefs
            <ErasValidatorPrefs<T>>::insert(&current_era, &v_stash, Self::validators(&v_stash).clone());

            //如果验证人的总质押不为0，则让eras_total_stakes等于总质押
            //如果为0，则eras_total_stakes则为u128的最大值
            if let Some(maybe_total_stakes) = eras_total_stakes.checked_add(&exposure_total) {
                eras_total_stakes = maybe_total_stakes;
            } else {
                eras_total_stakes = to_balance(u64::max_value() as u128);
            }

            // 5. 添加到validators_stakes验证人列表
            validators_stakes.push((v_stash.clone(), to_votes(exposure_total)))
        }

        // IV. 随机选举算法
        //要选举的数量 = 最小值(最大验证人的数量,当前验证人数量)
        let to_elect = (Self::validator_count() as usize).min(validators_stakes.len());

        // 如果没有验证人，就和原来一样
        if to_elect < minimum_validator_count {
            return None;
        }

        let elected_stashes = Self::do_election(validators_stakes, to_elect);
        log!(
            info,
            "💸 new validator set of size {:?} has been elected via for era {:?}",
            elected_stashes.len(),
            current_era,
        );

        // V.更新
        // 设置session的新验证人集合
        <CurrentElected<T>>::put(&elected_stashes);

        // 更新区块的质押
        <ErasTotalStakes<T>>::insert(&current_era, eras_total_stakes);

        // 返回新的验证者集合（即使和原来的集合一样）
        Some(elected_stashes)
    }

    fn do_election(
        mut validators_stakes: Vec<(T::AccountId, u128)>,
        to_elect: usize) -> Vec<T::AccountId> {
        // Select new validators by top-down their total `valid` stakes
        // then randomly choose some of them from the top validators

        //候选人数量 = 最小值(当前要选举的验证人数量,要选举的数量*2)
        let candidate_to_elect = validators_stakes.len().min(to_elect * 2);
        // 按照质押金额从高到低排序
        validators_stakes.sort_by(|a, b| b.1.cmp(&a.1));

        // 选择前candidate_to_elect个验证人
        let candidate_stashes = validators_stakes[0..candidate_to_elect]
            .iter()
            .map(|(who, stakes)| (who.clone(), *stakes))
            .collect::<Vec<(T::AccountId, u128)>>();

        // TODO: enable it back when the network is stable
        // // shuffle it
        // Self::shuffle_candidates(&mut candidate_stashes);

        // choose elected_stashes number of validators
        let elected_stashes = candidate_stashes[0..to_elect]
            .iter()
            .map(|(who, _stakes)| who.clone())
            .collect::<Vec<T::AccountId>>();
        elected_stashes
    }

    /// 计算质押上限
    fn calculate_total_stake_limit() -> u128 {
        //总发行量
        let total_issuance = T::Currency::total_issuance();
        //  如果有效质押比例小于某个值，我们应该增加总质押限额
        let (integer, frac) = Self::limit_ratio_according_to_effective_staking(total_issuance.clone());
        let frac = frac * total_issuance;
        let integer = BalanceOf::<T>::saturated_from(integer).saturating_mul(total_issuance);
        // This value can be larger than total issuance.
        let total_stake_limit = TryInto::<u128>::try_into(integer.saturating_add(frac))
            .ok()
            .unwrap();
        total_stake_limit
    }

    pub fn limit_ratio_according_to_effective_staking(total_issuance: BalanceOf<T>) -> (u128, Perbill) {
        // 获得全网有效质押比例
        let maybe_effective_stake_ratio = Self::maybe_get_effective_staking_ratio(total_issuance);
        //
        if let Some(effective_stake_ratio) = maybe_effective_stake_ratio {
            //?
            let (integer, frac) = total_stake_limit_ratio(effective_stake_ratio);
            return (integer.into(), frac);
        }
        return (0u128, Perbill::zero());
    }


    /// Insert new or update old stake limit
    pub fn upsert_stake_limit(account_id: &T::AccountId, limit: BalanceOf<T>) {
        <StakeLimit<T>>::insert(account_id, limit);
    }

    pub fn update_stage_one_stake_limit(workload_map: BTreeMap<T::AccountId, u128>) -> u64 {
        // In stage one, state limit / own workload is fixed to T::SPowerRatio
        let mut validators_count = 0;
        for (v_stash, _) in <Validators<T>>::iter() {
            validators_count += 1;
            let v_own_workload = workload_map.get(&v_stash).unwrap_or(&0u128);
            Self::upsert_stake_limit(
                &v_stash,
                Self::stage_one_stake_limit_of(*v_own_workload),
            );
        }
        validators_count
    }

    pub fn update_stage_two_stake_limit(workload_map: BTreeMap<T::AccountId, u128>, total_workload: u128, total_stake_limit: u128) -> u64 {
        let mut validators_count = 0;
        let byte_to_kilobyte = |workload_in_byte: u128| {
            workload_in_byte / 1024
        };

        // Decrease the precision to kb to avoid overflow
        let total_workload_in_kb = byte_to_kilobyte(total_workload);
        for (v_stash, _) in <Validators<T>>::iter() {
            validators_count += 1;
            let v_own_workload = workload_map.get(&v_stash).unwrap_or(&0u128);
            // Decrease the precision to kb to avoid overflow
            let v_own_workload_in_kb = byte_to_kilobyte(*v_own_workload);
            Self::upsert_stake_limit(
                &v_stash,
                Self::stage_two_stake_limit_of(v_own_workload_in_kb, total_workload_in_kb, total_stake_limit),
            );
        }
        validators_count
    }

    /// Calculate the stake limit by storage workloads, returns the stake limit value
    ///
    /// # <weight>
    /// - Independent of the arguments. Insignificant complexity.
    /// - O(1).
    /// - 0 DB entry.
    /// # </weight>
    pub fn stage_one_stake_limit_of(own_workloads: u128) -> BalanceOf<T> {
        // we treat 1 terabytes as 1_000_000_000_000 for make `mapping_ratio = 1`
        if let Some(storage_stakes) = own_workloads.checked_mul(T::SPowerRatio::get()) {
            storage_stakes.try_into().ok().unwrap()
        } else {
            Zero::zero()
        }
    }

    /// Calculate the stake limit by storage workloads, returns the stake limit value
    ///
    /// # <weight>
    /// - Independent of the arguments. Insignificant complexity.
    /// - O(1).
    /// - 0 DB entry.
    /// # </weight>
    pub fn stage_two_stake_limit_of(own_workloads_in_kb: u128, total_workloads_in_kb: u128, total_stake_limit: u128) -> BalanceOf<T> {
        // total_workloads cannot be zero, or system go panic!
        if total_workloads_in_kb == 0 {
            Zero::zero()
        } else {
            let workloads_to_stakes = (own_workloads_in_kb.wrapping_mul(total_stake_limit) / total_workloads_in_kb) as u128;
            workloads_to_stakes.try_into().ok().unwrap()
        }
    }

    // ///随机打乱验证人
    // fn shuffle_candidates(candidates_stakes: &mut Vec<(T::AccountId, u128)>) {
    //     // 1. Construct random seed, 👼 bless the randomness
    //     // seed = [ block_hash, phrase ]
    //     let phrase = b"candidates_shuffle";
    //     let bn = <frame_system::Module<T>>::block_number();
    //     let bh: T::Hash = <frame_system::Module<T>>::block_hash(bn);
    //     let seed = [
    //         &bh.as_ref()[..],
    //         &phrase.encode()[..]
    //     ].concat();
    //
    //     // we'll need a random seed here.
    //     let seed = T::Randomness::random(seed.as_slice());
    //     // seed needs to be nominationsd to be 32 bytes.
    //     let seed = <[u8; 32]>::decode(&mut TrailingZeroInput::new(seed.as_ref()))
    //         .expect("input is padded with zeroes; qed");
    //     let mut rng = ChaChaRng::from_seed(seed);
    //     for i in (0..candidates_stakes.len()).rev() {
    //         let random_index = (rng.next_u32() % (i as u32 + 1)) as usize;
    //         candidates_stakes.swap(random_index, i);
    //     }
    // }

    /// Remove all associated data of a stash account from the staking system.
    ///
    /// Assumes storage is upgraded before calling.
    ///
    /// This is called:
    /// - after a `withdraw_unbonded()` call that frees all of a stash's bonded balance.
    /// - through `reap_stash()` if the balance has fallen to zero (through slashing).
    fn kill_stash(stash: &T::AccountId, num_slashing_spans: u32) -> DispatchResult {
        let controller = <Bonded<T>>::get(stash).ok_or(Error::<T>::NotStash)?;

        slashing::clear_stash_metadata::<T>(stash, num_slashing_spans)?;

        <Bonded<T>>::remove(stash);
        <Ledger<T>>::remove(&controller);

        <Payee<T>>::remove(stash);
        Self::do_remove_validator(stash);
        Self::do_remove_nominator(stash);

        <StakeLimit<T>>::remove(stash);

        frame_system::Pallet::<T>::dec_consumers(stash);

        Ok(())
    }

    /// Clear all era information for given era.
    fn clear_era_information(era_index: EraIndex) {
        <ErasStakers<T>>::remove_prefix(era_index, None);
        <ErasStakersClipped<T>>::remove_prefix(era_index, None);
        <ErasValidatorPrefs<T>>::remove_prefix(era_index, None);
        <ErasStakingPayout<T>>::remove(era_index);
        <ErasMarketPayout<T>>::remove(era_index);
        <ErasTotalStakes<T>>::remove(era_index);
        <ErasAuthoringPayout<T>>::remove_prefix(era_index, None);
        <ErasRewardPoints<T>>::remove(era_index);
        <ErasStartSessionIndex<T>>::remove(era_index);
    }

    /// Apply previously-unapplied slashes on the beginning of a new era, after a delay.
    fn apply_unapplied_slashes(active_era: EraIndex) {
        let slash_defer_duration = T::SlashDeferDuration::get();
        <Self as Store>::EarliestUnappliedSlash::mutate(|earliest| if let Some(ref mut earliest) = earliest {
            let keep_from = active_era.saturating_sub(slash_defer_duration);
            for era in (*earliest)..keep_from {
                let era_slashes = <Self as Store>::UnappliedSlashes::take(&era);
                for slash in era_slashes {
                    slashing::apply_slash::<T>(slash);
                }
            }

            *earliest = (*earliest).max(keep_from)
        })
    }

    /// Add reward points to validators using their stash account ID.
    ///
    /// Validators are keyed by stash account ID and must be in the current elected set.
    ///
    /// For each element in the iterator the given number of points in u32 is added to the
    /// validator, thus duplicates are handled.
    ///
    /// At the end of the era each the total payout will be distributed among validator
    /// relatively to their points.
    ///
    /// COMPLEXITY: Complexity is `number_of_validator_to_reward x current_elected_len`.
    pub fn reward_by_ids(
        validators_points: impl IntoIterator<Item=(T::AccountId, u32)>
    ) {
        if let Some(active_era) = Self::active_era() {
            <ErasRewardPoints<T>>::mutate(active_era.index, |era_rewards| {
                for (validator, points) in validators_points.into_iter() {
                    *era_rewards.individual.entry(validator).or_default() += points;
                    era_rewards.total += points;
                }
            });
        }
    }

    /// Ensures that at the end of the current session there will be a new era.
    fn ensure_new_era() {
        match ForceEra::<T>::get() {
            Forcing::ForceAlways | Forcing::ForceNew => (),
            _ => ForceEra::<T>::put(Forcing::ForceNew),
        }
    }


    /// This function will add a nominator to the `Nominators` storage map,
    /// and keep track of the `CounterForNominators`.
    ///
    /// If the nominator already exists, their nominations will be updated.
    pub fn do_add_nominator(who: &T::AccountId, nominations: Nominations<T::AccountId, BalanceOf<T>>) {
        if !Nominators::<T>::contains_key(who) {
            CounterForNominators::<T>::mutate(|x| x.saturating_inc())
        }
        Nominators::<T>::insert(who, nominations);
    }

    /// This function will remove a nominator from the `Nominators` storage map,
    /// and keep track of the `CounterForNominators`.
    pub fn do_remove_nominator(who: &T::AccountId) {
        if Nominators::<T>::contains_key(who) {
            Nominators::<T>::remove(who);
            CounterForNominators::<T>::mutate(|x| x.saturating_dec());
        }
    }

    /// This function will add a validator to the `Validators` storage map,
    /// and keep track of the `CounterForValidators`.
    ///
    /// If the validator already exists, their preferences will be updated.
    pub fn do_add_validator(who: &T::AccountId, prefs: ValidatorPrefs) {
        if !Validators::<T>::contains_key(who) {
            CounterForValidators::<T>::mutate(|x| x.saturating_inc())
        }
        Validators::<T>::insert(who, prefs);
    }

    /// This function will remove a validator from the `Validators` storage map,
    /// and keep track of the `CounterForValidators`.
    pub fn do_remove_validator(who: &T::AccountId) {
        if Validators::<T>::contains_key(who) {
            Validators::<T>::remove(who);
            CounterForValidators::<T>::mutate(|x| x.saturating_dec());
        }
    }
}

/// In this implementation `new_session(session)` must be called before `end_session(session-1)`
/// i.e. the new session must be planned before the ending of the previous session.
///
/// Once the first new_session is planned, all session must start and then end in order, though
/// some session can lag in between the newest session planned and the latest session started.
impl<T: Config> pallet_session::SessionManager<T::AccountId> for Pallet<T> {
    fn new_session(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        log!(trace, "planning new session {}", new_index);
        CurrentPlannedSession::<T>::put(new_index);
        Self::new_session(new_index)
    }
    fn start_session(start_index: SessionIndex) {
        log!(trace, "starting session {}", start_index);
        Self::start_session(start_index)
    }
    fn end_session(end_index: SessionIndex) {
        log!(trace, "ending session {}", end_index);
        Self::end_session(end_index)
    }
}

impl<T: Config> historical::SessionManager<T::AccountId, Exposure<T::AccountId, BalanceOf<T>>>
for Pallet<T>
{
    fn new_session(
        new_index: SessionIndex,
    ) -> Option<Vec<(T::AccountId, Exposure<T::AccountId, BalanceOf<T>>)>> {
        <Self as pallet_session::SessionManager<_>>::new_session(new_index).map(|validators| {
            let current_era = Self::current_era()
                // Must be some as a new era has been created.
                .unwrap_or(0);

            validators.into_iter().map(|v| {
                let exposure = Self::eras_stakers(current_era, &v);
                (v, exposure)
            }).collect()
        })
    }
    fn new_session_genesis(
        new_index: SessionIndex,
    ) -> Option<Vec<(T::AccountId, Exposure<T::AccountId, BalanceOf<T>>)>> {
        <Self as pallet_session::SessionManager<_>>::new_session_genesis(new_index).map(|validators| {
            let current_era = Self::current_era()
                // Must be some as a new era has been created.
                .unwrap_or(0);

            validators.into_iter().map(|v| {
                let exposure = Self::eras_stakers(current_era, &v);
                (v, exposure)
            }).collect()
        })
    }
    fn start_session(start_index: SessionIndex) {
        <Self as pallet_session::SessionManager<_>>::start_session(start_index)
    }
    fn end_session(end_index: SessionIndex) {
        <Self as pallet_session::SessionManager<_>>::end_session(end_index)
    }
}

/// Add reward points to block authors:
/// * 20 points to the block producer for producing a (non-uncle) block in the relay chain,
/// * 2 points to the block producer for each reference to a previously unreferenced uncle, and
/// * 1 point to the producer of each referenced uncle block.
impl<T> pallet_authorship::EventHandler<T::AccountId, T::BlockNumber> for Pallet<T>
    where
        T: Config + pallet_authorship::Config + pallet_session::Config,
{
    fn note_author(author: T::AccountId) {
        Self::reward_by_ids(vec![(author, 20)])
    }
    fn note_uncle(author: T::AccountId, _age: T::BlockNumber) {
        Self::reward_by_ids(vec![
            (<pallet_authorship::Pallet<T>>::author(), 2),
            (author, 1),
        ])
    }
}

impl<T: Config> worker::Works<T::AccountId> for Pallet<T> {
    fn report_works(workload_map: BTreeMap<T::AccountId, u128>, total_workload: u128) -> Weight {
        let mut consumed_weight: Weight = 0;
        let mut add_db_reads_writes = |reads, writes| {
            consumed_weight += T::DbWeight::get().reads_writes(reads, writes);
        };
        // 1. 计算质押上限
        let total_stake_limit = Self::calculate_total_stake_limit();
        // 矿工数量
        let group_counts = workload_map.len() as u32;
        add_db_reads_writes(3, 0);
        // 2. total_workload * SPowerRatio < total_stake_limit => stage one
        //如果 总存储量*存储能力的比率 < 总质押限制
        //SPowerRatio 现为1 即 1TB可质押1CRU
        //如果 总存储量*存储能力的比率 > 总质押限制
        //可质押量为 我的存储量* 总质押限制/总质押量
        let validators_count: u64 = if total_workload.saturating_mul(T::SPowerRatio::get()) < total_stake_limit {
            Self::update_stage_one_stake_limit(workload_map)
        } else {
            Self::update_stage_two_stake_limit(workload_map, total_workload, total_stake_limit)
        };
        add_db_reads_writes(validators_count, validators_count);
        Self::deposit_event(Event::<T>::UpdateStakeLimitSuccess(group_counts));
        consumed_weight
    }
}


/// A `Convert` implementation that finds the stash of the given controller account,
/// if any.
pub struct StashOf<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Convert<T::AccountId, Option<T::AccountId>> for StashOf<T> {
    fn convert(controller: T::AccountId) -> Option<T::AccountId> {
        <Pallet<T>>::ledger(&controller).map(|l| l.stash)
    }
}

/// A typed conversion from stash account ID to the active exposure of nominators
/// on that account.
///
/// Active exposure is the exposure of the validator set currently validating, i.e. in
/// `active_era`. It can differ from the latest planned exposure in `current_era`.
pub struct ExposureOf<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Convert<T::AccountId, Option<Exposure<T::AccountId, BalanceOf<T>>>>
for ExposureOf<T>
{
    fn convert(validator: T::AccountId) -> Option<Exposure<T::AccountId, BalanceOf<T>>> {
        <Pallet<T>>::active_era()
            .map(|active_era| <Pallet<T>>::eras_stakers(active_era.index, &validator))
    }
}

/// This is intended to be used with `FilterHistoricalOffences`.
impl<T: Config>
OnOffenceHandler<T::AccountId, pallet_session::historical::IdentificationTuple<T>, Weight>
for Pallet<T>
    where
        T: pallet_session::Config<ValidatorId=<T as frame_system::Config>::AccountId>,
        T: pallet_session::historical::Config<
            FullIdentification=Exposure<<T as frame_system::Config>::AccountId, BalanceOf<T>>,
            FullIdentificationOf=ExposureOf<T>,
        >,
        T::SessionHandler: pallet_session::SessionHandler<<T as frame_system::Config>::AccountId>,
        T::SessionManager: pallet_session::SessionManager<<T as frame_system::Config>::AccountId>,
        T::ValidatorIdOf: Convert<
            <T as frame_system::Config>::AccountId,
            Option<<T as frame_system::Config>::AccountId>,
        >,
{
    fn on_offence(
        offenders: &[OffenceDetails<
            T::AccountId,
            pallet_session::historical::IdentificationTuple<T>,
        >],
        slash_fraction: &[Perbill],
        slash_session: SessionIndex,
    ) -> Weight {
        let reward_proportion = SlashRewardFraction::<T>::get();
        let mut consumed_weight: Weight = 0;
        let mut add_db_reads_writes = |reads, writes| {
            consumed_weight += T::DbWeight::get().reads_writes(reads, writes);
        };

        let active_era = {
            let active_era = Self::active_era();
            add_db_reads_writes(1, 0);
            if active_era.is_none() {
                // This offence need not be re-submitted.
                return consumed_weight;
            }
            active_era.expect("value checked not to be `None`; qed").index
        };
        let active_era_start_session_index = Self::eras_start_session_index(active_era)
            .unwrap_or_else(|| {
                frame_support::print("Error: start_session_index must be set for current_era");
                0
            });
        add_db_reads_writes(1, 0);

        let window_start = active_era.saturating_sub(T::BondingDuration::get());

        // Fast path for active-era report - most likely.
        // `slash_session` cannot be in a future active era. It must be in `active_era` or before.
        let slash_era = if slash_session >= active_era_start_session_index {
            active_era
        } else {
            let eras = BondedEras::<T>::get();
            add_db_reads_writes(1, 0);

            // Reverse because it's more likely to find reports from recent eras.
            match eras.iter().rev().filter(|&&(_, ref sesh)| sesh <= &slash_session).next() {
                Some(&(ref slash_era, _)) => *slash_era,
                // Before bonding period. defensive - should be filtered out.
                None => return consumed_weight,
            }
        };

        <Self as Store>::EarliestUnappliedSlash::mutate(|earliest| {
            if earliest.is_none() {
                *earliest = Some(active_era)
            }
        });
        add_db_reads_writes(1, 1);

        let slash_defer_duration = T::SlashDeferDuration::get();

        let invulnerables = Self::invulnerables();
        add_db_reads_writes(1, 0);

        for (details, slash_fraction) in offenders.iter().zip(slash_fraction) {
            let (stash, exposure) = &details.offender;

            // Skip if the validator is invulnerable.
            if invulnerables.contains(stash) {
                continue;
            }

            let unapplied = slashing::compute_slash::<T>(slashing::SlashParams {
                stash,
                slash: *slash_fraction,
                exposure,
                slash_era,
                window_start,
                now: active_era,
                reward_proportion,
            });

            if let Some(mut unapplied) = unapplied {
                let nominators_len = unapplied.others.len() as u64;
                let reporters_len = details.reporters.len() as u64;

                {
                    let upper_bound = 1 /* Validator/NominatorSlashInEra */ + 2 /* fetch_spans */;
                    let rw = upper_bound + nominators_len * upper_bound;
                    add_db_reads_writes(rw, rw);
                }
                unapplied.reporters = details.reporters.clone();
                if slash_defer_duration == 0 {
                    // Apply right away.
                    slashing::apply_slash::<T>(unapplied);
                    {
                        let slash_cost = (6, 5);
                        let reward_cost = (2, 2);
                        add_db_reads_writes(
                            (1 + nominators_len) * slash_cost.0 + reward_cost.0 * reporters_len,
                            (1 + nominators_len) * slash_cost.1 + reward_cost.1 * reporters_len,
                        );
                    }
                } else {
                    // Defer to end of some `slash_defer_duration` from now.
                    <Self as Store>::UnappliedSlashes::mutate(
                        active_era,
                        move |for_later| for_later.push(unapplied),
                    );
                    add_db_reads_writes(1, 1);
                }
            } else {
                add_db_reads_writes(4 /* fetch_spans */, 5 /* kick_out_if_recent */)
            }
        }

        consumed_weight
    }
}

/// Filter historical offences out and only allow those from the bonding period.
pub struct FilterHistoricalOffences<T, R> {
    _inner: sp_std::marker::PhantomData<(T, R)>,
}

impl<T, Reporter, Offender, R, O> ReportOffence<Reporter, Offender, O>
for FilterHistoricalOffences<Pallet<T>, R>
    where
        T: Config,
        R: ReportOffence<Reporter, Offender, O>,
        O: Offence<Offender>,
{
    fn report_offence(reporters: Vec<Reporter>, offence: O) -> Result<(), OffenceError> {
        // Disallow any slashing from before the current bonding period.
        let offence_session = offence.session_index();
        let bonded_eras = BondedEras::<T>::get();

        if bonded_eras.first().filter(|(_, start)| offence_session >= *start).is_some() {
            R::report_offence(reporters, offence)
        } else {
            <Pallet<T>>::deposit_event(
                Event::<T>::OldSlashingReportDiscarded(offence_session)
            );
            Ok(())
        }
    }

    fn is_known_offence(offenders: &[Offender], time_slot: &O::TimeSlot) -> bool {
        R::is_known_offence(offenders, time_slot)
    }
}

/// Check that list is sorted and has no duplicates.
fn is_sorted_and_unique(list: &[u32]) -> bool {
    list.windows(2).all(|w| w[0] < w[1])
}
