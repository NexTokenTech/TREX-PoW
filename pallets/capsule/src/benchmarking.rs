//! Benchmarking setup for pallet-difficulty

use super::*;

#[allow(unused)]
use crate::Pallet as Template;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::RawOrigin;

benchmarks! {
	send_predcit {
		let s in vec![vec![1,2,3,4,5],vec![3,4,5,6,7]];
		let caller: T::AccountId = whitelisted_caller();
	}: _(RawOrigin::Signed(caller), s)
	verify {
		assert_eq!(PredictStorage::<T>::get(), Some(s));
	}

	impl_benchmark_test_suite!(Template, crate::mock::new_test_ext(), crate::mock::Test);
}
