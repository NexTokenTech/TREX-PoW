use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config>  pallet_rewards::WeightInfo for WeightInfo<T> {
    fn on_initialize() -> Weight {
        (14_700_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn on_finalize() -> Weight {
        (121_300_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(5 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn unlock() -> Weight {
        (45_200_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn set_schedule() -> Weight {
        (32_500_000 as Weight).saturating_add(T::DbWeight::get().writes(4 as Weight))
    }
    fn set_lock_params() -> Weight {
        (0 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
}