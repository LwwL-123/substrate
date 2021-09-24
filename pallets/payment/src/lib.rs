#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>

pub use pallet::*;
use sp_std::convert::TryInto;
use sp_runtime::traits::Zero;
use frame_support::{ traits::{ Currency, ExistenceRequirement, WithdrawReasons, Imbalance},
					 PalletId, dispatch::DispatchResult, pallet_prelude::*};
use frame_system::pallet_prelude::*;
use sp_runtime::{traits::{AccountIdConversion, Saturating}};
use sp_runtime::Perbill;
use frame_support::sp_runtime::traits::Convert;
use primitives::p_payment::*;
use primitives::p_storage_order::*;
use primitives::p_worker::*;
use primitives::p_benefit::BenefitInterface;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;
type PositiveImbalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::PositiveImbalance;

pub(crate) const LOG_TARGET: &'static str = "payment";

#[macro_export]
macro_rules! log {
    ($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		log::$level!(
			target: crate::LOG_TARGET,
			concat!("[{:?}] 💸 ", $patter), <frame_system::Pallet<T>>::block_number() $(, $values)*
		)
    };
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// 可以分润的矿工账户数量
		type NumberOfIncomeMiner: Get<usize>;

		/// 金额转换数字
		type BalanceToNumber: Convert<BalanceOf<Self>, u128>;
		/// 数字转金额
		type NumberToBalance: Convert<u128,BalanceOf<Self>>;
		/// 支付费用和持有余额的货币。
		type Currency: Currency<Self::AccountId>;
		/// 订单接口
		type StorageOrderInterface: StorageOrderInterface<AccountId = Self::AccountId, BlockNumber = Self::BlockNumber>;
		/// worker接口
		type WorkerInterface:  WorkerInterface<AccountId = Self::AccountId, BlockNumber = Self::BlockNumber,Balance = BalanceOf<Self>>;
		/// 质押池分配比率
		type StakingRatio: Get<Perbill>;
		/// 存储池分配比率
		type StorageRatio: Get<Perbill>;
		/// 折扣接口
		type BenefitInterface: BenefitInterface<Self::AccountId, BalanceOf<Self>, NegativeImbalanceOf<Self>>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// The pallet's runtime storage items.
	// https://substrate.dev/docs/en/knowledgebase/runtime/storage
	#[pallet::storage]
	#[pallet::getter(fn something)]
	// Learn more about declaring storage items:
	// https://substrate.dev/docs/en/knowledgebase/runtime/storage#declaring-storage-items
	pub type Something<T> = StorageValue<_, u32>;

	/// 订单支付信息
	#[pallet::storage]
	#[pallet::getter(fn order_price)]
	pub(super) type OrderPrice<T: Config> = StorageMap<_, Twox64Concat, u64, PayoutInfo<BalanceOf<T>,T::BlockNumber>, OptionQuery>;


	/// 订单金额拆分数据暂存记录
	#[pallet::storage]
	#[pallet::getter(fn order_calculate_block)]
	pub(super) type OrderSplitAmount<T: Config> = StorageMap<_,Twox64Concat, u64 , (BalanceOf<T>,BalanceOf<T>,BalanceOf<T>), OptionQuery>;

	/// 矿工待领取金额
	#[pallet::storage]
	#[pallet::getter(fn miner_price)]
	pub(super) type MinerPrice<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, BalanceOf<T>, OptionQuery>;

	/// 矿工清算订单金额
	#[pallet::storage]
	#[pallet::getter(fn miner_order_calculate)]
	pub(super) type MinerOrderCalculate<T: Config> =  StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, u64, BalanceOf<T>, ValueQuery>;

	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		SomethingStored(u32, T::AccountId),

		/// 订单清算成功
		CalculateSuccess(u64),

		/// 领取收益
		Withdrawal(BalanceOf<T>, T::AccountId),
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_now: T::BlockNumber) -> Weight {
			0
		}
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
		/// 订单金额已经配置
		StorageOrderPriceSetted,
		/// 用户余额不足
		InsufficientCurrency,
		/// 订单支付信息不存在
		OrderPayoutInfoNotExist,
		/// 订单不在清算报酬期
		NotInRewardPeriod
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T:Config> Pallet<T> {

		/// Calculate the reward for a order
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn calculate_reward(
			origin: OriginFor<T>,
			order_index: u64,
		) -> DispatchResult {
			ensure_signed(origin)?;
			// 校验订单是否存在
			ensure!(OrderPrice::<T>::contains_key(&order_index), Error::<T>::OrderPayoutInfoNotExist);
			let mut payout_info = OrderPrice::<T>::get(&order_index).unwrap();
			let curr_bn = <frame_system::Pallet<T>>::block_number();
			// 校验订单是否清算完成
			ensure!(payout_info.calculate <= payout_info.deadline, Error::<T>::NotInRewardPeriod);
			// 获得当前计算区块
			let calculated_block = curr_bn.min(payout_info.deadline);
			// 获得支付人数
			let miners = T::WorkerInterface::order_miners(order_index);
			let target_reward_count = miners.len().min(T::NumberOfIncomeMiner::get()) as u32;

			if target_reward_count > 0 {
				// 计算当前支付金额
				let reward_count  = TryInto::<T::BlockNumber>::try_into(target_reward_count).ok().unwrap();
				let one_payout_amount = (Perbill::from_rational(calculated_block - payout_info.calculate,
																(payout_info.deadline - payout_info.calculate) * reward_count)
					* payout_info.amount).saturating_sub(1u32.into());
				let mut rewarded_amount: BalanceOf<T> = Zero::zero();
				let mut rewarded_count: usize = 0;
				// 清算数据添加
				for miner in &miners {
					if Self::maybe_reward_merchant(&miner, &one_payout_amount) {
						rewarded_amount += one_payout_amount;
						rewarded_count += 1;
						match MinerPrice::<T>::get(miner) {
							Some(t) => {
								let income_after = t + one_payout_amount;
								MinerPrice::<T>::insert(miner,income_after);
							}
							None => {
								MinerPrice::<T>::insert(miner, one_payout_amount);
							}
						}
						MinerOrderCalculate::<T>::mutate(miner, &order_index, |price| { *price = price.saturating_add(one_payout_amount) });
						if rewarded_count == T::NumberOfIncomeMiner::get() {
							break;
						}
					}
				}
				// 更新当前订单支付数据
				payout_info.calculate = calculated_block;
				payout_info.amount = payout_info.amount.saturating_sub(rewarded_amount);
				OrderPrice::<T>::insert(&order_index,payout_info.clone());
			}
			//判断当前订单是否已经完成
			if payout_info.deadline <= curr_bn && payout_info.deadline == calculated_block {
				//将订单修改为已清算状态
				T::StorageOrderInterface::update_order_status_to_cleared(&order_index);
			};
			Self::deposit_event(Event::CalculateSuccess(order_index));
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn withdrawal(
			origin : OriginFor<T>
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			//查询矿工待领取金额
			let price_opt = MinerPrice::<T>::get(&who);

			if price_opt.is_some() {
				let amount :BalanceOf<T> = price_opt.unwrap();
				if T::BalanceToNumber::convert(amount) > 0 {
					//从资金池中进行转账
					T::Currency::transfer(&Self::storage_pool(),&who,amount, ExistenceRequirement::AllowDeath)?;
					// 记录累计收益
					T::WorkerInterface::record_miner_income(&who,amount);
					MinerPrice::<T>::remove(&who);
					Self::deposit_event(Event::Withdrawal(amount, who));
				}
			}
			Ok(())
		}
	}
}

