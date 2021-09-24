#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::DispatchResult;
use codec::{Encode, Decode};
use sp_debug_derive::RuntimeDebug;

/// 订单支出信息
#[derive(Encode, Decode, RuntimeDebug, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct PayoutInfo<Balance, BlockNumber> {
    /// 订单支出给存储账户
    pub amount: Balance,
    /// 过期时间
    pub deadline: BlockNumber,
    /// 计算区块
    pub calculate: BlockNumber
}

/// 支付接口
pub trait PaymentInterface {
    type AccountId;
    type BlockNumber;
    type Balance;
    /// 订单支付/续费
    fn pay_order(order_index: &u64, file_base_price: Self::Balance, order_price: Self::Balance, tips: Self::Balance, deadline: Self::BlockNumber, account_id: &Self::AccountId) -> DispatchResult;
    /// 订单取消退款
    fn cancel_order(order_index: &u64, account_id: &Self::AccountId);
    /// 用于分配支付支出
    fn withdraw_staking_pool() -> Self::Balance;
    /// 订单拆分数据存入相应金额池中
    fn transfer_reserved_and_storage_and_staking_pool_by_temporary_pool(order_index: &u64);
    /// 通过账户和订单查询清算价格
    fn get_calculate_income_by_miner_and_order_index(account_id: &Self::AccountId, order_index: &u64) -> Self::Balance;
}
