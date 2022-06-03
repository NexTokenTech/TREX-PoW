use super::*;
use crate as pallet_rewards;

use codec::Encode;
use frame_support::{
    parameter_types,
    traits::{Everything, OnInitialize},
};
use frame_support::pallet_prelude::GenesisBuild;
use frame_system::{self as system};
use sp_core::H256;
use sp_runtime::{
    testing::{DigestItem, Header},
    traits::{BlakeTwo256, IdentityLookup},
    Digest,
};
use sp_std::{cmp, collections::btree_map::BTreeMap};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime! {
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Rewards: pallet_rewards::{Pallet, Call, Storage, Config<T>, Event<T>},
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

type Balance = u128;
type BlockNumber = u64;

impl system::Config for Test {
    type BaseCallFilter = Everything;
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = BlockNumber;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type BlockWeights = ();
    type BlockLength = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    /// The set code logic, just the default since we're not a parachain.
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Test {
    type Balance = Balance;
    type DustRemoval = ();
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type WeightInfo = ();
}

const DOLLARS: Balance = 1;
const DAYS: BlockNumber = 1;

impl pallet_rewards::Config for Test {
    type Event = Event;
    type Currency = Balances;
    type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext(author: u64) -> sp_io::TestExternalities {
    let mut t = system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();
    pallet_rewards::GenesisConfig::<Test> {
        rewards: 60,
        mints: BTreeMap::new(),
        storage_value: Default::default()
    }
        .assimilate_storage(&mut t)
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        let current_block = 1;
        let parent_hash = System::parent_hash();
        let pre_digest = DigestItem::PreRuntime(sp_consensus_pow::POW_ENGINE_ID, author.encode());
        System::initialize(
            &current_block,
            &parent_hash,
            &Digest {
                logs: vec![pre_digest],
            }
        );
        System::set_block_number(current_block);

        Balances::on_initialize(current_block);
        Rewards::on_initialize(current_block);
    });
    ext
}