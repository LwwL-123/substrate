#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>

pub use pallet::*;
use sp_std::vec::Vec;
use frame_support::{
	traits::{Currency},
	dispatch::DispatchResult, pallet_prelude::*
};
use sp_runtime::traits::Convert;

use primitives::p_storage_order::*;
use primitives::p_payment::*;


#[frame_support::pallet]
pub mod pallet {
	use frame_system::pallet_prelude::*;
	use super::*;

	type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// 订单等待时间
		type OrderWaitingTime: Get<Self::BlockNumber>;
		// 每byte 每天的价格
		type PerByteDayPrice: Get<u64>;

		/// 支付费用和持有余额的货币。
		type Currency: Currency<Self::AccountId>;

		/// 金额转换数字
		type BalanceToNumber: Convert<BalanceOf<Self>, u128>;
		// 区块高度转数字
		type BlockNumberToNumber: Convert<Self::BlockNumber,u128>;

		/// 订单接口
		type PaymentInterface: PaymentInterface<AccountId = Self::AccountId, BlockNumber = Self::BlockNumber,Balance = BalanceOf<Self>>;
	}


	/// 存储订单个数
	#[pallet::storage]
	#[pallet::getter(fn order_count)]
	pub(super) type OrderCount<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// 有效文件数
	#[pallet::storage]
	#[pallet::getter(fn valid_files_count)]
	pub(super) type ValidFilesCount<T: Config> = StorageValue<_,u64,ValueQuery>;

	/// 总副本数
	#[pallet::storage]
	#[pallet::getter(fn total_copies)]
	pub(super) type TotalCopies<T: Config> = StorageValue<_,u64,ValueQuery>;

	/// 存储订单信息
	#[pallet::storage]
	#[pallet::getter(fn order_info)]
	pub(super) type OrderInfo<T: Config> = StorageMap<_, Twox64Concat, u64, StorageOrder<T::AccountId, T::BlockNumber>, OptionQuery>;

	/// 用户订单个数
	#[pallet::storage]
	#[pallet::getter(fn user_order_count)]
	pub(super) type UserOrderCount<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, u64, ValueQuery>;

	/// 用户订单数据
	#[pallet::storage]
	#[pallet::getter(fn user_order_index)]
	pub(super) type UserOrderIndex<T: Config> = StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, u64, u64, OptionQuery>;

	/// 块高存储订单集合
	#[pallet::storage]
	#[pallet::getter(fn order_set_of_block)]
	pub(super) type OrderSetOfBlock<T: Config> = StorageMap<_, Twox64Concat, T::BlockNumber, Vec<u64>, OptionQuery>;


	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// 订单创建
		OrderCreated(u64, Vec<u8>, T::AccountId, Vec<u8>, T::BlockNumber, u32),
		/// 订单完成
		OrderFinish(u64),
		/// 订单取消
		OrderCanceled(u64, Vec<u8>),
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(now: T::BlockNumber) -> Weight {
			//判断当前块高是否大于订单等待时长
			if now >= T::OrderWaitingTime::get() {
				//获得等待时长之前的块索引
				let block_index = now - T::OrderWaitingTime::get();
				let order_set = OrderSetOfBlock::<T>::get(&block_index).unwrap_or(Vec::<u64>::new());
				for order_index in &order_set {
					//获取订单信息
					match OrderInfo::<T>::get(order_index) {
						Some(mut order_info) => {
							if let StorageOrderStatus::Pending = order_info.status {
								order_info.status = StorageOrderStatus::Canceled;
								OrderInfo::<T>::insert(order_index,order_info.clone());
								// 退款
								T::PaymentInterface::cancel_order(&order_index,&order_info.price,&order_info.storage_deadline,&order_info.account_id);
								//发送订单取消时间事件
								Self::deposit_event(Event::OrderCanceled(order_index.clone() , order_info.cid));
							}
						},
						None => ()
					}
				}
			}
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
		/// 非法矿工
		IllegalMiner,
		/// 非法文件CID
		IllegalFileCID,
		/// 订单已经取消
		OrderCancelled,
		/// 已经调用订单完成
		AlreadyCallOrderFinish,
		/// 订单不存在
		OrderDoesNotExist,
		/// 订单价格错误
		OrderPriceError,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T:Config> Pallet<T> {

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn create_order(
			origin: OriginFor<T>,
			cid: Vec<u8>,
			file_name: Vec<u8>,
			price: BalanceOf<T>,
			duration: T::BlockNumber,
			size: u32
		) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://substrate.dev/docs/en/knowledgebase/runtime/origin
			// https://substrate.dev/docs/en/knowledgebase/runtime/origin
			let who = ensure_signed(origin)?;
			//获取订单长度
			let order_index = OrderCount::<T>::get();
			//获得当前块高
			let block_number = <frame_system::Pallet<T>>::block_number();
			//获得存储期限
			let storage_deadline = block_number + duration;
			//创建订单
			let order = StorageOrder::new(
				order_index,
				cid.clone(),
				who.clone(),
				file_name.clone(),
				T::BalanceToNumber::convert(price.clone()),
				storage_deadline,
				size,
				block_number.clone());
			let per_day_block = primitives::constants::time::DAYS as u128;
			let per_byte_day_price = T::PerByteDayPrice::get() as u128;
			let size_u128 = size as u128;
			let duration_u128 = T::BlockNumberToNumber::convert(duration);
			let price_u128 = T::BalanceToNumber::convert(price);
			ensure!( (size_u128 * duration_u128  * per_byte_day_price / per_day_block )  <=  price_u128, Error::<T>::OrderPriceError);
			// 支付模块记录订单金额
			T::PaymentInterface::pay_order(&order_index,&price,&order.storage_deadline,&who)?;
			//存入区块数据
			OrderInfo::<T>::insert(&order_index, order.clone());
			//获得用户索引个数
			let user_order_index = UserOrderCount::<T>::get(&who);
			//存入用户索引数据
			UserOrderIndex::<T>::insert(&who,&user_order_index,order_index);
			//订单长度+1
			OrderCount::<T>::put(order_index + 1);
			//用户索引个数+1
			UserOrderCount::<T>::insert(&who,user_order_index + 1);
			//添加块高存储订单集合
			let mut order_set = OrderSetOfBlock::<T>::get(&block_number).unwrap_or(Vec::<u64>::new());
			order_set.push(order_index);
			OrderSetOfBlock::<T>::insert(&block_number,order_set);
			//发送订单创建事件
			Self::deposit_event(Event::OrderCreated(order.index,order.cid,order.account_id,order.file_name,
													order.storage_deadline,order.file_size));
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}
	}
}


