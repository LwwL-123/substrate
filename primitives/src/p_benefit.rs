use crate::EraIndex;
use sp_runtime::{DispatchError, Perbill};
use frame_support::traits::WithdrawReasons;

pub trait BenefitInterface<AccountId, Balance, NegativeImbalance> {
    fn update_era_benefit(next_era: EraIndex, total_benefits: Balance) -> Balance;

    fn update_reward(who: &AccountId, value: Balance);

    fn maybe_reduce_fee(who: &AccountId, fee: Balance, reasons: WithdrawReasons) -> Result<NegativeImbalance, DispatchError>;

    fn maybe_free_count(who: &AccountId) -> bool;

    fn get_collateral_and_reward(who: &AccountId) -> (Balance, Balance);

    fn get_market_funds_ratio(who: &AccountId) -> Perbill;
}