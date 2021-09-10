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
}
