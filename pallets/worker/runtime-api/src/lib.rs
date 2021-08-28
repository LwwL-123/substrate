#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

// Here we declare the runtime API. It is implemented it the `impl` block in
// runtime amalgamator file (the `runtime/src/lib.rs`)
pub use primitives::p_worker::MinerOrderPage;
use codec::Codec;

sp_api::decl_runtime_apis! {

    pub trait WorkerApi<AccountId,BlockNumber> where
        AccountId: Codec,
        BlockNumber: Codec
    {
        fn page_miner_order(account_id: AccountId, current: u64, size: u64, sort: u8) -> MinerOrderPage<AccountId,BlockNumber>;
    }
}
