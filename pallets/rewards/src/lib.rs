// #![feature(mixed_integer_ops)]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode};
use core::default::Default;
use frame_support::{
	traits::{Currency, LockIdentifier, LockableCurrency},
	weights::Weight,
};
use frame_system::{ensure_root};
pub use pallet::*;
// use scale_info::TypeInfo;
use sp_consensus_pow::POW_ENGINE_ID;
use sp_runtime::traits::{Zero};
use sp_std::{
	collections::btree_map::BTreeMap, iter::FromIterator, ops::Bound::Included, prelude::*,
};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod default_weights;

pub trait WeightInfo {
	fn on_initialize() -> Weight;
	fn on_finalize() -> Weight;
	fn set_schedule() -> Weight;
}

// const REWARDS_ID: LockIdentifier = *b"rewards ";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	/// Config for rewards.
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// An implementation of on-chain currency.
		type Currency: LockableCurrency<Self::AccountId>;
		/// Weights for this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new schedule has been set.
		ScheduleSet,
		/// Reward has been sent.
		Rewarded(T::AccountId, BalanceOf<T>),
		/// Reward has been changed.
		RewardChanged(BalanceOf<T>)
	}

	/// Type alias for currency balance.
	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::pallet]
	// use 'without_storage_info' to resolve "MaxEncodedLen" issue.
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Reward set is too low.
		RewardTooLow,
		/// Reward curve is not sorted.
		NotSorted,
	}

	#[pallet::storage]
	#[pallet::getter(fn author)]
	pub type Author<T: Config> = StorageValue<_, T::AccountId>;

	#[pallet::storage]
	#[pallet::getter(fn reward)]
	pub type Reward<T: Config> = StorageValue<_, BalanceOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn reward_changes)]
	pub type RewardChanges<T: Config> = StorageValue<_, BTreeMap<T::BlockNumber, BalanceOf<T>>>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub rewards: BalanceOf<T>,
	}
	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				rewards: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			<Reward<T>>::put(self.rewards);
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(now: T::BlockNumber) {
			if let Some(author) = <Author<T>>::get() {
				let reward = Reward::<T>::get().unwrap_or_default();
				Self::do_reward(&author, reward, now);
			}

			<Author<T>>::kill();
		}

		/// Weight: see `begin_block`
		fn on_initialize(now: T::BlockNumber) -> Weight {
			let author = frame_system::Pallet::<T>::digest()
				.logs
				.iter()
				.filter_map(|s| s.as_pre_runtime())
				.filter_map(|(id, mut data)| {
					if id == POW_ENGINE_ID {
						T::AccountId::decode(&mut data).ok()
					} else {
						None
					}
				})
				.next();

			if let Some(author) = author {
				<Author<T>>::put(author);
			}

			RewardChanges::<T>::mutate(|reward_changes| {
				let mut removing = Vec::new();

				for (block_number, reward) in reward_changes
					.clone()
					.unwrap_or_default()
					.range((Included(Zero::zero()), Included(now)))
				{
					Reward::<T>::set(Some(*reward));
					removing.push(*block_number);

					Self::deposit_event(Event::<T>::RewardChanged(*reward));
				}

				for block_number in removing {
					reward_changes.clone().unwrap().remove(&block_number);
				}
			});

			T::WeightInfo::on_initialize().saturating_add(T::WeightInfo::on_finalize())
		}

		fn on_runtime_upgrade() -> frame_support::weights::Weight {
			0
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::set_schedule())]
		pub fn set_schedule(
			origin: OriginFor<T>,
			reward: BalanceOf<T>,
			reward_changes: Vec<(T::BlockNumber, BalanceOf<T>)>,
		) -> DispatchResult {
			ensure_root(origin)?;

			let reward_changes = BTreeMap::from_iter(reward_changes.into_iter());

			ensure!(reward >= T::Currency::minimum_balance(), Error::<T>::RewardTooLow);
			for (_, reward_change) in &reward_changes {
				ensure!(*reward_change >= T::Currency::minimum_balance(), Error::<T>::RewardTooLow);
			}

			Reward::<T>::put(reward);
			Self::deposit_event(Event::<T>::RewardChanged(reward));

			RewardChanges::<T>::put(reward_changes);
			Self::deposit_event(Event::<T>::ScheduleSet);
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn do_reward(author: &T::AccountId, reward: BalanceOf<T>, _when: T::BlockNumber) {
		let miner_total = reward;

		drop(T::Currency::deposit_creating(&author, miner_total));
	}
}
