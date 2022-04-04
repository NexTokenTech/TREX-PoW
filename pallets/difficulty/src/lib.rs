#![feature(associated_type_defaults)]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	traits::{Get, OnTimestampSet},
};
use cp_constants::{
	Difficulty, CLAMP_FACTOR, DIFFICULTY_ADJUST_WINDOW, DIFFICULTY_DAMP_FACTOR, MAX_DIFFICULTY,
	MIN_DIFFICULTY,
};
use scale_info::TypeInfo;
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

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
			self.initial_difficulty;
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

	impl<T: Config> OnTimestampSet<T::Moment> for Pallet<T>{
		fn on_timestamp_set(moment: T::Moment) {
			todo!()
		}
	}
}

