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
    /// 是否有收益
    pub income_flag: bool
}

impl<AccountId, BlockNumber> MinerOrder<AccountId, BlockNumber> {
    pub fn new (order: StorageOrder<AccountId, BlockNumber>, income_flag: bool) -> Self {
        MinerOrder {
            order,
            income_flag,
        }
    }
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
    pub total_storage: u64,
    /// 已用存储
    pub used_storage: u64
}

impl ReportInfo {
    pub fn new (orders: Vec<u64>, total_storage: u64, used_storage: u64) -> Self {
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
}
