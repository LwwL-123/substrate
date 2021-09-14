#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit="256"]

mod impls;// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use sp_std::prelude::*;
use sp_core::{
	crypto::KeyTypeId,
	OpaqueMetadata,
};
use sp_runtime::{Perquintill, FixedPointNumber, ApplyExtrinsicResult, generic, create_runtime_str, impl_opaque_keys, transaction_validity::{TransactionValidity, TransactionSource, TransactionPriority}};
use sp_runtime::traits::{
	BlakeTwo256, Block as BlockT, AccountIdLookup, NumberFor, ConvertInto,
	OpaqueKeys,
};
use sp_api::impl_runtime_apis;
use sp_consensus_babe;
use pallet_grandpa::{AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList};
use pallet_grandpa::fg_primitives;
use sp_version::RuntimeVersion;
#[cfg(feature = "std")]
use sp_version::NativeVersion;

// A few exports that help ease life for downstream crates.
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use pallet_timestamp::Call as TimestampCall;
pub use pallet_balances::Call as BalancesCall;
pub use pallet_balances;
pub use sp_runtime::{Permill, Perbill};
pub use frame_support::{
	construct_runtime, parameter_types, StorageValue,
	traits::{KeyOwnerProofSystem, Randomness,Currency, Imbalance, OnUnbalanced, LockIdentifier,
			 U128CurrencyToVote, MaxEncodedLen,},
	weights::{
		Weight, IdentityFee,
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
		DispatchClass,
	},
	debug,RuntimeDebug,
};
pub use pallet_transaction_payment::{Multiplier, TargetedFeeAdjustment, FeeDetails, OnChargeTransaction};
use sp_runtime::curve::PiecewiseLinear;
use pallet_session::{historical as pallet_session_historical};
pub use pallet_staking::StakerStatus;
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot,
};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;

/// Implementations of some helper traits passed into runtime modules as associated types.
use impls::{CurrencyToVoteHandler, Author, OneTenthFee, CurrencyAdapter};

/// 引用元数据
pub use primitives::{
	p_storage_order::OrderPage,
	p_worker::MinerOrderPage,
	constants::{time::*,currency::*},
	*
};

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
	use super::*;

	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;

	impl_opaque_keys! {
		pub struct SessionKeys {
			pub babe: Babe,
			pub grandpa: Grandpa,
			pub authority_discovery: AuthorityDiscovery,
		}
	}
}
/// We assume that ~10% of the block weight is consumed by `on_initialize` handlers.
/// This is used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 6 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;


// To learn more about runtime versioning and what each of the following value means:
//   https://substrate.dev/docs/en/knowledgebase/runtime/upgrades#runtime-versioning
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("ttc-chain"),
	impl_name: create_runtime_str!("ttc-chain"),
	authoring_version: 1,
	// The version of the runtime specification. A full node will not attempt to use its native
	//   runtime in substitute for the on-chain Wasm runtime unless all of `spec_name`,
	//   `spec_version`, and `authoring_version` are the same between Wasm and native.
	// This value is set to 100 to notify Polkadot-JS App (https://polkadot.js.org/apps) to use
	//   the compatible custom types.
	spec_version: 104,
	impl_version: 1,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
};

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}



parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
	pub const MaximumBlockWeight: Weight = 2 * WEIGHT_PER_SECOND;
	pub const Version: RuntimeVersion = VERSION;
	pub RuntimeBlockLength: BlockLength =
		BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have some extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();

	pub const SS58Prefix: u16 = 42;
}

// Configure FRAME pallets to include in runtime.

impl frame_system::Config for Runtime {
	/// The basic call filter to use in dispatchable.
	type BaseCallFilter = ();
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = RuntimeBlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = RuntimeBlockLength;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The aggregated dispatch type that is available for extrinsics.
	type Call = Call;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = AccountIdLookup<AccountId, ()>;
	/// The index type for storing how many extrinsics an account has signed.
	type Index = Index;
	/// The index type for blocks.
	type BlockNumber = BlockNumber;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The header type.
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// The ubiquitous event type.
	type Event = Event;
	/// The ubiquitous origin type.
	type Origin = Origin;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// Version of the runtime.
	type Version = Version;
	/// Converts a module to the index of the module in `construct_runtime!`.
	///
	/// This type is being generated by `construct_runtime!`.
	type PalletInfo = PalletInfo;
	/// What to do if a new account is created.
	type OnNewAccount = ();
	/// What to do if an account is fully reaped from the system.
	type OnKilledAccount = ();
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// Weight information for the extrinsics of this pallet.
	type SystemWeightInfo = ();
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	/// The set code logic, just the default since we're not a parachain.
	type OnSetCode = ();
}

