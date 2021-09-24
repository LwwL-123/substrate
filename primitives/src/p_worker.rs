#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use codec::{Encode, Decode};
use sp_std::vec::Vec;
use sp_debug_derive::RuntimeDebug;
use crate::p_storage_order::StorageOrder;

#[derive(Encode, Decode, RuntimeDebug,Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct MinerOrder<AccountId, BlockNumber> {
    /// 订单信息
    pub order: StorageOrder<AccountId, BlockNumber>,
    /// 预估收益金额
    #[cfg_attr(feature = "std", serde(serialize_with = "string_serialize"))]
    pub estimated_income: u128,
    /// 清算金额
    #[cfg_attr(feature = "std", serde(serialize_with = "string_serialize"))]
    pub calculate_income: u128
}

impl<AccountId, BlockNumber> MinerOrder<AccountId, BlockNumber> {
    pub fn new (order: StorageOrder<AccountId, BlockNumber>, estimated_income: u128,calculate_income: u128) -> Self {
        MinerOrder {
            order,
            estimated_income,
            calculate_income
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

#[derive(Encode, Decode, RuntimeDebug,Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct MinerOrderPage<AccountId, BlockNumber> {
    /// 内容
    pub content: Vec<MinerOrder<AccountId, BlockNumber>>,
    /// 页面总数
    pub total: u64
}

impl<AccountId, BlockNumber> MinerOrderPage<AccountId, BlockNumber> {
    pub fn new (content: Vec<MinerOrder<AccountId, BlockNumber>>, total: u64) -> Self {
        MinerOrderPage {
            content,
            total
        }
    }
}

#[derive(Encode, Decode, RuntimeDebug,Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct ReportInfo {
    /// 订单索引
    pub orders: Vec<u64>,
    /// 总存储量
    pub total_storage: u128,
    /// 已用存储
    pub used_storage: u128
}

impl ReportInfo {
    pub fn new (orders: Vec<u64>, total_storage: u128, used_storage: u128) -> Self {
        ReportInfo {
            orders,
            total_storage,
            used_storage
        }
    }
}

/// 支付接口
pub trait WorkerInterface {
    type AccountId;
    type BlockNumber;
    type Balance;
    /// 获取订单矿工列表
    fn order_miners(order_id: u64) -> Vec<Self::AccountId>;
    /// 记录矿工收益
    fn record_miner_income(account_id: &Self::AccountId,income: Self::Balance);
    /// 获得总资源空间和可用空间
    fn get_total_and_used() -> (u128, u128);
}
