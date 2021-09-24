#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>

pub use pallet::*;
use sp_std::vec::Vec;
use frame_support::{
	traits::{Currency},
	ensure,
	dispatch::{DispatchResult, DispatchResultWithPostInfo}, pallet_prelude::*,
	weights::{
		Pays
	}
};
use sp_runtime::traits::{Convert, Zero};
use sp_runtime::Perbill;
use primitives::p_storage_order::StorageOrderInterface;
use primitives::p_storage_order::StorageOrderStatus;
use primitives::p_worker::*;
use primitives::p_benefit::BenefitInterface;
use primitives::p_payment::PaymentInterface;

mod zk;

type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

use sp_std::collections::btree_map::BTreeMap;
/// An event handler for reporting works
pub trait Works<AccountId> {
	fn report_works(workload_map: BTreeMap<AccountId, u128>, total_workload: u128) -> Weight;
}

impl<AId> Works<AId> for () {
	fn report_works(_: BTreeMap<AId, u128>, _: u128) -> Weight { 0 }
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

		/// 工作量证明上报间隔
		#[pallet::constant]
		type ReportInterval: Get<Self::BlockNumber>;

		/// 支付费用和持有余额的货币。
		type Currency: Currency<Self::AccountId>;

		/// 金额转换数字
		type BalanceToNumber: Convert<BalanceOf<Self>, u128>;

		/// 数字转金额
		type NumberToBalance: Convert<u128,BalanceOf<Self>>;

		/// 订单接口
		type StorageOrderInterface: StorageOrderInterface<AccountId = Self::AccountId, BlockNumber = Self::BlockNumber>;

		/// 平均收益限额
		type NumberOfIncomeMiner: Get<usize>;

		/// 工作量证明上报接口
		type Works: Works<Self::AccountId>;

		/// 折扣接口
		type BenefitInterface: BenefitInterface<Self::AccountId, BalanceOf<Self>, NegativeImbalanceOf<Self>>;

		/// 订单支付接口
		type PaymentInterface: PaymentInterface<AccountId = Self::AccountId, BlockNumber = Self::BlockNumber,Balance = BalanceOf<Self>>;