impl pallet_utility::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
}

impl pallet_randomness_collective_flip::Config for Runtime {}


pub const BABE_GENESIS_EPOCH_CONFIG: sp_consensus_babe::BabeEpochConfiguration =
	sp_consensus_babe::BabeEpochConfiguration {
		c: PRIMARY_PROBABILITY,
		allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots
	};

parameter_types! {
    pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
    pub const ExpectedBlockTime: u64 = MILLISECS_PER_BLOCK;
	pub const ReportLongevity: u64 =
		BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
}

impl pallet_babe::Config for Runtime {
	type EpochDuration = EpochDuration;
	type ExpectedBlockTime = ExpectedBlockTime;
	type EpochChangeTrigger = pallet_babe::ExternalTrigger;

	type KeyOwnerProofSystem = Historical;

	type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
		KeyTypeId,
		pallet_babe::AuthorityId,
	)>>::Proof;

	type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
		KeyTypeId,
		pallet_babe::AuthorityId,
	)>>::IdentificationTuple;

	type HandleEquivocation = pallet_babe::EquivocationHandler<Self::KeyOwnerIdentification, Offences, ReportLongevity>;

	type WeightInfo = ();

}

impl pallet_grandpa::Config for Runtime {
	type Event = Event;
	type Call = Call;

	type KeyOwnerProofSystem = Historical;

	type KeyOwnerProof =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

	type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
		KeyTypeId,
		GrandpaId,
	)>>::IdentificationTuple;

	type HandleEquivocation = pallet_grandpa::EquivocationHandler<Self::KeyOwnerIdentification, Offences, ReportLongevity>;

	type WeightInfo = ();
}

parameter_types! {
    pub OffencesWeightSoftLimit: Weight = Perbill::from_percent(60) * MaximumBlockWeight::get();
}

impl pallet_offences::Config for Runtime {
	type Event = Event;
	type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
	type OnOffenceHandler = Staking;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Babe;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = pallet_timestamp::weights::SubstrateWeight<Runtime>;
}

type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;

pub struct DealWithFees;
impl OnUnbalanced<NegativeImbalance> for DealWithFees {
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item=NegativeImbalance>) {
		if let Some(fees) = fees_then_tips.next() {
			// for fees, 80% to treasury, 20% to author
			let mut split = fees.ration(80, 20);
			if let Some(tips) = fees_then_tips.next() {
				// for tips, if any, 80% to treasury, 20% to author (though this can be anything)
				tips.ration_merge_into(80, 20, &mut split);
			}
			//todo 目前给国库的金额取消
			//Treasury::on_unbalanced(split.0);
			Author::on_unbalanced(split.1);
		}
	}
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 500;
	pub const MaxLocks: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = MaxLocks;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const TransactionByteFee: Balance = MILLICENTS / 100;
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(1, 100_000);
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
}

impl pallet_transaction_payment::Config for Runtime {
	type OnChargeTransaction = CurrencyAdapter<Balances, Benefits, DealWithFees>;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = OneTenthFee<Balance>;
	type FeeMultiplierUpdate =
	TargetedFeeAdjustment<Self, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier>;
}

impl pallet_sudo::Config for Runtime {
	type Event = Event;
	type Call = Call;
}


parameter_types! {
	pub const UncleGenerations: BlockNumber = 5;
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
		RuntimeBlockWeights::get().max_block;
	pub const MaxScheduledPerBlock: u32 = 50;
}

/// scheduler Runtime  config
impl pallet_scheduler::Config for Runtime {
	type Event = Event;
	type Origin = Origin;
	type PalletsOrigin = OriginCaller;
	type Call = Call;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Runtime>;
}

/// authorship Runtime config
impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = Staking;
}

parameter_types! {
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
}

///session Runtime config
impl pallet_session::Config for Runtime {
	type Event = Event;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_staking::StashOf<Self>;
	type ShouldEndSession = Babe;
	type NextSessionRotation = Babe;
	type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
	type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type Keys = opaque::SessionKeys;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
	type WeightInfo = pallet_session::weights::SubstrateWeight<Runtime>;
}

impl pallet_session::historical::Config for Runtime {
	type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
}

pallet_staking_reward_curve::build! {
	const REWARD_CURVE: PiecewiseLinear<'static> = curve!(
		min_inflation: 0_025_000,
		max_inflation: 0_100_000,
		ideal_stake: 0_500_000,
		falloff: 0_050_000,
		max_piece_count: 40,
		test_precision: 0_005_000,
	);
}


