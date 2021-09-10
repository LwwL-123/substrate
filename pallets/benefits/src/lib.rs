#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>

use sp_std::prelude::*;
use frame_support::{
    weights::{Weight},
    dispatch::HasCompact,
    traits::{Currency, ReservableCurrency, Get,
             WithdrawReasons, ExistenceRequirement, Imbalance},
    pallet_prelude::*,
};


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
    // 解锁到期的金额
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
        // The ratio between total benefits limitation and total reward
        #[pallet::constant]
        type BenefitsLimitRatio: Get<Perbill>;
        // The ratio that benefits will cost, the remaining fee would still be charged
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
        /// 提取解锁的福利资金
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
        pub fn add_benefit_funds(origin:OriginFor<T>, #[pallet::compact] value: BalanceOf<T>, funds_type: FundsType) -> DispatchResult {
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
        pub fn cut_benefit_funds(origin:OriginFor<T>, #[pallet::compact] value: BalanceOf<T>, funds_type: FundsType) -> DispatchResult {
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


        /// 提取解锁的福利资金
        #[pallet::weight(T::WeightInfo::withdraw_benefit_funds())]
        pub fn withdraw_benefit_funds(origin:OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut unreserved_value: BalanceOf<T> = Zero::zero();
            if <SworkBenefits<T>>::contains_key(&who) {
                // 检查并更新work账户金额
                Self::check_and_update_swork_funds(&who);
                let mut benefit = Self::swork_benefits(&who);

                // 2. 更新总金额
                let old_total_funds = benefit.total_funds;
                let active_era = Self::current_benefits().active_era;
                // 解锁到期的锁定金额
                benefit = benefit.consolidate_unlocked(active_era);

                // 3. 取消保留金额
                let to_unreserved_value = old_total_funds.saturating_sub(benefit.total_funds);
                T::Currency::unreserve(&who, to_unreserved_value);

                // 4. 更新或删除报告矿工的费用减免优惠
                // 如果未解锁金额为0且活跃金额为0，删除SworkBenefits，否则将最近benefit插入
                if benefit.unlocking_funds.is_empty() && benefit.active_funds.is_zero() {
                    <SworkBenefits<T>>::remove(&who);
                } else {
                    <SworkBenefits<T>>::insert(&who, benefit);
                }

                unreserved_value = unreserved_value.saturating_add(to_unreserved_value);
            }
            if <MarketBenefits<T>>::contains_key(&who) {
                Self::check_and_update_market_funds(&who);
                let mut benefit = Self::market_benefits(&who);

                // 2. 更新总金额
                let old_total_funds = benefit.total_funds;
                let active_era = Self::current_benefits().active_era;
                benefit = benefit.consolidate_unlocked(active_era);

                // 3. 取消保留金额
                let to_unreserved_value = old_total_funds.saturating_sub(benefit.total_funds);
                T::Currency::unreserve(&who, to_unreserved_value);

                // 4. 更新或删除市场的费用减免优惠
                if benefit.unlocking_funds.is_empty() && benefit.active_funds.is_zero() && benefit.file_reward.is_zero() {
                    <MarketBenefits<T>>::remove(&who);
                } else {
                    <MarketBenefits<T>>::insert(&who, benefit);
                }

                unreserved_value = unreserved_value.saturating_add(to_unreserved_value);
            }

            // 5. Emit success
            Self::deposit_event(Event::<T>::WithdrawBenefitFundsSuccess(who.clone(), unreserved_value));

            Ok(())
        }

        /// 返还福利基金
        #[pallet::weight(T::WeightInfo::rebond_benefit_funds())]
        pub fn rebond_benefit_funds(origin:OriginFor<T>, #[pallet::compact] value: BalanceOf<T>, funds_type: FundsType) -> DispatchResult {
            let who = ensure_signed(origin)?;

            match funds_type {
                FundsType::SWORK => {
                    // 1. 获得benefit
                    ensure!(<SworkBenefits<T>>::contains_key(&who), Error::<T>::InvalidTarget);
                    let mut benefit = Self::swork_benefits(&who);
                    ensure!(!benefit.unlocking_funds.is_empty(), Error::<T>::NoUnlockChunk);

                    // 2. 重新绑定
                    benefit = benefit.rebond(value);

                    ensure!(benefit.active_funds >= T::Currency::minimum_balance(), Error::<T>::InsufficientValue);
                    // 3. 根据活跃资金更新总费用减免计数
                    benefit.total_fee_reduction_count = Self::calculate_total_fee_reduction_count(&benefit.active_funds);
                    <SworkBenefits<T>>::insert(&who, benefit);
                },
                FundsType::MARKET => {
                    // 1. 获取benefit
                    ensure!(<MarketBenefits<T>>::contains_key(&who), Error::<T>::InvalidTarget);
                    let mut benefit = Self::market_benefits(&who);
                    let old_active_funds = benefit.active_funds;
                    ensure!(!benefit.unlocking_funds.is_empty(), Error::<T>::NoUnlockChunk);

                    // 2. 重新绑定金额
                    benefit = benefit.rebond(value);

                    ensure!(benefit.active_funds >= T::Currency::minimum_balance(), Error::<T>::InsufficientValue);
                    let new_active_funds = benefit.active_funds;
                    <MarketBenefits<T>>::insert(&who, benefit);
                    // 3. Update current benefits
                    <CurrentBenefits<T>>::mutate(|benefits| { benefits.total_market_active_funds = benefits.total_market_active_funds.saturating_add(new_active_funds).saturating_sub(old_active_funds);});
                }
            };
            // 4.event
            Self::deposit_event(Event::<T>::RebondBenefitFundsSuccess(who.clone(), value, funds_type));
            Ok(())
        }
    }






}


