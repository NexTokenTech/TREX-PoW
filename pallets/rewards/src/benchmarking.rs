//! Benchmarking for Rewards pallet.

use super::*;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::traits::{OnFinalize, OnInitialize};
use frame_system::{DigestItemOf, EventRecord, RawOrigin};
use sp_runtime::traits::Bounded;

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
	let events = frame_system::Module::<T>::events();
	let system_event: <T as frame_system::Config>::Event = generic_event.into();
	// compare to the last event record
	let EventRecord { event, .. } = &events[events.len() - 1];
	assert_eq!(event, &system_event);
}

benchmarks! {
	// Worst case: Author info is in digest.
	on_initialize {
		let author: T::AccountId = account("author", 0, 0);
		let author_digest = DigestItemOf::<T>::PreRuntime(sp_consensus_pow::POW_ENGINE_ID, author.encode());
		frame_system::Module::<T>::deposit_log(author_digest);

		Reward::<T>::put(T::Currency::minimum_balance());

		// Whitelist transient storage items
		frame_benchmarking::benchmarking::add_to_whitelist(Author::<T>::hashed_key().to_vec().into());

		let block_number = frame_system::Module::<T>::block_number();
	}: { crate::Module::<T>::on_initialize(block_number); }
	verify {
		assert_eq!(Author::<T>::get(), Some(author));
	}

	// Worst case: This author already has `max_locks` locked up, produces a new block, and we unlock
	// everything in addition to creating brand new locks for the new reward.
	on_finalize {
		let author: T::AccountId = account("author", 0, 0);
		let reward = BalanceOf::<T>::max_value();

		// Setup pallet variables
		Author::<T>::put(&author);
		Reward::<T>::put(reward);

		// Whitelist transient storage items
		frame_benchmarking::benchmarking::add_to_whitelist(Author::<T>::hashed_key().to_vec().into());

		let block_number = frame_system::Module::<T>::block_number();
	}: { crate::Module::<T>::on_finalize(block_number); }
	verify {
		assert!(Author::<T>::get().is_none());
		assert!(RewardLocks::<T>::get(&author).iter().count() > 0);
	}

	set_schedule {

	}: _(RawOrigin::Root, T::Currency::minimum_balance(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new())

}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		new_test_ext(0).execute_with(|| {
			assert_ok!(test_benchmark_on_finalize::<Test>());
			assert_ok!(test_benchmark_on_initialize::<Test>());
			assert_ok!(test_benchmark_set_schedule::<Test>());
		});
	}
}