parameter_types! {
	/// We prioritize im-online heartbeats over election solution submission.
	pub const StakingUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 2;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime where
	Call: From<C>,
{
	type Extrinsic = UncheckedExtrinsic;
	type OverarchingCall = Call;
}

parameter_types! {
	pub const SessionsPerEra: sp_staking::SessionIndex = 6;
	pub const BondingDuration: pallet_staking::EraIndex = 24 * 28;
	pub const SlashDeferDuration: pallet_staking::EraIndex = 24 * 7; // 1/4 the bonding duration.
	pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
	pub const MaxNominatorRewardedPerValidator: u32 = 256;
	pub OffchainRepeat: BlockNumber = 5;
	// 60 eras means 15 days if era = 6 hours
    pub const MarketStakingPotDuration: u32 = 60;
}

/// staking Runtime config
impl pallet_staking::Config for Runtime {
	const MAX_NOMINATIONS: u32 = 30;
	//质押余额
	//调用Balances模块
	type Currency = Balances;
	//用于计算周期持续的时间，它保证启动的时候在on_finalize，在创世模块的时候不使用
	//调用Timestamp模块
	type UnixTime = Timestamp;
	//将余额转换为选举用的数字
	type CurrencyToVote = CurrencyToVoteHandler;
	//抵押者的总奖励=年通膨胀率*代币发行总量/每年周期数           //年通膨胀率=npos_token_staked / total_tokens
	// staker_payout = yearly_inflation(npos_token_staked / total_tokens) * total_tokens / era_per_year
	//RewardRemainder剩余的奖励 = 每年最大膨胀率*代币总数/每年周期数-给抵押者的总奖励
	//remaining_payout = max_yearly_inflation * total_tokens / era_per_year - staker_payout
	//如果最大奖励减去实际奖励还有剩余奖励，将剩余奖励收归国库，用于支持生态发展支出
	//Treasury模块：提供一个资金池，能够由抵押者们来管理，在这个国库系统中，能够从这个资金池中发起发费提案。
	type RewardRemainder = ();
	//类型时间
	type Event = Event;
	//将惩罚的钱收入国库
	type Slash = (); // send the slashed funds to the treasury.
	//奖励，奖励会在主函数进行
	type Reward = (); // rewards are minted from the void
	//每个周期Session数量
	type SessionsPerEra = SessionsPerEra;
	//必须存放的时间
	type BondingDuration = BondingDuration;
	//惩罚延迟的时间，必须小于BondingDuration，设置成0就会立刻惩罚，没有时间干预
	type SlashDeferDuration = SlashDeferDuration;
	/// A super-majority of the council can cancel the slash.
	//议会的绝大多数人认同可以取消延迟的惩罚
	type SlashCancelOrigin = frame_system::EnsureRoot<Self::AccountId>;
	//给session提供的接口
	type SessionInterface = Self;
	//准确估计下一个session的改变，或者做一个最好的猜测
	type NextNewSession = Session;
	//为每个验证者奖励的提名者的最大数目。
	//对于每个验证者，只有$ MaxNominatorrewardedPervalidator最大的Stakers可以申请他们的奖励。 这用于限制提名人支付的I / O成本。
	type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
	//权重信息
	type WeightInfo = pallet_staking::weights::SubstrateWeight<Runtime>;
	type SPowerRatio = ();
	type MarketStakingPotDuration = MarketStakingPotDuration;
	// type MarketStakingPot = ();
	type BenefitInterface = Benefits;
}



sp_npos_elections::generate_solution_type!(
	#[compact]
	pub struct NposCompactSolution16::<
		VoterIndex = u32,
		TargetIndex = u16,
		Accuracy = sp_runtime::PerU16,
	>(16)
);

pub const MAX_NOMINATIONS: u32 =
	<NposCompactSolution16 as sp_npos_elections::CompactSolution>::LIMIT as u32;


///authority discovery  Runtime config
impl pallet_authority_discovery::Config for Runtime {}


parameter_types! {
	pub const OrderWaitingTime: BlockNumber = 30 * MINUTES;
	pub const PerByteDayPrice: u64 = 10;
	// 文件基础费用
	pub const FileBaseInitFee: Balance = 0;
	// 文件个数费用
	pub const FilesCountInitPrice: Balance = 1 * MICRO;
	// 文件每天价格费用 每MB每天费用
	pub const FileSizeInitPrice: Balance = 10 * NANO;
	// 存储参考比率. reported_files_size / total_capacity
	pub const StorageReferenceRatio: (u128, u128) = (25, 100); // 25/100 = 25%
	// 价格上升比率
	pub StorageIncreaseRatio: Perbill = Perbill::from_rational(1u64, 100000);
	// 价格下浮比率
	pub StorageDecreaseRatio: Perbill = Perbill::from_rational(1u64, 100000);
}

/// storage order Runtime config
impl storage_order::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type OrderWaitingTime = OrderWaitingTime;
	type PerByteDayPrice = PerByteDayPrice;
	type BalanceToNumber = ConvertInto;
	type BlockNumberToNumber = ConvertInto;
	type PaymentInterface = Payment;

	type FileBaseInitFee = FileBaseInitFee;
	type FilesCountInitPrice = FilesCountInitPrice;
	type FileSizeInitPrice = FileSizeInitPrice;
	type StorageReferenceRatio = StorageReferenceRatio;
	type StorageIncreaseRatio = StorageIncreaseRatio;
	type StorageDecreaseRatio = StorageDecreaseRatio;
	type WorkerInterface = Worker;
}