impl<T: Config> Pallet<T> {

	// Add public immutables and private mutables.
	pub fn page_user_order(account_id: T::AccountId, current: u64, size: u64, sort: u8) -> OrderPage<T::AccountId,T::BlockNumber> {
		let total = UserOrderCount::<T>::get(&account_id) as u64;
		let current = if current == 0 { 1 } else { current };
		let list =  if sort == 0 {
			let begin = (current - 1) * size;
			let end = if current * size > total { total } else { current * size };
			Self::get_order_list(&account_id, begin, end)
		} else {
			let begin = if total >  current * size { total - current * size } else { 0 };
			let end = if total > (current - 1) * size { total - (current - 1) * size } else { 0 };
			let mut list = Self::get_order_list(&account_id,begin, end);
			list.reverse();
			list
		};
		OrderPage::new(list, total)
	}

	fn get_order_list(account_id: &T::AccountId,begin: u64,end: u64) -> Vec<StorageOrder<T::AccountId,T::BlockNumber>> {
		let mut list = Vec::<StorageOrder<T::AccountId,T::BlockNumber>>::new();
		for i in begin..end {
			match UserOrderIndex::<T>::get(account_id,i){
				Some(index) =>{
					match OrderInfo::<T>::get(index){
						Some(t) => {
							list.push(t);
						},
						None => ()
					}
				},
				None => ()
			}

		}
		list
	}
}

impl<T: Config> StorageOrderInterface for Pallet<T> {
	type AccountId = T::AccountId;
	type BlockNumber = T::BlockNumber;

	fn get_storage_order(order_index: &u64) -> Option<StorageOrder<Self::AccountId, Self::BlockNumber>> {
		OrderInfo::<T>::get(order_index)
	}

	fn add_order_replication(order_index: &u64) {
		//获取订单
		if let Some(mut order_info) = OrderInfo::<T>::get(order_index){
			//校验文件状态 如果文件状态为待处理则改为已完成
			if let StorageOrderStatus::Canceled = &order_info.status {
				return;
			}
			if let StorageOrderStatus::Pending = &order_info.status {
				order_info.status = StorageOrderStatus::Finished;
				let count = ValidFilesCount::<T>::get();
				ValidFilesCount::<T>::put(count + 1);
			}
			//订单信息副本数+1
			order_info.replication = order_info.replication + 1;
			OrderInfo::<T>::insert(order_index,order_info);
			let replicationCount = TotalCopies::<T>::get();
			TotalCopies::<T>::put(replicationCount + 1);

		}
	}

	fn sub_order_replication(order_index: &u64) {
		//获取订单
		if let Some(mut order_info) = OrderInfo::<T>::get(order_index) {
			//校验文件状态 如果文件状态为待处理则改为已完成
			if let StorageOrderStatus::Canceled = &order_info.status {
				return;
			}
			//订单信息副本数-1
			if order_info.replication > 0 {
				order_info.replication = order_info.replication - 1;
				let replicationCount = TotalCopies::<T>::get();
				TotalCopies::<T>::put(replicationCount - 1);
			}else if order_info.replication == 0 {
				if  ValidFilesCount::<T>::get() > 0{
					let count = ValidFilesCount::<T>::get();
					ValidFilesCount::<T>::put(count - 1);
				}
			}
			OrderInfo::<T>::insert(order_index,order_info);
		}
	}
}
