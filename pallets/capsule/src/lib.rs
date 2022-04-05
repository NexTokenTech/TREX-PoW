#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;
pub mod weights;
pub use weights::CapsuleWeight;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_std::vec::Vec;
	use scale_info::prelude::string::String;
	use codec::alloc::string::ToString;
	// We use `alt_serde`, and Xanewok-modified `serde_json` so that we can compile the program
	// with serde(features `std`) and alt_serde(features `no_std`).
	use serde_json::Value;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Weight information for extrinsics in this pallet.
		type CapsuleWeight: CapsuleWeight;
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	// The pallet's runtime storage items.
	// https://docs.substrate.io/v3/runtime/storage
	// Learn more about declaring storage items:
	// https://docs.substrate.io/v3/runtime/storage#declaring-storage-items
	#[pallet::storage]
	#[pallet::getter(fn something)]
	pub type CapsuleStorage<T> = StorageValue<_, Vec<u8>>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Capsule Data Send Event
		CapsuleDataSent(T::AccountId, Vec<u8>),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		CapsuleInfoSentOverflow,
	}

	// Struct for holding Capsule information.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct CapsuleData<T: Config> {
		pub cipher_list: Vec<u8>,
		pub from: T::AccountId,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// An example dispatchable that takes a singles value as a parameter, writes the value to
		/// storage and emits an event. This function must be dispatched by a signed extrinsic.
		/// #[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[pallet::weight(
			T::CapsuleWeight::send_capsule_data(cipher_list.len() as u32
			))]
		pub fn send_capsule_data(
			origin: OriginFor<T>,
			_from: T::AccountId,
			cipher_list: Vec<u8>,
		) -> DispatchResultWithPostInfo
		{
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://docs.substrate.io/v3/runtime/origins
			let who = ensure_signed(origin)?;

			let current_block_number = <frame_system::Pallet<T>>::block_number();

			// get json string from matching chiper_text
			let cipher_json_str_result = String::from_utf8(cipher_list.clone());
			let cipher_json_str = match cipher_json_str_result {
				Ok(ciphers_json) => ciphers_json,
				Err(_) => "".to_string()
			};
			let serde_json_value:Option<Value> = serde_json::from_str(&cipher_json_str).unwrap_or(alt_serde::__private::Some(Value::Null));
			let cipher_array = match serde_json_value {
				Some(Value::Array(vector))=>{
					vector
				},
				Some(Value::Null)=>Vec::new(),
				_=>Vec::new()
			};
			let mut is_validate = true;
			for cipher in cipher_array{
				let release_height = match &cipher["release_block_num"] {
					Value::Number(value) => value.as_u64().unwrap(),
					_ => 0u64
				} as u32;
				let current_block:Result<u32,<<T as frame_system::Config>::BlockNumber as TryInto<u32>>::Error> = current_block_number.try_into();
				let current_block_index = match current_block {
					Ok(block_num) => block_num,
					Err(_) => 0u32
				};
				if release_height <= current_block_index {
					is_validate = false;
					break
				}
			}
			if is_validate == false {
				Ok((Some(0), Pays::No).into())
			}else{
				//construct InfoData Struct for CapsuleStorage
				let owner = who.clone();
				let ciphers = cipher_list.clone();
				let capsule_data = CapsuleData::<T> { cipher_list: ciphers, from: owner };

				//encode InfoData instance to vec<u8>
				let capsule_byte_data = capsule_data.encode();
				// Update storage.
				<CapsuleStorage<T>>::put(&capsule_byte_data);

				// Emit an event.
				Self::deposit_event(Event::CapsuleDataSent(who, capsule_byte_data));
				// Return a successful DispatchResultWithPostInfo
				Ok((Some(T::CapsuleWeight::send_capsule_data(cipher_list.len() as u32)), Pays::Yes).into())
			}
			// ensure!(is_validate == true, "fail");
		}
	}
}
