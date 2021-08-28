#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>

use sp_std::prelude::*;
use frame_support::{
    decl_event, decl_storage, decl_module, decl_error, ensure,
    weights::{Weight},
    dispatch::HasCompact,
    traits::{Currency, ReservableCurrency, Get,
             WithdrawReasons, ExistenceRequirement, Imbalance},
    pallet_prelude::*,
};


use frame_system::ensure_signed;
use codec::{Encode, Decode};
#[cfg(feature = "std")]
use sp_runtime::DispatchError;

use sp_runtime::{
    DispatchResult, Perbill, RuntimeDebug,
    traits::{Zero, Saturating, AtLeast32BitUnsigned}, SaturatedConversion
};
pub use pallet::*;
use primitives::{EraIndex, p_benefit::BenefitInterface};
pub mod weight;

type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

const MAX_UNLOCKING_CHUNKS: usize = 16;

pub trait WeightInfo {
    fn add_benefit_funds() -> Weight;
    fn cut_benefit_funds() -> Weight;
    fn rebond_benefit_funds() -> Weight;
    fn withdraw_benefit_funds() -> Weight;
}

/// 只是一个 Balance/BlockNumber 元组，用于在一块资金被解锁时进行编码。
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct FundsUnlockChunk<Balance: HasCompact> {
    /// 需要解锁的资金数量
    #[codec(compact)]
    value: Balance,
    /// 将被解锁的era编号。
    #[codec(compact)]
    era: EraIndex,
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct EraBenefits<Balance: HasCompact> {
    /// 一个era的总降费额度
    #[codec(compact)]
    pub total_fee_reduction_quota: Balance,
    /// 市场活跃资金总额
    #[codec(compact)]
    pub total_market_active_funds: Balance,
    /// 已使用的总费用减免额度
    #[codec(compact)]
    pub used_fee_reduction_quota: Balance,
    /// 最新活跃era编号
    #[codec(compact)]
    pub active_era: EraIndex
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct MarketBenefit<Balance: HasCompact> {
    /// 总资金金额
    #[codec(compact)]
    pub total_funds: Balance,
    /// 自己的活跃资金金额
    #[codec(compact)]
    pub active_funds: Balance,
    /// 已使用的费用减免
    #[codec(compact)]
    pub used_fee_reduction_quota: Balance,
    /// 市场文件奖励金额
    #[codec(compact)]
    pub file_reward: Balance,
    /// 最新更新的活跃era编号
    #[codec(compact)]
    pub refreshed_at: EraIndex,
    /// 所有被解锁的余额
    pub unlocking_funds: Vec<FundsUnlockChunk<Balance>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct SworkBenefit<Balance: HasCompact> {
    /// 总资金金额
    #[codec(compact)]
    pub total_funds: Balance,
    /// 自己的活跃资金金额
    #[codec(compact)]
    pub active_funds: Balance,
    /// 报告矿工的总减少次数
    pub total_fee_reduction_count: u32,
    /// 已使用的费用减免
    pub used_fee_reduction_count: u32,
    /// 最新更新的活跃era编号
    #[codec(compact)]
    pub refreshed_at: EraIndex,
    /// 所有被解锁的余额
    pub unlocking_funds: Vec<FundsUnlockChunk<Balance>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
pub enum FundsType {
    SWORK = 0,
    MARKET = 1,
}

impl<Balance: HasCompact + Copy + Saturating + AtLeast32BitUnsigned> SworkBenefit<Balance> {
    fn consolidate_unlocked(mut self, current_era: EraIndex) -> Self {
        //总金额
        let mut total_funds = self.total_funds;
        // 当前era之前的总解锁金额
        let unlocking_funds = self
            .unlocking_funds
            .into_iter()
            .filter(|chunk| {
                if chunk.era > current_era {
                    true
                } else {
                    total_funds = total_funds.saturating_sub(chunk.value);
                    false
                }
            })
            .collect();
        self.total_funds = total_funds;
        self.unlocking_funds = unlocking_funds;
        self
    }

    /// 重新绑定计划解锁的资金。
    fn rebond(mut self, value: Balance) -> Self {
        let mut unlocking_balance: Balance = Zero::zero();

        while let Some(last) = self.unlocking_funds.last_mut() {
            if unlocking_balance + last.value <= value {
                unlocking_balance += last.value;
                self.active_funds += last.value;
                self.unlocking_funds.pop();
            } else {
                let diff = value - unlocking_balance;

                unlocking_balance += diff;
                self.active_funds += diff;
                last.value -= diff;
            }

            if unlocking_balance >= value {
                break
            }
        }

        self
    }
}

// TODO: Refine the following code and merge with SworkBenefit
impl<Balance: HasCompact + Copy + Saturating + AtLeast32BitUnsigned> MarketBenefit<Balance> {
    fn consolidate_unlocked(mut self, current_era: EraIndex) -> Self {
        let mut total_funds = self.total_funds;
        let unlocking_funds = self
            .unlocking_funds
            .into_iter()
            .filter(|chunk| {
                if chunk.era > current_era {
                    true
                } else {
                    total_funds = total_funds.saturating_sub(chunk.value);
                    false
                }
            })
            .collect();
        self.total_funds = total_funds;
        self.unlocking_funds = unlocking_funds;
        self
    }

    /// 重新绑定计划解锁的资金。
    fn rebond(mut self, value: Balance) -> Self {
        let mut unlocking_balance: Balance = Zero::zero();

        while let Some(last) = self.unlocking_funds.last_mut() {
            if unlocking_balance + last.value <= value {
                unlocking_balance += last.value;
                self.active_funds += last.value;
                self.unlocking_funds.pop();
            } else {
                let diff = value - unlocking_balance;

                unlocking_balance += diff;
                self.active_funds += diff;
                last.value -= diff;
            }

            if unlocking_balance >= value {
                break
            }
        }

        self
    }
}


#[frame_support::pallet]
pub mod pallet {
    use frame_system::pallet_prelude::*;
    use super::*;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: ReservableCurrency<Self::AccountId>;
        // The amount for one report work extrinsic
        type BenefitReportWorkCost: Get<BalanceOf<Self>>;
        // The ratio between total benefit limitation and total reward
        #[pallet::constant]
        type BenefitsLimitRatio: Get<Perbill>;
        // The ratio that benefit will cost, the remaining fee would still be charged
        #[pallet::constant]
        type BenefitMarketCostRatio: Get<Perbill>;
        /// Number of eras that staked funds must remain bonded for.
        #[pallet::constant]
        type BondingDuration: Get<EraIndex>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    // The pallet's runtime storage items.
    /// 总体福利
    #[pallet::storage]
    #[pallet::getter(fn current_benefits)]
    pub type CurrentBenefits<T: Config> = StorageValue<_, EraBenefits<BalanceOf<T>>, ValueQuery>;

    /// 市场福利
    #[pallet::storage]
    #[pallet::getter(fn market_benefits)]
    pub type MarketBenefits<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        MarketBenefit<BalanceOf<T>>,
        ValueQuery,
        >;

    /// 矿工福利
    #[pallet::storage]
    #[pallet::getter(fn swork_benefits)]
    pub type SworkBenefits<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        SworkBenefit<BalanceOf<T>>,
        ValueQuery,
    >;




    // Pallets use events to inform users when important changes are made.
    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 添加优惠金额成功.
        /// 账户/数量/优惠类型
        AddBenefitFundsSuccess(T::AccountId, BalanceOf<T>, FundsType),
        /// 锁定优惠金额成功
        /// 账户/金额/优惠类型
        CutBenefitFundsSuccess(T::AccountId, BalanceOf<T>, FundsType),
        /// 重新绑定优惠金额成功
        /// 账户/数量/优惠类型
        RebondBenefitFundsSuccess(T::AccountId, BalanceOf<T>, FundsType),
        /// Withdraw benefit funds success
        /// 账户/数量/优惠类型
        WithdrawBenefitFundsSuccess(T::AccountId, BalanceOf<T>),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        /// 没有足够的钱
        InsuffientBalance,
        /// 没有优惠记录
        InvalidTarget,
        /// 无法准备更多解锁块
        NoMoreChunks,
        /// 不解锁区块无法重新绑定
        NoUnlockChunk,
        /// 不能以低于最低余额的价值进行绑定
        InsufficientValue
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 添加福利金额
        #[pallet::weight(T::WeightInfo::add_benefit_funds())]
        pub fn add_benefit_funds(origin:OriginFor<T>, value: BalanceOf<T>, funds_type: FundsType) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 1. 把福利从余额移动到预留余额
            T::Currency::reserve(&who, value.clone()).map_err(|_| Error::<T>::InsuffientBalance)?;

            // 2. 改变福利金额
            match funds_type {
                FundsType::SWORK => {
                    <SworkBenefits<T>>::mutate(&who, |swork_benefit| {
                        swork_benefit.active_funds += value.clone();
                        swork_benefit.total_funds += value.clone();
                        // 计算总降费次数
                        swork_benefit.total_fee_reduction_count = Self::calculate_total_fee_reduction_count(&swork_benefit.active_funds);
                    });
                },
                FundsType::MARKET => {
                    <MarketBenefits<T>>::mutate(&who, |market_benefit| {
                        market_benefit.active_funds += value.clone();
                        market_benefit.total_funds += value.clone();
                    });
                    <CurrentBenefits<T>>::mutate(|benefits| { benefits.total_market_active_funds += value.clone();});
                }
            }

            // 3. Emit success
            Self::deposit_event(Event::<T>::AddBenefitFundsSuccess(who.clone(), value, funds_type));

            Ok(())
        }

        /// 锁定优惠金额
        #[pallet::weight(T::WeightInfo::cut_benefit_funds())]
        pub fn cut_benefit_funds(origin:OriginFor<T>, value: BalanceOf<T>, funds_type: FundsType) -> DispatchResult {
            let who = ensure_signed(origin)?;

            match funds_type {
                FundsType::SWORK => {
                    // 1. 获取收益
                    ensure!(<SworkBenefits<T>>::contains_key(&who), Error::<T>::InvalidTarget);
                    let mut benefit = Self::swork_benefits(&who);

                    // 2. 判断是否超过最大的解锁区块
                    ensure!(
                        benefit.unlocking_funds.len() < MAX_UNLOCKING_CHUNKS,
                        Error::<T>::NoMoreChunks
                    );
                    // 3. 保证 要削减的值 < 账户中活跃的金额
                    let mut value = value;
                    // value = 最小值（要削减的值，账户中活跃的金额）
                    value = value.min(benefit.active_funds);

                    if !value.is_zero() {
                        //将活跃的金额 - 要削减的值
                        benefit.active_funds -= value;

                        // 4. 避免在benefit系统中留下狠小的金额
                        if benefit.active_funds < T::Currency::minimum_balance() {
                            value += benefit.active_funds;
                            benefit.active_funds = Zero::zero();
                        }

                        // 5. 更新benefit
                        // 要解锁的era = 当且活跃era编号 + 绑定的纪元编号
                        let era = Self::current_benefits().active_era + T::BondingDuration::get();
                        // 将金额放入并锁定
                        benefit.unlocking_funds.push(FundsUnlockChunk { value, era });
                        benefit.total_fee_reduction_count = Self::calculate_total_fee_reduction_count(&benefit.active_funds);
                        <SworkBenefits<T>>::insert(&who, benefit);
                    }
                },
                FundsType::MARKET => {
                    // 1. 获取收益
                    ensure!(<MarketBenefits<T>>::contains_key(&who), Error::<T>::InvalidTarget);
                    let mut benefit = Self::market_benefits(&who);

                    // 2. 判断是否超过最大的解锁区块
                    ensure!(
                        benefit.unlocking_funds.len() < MAX_UNLOCKING_CHUNKS,
                        Error::<T>::NoMoreChunks
                    );
                    // 3. 保证 要削减的值 < 账户中活跃的金额
                    let mut value = value;
                    value = value.min(benefit.active_funds);

                    if !value.is_zero() {
                        benefit.active_funds -= value;

                        // 4. 避免在benefit系统中留下狠小的金额
                        if benefit.active_funds < T::Currency::minimum_balance() {
                            value += benefit.active_funds;
                            benefit.active_funds = Zero::zero();
                        }

                        // 5. 更新benefit
                        let era = Self::current_benefits().active_era + T::BondingDuration::get();
                        benefit.unlocking_funds.push(FundsUnlockChunk { value, era });
                        <MarketBenefits<T>>::insert(&who, benefit);
                        // 减少总体的活跃金额
                        <CurrentBenefits<T>>::mutate(|benefits| { benefits.total_market_active_funds = benefits.total_market_active_funds.saturating_sub(value.clone());});
                    }
                }
            };
            // 6. Send event
            Self::deposit_event(Event::<T>::CutBenefitFundsSuccess(who.clone(), value, funds_type));
            Ok(())
        }



    }






}


impl<T: Config> Pallet<T> {
    pub fn calculate_total_fee_reduction_count(active_funds: &BalanceOf<T>) -> u32 {
        // 降费金额 / 一次降费的金额 = 降费次数
        (*active_funds / T::BenefitReportWorkCost::get()).saturated_into()
    }
}

// impl<T:Config> BenefitInterface<T::AccountId, BalanceOf<T>,NegativeImbalanceOf<T>> for Pallet<T>{
//     fn update_era_benefit(next_era: EraIndex, total_benefits: BalanceOf<T>) -> BalanceOf<T> {
//         todo!()
//     }
//
//     fn update_reward(who:T::AccountId, value: BalanceOf<T>) {
//         todo!()
//     }
//
//     fn maybe_reduce_fee(who:T::AccountId, fee: BalanceOf<T>, reasons: WithdrawReasons) -> Result<NegativeImbalanceOf<T>, DispatchError> {
//         todo!()
//     }
//
//     fn maybe_free_count(who:T::AccountId) -> bool {
//         todo!()
//     }
//
//     fn get_collateral_and_reward(who:T::AccountId) -> (BalanceOf<T>, BalanceOf<T>) {
//         todo!()
//     }
//
//     fn get_market_funds_ratio(who:T::AccountId) -> Perbill {
//         todo!()
//     }
// }