parameter_types! {
	// 工作量上报间隔
	pub const ReportInterval: BlockNumber = 6 * HOURS;
	//定义文件副本收益限额 eg：前10可获得奖励
	pub const AverageIncomeLimit: u8 = 4;
}

/// worker Runtime config
impl worker::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type ReportInterval = ReportInterval;
	type BalanceToNumber = ConvertInto;
	type StorageOrderInterface = StorageOrder;
	type AverageIncomeLimit = AverageIncomeLimit;
	type Works = Staking;
	type BenefitInterface = Benefits;
}

parameter_types! {
	pub const NumberOfIncomeMiner: usize = 4;
	//文件质押比率 72%
	pub const StakingRatio: Perbill = Perbill::from_percent(72);
    //文件存储比率 18%
	pub const StorageRatio: Perbill = Perbill::from_percent(18);
}

/// Configure the payment in pallets/payment.
impl payment::Config for Runtime {
	type Event = Event;
	type NumberOfIncomeMiner = NumberOfIncomeMiner;
	type BalanceToNumber = ConvertInto;
	type NumberToBalance = ConvertInto;
	type Currency = Balances;
	type StorageOrderInterface = StorageOrder;
	type WorkerInterface = Worker;
	type StakingRatio = StakingRatio;
	type StorageRatio = StorageRatio;
	type BenefitInterface = Benefits;
}

parameter_types! {
    pub const BenefitReportWorkCost: Balance = 3 * DOLLARS;
    pub const BenefitsLimitRatio: Perbill = Perbill::from_percent(1);
    pub const BenefitMarketCostRatio: Perbill = Perbill::one();
}

