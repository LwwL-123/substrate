//! RPC interface for the transaction payment module.

use std::convert::TryInto;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::{Block as BlockT, MaybeDisplay}};
use std::sync::Arc;
use storage_order_runtime_api::OrderPage as OrderPage;
pub use storage_order_runtime_api::StorageOrderApi as StorageRuntimeApi;
use codec::Codec;
use sp_rpc::number::NumberOrHex;

#[rpc]
pub trait StorageOrderApi<Block, AccountId, BlockNumber, Balance> {
    #[rpc(name = "storageOrder_pageUserOrder")]
    fn page_user_order(&self, account_id: AccountId, current: u64, size: u64, sort: u8) -> Result<OrderPage<AccountId,BlockNumber>>;
    #[rpc(name = "storageOrder_getOrderPrice")]
    fn get_order_price(&self, file_size: u64, duration: BlockNumber) -> Result<NumberOrHex>;
}

/// A struct that implements the `StorageOrderApi`.
pub struct StorageOrder<C, M> {
    // If you have more generics, no need to StorageOrder<C, M, N, P, ...>
    // just use a tuple like StorageOrder<C, (M, N, P, ...)>
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> StorageOrder<C, M> {
    /// Create new `StorageOrder` instance with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

/// Error type of this RPC api.
// pub enum Error {
// 	/// The transaction was not decodable.
// 	DecodeError,
// 	/// The call to runtime failed.
// 	RuntimeError,
// }
//
// impl From<Error> for i64 {
// 	fn from(e: Error) -> i64 {
// 		match e {
// 			Error::RuntimeError => 1,
// 			Error::DecodeError => 2,
// 		}
// 	}
// }

impl<C, Block, AccountId, BlockNumber, Balance> StorageOrderApi<<Block as BlockT>::Hash,AccountId,BlockNumber,Balance> for StorageOrder<C, Block>
    where
        Block: BlockT,
        C: Send + Sync + 'static,
        C: ProvideRuntimeApi<Block>,
        C: HeaderBackend<Block>,
        C::Api: StorageRuntimeApi<Block, AccountId, BlockNumber, Balance>,
        AccountId: Clone + std::fmt::Display + Codec,
        BlockNumber: Clone + std::fmt::Display + Codec,
        Balance: Codec + MaybeDisplay + Copy + TryInto<NumberOrHex> + std::ops::Add<Output = Balance>,
{
    fn page_user_order(&self, account_id: AccountId, current: u64, size: u64, sort: u8) -> Result<OrderPage<AccountId, BlockNumber>> {
        let api = self.client.runtime_api();
        let best = self.client.info().best_hash;
        let at = BlockId::hash(best);
        let runtime_api_result = api.page_user_order(&at,account_id,current,size,sort);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876), // No real reason for this value
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_order_price(&self, file_size: u64, duration: BlockNumber) -> Result<NumberOrHex> {
        let api = self.client.runtime_api();
        let best = self.client.info().best_hash;
        let at = BlockId::hash(best);
        let (file_base_price, order_price) = api.get_order_price(&at,file_size,duration).map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876), // No real reason for this value
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })?;
        let try_into_rpc_balance = |value: Balance| value.try_into().map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: format!("{} doesn't fit in NumberOrHex representation", value),
            data: None,
        });
        try_into_rpc_balance(file_base_price + order_price)
    }
}