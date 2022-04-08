#![feature(associated_type_defaults)]
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	traits::{OnTimestampSet},
};
use cp_constants::{
	Difficulty, DIFFICULTY_ADJUST_WINDOW,
	MIN_DIFFICULTY,DIFFICULTY_DAMP_FACTOR,CLAMP_FACTOR
};
pub use pallet::*;
use sp_std::cmp::{max, min};
use sp_runtime::traits::UniqueSaturatedInto;
use fast_math::log2;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

/// Move value linearly toward a goal
pub fn damp(actual: u128, goal: u128, damp_factor: u128) -> u128 {
	(actual + (damp_factor - 1) * goal) / damp_factor
}

/// limit value to be within some factor from a goal
pub fn clamp(block_time_target: u128, measured_block_time: u128) -> i128 {
	// TODO: round function
	let log2_result = log2((block_time_target / measured_block_time).pow(2) as f32);
	max(min(log2_result as i32, CLAMP_FACTOR as i32), -(CLAMP_FACTOR as i32)) as i128
}

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use super::*;

	#[derive(Encode, Decode, TypeInfo, RuntimeDebug, Clone, Copy, Eq, PartialEq)]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct DifficultyAndTimestamp<M> {
		pub difficulty: Difficulty,
		pub timestamp: M,
	}

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_timestamp::Config {
		/// Target block time in millseconds.
		#[pallet::constant]
		type TargetBlockTime: Get<Self::Moment>;
	}

	#[pallet::pallet]
	// use 'without_storage_info' to resolve "MaxEncodedLen" issue.
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::genesis_config]
	pub struct GenesisConfig {
		pub initial_difficulty: Difficulty,
	}

	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			GenesisConfig{
				initial_difficulty: MIN_DIFFICULTY as Difficulty
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			<CurrentDifficulty<T>>::put(self.initial_difficulty);
		}
	}

	type Difficulties<T> = [Option<DifficultyAndTimestamp<T>>; DIFFICULTY_ADJUST_WINDOW];

	#[pallet::type_value]
	pub fn PastDifficultiesEmpty<T: Config>() -> Difficulties<T::Moment> {[None; DIFFICULTY_ADJUST_WINDOW]}

	#[pallet::storage]
	pub(super) type PastDifficultiesAndTimestamps<T: Config> = StorageValue<_, [Option<DifficultyAndTimestamp<T::Moment>>; DIFFICULTY_ADJUST_WINDOW], ValueQuery, PastDifficultiesEmpty<T>>;

	#[pallet::storage]
	#[pallet::getter(fn difficulty)]
	pub type CurrentDifficulty<T> = StorageValue<_, Difficulty>;

	#[pallet::storage]
	#[pallet::getter(fn initial_difficulty)]
	pub type InitialDifficulty<T> = StorageValue<_, Difficulty>;

	impl<T: Config> OnTimestampSet<T::Moment> for Pallet<T>{
		fn on_timestamp_set(moment: T::Moment) {
			// todo!()
			let block_time =
				UniqueSaturatedInto::<u128>::unique_saturated_into(T::TargetBlockTime::get());
			let block_time_window = DIFFICULTY_ADJUST_WINDOW as u128 * block_time;

			let mut data = PastDifficultiesAndTimestamps::<T>::get();

			for i in 1..data.len() {
				data[i - 1] = data[i];
			}

			const DIFFICULTY_DEFAULT:Difficulty = MIN_DIFFICULTY as Difficulty;
			data[data.len() - 1] = Some(DifficultyAndTimestamp {
				timestamp: moment,
				difficulty: Self::difficulty().unwrap_or(DIFFICULTY_DEFAULT),
			});

			let mut ts_delta = 0;
			for i in 1..(DIFFICULTY_ADJUST_WINDOW as usize) {
				let prev: Option<u128> = data[i - 1].map(|d| d.timestamp.unique_saturated_into());
				let cur: Option<u128> = data[i].map(|d| d.timestamp.unique_saturated_into());

				let delta = match (prev, cur) {
					(Some(prev), Some(cur)) => cur.saturating_sub(prev),
					_ => block_time.into(),
				};
				ts_delta += delta;
			}

			if ts_delta == 0 {
				ts_delta = 1;
			}

			// adjust time delta toward goal subject to dampening and clamping
			let adj_ts = clamp(
				damp(ts_delta, block_time_window, DIFFICULTY_DAMP_FACTOR),
				block_time_window,
			);
			let difficulty = Self::difficulty().unwrap_or(DIFFICULTY_DEFAULT) as i128 + adj_ts;
			let difficulty_final = difficulty as Difficulty;

			<PastDifficultiesAndTimestamps<T>>::put(data);
			// <CurrentDifficulty<T>>::put(difficulty_final);
			<CurrentDifficulty<T>>::put(difficulty_final);
		}
	}
}

