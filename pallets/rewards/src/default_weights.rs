#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_balances.
pub trait WeightInfo {
	fn on_initialize() -> Weight;
	fn on_finalize() -> Weight;
	fn unlock() -> Weight;
	fn set_schedule() -> Weight;
	fn set_lock_params() -> Weight;
}

/// Weights for pallet_balances using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: System Account (r:1 w:1)
	fn on_initialize() -> Weight {
		(14_800_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	fn on_finalize() -> Weight {
		(121_500_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(5 as Weight))
			.saturating_add(T::DbWeight::get().writes(3 as Weight))
	}
	fn unlock() -> Weight {
		(46_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn set_schedule() -> Weight {
		(32_900_000 as Weight).saturating_add(T::DbWeight::get().writes(4 as Weight))
	}
	fn set_lock_params() -> Weight {
		(0 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: System Account (r:1 w:1)
	fn on_initialize() -> Weight {
		(14_800_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(2 as Weight))
			.saturating_add(RocksDbWeight::get().writes(2 as Weight))
	}
	fn on_finalize() -> Weight {
		(121_500_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(5 as Weight))
			.saturating_add(RocksDbWeight::get().writes(3 as Weight))
	}
	fn unlock() -> Weight {
		(46_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(1 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	fn set_schedule() -> Weight {
		(32_900_000 as Weight).saturating_add(RocksDbWeight::get().writes(4 as Weight))
	}
	fn set_lock_params() -> Weight {
		(0 as Weight).saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
}
