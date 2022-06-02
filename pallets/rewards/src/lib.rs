#![feature(mixed_integer_ops)]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use core::default::Default;
use frame_support::{
	traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
	weights::Weight,
};
use frame_system::{ensure_root, ensure_signed};
pub use pallet::*;
use scale_info::TypeInfo;
use sp_consensus_pow::POW_ENGINE_ID;
use sp_runtime::traits::{Saturating, Zero};
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

pub struct LockBounds {
	pub period_max: u16,
	pub period_min: u16,
	pub divide_max: u16,
	pub divide_min: u16,
}

#[derive(Encode, Decode, TypeInfo, Clone, Copy, PartialEq, Eq, Debug)]
pub struct LockParameters {
	pub period: u16,
	pub divide: u16,
}

pub trait WeightInfo {
	fn on_initialize() -> Weight;
	fn on_finalize() -> Weight;
	fn unlock() -> Weight;
	fn set_schedule() -> Weight;
	fn set_lock_params() -> Weight;
}

const REWARDS_ID: LockIdentifier = *b"rewards ";

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
		/// Generate reward locks.
		type GenerateRewardLocks: GenerateRewardLocks<Self>;
		/// Weights for this pallet.
		type WeightInfo: WeightInfo;
		/// Lock Parameters Bounds.
		type LockParametersBounds: Get<LockBounds>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new schedule has been set.
		ScheduleSet,
		/// Reward has been sent.
		Rewarded(T::AccountId, BalanceOf<T>),
		/// Reward has been changed.
		RewardChanged(BalanceOf<T>),
		/// Lock Parameters have been changed.
		LockParamsChanged(LockParameters),
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
		/// Lock parameters are out of bounds.
		LockParamsOutOfBounds,
		/// Lock period is not a mutiple of the divide.
		LockPeriodNotDivisible,
	}

	#[pallet::storage]
	#[pallet::getter(fn author)]
	pub type Author<T: Config> = StorageValue<_, T::AccountId>;

	#[pallet::storage]
	#[pallet::getter(fn reward)]
	pub type Reward<T: Config> = StorageValue<_, BalanceOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn reward_locks)]
	pub type RewardLocks<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BTreeMap<T::BlockNumber, BalanceOf<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn reward_changes)]
	pub type RewardChanges<T: Config> = StorageValue<_, BTreeMap<T::BlockNumber, BalanceOf<T>>>;

	#[pallet::storage]
	#[pallet::getter(fn lock_params)]
	pub type LockParams<T: Config> = StorageValue<_, LockParameters>;

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

	/// Trait for generating reward locks.
	pub trait GenerateRewardLocks<T: Config> {
		/// Generate reward locks.
		fn generate_reward_locks(
			current_block: T::BlockNumber,
			total_reward: BalanceOf<T>,
			lock_parameters: Option<LockParameters>,
		) -> BTreeMap<T::BlockNumber, BalanceOf<T>>;

		fn max_locks(lock_bounds: LockBounds) -> u32;
	}

	// Default implemention for generating reward locks trait
	impl<T: Config> GenerateRewardLocks<T> for () {
		fn generate_reward_locks(
			_current_block: T::BlockNumber,
			_total_reward: BalanceOf<T>,
			_lock_parameters: Option<LockParameters>,
		) -> BTreeMap<T::BlockNumber, BalanceOf<T>> {
			Default::default()
		}

		fn max_locks(_lock_bounds: LockBounds) -> u32 {
			0
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

		#[pallet::weight(T::WeightInfo::set_lock_params())]
		pub fn set_lock_params(
			origin: OriginFor<T>,
			lock_params: LockParameters,
		) -> DispatchResult {
			ensure_root(origin)?;

			let bounds = T::LockParametersBounds::get();
			ensure!(
				(bounds.period_min..=bounds.period_max).contains(&lock_params.period)
					&& (bounds.divide_min..=bounds.divide_max).contains(&lock_params.divide),
				Error::<T>::LockParamsOutOfBounds
			);
			ensure!(
				lock_params.period % lock_params.divide == 0,
				Error::<T>::LockPeriodNotDivisible
			);

			<LockParams<T>>::put(lock_params);
			Self::deposit_event(Event::<T>::LockParamsChanged(lock_params));
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}

		/// Unlock any vested rewards for `target` account.
		#[pallet::weight(T::WeightInfo::unlock())]
		pub fn unlock(origin: OriginFor<T>, target: T::AccountId) -> DispatchResult {
			ensure_signed(origin)?;

			let locks = Self::reward_locks(&target).unwrap();
			let current_number = frame_system::Pallet::<T>::block_number();
			Self::update_reward_locks(&target, locks, current_number);
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn do_reward(author: &T::AccountId, reward: BalanceOf<T>, when: T::BlockNumber) {
		let miner_total = reward;

		let miner_reward_locks = T::GenerateRewardLocks::generate_reward_locks(
			when,
			miner_total,
			<LockParams<T>>::get(),
		);

		drop(T::Currency::deposit_creating(&author, miner_total));

		if miner_reward_locks.len() > 0 {
			let mut locks = Self::reward_locks(&author).unwrap_or_default();

			for (new_lock_number, new_lock_balance) in miner_reward_locks {
				let old_balance =
					*locks.get(&new_lock_number).unwrap_or(&BalanceOf::<T>::default());
				let new_balance = old_balance.saturating_add(new_lock_balance);
				locks.insert(new_lock_number, new_balance);
			}

			Self::update_reward_locks(&author, locks, when);
		}
	}

	pub fn update_reward_locks(
		author: &T::AccountId,
		mut locks: BTreeMap<T::BlockNumber, BalanceOf<T>>,
		current_number: T::BlockNumber,
	) {
		let mut expired = Vec::new();
		let mut total_locked: BalanceOf<T> = Zero::zero();

		for (block_number, locked_balance) in &locks {
			if block_number <= &current_number {
				expired.push(*block_number);
			} else {
				total_locked = total_locked.saturating_add(*locked_balance);
			}
		}

		for block_number in expired {
			locks.remove(&block_number);
		}

		T::Currency::set_lock(
			REWARDS_ID,
			&author,
			total_locked,
			WithdrawReasons::except(WithdrawReasons::TRANSACTION_PAYMENT),
		);

		<RewardLocks<T>>::insert(author, locks);
	}
}
