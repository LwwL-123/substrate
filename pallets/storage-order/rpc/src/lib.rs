//! RPC interface for the transaction payment module.

use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;
use storage_order_runtime_api::OrderPage as OrderPage;
pub use storage_order_runtime_api::StorageOrderApi as StorageRuntimeApi;
use codec::Codec;

#[rpc]
pub trait StorageOrderApi<Block, AccountId, BlockNumber> {
    #[rpc(name = "storageOrder_pageUserOrder")]
    fn page_user_order(&self, account_id: AccountId, current: u64, size: u64, sort: u8) -> Result<OrderPage<AccountId,BlockNumber>>;
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

impl<C, Block, AccountId, BlockNumber> StorageOrderApi<<Block as BlockT>::Hash,AccountId,BlockNumber> for StorageOrder<C, Block>
    where
        Block: BlockT,
        C: Send + Sync + 'static,
        C: ProvideRuntimeApi<Block>,
        C: HeaderBackend<Block>,
        C::Api: StorageRuntimeApi<Block, AccountId, BlockNumber>,
        AccountId: Clone + std::fmt::Display + Codec,
        BlockNumber: Clone + std::fmt::Display + Codec
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
}