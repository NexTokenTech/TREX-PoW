#![feature(associated_type_defaults)]
#![cfg_attr(not(feature = "std"), no_std)]

use cp_constants::{
	Difficulty, CLAMP_FACTOR, DIFFICULTY_ADJUST_WINDOW, MAX_DIFFICULTY, MIN_DIFFICULTY,
};
use fast_math::log2;
use frame_support::traits::OnTimestampSet;
use num_traits::float::FloatCore;
pub use pallet::*;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::cmp::{max, min};

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
	let log2_resource = (block_time_target / measured_block_time).pow(2);
	let log2_result = log2(log2_resource as f32);
	let round_result = log2_result.round();
	max(min(round_result as i128, CLAMP_FACTOR as i128), -(CLAMP_FACTOR as i128)) as i128
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

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
			GenesisConfig { initial_difficulty: MIN_DIFFICULTY as Difficulty }
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
	pub fn PastDifficultiesEmpty<T: Config>() -> Difficulties<T::Moment> {
		[None; DIFFICULTY_ADJUST_WINDOW]
	}

	#[pallet::storage]
	pub(super) type PastDifficultiesAndTimestamps<T: Config> = StorageValue<
		_,
		[Option<DifficultyAndTimestamp<T::Moment>>; DIFFICULTY_ADJUST_WINDOW],
		ValueQuery,
		PastDifficultiesEmpty<T>,
	>;

	#[pallet::storage]
	pub type CurrentHeight<T> = StorageValue<_, u32>;

	#[pallet::storage]
	#[pallet::getter(fn difficulty)]
	pub type CurrentDifficulty<T> = StorageValue<_, Difficulty>;

	#[pallet::storage]
	#[pallet::getter(fn initial_difficulty)]
	pub type InitialDifficulty<T> = StorageValue<_, Difficulty>;

	impl<T: Config> OnTimestampSet<T::Moment> for Pallet<T> {
		fn on_timestamp_set(moment: T::Moment) {
			const DIFFICULTY_DEFAULT: Difficulty = MIN_DIFFICULTY as Difficulty;
			// Get target time window size
			let block_time =
				UniqueSaturatedInto::<u128>::unique_saturated_into(T::TargetBlockTime::get());
			let block_time_window = block_time * DIFFICULTY_ADJUST_WINDOW as u128;

			// Get the window history data
			let mut data = PastDifficultiesAndTimestamps::<T>::get();

			// get the window current_height
			let mut current_height = CurrentHeight::<T>::get().unwrap_or(0u32);

			// panic if current height pointer is over the boundray.
			if current_height >= DIFFICULTY_ADJUST_WINDOW as u32{
				panic!("current height pointer out of bounds!");
			}
			// It's time to adjust the difficulty
			if current_height == (DIFFICULTY_ADJUST_WINDOW - 1) as u32 {
				// Set DIFFICULTY_ADJUST_WINDOW last element
				data[current_height as usize] = Some(DifficultyAndTimestamp {
					timestamp: moment,
					difficulty: Self::difficulty().unwrap_or(DIFFICULTY_DEFAULT),
				});

				// Calculates the actual time interval within DIFFICULTY_ADJUST_WINDOW,consider whether to add damped oscillation.
				let mut ts_delta = 0u128;
				for i in 1..(DIFFICULTY_ADJUST_WINDOW as usize) {
					let prev: Option<u128> =
						data[i - 1].map(|d| d.timestamp.unique_saturated_into());
					let cur: Option<u128> = data[i].map(|d| d.timestamp.unique_saturated_into());

					let delta = match (prev, cur) {
						(Some(prev), Some(cur)) => cur.saturating_sub(prev) / 1000,
						_ => block_time.into(),
					};
					ts_delta = ts_delta.saturating_add(delta);
				}

				if ts_delta == 0 {
					ts_delta = 1;
				}

				// Damping needs to be verified again, first without damping
				// let damp_value = damp(ts_delta, block_time_window, DIFFICULTY_DAMP_FACTOR);

				// adjust time delta toward goal subject to clamping
				let adj_ts = clamp(block_time_window, ts_delta);

				// Difficulty adjustment and storage
				let difficulty = Self::difficulty().unwrap_or(DIFFICULTY_DEFAULT) as i128 + adj_ts;
				let mut difficulty_final = MIN_DIFFICULTY;
				if difficulty < MIN_DIFFICULTY as i128{
					difficulty_final = MIN_DIFFICULTY;
				} else if difficulty > MAX_DIFFICULTY as i128 {
					difficulty_final = MAX_DIFFICULTY;
				} else {
					difficulty_final = difficulty as u128;
				}

				// current_height to zero
				current_height = 0;
				//storage
				<PastDifficultiesAndTimestamps<T>>::put(data);
				<CurrentDifficulty<T>>::put(difficulty_final);
				<CurrentHeight<T>>::put(current_height);
			} else {
				// If the window threshold is not reached, no difficulty adjustment is required
				data[current_height as usize] = Some(DifficultyAndTimestamp {
					timestamp: moment,
					difficulty: Self::difficulty().unwrap_or(DIFFICULTY_DEFAULT),
				});
				current_height = current_height.saturating_add(1);
				<PastDifficultiesAndTimestamps<T>>::put(data);
				<CurrentHeight<T>>::put(current_height);
			}
		}
	}
}