const PALLET_ID: PalletId = PalletId(*b"ttchain!");

impl <T:Config> Pallet<T> {

	/// StakingPod
	pub fn staking_pool() -> T::AccountId { PALLET_ID.into_sub_account(b"staking") }
	/// StoragePod
	pub fn storage_pool() -> T::AccountId { PALLET_ID.into_sub_account(b"storage") }
	/// ReservedPod
	pub fn reserved_pool() -> T::AccountId { PALLET_ID.into_sub_account(b"reserved") }
	/// temporaryPod
	pub fn temporary_pool() -> T::AccountId { PALLET_ID.into_sub_account(b"temporary") }


	// 将订单分到不同的模块
	// Currently
	// 10% into reserved pool
	// 72% into staking pool
	// 18% into storage pool
	fn split_into_reserved_and_storage_and_staking_pool(who: &T::AccountId, value: BalanceOf<T>, base_fee: BalanceOf<T>, tips: BalanceOf<T>) -> (BalanceOf<T>, BalanceOf<T> ,BalanceOf<T>) {
		// Split the original amount into three parts
		let staking_amount = T::StakingRatio::get() * value;
		let storage_amount = T::StorageRatio::get() * value;
		let reserved_amount = value - staking_amount - storage_amount;

		// Add the tips into storage amount
		let storage_amount = storage_amount + tips;

		// Check the discount for the reserved amount, reserved_amount = max(0, reserved_amount - discount_amount)
		let discount_amount = T::BenefitInterface::get_market_funds_ratio(who) * value;
		let reserved_amount = reserved_amount.saturating_sub(discount_amount);
		let reserved_amount = reserved_amount.saturating_add(base_fee);
		(staking_amount, storage_amount, reserved_amount)
	}
	// 将金额存入存储，质押，保留池中
	fn transfer_reserved_and_storage_and_staking_pool(who: &T::AccountId, staking_amount: BalanceOf<T>, storage_amount: BalanceOf<T>, reserved_amount: BalanceOf<T>, liveness: ExistenceRequirement) -> DispatchResult {
		T::Currency::transfer(&who, &Self::reserved_pool(), reserved_amount, liveness)?;
		T::Currency::transfer(&who, &Self::staking_pool(), staking_amount, liveness)?;
		T::Currency::transfer(&who, &Self::storage_pool(), storage_amount, liveness)?;
		Ok(())
	}