		/// 存储池分配比率
		type StorageRatio: Get<Perbill>;
	}

	/// 矿工个数
	#[pallet::storage]
	#[pallet::getter(fn miner_count)]
	pub(super) type MinerCount<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// 矿工列表
	#[pallet::storage]
	#[pallet::getter(fn miners)]
	pub(super) type Miners<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

	/// 总存储
	#[pallet::storage]
	#[pallet::getter(fn total_storage)]
	pub(super) type TotalStorage<T: Config> = StorageValue<_, u128, ValueQuery>;

	/// 已用存储
	#[pallet::storage]
	#[pallet::getter(fn used_storage)]
	pub(super) type UsedStorage<T: Config> = StorageValue<_, u128, ValueQuery>;

	/// 矿工收益
	#[pallet::storage]
	#[pallet::getter(fn miner_income)]
	pub(super) type MinerIncome<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, BalanceOf<T>, OptionQuery>;

	/// 矿工总存储
	#[pallet::storage]
	#[pallet::getter(fn miner_total_storage)]
	pub(super) type MinerTotalStorage<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, u128, ValueQuery>;

	/// 矿工已用存储
	#[pallet::storage]
	#[pallet::getter(fn miner_used_storage)]
	pub(super) type MinerUsedStorage<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, u128, ValueQuery>;

	/// 矿工订单数据
	#[pallet::storage]
	#[pallet::getter(fn miner_order)]
	pub(super) type MinerOrderSet<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, Vec<u64>, ValueQuery>;

	/// 订单对应矿工集合
	#[pallet::storage]
	#[pallet::getter(fn miner_set_of_order)]
	pub(super) type MinerSetOfOrder<T: Config> = StorageMap<_, Twox64Concat, u64, Vec<T::AccountId>, ValueQuery>;

	/// 时空证明报告
	#[pallet::storage]
	#[pallet::getter(fn report)]
	pub(super) type Report<T: Config> = StorageDoubleMap<_, Twox64Concat, T::BlockNumber, Twox64Concat, T::AccountId, ReportInfo, OptionQuery>;

	/// 领取收益标记位
	#[pallet::storage]
	#[pallet::getter(fn miner_order_income)]
	pub(super) type MinerOrderIncome<T: Config> = StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, u64, bool, ValueQuery>;

	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// 复制证明完成
		ProofOfReplicationFinish(u64),
		/// 注册成功
		RegisterSuccess(T::AccountId),
		/// 时空证明完成
		ProofOfSpacetimeFinish(T::AccountId),
		/// 健康检查完成
		HealthCheck(T::BlockNumber),
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(now: T::BlockNumber) -> Weight {
			if (now % T::ReportInterval::get()).is_zero() {
				//获得当前阶段
				let block_number = now -  T::ReportInterval::get();
				//当前阶段进行健康检查 进行工作量证明上报
				let (workload_map,total_storage) = Self::health_check(&block_number);
				//工作量证明上报
				T::Works::report_works(workload_map,total_storage);
				//发送健康检查事件
				Self::deposit_event(Event::HealthCheck(block_number));
			}
			0
		}
	}


	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// 非法矿工
		IllegalMiner,
		/// 非法文件CID
		IllegalFileCID,
		/// 已经调用订单完成
		AlreadyCallOrderFinish,
		/// 订单不存在
		OrderDoesNotExist,
		/// 零知识证明无效
		ProofInvalid,
		/// 证明与订单不匹配
		ProofNotMatchOrder,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T:Config> Pallet<T> {

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn register(
			origin: OriginFor<T>,
			total_storage: u128,
			used_storage: u128
		) -> DispatchResult {
			//判断是否签名正确
			let who = ensure_signed(origin)?;
			//查询当前矿工节点
			let mut miners = Miners::<T>::get();
			//遍历矿工是否存在，如果存在则进行覆盖操作，如果不存在则进行添加
			if let Err(index) = miners.binary_search(&who){
				//添加矿工
				miners.insert(index, who.clone());
				Miners::<T>::put(miners);
				//矿工个数+1
				let count = MinerCount::<T>::get();
				MinerCount::<T>::put(count + 1);
			}
			//更新存储空间数据
			Self::update_storage(&who,total_storage,used_storage);
			//存入当前阶段时空证明
			//获得当前块高
			let block_number = <frame_system::Pallet<T>>::block_number();
			//计算当前阶段
			let block_number = block_number - (block_number % T::ReportInterval::get());
			Report::<T>::insert(block_number,&who,ReportInfo::new(Vec::<u64>::new(),total_storage,used_storage));
			Self::deposit_event(Event::RegisterSuccess(who));
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn proof_of_replication(
			origin: OriginFor<T>,
			miner: T::AccountId,
			order_index: u64,
			cid: Vec<u8>,
			public_input: Vec<u8>,
			proof: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			//校验是否为矿工
			ensure!(&who == &miner, Error::<T>::IllegalMiner);
			//校验矿工是加入节点
			ensure!(Miners::<T>::get().contains(&miner), Error::<T>::IllegalMiner);
			//获取订单
			let order_info = T::StorageOrderInterface::get_storage_order(&order_index).ok_or(Error::<T>::OrderDoesNotExist)?;
			//检验文件cid是否正确
			ensure!(&order_info.cid == &cid, Error::<T>::IllegalFileCID);
			//校验文件状态 如果文件状态为取消状态则不能进行上报
			if let StorageOrderStatus::Canceled = &order_info.status {
				Err(Error::<T>::OrderDoesNotExist)?
			}
			//判断订单是否已经提交
			let miners = MinerSetOfOrder::<T>::get(&order_index);
			ensure!(!miners.contains(&miner), Error::<T>::AlreadyCallOrderFinish);

			//验证零知识证明
			let zk_validate = zk::poreq_validate(&proof,&public_input);
			ensure!(zk_validate,Error::<T>::ProofInvalid);

			//添加订单矿工信息
			Self::add_miner_set_of_order(&order_index,miner.clone());
			//添加订单副本
			T::StorageOrderInterface::add_order_replication(&order_index);
			T::StorageOrderInterface::update_storage_order_public_input(&order_index,public_input);
			//存入矿工订单数据
			let mut orders = MinerOrderSet::<T>::get(&miner);
			orders.push(order_index);
			MinerOrderSet::<T>::insert(&miner,orders);
			//发送订单完成事件
			Self::deposit_event(Event::ProofOfReplicationFinish(order_index));
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn proof_of_spacetime(
			origin: OriginFor<T>,
			orders: Vec<u64>,
			proofs: Vec<Vec<u8>>,
			total_storage: u128,
			used_storage: u128
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			//获得当前阶段
			//获得当前块高
			let block_number = <frame_system::Pallet<T>>::block_number();
			//计算当前阶段
			let block_number = block_number - (block_number % T::ReportInterval::get());
			// 校验订单数与证明数匹配
			ensure!(orders.len() == proofs.len(),Error::<T>::ProofNotMatchOrder);
			//通过矿工查询订单列表
			let miner_orders = MinerOrderSet::<T>::get(&who);
			//判断订单列表是否为空
			if miner_orders.is_empty(){
				//遍历时空证明参数订单
				for index in &orders {
					//添加订单矿工信息
					Self::add_miner_set_of_order(index,who.clone());
					//添加订单副本
					T::StorageOrderInterface::add_order_replication(index);
				}
				//添加入矿工订单中
				MinerOrderSet::<T>::insert(&who,orders.clone());
			}else{
				// 校验订单的时空证明
				for index in 0..orders.len() {
					let order_opt = T::StorageOrderInterface::get_storage_order(&orders[index]);
					if order_opt.is_some() {
						let proof = &proofs[index];
						let order = order_opt.unwrap();

						//验证零知识证明
						let zk_validate = zk::poreq_validate(proof,&order.public_input);
						ensure!(zk_validate,Error::<T>::ProofInvalid);
					}
				}
				//订单过滤
				let miner_orders = miner_orders.into_iter().filter(|index| {
					let result = orders.contains(index);
					if !result {
						//在订单矿工数据中删除该矿工
						Self::sub_miner_set_of_order(index,&who);
						//减掉订单信息副本
						T::StorageOrderInterface::sub_order_replication(index);
					}
					result
				}).collect::<Vec<u64>>();

				//修改矿工订单列表
				MinerOrderSet::<T>::insert(&who,miner_orders);
			}
			//更新存储空间数据
			Self::update_storage(&who,total_storage,used_storage);
			//存入时空证明
			Report::<T>::insert(block_number,&who,ReportInfo::new(orders,total_storage,used_storage));
			Self::deposit_event(Event::ProofOfSpacetimeFinish(who.clone()));
			//存入时空证明后进行判断是否可以免除手续费
			if T::BenefitInterface::maybe_free_count(&who) {
				return Ok(Pays::No.into());
			}
			Ok(Pays::Yes.into())
		}
	}
}


impl<T: Config> Pallet<T> {

	///更新个人存储
	fn update_storage(account_id: &T::AccountId, total_storage: u128, used_storage: u128) {
		let old_miner_total_storage = MinerTotalStorage::<T>::get(account_id);
		let old_miner_used_storage = MinerUsedStorage::<T>::get(account_id);
		//添加矿工总存储
		MinerTotalStorage::<T>::insert(account_id,total_storage);
		//添加矿工已用存储
		MinerUsedStorage::<T>::insert(account_id,used_storage);
		//添加矿工总存储
		let total_storage = TotalStorage::<T>::get() - old_miner_total_storage + total_storage;
		TotalStorage::<T>::put(total_storage);
		//添加矿工已用存储
		let used_storage = UsedStorage::<T>::get() - old_miner_used_storage + used_storage;
		UsedStorage::<T>::put(used_storage);
	}

	///进行健康检查
	fn health_check(block_number: &T::BlockNumber) -> (BTreeMap<T::AccountId, u128>, u128) {
		//工作量上报数据
		let mut workload_map = BTreeMap::<T::AccountId, u128>::new();
		//查询当前矿工节点
		let miners = Miners::<T>::get().into_iter().filter(|miner| {
			let result = Report::<T>::contains_key(block_number, miner);
			//如果不存在
			if !result {
				//获得矿工订单列表
				let orders = MinerOrderSet::<T>::get(miner);
				//删除订单矿工信息
				orders.into_iter().for_each(|order_index| {
					//在订单矿工数据中删除该矿工
					Self::sub_miner_set_of_order(&order_index,miner);
					//减掉订单信息副本
					T::StorageOrderInterface::sub_order_replication(&order_index);
				});
				//删除矿工订单信息
				MinerOrderSet::<T>::remove(miner);
				//删除矿工存储
				MinerTotalStorage::<T>::remove(&miner);
				MinerUsedStorage::<T>::remove(&miner);
				workload_map.insert(miner.clone(), 0);
			} else {
				//添加工作量上报数据
				let total_storage = MinerTotalStorage::<T>::get(&miner);
				//todo 关于副本系数获得有效存储空间
				workload_map.insert(miner.clone(), total_storage);
			}
			result
		}).collect::<Vec<T::AccountId>>();
		//维护矿工节点个数信息
		MinerCount::<T>::put(miners.len() as u64);
		//维护矿工信息
		Miners::<T>::put(miners);
		//更新总存储
		let total_storage = MinerTotalStorage::<T>::iter_values().sum::<u128>();
		TotalStorage::<T>::put(total_storage);
		//更新总使存储
		let used_storage = MinerUsedStorage::<T>::iter_values().sum::<u128>();
		UsedStorage::<T>::put(used_storage);
		(workload_map,total_storage)
	}

	///添加订单矿工信息
	fn add_miner_set_of_order(order_index: &u64, miner: T::AccountId){
		let mut miners = MinerSetOfOrder::<T>::get(order_index);
		if miners.contains(&miner) {
			return;
		}
		//判断当前矿工是否在收益列表 如果是则进行修改
		if miners.len() < T::NumberOfIncomeMiner::get() - 1 {
			MinerOrderIncome::<T>::insert(&miner,order_index,true);
		}
		//添加订单信息
		miners.push(miner.clone());
		MinerSetOfOrder::<T>::insert(order_index,miners);

	}

	///删除矿工订单信息
	fn sub_miner_set_of_order(order_index: &u64, miner: &T::AccountId){
		let mut miners = MinerSetOfOrder::<T>::get(order_index);
		//查询当前订单是否在收益列表中 如果在则将其去除并将第11位存入收益列表
		if MinerOrderIncome::<T>::get(miner,order_index) {
			let i = T::NumberOfIncomeMiner::get();
			if let Some(other_miner) = miners.get(i) {
				MinerOrderIncome::<T>::insert(other_miner,order_index,true);
			}
			//将删除的矿工在收益中删除
			MinerOrderIncome::<T>::insert(miner,order_index,false);
		}
		//在矿工列表中删除
		miners.retain(|x| x != miner);
		MinerSetOfOrder::<T>::insert(order_index,miners);
	}

	/// 分页查询矿工订单
	pub fn page_miner_order(account_id: T::AccountId, current: u64, size: u64, sort: u8) -> MinerOrderPage<T::AccountId,T::BlockNumber> {
		let orders = MinerOrderSet::<T>::get(&account_id);
		let total = orders.len() as u64;
		let current = if current == 0 { 1 } else { current };
		let list =  if sort == 0 {
			let begin = (current - 1) * size;
			let end = if current * size > total { total } else { current * size };
			Self::get_miner_order_list(&account_id, orders, begin, end)
		} else {
			let begin = if total >  current * size { total - current * size } else { 0 };
			let end = if total > (current - 1) * size { total - (current - 1) * size } else { 0 };
			let mut list = Self::get_miner_order_list(&account_id, orders, begin, end);
			list.reverse();
			list
		};
		MinerOrderPage::new(list, total)
	}

	/// 查询订单列表
	fn get_miner_order_list(account_id: &T::AccountId, orders: Vec<u64>,begin: u64,end: u64) -> Vec<MinerOrder<T::AccountId,T::BlockNumber>> {
		let mut list = Vec::<MinerOrder<T::AccountId,T::BlockNumber>>::new();
		if let Some(sub_orders) = orders.get(begin as usize..end as usize){
			for index in sub_orders {
				match T::StorageOrderInterface::get_storage_order(index){
					Some(t) => {
						//当前收益标志位
						let income_flag = MinerOrderIncome::<T>::get(account_id,index);
						//当前预估收益
						let mut estimated_income = 0;
						//获得当前订单存储人数
						let target_reward_count = MinerSetOfOrder::<T>::get(index).len().min(T::NumberOfIncomeMiner::get()) as u128;
						if target_reward_count > 0 && income_flag {
							//通过当前订单金额进行计算预估收益 （订单金额×存储金额比率 + 小费） /订单存储人数
							let income = (T::StorageRatio::get() * T::NumberToBalance::convert(t.order_price) + T::NumberToBalance::convert(t.tips)) / T::NumberToBalance::convert(target_reward_count);
							estimated_income = T::BalanceToNumber::convert(income);
						}
						//查询当前清算价格
						let calculate_income = T::PaymentInterface::get_calculate_income_by_miner_and_order_index(account_id, index);
						let calculate_income = T::BalanceToNumber::convert(calculate_income);
						list.push(MinerOrder::new(t,estimated_income, calculate_income));
					},
					None => ()
				}
			}
		}
		list
	}
}

impl<T: Config> WorkerInterface for Pallet<T> {
	type AccountId = T::AccountId;
	type BlockNumber = T::BlockNumber;
	type Balance = BalanceOf<T>;

	/// 获取订单矿工列表
	fn order_miners(order_id: u64) -> Vec<Self::AccountId>{
		MinerSetOfOrder::<T>::get(order_id)
	}
	/// 记录矿工收益
	fn record_miner_income(account_id: &Self::AccountId,income: Self::Balance){
		match MinerIncome::<T>::get(account_id) {
			Some(exists_income) => {
				let total_income = income + exists_income;
				MinerIncome::<T>::insert(account_id,total_income);
			},
			None => {
				MinerIncome::<T>::insert(account_id,income);
			}
		}
	}

	fn get_total_and_used() -> (u128, u128) {
		(Self::total_storage(), Self::used_storage())
	}
}
