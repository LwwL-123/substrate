#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use codec::{Encode, Decode};
use sp_std::vec::Vec;
use sp_debug_derive::RuntimeDebug;
use sp_std::vec;



pub const BASE_FEE_UPDATE_SLOT: u32 = 600;
pub const BASE_FEE_UPDATE_OFFSET: u32 = 22;

pub const PRICE_UPDATE_SLOT: u32 = 300;
pub const PRICE_UPDATE_OFFSET: u32 = 10;
pub const FILES_COUNT_REFERENCE: u64 = 20_000_000; // 20_000_000 / 50_000_000 = 40%


#[derive( Encode, Decode, RuntimeDebug, PartialEq, Eq, Copy, Clone)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum StorageOrderStatus {
    /// 待处理.
    Pending,
    /// 已完成.
    Finished,
    /// 已取消.
    Canceled,
    /// 已清算
    Cleared,
}

impl Default for StorageOrderStatus {
    fn default() -> Self {
        StorageOrderStatus::Pending
    }
}

#[derive(Encode, Decode, RuntimeDebug, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct StorageOrder<AccountId, BlockNumber> {
    /// 订单索引
    pub index: u64,
    /// cid
    pub cid: Vec<u8>,
    /// AccountId
    pub account_id: AccountId,
    /// 文件名
    pub file_name: Vec<u8>,
    /// 支付价格
    #[cfg_attr(feature = "std", serde(serialize_with = "string_serialize"))]
    pub price: u128,
    /// 文件基本价格
    #[cfg_attr(feature = "std", serde(serialize_with = "string_serialize"))]
    pub file_base_price: u128,
    /// 文件存储价格
    #[cfg_attr(feature = "std", serde(serialize_with = "string_serialize"))]
    pub order_price: u128,
    /// 小费
    #[cfg_attr(feature = "std", serde(serialize_with = "string_serialize"))]
    pub tips: u128,
    /// 存储期限
    pub storage_deadline: BlockNumber,
    /// 文件大小
    pub file_size: u64,
    /// 块高
    pub block_number: BlockNumber,
    /// 订单状态
    pub status: StorageOrderStatus,
    /// 副本数
    pub replication: u32,
    /// comm_d
    pub public_input: Vec<u8>,
}

impl<AccountId, BlockNumber> StorageOrder<AccountId, BlockNumber> {

    pub fn new (index: u64, cid: Vec<u8>, account_id: AccountId, file_name: Vec<u8>,
                price: u128, file_base_price: u128,order_price: u128,tips: u128,
                storage_deadline: BlockNumber, file_size: u64, block_number: BlockNumber) -> Self {
        StorageOrder {
            index,
            cid,
            account_id,
            file_name,
            price,
            file_base_price,
            order_price,
            tips,
            storage_deadline,
            file_size,
            block_number,
            status: StorageOrderStatus::Pending,
            replication: 0,
            public_input: vec![],
        }
    }
}

#[derive(Encode, Decode, RuntimeDebug, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OrderPage<AccountId, BlockNumber> {
    /// 内容
    pub content: Vec<StorageOrder<AccountId, BlockNumber>>,
    /// cid
    pub total: u64
}

impl<AccountId, BlockNumber> OrderPage<AccountId, BlockNumber> {
    pub fn new (content: Vec<StorageOrder<AccountId, BlockNumber>>, total: u64) -> Self {
        OrderPage {
            content,
            total
        }
    }
}

// u128 does not serialize well into JSON for `handlebars`, so we represent it as a string.
#[cfg(feature = "std")]
fn string_serialize<S>(x: &u128, s: S) -> Result<S::Ok, S::Error> where
    S: serde::Serializer
{
    s.serialize_str(&x.to_string())
}

pub trait StorageOrderInterface {
    type AccountId;
    type BlockNumber;

    /// 通过订单index获得存储订单信息
    fn get_storage_order(order_index: &u64) -> Option<StorageOrder<Self::AccountId,Self::BlockNumber>>;
    /// 更新存储文件的public_input
    fn update_storage_order_public_input(order_index: &u64,public_input: Vec<u8>);
    /// 添加订单副本
    fn add_order_replication(order_index: &u64);
    /// 减少订单副本
    fn sub_order_replication(order_index: &u64);
    /// 更新订单状态为已清算
    fn update_order_status_to_cleared(order_index: &u64);
}