///  benefits Runtime config
impl benefits::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type BenefitReportWorkCost = BenefitReportWorkCost;
	type BenefitsLimitRatio = BenefitsLimitRatio;
	type BenefitMarketCostRatio = BenefitMarketCostRatio;
	type BondingDuration = BondingDuration;
	type WeightInfo = benefits::weight::WeightInfo<Runtime>;
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		//基本模块
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Utility: pallet_utility::{Pallet, Call, Event},

		//session必备前置模块
		//BABE 模块通过从 VRF 算法的输出中收集链上随机因子，和有效管理区块的周期更替来实现 BABE 共识机制的部份功能。
		Babe: pallet_babe::{Pallet, Call, Storage, Config, ValidateUnsigned},
		//Timestamp模块提供了获取和设置链上时间的功能。
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		//Balances 模块提供了帐户和余额的管理功能。
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		//TransactionPayment：提供计算预分派事务费用的基本逻辑。
		TransactionPayment: pallet_transaction_payment::{Pallet, Storage},

		//共识模块
		//Authorship 模块用于追踪当前区块的创建者，以及邻近的 “叔块”。
		Authorship: pallet_authorship::{Pallet, Call, Storage, Inherent},
		//质押模块
		Staking: pallet_staking::{Pallet, Call, Config<T>, Storage, Event<T>},
		//Session 模块允许验证人管理其会话密钥，提供了更改会话长度的及处理会话轮换的功能。
		Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>},
		//历史模块
		Historical: pallet_session_historical::{Pallet},
		//GRANDPA模块通过维护一个服务于native代码的GRANDPA权威集，以拓展GRANDPA的共识系统。
		Grandpa: pallet_grandpa::{Pallet, Call, Storage, Config, Event},
		//Substrate 的 core/authority-discovery 库使用了 Authority Discovery 模块来获取当前验证者集，获取本节点验证者ID，以及签署和验证与本节点与其他权威节点之间交换的消息。
		AuthorityDiscovery: pallet_authority_discovery::{Pallet, Config},
		//Offences惩罚模块
        Offences: pallet_offences::{Pallet, Storage, Event},

		//其他模块
		//Sudo pallet用来授予某个账户 (称为 "sudo key") 权限去执行需要Root权限的交易函数，或者是指定一个新账户来替代掉原来的sudo key。
		Sudo: pallet_sudo::{Pallet, Call, Config<T>, Storage, Event<T>},
		RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Pallet, Storage},
		//Scheduler:利用此模块可实现在指定区块号或指定周期的计划函数调用。 这些提前设定的可调用函数，可以包含调用者名称或匿名，也可跟着被取消。
		Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>},

		//ttc-pallet
		// Include the custom logic from the pallet-template in the runtime.
		StorageOrder: storage_order::{Pallet, Call, Storage, Event<T>},
		Worker: worker::{Pallet, Call, Storage, Event<T>},
		Payment: payment::{Pallet, Call, Storage, Event<T>},
		Benefits: benefits::{Pallet, Call, Storage, Event<T>},
	}
);

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPallets,
>;

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block);
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			Runtime::metadata().into()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
		fn authorities() -> Vec<AuthorityDiscoveryId> {
			AuthorityDiscovery::authorities()
		}
	}
	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			opaque::SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> GrandpaAuthorityList {
			Grandpa::grandpa_authorities()
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			_equivocation_proof: fg_primitives::EquivocationProof<
				<Block as BlockT>::Hash,
				NumberFor<Block>,
			>,
			_key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			None
		}

		fn generate_key_ownership_proof(
			_set_id: fg_primitives::SetId,
			_authority_id: GrandpaId,
		) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
			// NOTE: this is the only implementation possible since we've
			// defined our key owner proof type as a bottom type (i.e. a type
			// with no values).
			None
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
	}

	impl storage_order_runtime_api::StorageOrderApi<Block, AccountId, BlockNumber> for Runtime {
		fn page_user_order(account_id: AccountId, current: u64, size: u64, sort: u8) -> OrderPage<AccountId, BlockNumber> {
			StorageOrder::page_user_order(account_id, current, size, sort)
		}
	}

	impl worker_runtime_api::WorkerApi<Block, AccountId, BlockNumber> for Runtime {
		fn page_miner_order(account_id: AccountId, current: u64, size: u64, sort: u8) -> MinerOrderPage<AccountId, BlockNumber> {
			Worker::page_miner_order(account_id, current, size, sort)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark, TrackedStorageKey};

			use frame_system_benchmarking::Pallet as SystemBench;
			impl frame_system_benchmarking::Config for Runtime {}

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);

			add_benchmark!(params, batches, frame_system, SystemBench::<Runtime>);
			add_benchmark!(params, batches, pallet_balances, Balances);
			add_benchmark!(params, batches, pallet_timestamp, Timestamp);
			add_benchmark!(params, batches, pallet_template, TemplateModule);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}

	impl sp_consensus_babe::BabeApi<Block> for Runtime {
		fn configuration() -> sp_consensus_babe::BabeGenesisConfiguration {
			// The choice of `c` parameter (where `1 - c` represents the
			// probability of a slot being empty), is done in accordance to the
			// slot duration and expected target block time, for safely
			// resisting network delays of maximum two seconds.
			// <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
			sp_consensus_babe::BabeGenesisConfiguration {
				slot_duration: Babe::slot_duration(),
				epoch_length: EpochDuration::get(),
				c: BABE_GENESIS_EPOCH_CONFIG.c,
				genesis_authorities: Babe::authorities(),
				randomness: Babe::randomness(),
				allowed_slots: BABE_GENESIS_EPOCH_CONFIG.allowed_slots,
			}
		}

		fn current_epoch_start() -> sp_consensus_babe::Slot {
			Babe::current_epoch_start()
		}

		fn current_epoch() -> sp_consensus_babe::Epoch {
			Babe::current_epoch()
		}

		fn next_epoch() -> sp_consensus_babe::Epoch {
			Babe::next_epoch()
		}

		fn generate_key_ownership_proof(
			_slot: sp_consensus_babe::Slot,
			authority_id: sp_consensus_babe::AuthorityId,
		) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {
			use codec::Encode;
			Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
				.map(|p| p.encode())
				.map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
			key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			let key_owner_proof = key_owner_proof.decode()?;

			Babe::submit_unsigned_equivocation_report(
				equivocation_proof,
				key_owner_proof,
			)
		}

	}
}
