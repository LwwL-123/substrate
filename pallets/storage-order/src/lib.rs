#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>

pub use pallet::*;
use sp_std::{vec::Vec,  convert::TryInto};
use frame_support::{
	traits::{Currency},
	dispatch::DispatchResult, pallet_prelude::*
};
use sp_runtime::traits::{Convert, Saturating, Zero, CheckedMul};

use primitives::p_storage_order::*;
use primitives::p_payment::*;
use primitives::p_worker::WorkerInterface;
use sp_runtime::Perbill;
use frame_system::{self as system};

type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as system::Config>::AccountId>>::Balance;

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

		/// 订单等待时间
		type OrderWaitingTime: Get<Self::BlockNumber>;
		// 每byte 每天的价格
		type PerByteDayPrice: Get<u64>;

		/// 支付费用和持有余额的货币。
		type Currency: Currency<Self::AccountId>;

		/// 金额转换数字
		type BalanceToNumber: Convert<BalanceOf<Self>, u128>;
		/// 区块高度转数字
		type BlockNumberToNumber: Convert<Self::BlockNumber,u128>;

		/// 订单接口
		type PaymentInterface: PaymentInterface<AccountId = Self::AccountId, BlockNumber = Self::BlockNumber,Balance = BalanceOf<Self>>;

		/// 文件基本费用
		type FileBaseInitFee: Get<BalanceOf<Self>>;
		/// 文件个数费用
		type FilesCountInitPrice: Get<BalanceOf<Self>>;
		/// 文件每天价格费用 每MB每天费用
		type FileSizeInitPrice: Get<BalanceOf<Self>>;
		/// 存储参考比率. reported_files_size / total_capacity
		type StorageReferenceRatio: Get<(u128, u128)>;
		/// 价格上升比率
		type StorageIncreaseRatio: Get<Perbill>;
		/// 价格下浮比率
		type StorageDecreaseRatio: Get<Perbill>;
		/// worker接口
		type WorkerInterface: WorkerInterface<AccountId = Self::AccountId, BlockNumber = Self::BlockNumber,Balance = BalanceOf<Self>>;
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

	/// 文件基本费用
	#[pallet::storage]
	#[pallet::getter(fn file_base_fee)]
	pub(super) type FileBaseFee<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery, T::FileBaseInitFee>;

	/// 文件字节费用 每MB每天的价格，根据市场存储比率进行价格浮动
	#[pallet::storage]
	#[pallet::getter(fn file_size_price)]
	pub(super) type FileSizePrice<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery, T::FileSizeInitPrice>;

	/// 文件按件费用 根据有效订单量进行价格浮动
	#[pallet::storage]
	#[pallet::getter(fn files_count_price)]
	pub(super) type FilesCountPrice<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery, T::FilesCountInitPrice>;

	/// Added files count in the past one period(one hour)
	#[pallet::storage]
	#[pallet::getter(fn added_files_count)]
	pub(super) type AddedFilesCount<T: Config> = StorageValue<_, u32, ValueQuery>;

	/// 添加订单个数 每次创建订单时都会添加 并每隔一定时间清除
	#[pallet::storage]
	#[pallet::getter(fn added_order_count)]
	pub(super) type AddedOrderCount<T: Config> = StorageValue<_, u32, ValueQuery>;

	/// 添加新订单标志，如果本阶段创建订单则可以进行价格更新
	#[pallet::storage]
	#[pallet::getter(fn new_order)]
	pub(super) type NewOrder<T: Config> = StorageValue<_, bool, ValueQuery>;

	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// 订单创建
		OrderCreated(u64, Vec<u8>, T::AccountId, Vec<u8>, T::BlockNumber, u64),
		/// 订单完成
		OrderFinish(u64),
		/// 订单取消
		OrderCanceled(u64, Vec<u8>),
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(now: T::BlockNumber) -> Weight {
			let mut consumed_weight: Weight = 0;
			let mut add_db_reads_writes = |reads, writes| {
				consumed_weight += T::DbWeight::get().reads_writes(reads, writes);
			};
			//判断当前块高是否大于订单等待时长
			if now >= T::OrderWaitingTime::get() {
				//获得等待时长之前的块索引
				let block_index = now - T::OrderWaitingTime::get();
				//更新等待订单状态
				Self::update_waiting_order_status(block_index);
			}
			let now = TryInto::<u32>::try_into(now).ok().unwrap();
			//更新基本费用
			if ((now + PRICE_UPDATE_OFFSET) % PRICE_UPDATE_SLOT).is_zero() && Self::new_order(){
				Self::update_file_price();
				Self::update_files_count_price();
				NewOrder::<T>::put(false);
				add_db_reads_writes(8, 3);
			}
			//更新文件字节费用和文件按件费用
			if ((now + BASE_FEE_UPDATE_OFFSET) % BASE_FEE_UPDATE_SLOT).is_zero() {
				Self::update_base_fee();
				add_db_reads_writes(3, 3);
			}
			add_db_reads_writes(1, 0);
			consumed_weight
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
			tips: BalanceOf<T>,
			duration: T::BlockNumber,
			size: u64
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
			//计算价格
			let (file_base_price, order_price) = Self::get_order_price(size,duration);
			//创建订单
			let order = StorageOrder::new(
				order_index,
				cid.clone(),
				who.clone(),
				file_name.clone(),
				T::BalanceToNumber::convert(file_base_price + order_price + tips),
				storage_deadline,
				size,
				block_number.clone());
			// 支付模块记录订单金额
			T::PaymentInterface::pay_order(&order_index,file_base_price,order_price,tips,order.storage_deadline,&who)?;
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
			//添加订单创建个数
			AddedOrderCount::<T>::mutate(|count| {*count = count.saturating_add(1)});
			//创建订单标志位
			NewOrder::<T>::put(true);
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

	/// 更新等待订单状态
	/// block_index 为该区块内生成的待处理订单改为已取消状态
	fn update_waiting_order_status(block_index: T::BlockNumber){
		let order_set = OrderSetOfBlock::<T>::get(&block_index).unwrap_or(Vec::<u64>::new());
		for order_index in &order_set {
			//获取订单信息
			match OrderInfo::<T>::get(order_index) {
				Some(mut order_info) => {
					if let StorageOrderStatus::Pending = order_info.status {
						order_info.status = StorageOrderStatus::Canceled;
						OrderInfo::<T>::insert(order_index,order_info.clone());
						// 退款
						T::PaymentInterface::cancel_order(&order_index,&order_info.account_id);
						//发送订单取消时间事件
						Self::deposit_event(Event::OrderCanceled(order_index.clone() , order_info.cid));
					}
				},
				None => ()
			}
		}
	}

	/// 更新文件价格
	fn update_file_price() {
		let (total_capacity, files_size) = T::WorkerInterface::get_total_and_used();
		let (numerator, denominator) = T::StorageReferenceRatio::get();
		// Too much supply => decrease the price
		if files_size.saturating_mul(denominator) <= total_capacity.saturating_mul(numerator) {
			FileSizePrice::<T>::mutate(|file_price| {
				let gap = T::StorageDecreaseRatio::get() * file_price.clone();
				*file_price = file_price.saturating_sub(gap);
			});
		} else {
			FileSizePrice::<T>::mutate(|file_price| {
				let gap = (T::StorageIncreaseRatio::get() * file_price.clone()).max(BalanceOf::<T>::from(1u32));
				*file_price = file_price.saturating_add(gap);
			});
		}
	}

	/// 更新文件按件价格
	fn update_files_count_price() {
		let files_count = Self::valid_files_count();
		if files_count > FILES_COUNT_REFERENCE {
			FilesCountPrice::<T>::mutate(|price| {
				let gap = (T::StorageIncreaseRatio::get() * price.clone()).max(BalanceOf::<T>::from(1u32));
				*price = price.saturating_add(gap);
			})
		} else {
			FilesCountPrice::<T>::mutate(|price| {
				let gap = T::StorageDecreaseRatio::get() * price.clone();
				*price = price.saturating_sub(gap);
			})
		}
	}

	/// 更新文件基础价格
	fn update_base_fee() {
		// get added files count and clear the record
		let added_files_count = Self::added_files_count();
		AddedFilesCount::<T>::put(0);
		// get orders count and clear the record
		let orders_count = Self::added_order_count();
		AddedOrderCount::<T>::put(0);
		// decide what to do
		let (is_to_decrease, ratio) = Self::base_fee_ratio(added_files_count.checked_div(orders_count));
		// update the file base fee
		FileBaseFee::<T>::mutate(|price| {
			let gap = ratio * price.clone();
			if is_to_decrease {
				*price = price.saturating_sub(gap);
			} else {
				*price = price.saturating_add(gap);
			}
		})
	}

	/// return (bool, ratio)
    /// true => decrease the price, false => increase the price
	fn base_fee_ratio(maybe_alpha: Option<u32>) -> (bool, Perbill) {
		match maybe_alpha {
			// New order => check the alpha
			Some(alpha) => {
				match alpha {
					0 ..= 5 => (false, Perbill::from_percent(30)),
					6 => (false,Perbill::from_percent(25)),
					7 => (false,Perbill::from_percent(21)),
					8 => (false,Perbill::from_percent(18)),
					9 => (false,Perbill::from_percent(16)),
					10 => (false,Perbill::from_percent(15)),
					11 => (false,Perbill::from_percent(13)),
					12 => (false,Perbill::from_percent(12)),
					13 => (false,Perbill::from_percent(11)),
					14 ..= 15 => (false,Perbill::from_percent(10)),
					16 => (false,Perbill::from_percent(9)),
					17 ..= 18 => (false,Perbill::from_percent(8)),
					19 ..= 21 => (false,Perbill::from_percent(7)),
					22 ..= 25 => (false,Perbill::from_percent(6)),
					26 ..= 30 => (false,Perbill::from_percent(5)),
					31 ..= 37 => (false,Perbill::from_percent(4)),
					38 ..= 49 => (false,Perbill::from_percent(3)),
					50 ..= 100 => (false,Perbill::zero()),
					_ => (true, Perbill::from_percent(3))
				}
			},
			// No new order => decrease the price
			None => (true, Perbill::from_percent(3))
		}
	}

	/// Calculate file price
	/// Include the file base fee, file size price and files count price
	/// return => (file_base_fee, file_size_price + files_count_price)
	pub fn get_order_price(file_size: u64,duration: T::BlockNumber,) -> (BalanceOf<T>, BalanceOf<T>) {
		// 1. Calculate file size price
		// Rounded file size from `bytes` to `megabytes`
		let mut rounded_file_size = (file_size / 1_048_576) as u32;
		if file_size % 1_048_576 != 0 {
			rounded_file_size += 1;
		}
		let price = Self::file_size_price();
		// 将时间转换为天书 不满一天按一天算
		let per_day_block = primitives::constants::time::DAYS as u128;
		let duration_u128 = T::BlockNumberToNumber::convert(duration);
		let mut file_days = (duration_u128 / per_day_block) as u32;
		if duration_u128 % per_day_block != 0 {
			file_days += 1;
		}
		// Convert file size into `Currency`
		let amount = price.checked_mul(&BalanceOf::<T>::from(rounded_file_size * file_days));
		let file_size_price = match amount {
			Some(value) => value,
			None => Zero::zero(),
		};
		// 2. Get file base fee
		let file_base_fee = Self::file_base_fee();
		// 3. Get files count price
		let files_count_price = Self::files_count_price();

		(file_base_fee, file_size_price + files_count_price)
	}
}