impl<T: Config> Pallet<T> {
    pub fn calculate_total_fee_reduction_count(active_funds: &BalanceOf<T>) -> u32 {
        // 降费金额 / 一次降费的金额 = 降费次数
        (*active_funds / T::BenefitReportWorkCost::get()).saturated_into()
    }

    fn check_and_update_swork_funds(who: &T::AccountId) {
        let mut swork_benefit = Self::swork_benefits(&who);
        // 获取账户的预留金额
        let reserved_value = T::Currency::reserved_balance(who);
        // 如果 swork福利总金额 < 预留金额
        if swork_benefit.total_funds <= reserved_value {
            return;
        }
        // 修复金额错误问题
        let old_total_funds = swork_benefit.total_funds;
        swork_benefit.total_funds = reserved_value;
        swork_benefit.active_funds = swork_benefit.active_funds.saturating_add(swork_benefit.total_funds).saturating_sub(old_total_funds);
        swork_benefit.total_fee_reduction_count = Self::calculate_total_fee_reduction_count(&swork_benefit.active_funds);
        <SworkBenefits<T>>::insert(&who, swork_benefit);
    }

    fn check_and_update_market_funds(who: &T::AccountId) {
        let mut market_benefit = Self::market_benefits(&who);
        let reserved_value = T::Currency::reserved_balance(who);
        if market_benefit.total_funds <= reserved_value {
            return;
        }
        // Something wrong, fix it
        let old_total_funds = market_benefit.total_funds;
        let old_active_funds = market_benefit.active_funds;
        market_benefit.total_funds = reserved_value;
        market_benefit.active_funds = market_benefit.active_funds.saturating_add(market_benefit.total_funds).saturating_sub(old_total_funds);
        <CurrentBenefits<T>>::mutate(|benefits| { benefits.total_market_active_funds = benefits.total_market_active_funds.saturating_add(market_benefit.active_funds).saturating_sub(old_active_funds);});
        <MarketBenefits<T>>::insert(&who, market_benefit);
    }

    /// 返回值是上一个时代使用的费用配额
    pub fn do_update_era_benefit(next_era: EraIndex, total_fee_reduction_quota: BalanceOf<T>) -> BalanceOf<T> {
        // 获取整体福利信息
        let mut current_benefits = Self::current_benefits();
        // 存储上一个时代的使用费减免
        let used_fee_reduction_quota = current_benefits.used_fee_reduction_quota;
        // 开启下一个纪元并为其设置活跃纪元
        current_benefits.active_era = next_era;
        // 为下一个时代设定新的总收益
        current_benefits.total_fee_reduction_quota = total_fee_reduction_quota;
        // 将已使用的福利重置为零
        current_benefits.used_fee_reduction_quota = Zero::zero();
        <CurrentBenefits<T>>::put(current_benefits);
        // 返还上个时代用过的金额
        used_fee_reduction_quota
    }

    pub fn maybe_do_reduce_fee(who: &T::AccountId, fee: BalanceOf<T>, reasons: WithdrawReasons) -> Result<NegativeImbalanceOf<T>, DispatchError> {
        let mut current_benefits = Self::current_benefits();
        let mut market_benefit = Self::market_benefits(who);
        // 更新降费
        // 如果市场最近更新的era编号<最近的活跃era编号，则更新市场era编号，并把已使用降费设置为0
        Self::maybe_refresh_market_benefit(current_benefits.active_era, &mut market_benefit);
        // 计算自己的降费份额
        let fee_reduction_benefits_quota = Self::calculate_fee_reduction_quota(market_benefit.active_funds,
                                                                               current_benefits.total_market_active_funds,
                                                                               current_benefits.total_fee_reduction_quota);
        // 尝试免费减费
        // 查看此人有自己的减费额度和总收益
        let fee_reduction_benefit_cost = T::BenefitMarketCostRatio::get() * fee;
        let (charged_fee, used_fee_reduction) = if market_benefit.used_fee_reduction_quota + fee_reduction_benefit_cost <= fee_reduction_benefits_quota
            && current_benefits.used_fee_reduction_quota + fee_reduction_benefit_cost <= current_benefits.total_fee_reduction_quota {
            // 可以免这个费用
            (fee - fee_reduction_benefit_cost, fee_reduction_benefit_cost)
        } else {
            // 免收这笔费用是不行的
            // 收取的费用为 100%
            // 费用减免为 0%
            (fee, Zero::zero())
        };
        // 尝试提取货币
        let result = match T::Currency::withdraw(who, charged_fee, reasons, ExistenceRequirement::KeepAlive) {
            Ok(mut imbalance) => {
                // 如果没有 active_funds 以节省数据库写入时间，则不会更新减少细节
                if !used_fee_reduction.is_zero() {
                    // 更新降费信息
                    current_benefits.used_fee_reduction_quota += used_fee_reduction;
                    market_benefit.used_fee_reduction_quota += used_fee_reduction;
                    <MarketBenefits<T>>::insert(&who, market_benefit);
                    <CurrentBenefits<T>>::put(current_benefits);
                    // 发放免费费用
                    let new_issued = T::Currency::issue(used_fee_reduction.clone());
                    imbalance.subsume(new_issued);
                }
                Ok(imbalance)
            }
            Err(err) => Err(err),
        };
        result
    }