	// 判断是否可以领取奖励
	fn maybe_reward_merchant(who: &T::AccountId, amount: &BalanceOf<T>, ) -> bool {
		// 查询当前市场活动金额
		let (collateral, _) = T::BenefitInterface::get_collateral_and_reward(who);
		// 当带领取金额小于活动金额时则可以领取订单结算金额
		if let Some(reward) = MinerPrice::<T>::get(who) {
			if (reward + *amount) <= collateral {
				T::BenefitInterface::update_reward(&who, reward + *amount);
				return true;
			}
		}
		//todo 测试用
		//false
		true
	}
}


impl<T: Config> PaymentInterface for Pallet<T> {
	type AccountId = T::AccountId;
	type BlockNumber = T::BlockNumber;
	type Balance = BalanceOf<T>;

	fn pay_order(order_index: &u64, file_base_price: Self::Balance, order_price: Self::Balance, tips: Self::Balance,deadline: Self::BlockNumber, account_id: &Self::AccountId) -> DispatchResult{
		//校验账户余额
		ensure!(T::Currency::free_balance(&account_id) >= (file_base_price + order_price + tips), Error::<T>::InsufficientCurrency);
		// 将订单进行拆分并进行折扣计算 返回存储金额，质押金额，保留金额
		let (staking_amount, storage_amount, reserved_amount) =
			Self::split_into_reserved_and_storage_and_staking_pool(
				&account_id,
				order_price,
				file_base_price,
				tips);
		match OrderPrice::<T>::get(order_index) {
			// 没有订单则为新订单支付
			None => {
				// 存储订单金额到暂存池中
				T::Currency::transfer(
					&account_id,
					&Self::temporary_pool(),
					staking_amount + storage_amount + reserved_amount,
					ExistenceRequirement::AllowDeath)?;
				// 记录金额分割缓存数据
				OrderSplitAmount::<T>::insert(order_index,(staking_amount, storage_amount, reserved_amount));
				// 获得当前区块
				let curr_bn = <frame_system::Pallet<T>>::block_number();
				// 记录订单金额
				OrderPrice::<T>::insert(order_index,PayoutInfo {
					amount: storage_amount,
					deadline,
					calculate: curr_bn
				});
				Ok(())
			},
			// 已有订单金额，则进行续费操作
			Some(mut payout_info) => {
				// 将订单金额拆分存入不同存储池中
				Self::transfer_reserved_and_storage_and_staking_pool(
					&account_id,
					staking_amount,
					storage_amount,
					reserved_amount,
					ExistenceRequirement::AllowDeath)?;
				// 更新订单支出数据
				payout_info.amount = payout_info.amount + storage_amount;
				// 更新订单到期数据
				payout_info.deadline = deadline;
				OrderPrice::<T>::insert(order_index,payout_info);
				// 更新订单金额信息
				Err(Error::<T>::NoneValue)?
			},
		}
	}

	fn cancel_order(order_index: &u64, account_id: &Self::AccountId){
		match OrderSplitAmount::<T>::get(order_index) {
			Some((staking_amount , storage_amount , reserved_amount)) => {
				let dispatch_result = T::Currency::transfer(
					&Self::temporary_pool(),
					&account_id,
					staking_amount + storage_amount + reserved_amount,
					ExistenceRequirement::AllowDeath);
				if dispatch_result.is_ok() {
					//删除订单支出数据
					OrderPrice::<T>::remove(order_index);
					//删除订单金额拆分数据暂存记录
					OrderSplitAmount::<T>::remove(order_index);
				}
			},
			None => ()
		}
	}

	fn withdraw_staking_pool() -> BalanceOf<T> {
		let staking_pool = Self::staking_pool();
		// Leave the minimum balance to keep this account live.
		let staking_amount = T::Currency::free_balance(&staking_pool);
		let mut imbalance = <PositiveImbalanceOf<T>>::zero();
		imbalance.subsume(T::Currency::burn(staking_amount.clone()));
		if let Err(_) = T::Currency::settle(&staking_pool, imbalance, WithdrawReasons::TRANSFER, ExistenceRequirement::AllowDeath) {
			log!(warn, "🏢 Something wrong during withdrawing staking pot. Admin/Council should pay attention to it.");
			return Zero::zero();
		}
		staking_amount
	}
	//订单成功后将订单金额存入相应金额池中
	fn transfer_reserved_and_storage_and_staking_pool_by_temporary_pool(order_index: &u64) {
		// 查询当前订单金额拆分数据
		match OrderSplitAmount::<T>::get(order_index) {
			// 如果当前订单拆分数据存在则将数据进行交易
			Some((staking_amount , storage_amount , reserved_amount)) => {
				let dispatch_result = Self::transfer_reserved_and_storage_and_staking_pool(
					&Self::temporary_pool(),
					staking_amount,
					storage_amount,
					reserved_amount,
					ExistenceRequirement::AllowDeath);
				if dispatch_result.is_ok() {
					//删除订单金额拆分数据暂存记录
					OrderSplitAmount::<T>::remove(order_index);
				}
			},
			None => ()
		}
	}

	fn get_calculate_income_by_miner_and_order_index(account_id: &Self::AccountId, order_index: &u64) -> Self::Balance {
		MinerOrderCalculate::<T>::get(account_id,order_index)
	}
}