impl<T: Config> StorageOrderInterface for Pallet<T> {
	type AccountId = T::AccountId;
	type BlockNumber = T::BlockNumber;

	fn get_storage_order(order_index: &u64) -> Option<StorageOrder<Self::AccountId, Self::BlockNumber>> {
		OrderInfo::<T>::get(order_index)
	}

	/// 更新存储文件的comm_c,comm_r
	fn update_storage_order_public_input(order_index: &u64,public_input: Vec<u8>){
		//获取订单
		if let Some(mut order_info) = OrderInfo::<T>::get(order_index){
			//校验文件状态 如果文件状态为待处理则改为已完成
			if let StorageOrderStatus::Canceled = &order_info.status {
				return;
			}
			//更新订单的comm_c 和 comm_r
			order_info.replication = order_info.replication + 1;
			if order_info.public_input.is_empty(){
				order_info.public_input = public_input;
			}
			OrderInfo::<T>::insert(order_index,order_info);
		}
	}
	/// 添加订单副本数
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
			let replication_count = TotalCopies::<T>::get();
			TotalCopies::<T>::put(replication_count + 1);
			AddedFilesCount::<T>::mutate(|count| {*count = count.saturating_add(1)});
		}
	}
	/// 减少订单副本数
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
				let replication_count = TotalCopies::<T>::get();
				TotalCopies::<T>::put(replication_count - 1);
			}
			if order_info.replication == 0 {
				if  ValidFilesCount::<T>::get() > 0{
					let count = ValidFilesCount::<T>::get();
					ValidFilesCount::<T>::put(count - 1);
				}
			}
			OrderInfo::<T>::insert(order_index,order_info);
		}
	}
	/// 将订单改为已清算状态
	fn update_order_status_to_cleared(order_index: &u64) {
		//校验订单状态
		if let Some(mut order_info) = OrderInfo::<T>::get(order_index) {
			//如果为已完成则进入已清算
			if let StorageOrderStatus::Finished = order_info.status {
				//订单状态修改
				order_info.status = StorageOrderStatus::Cleared;
				OrderInfo::<T>::insert(order_index,order_info);
				//订单有效文件-1
				ValidFilesCount::<T>::mutate(|count| {*count = count.saturating_sub(1)} );
			}
		}
	}
}