    // 自己市场的活跃金额/总的活跃金额 * 总的降费
    pub fn calculate_fee_reduction_quota(market_active_funds: BalanceOf<T>, total_market_active_funds: BalanceOf<T>, total_fee_reduction_quota: BalanceOf<T>) -> BalanceOf<T> {
        Perbill::from_rational(market_active_funds, total_market_active_funds) * total_fee_reduction_quota
    }

    pub fn maybe_refresh_market_benefit(latest_active_era: EraIndex, market_benefit: &mut MarketBenefit<BalanceOf<T>>) {
        // 如果市场最近更新的era编号<最近的活跃era编号，则更新市场era编号，并把已使用降费设置为0
        if market_benefit.refreshed_at < latest_active_era {
            market_benefit.refreshed_at = latest_active_era;
            market_benefit.used_fee_reduction_quota = Zero::zero();
        }
    }

    pub fn maybe_do_free_count(who: &T::AccountId) -> bool {
        let active_era = Self::current_benefits().active_era;
        let mut swork_benefit = Self::swork_benefits(who);
        Self::maybe_refresh_swork_benefits(active_era, &mut swork_benefit);
        // 如果没有赌注以节省数据库写入时间，则不会更新减少细节
        if swork_benefit.used_fee_reduction_count < swork_benefit.total_fee_reduction_count {
            swork_benefit.used_fee_reduction_count += 1;
            <SworkBenefits<T>>::insert(&who, swork_benefit);
            return true;
        }
        return false;
    }

    pub fn maybe_refresh_swork_benefits(latest_active_era: EraIndex, swork_benefit: &mut SworkBenefit<BalanceOf<T>>) {
        // 如果swork最近更新的era编号<最近的活跃era编号，则更新sworker的era编号，并把已使用降费设置为0
        if swork_benefit.refreshed_at < latest_active_era {
            swork_benefit.refreshed_at = latest_active_era;
            swork_benefit.used_fee_reduction_count = 0;
        }
    }
}

impl<T:Config> BenefitInterface<T::AccountId, BalanceOf<T>,NegativeImbalanceOf<T>> for Pallet<T>{
    // 更新era奖励
    fn update_era_benefit(next_era: EraIndex, total_reward: BalanceOf<T>) -> BalanceOf<T> {
        Self::do_update_era_benefit(next_era, T::BenefitsLimitRatio::get() * total_reward)
    }

    // 更新市场文件奖励
    fn update_reward(who:&T::AccountId, value: BalanceOf<T>) {
        let mut market_benefit = Self::market_benefits(who);
        market_benefit.file_reward = value;

        // 去除死掉的benefit
        if market_benefit.unlocking_funds.is_empty() && market_benefit.active_funds.is_zero() && market_benefit.file_reward.is_zero() {
            <MarketBenefits<T>>::remove(&who);
        } else {
            <MarketBenefits<T>>::insert(&who, market_benefit);
        }
    }

    // 可能的降费
    fn maybe_reduce_fee(who:&T::AccountId, fee: BalanceOf<T>, reasons: WithdrawReasons) -> Result<NegativeImbalanceOf<T>, DispatchError> {
        Self::maybe_do_reduce_fee(who, fee, reasons)
    }

    // 可能降费的次数
    fn maybe_free_count(who:&T::AccountId) -> bool {
        Self::maybe_do_free_count(who)

    }

    // 获得账户的奖励金额
    fn get_collateral_and_reward(who:&T::AccountId) -> (BalanceOf<T>, BalanceOf<T>) {
        let market_benefits = Self::market_benefits(who);
        (market_benefits.active_funds, market_benefits.file_reward)
    }


    fn get_market_funds_ratio(who:&T::AccountId) -> Perbill {
        let market_benefit = Self::market_benefits(who);
        if !market_benefit.active_funds.is_zero() {
            // 市场的活跃金额 / 总的活跃金额
            return Perbill::from_rational(market_benefit.active_funds, Self::current_benefits().total_market_active_funds);
        }
        return Perbill::zero();
    }
}
