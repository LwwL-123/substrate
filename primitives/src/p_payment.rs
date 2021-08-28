use sp_runtime::DispatchResult;

/// 支付接口
pub trait PaymentInterface {
    type AccountId;
    type BlockNumber;
    type Balance;
    /// 记录订单金额
    fn pay_order(order_index: &u64, order_price: &Self::Balance,deadline: &Self::BlockNumber, account_id: &Self::AccountId) -> DispatchResult;
    /// 订单取消退款
    fn cancel_order(order_index: &u64, order_price: &u128,deadline: &Self::BlockNumber, account_id: &Self::AccountId);
}
