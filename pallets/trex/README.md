License: GPL-3.0-or-later
### Submit Extrinsic from Custom Pallets using subxt API
First of all, you need to know what is Extrinsic. So, a piece of data that is bundled into a block in order to express something from the "external" (i.e. off-chain) world is called an extrinsic. So, before building the subxt API let us see how to make a dispatch call in your own custom pallet.

### Create a Dispatch call in your pallet
A **dispatch call** is like a function that changes the state of the blockchain by changing the storage of your substrate chain. It fires an event to let all the other nodes know about the change in the blockchain state.

A **dispatch call** looks like this.

Before this you also be ready for function weight defines in **weights.rs**

``` rust
#[pallet::call]
impl<T: Config> Pallet<T>{
	/// An example dispatchable that takes a singles value as a parameter, writes the value to
	/// storage and emits an event. This function must be dispatched by a signed extrinsic.
	/// #[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
	#[pallet::weight(T::TREXWeight::send_trex_data())]
	pub fn send_trex_data(origin: OriginFor<T>, _from: T::AccountId, message: Vec<u8>, release_block_height: u32) -> DispatchResult {
		let who = ensure_signed(origin)?;
	
		//construct InfoData Struct for TREXStorage
		let owner = who.clone();
		let trex_message = message.clone();
		let trex_data = TREXData::<T>{
			release_block_height,
			message:trex_message,
			from:owner
		};
	
		//encode InfoData instance to vec<u8>
		let trex_byte_data = trex_data.encode();
		// Update storage.
		<TREXStorage<T>>::put(&trex_byte_data);
	
		// Emit an event.
		Self::deposit_event(Event::TREXDataSent(who, trex_byte_data));
		// Return a successful DispatchResultWithPostInfo
		Ok(())
	}
}
``` 

The **#[pallet::call]** macro tells that the following implementation contains dispatch calls. The function **send trex data** is a dispatch call to submit a Extrinsic to the blockchain.

The **#[pallet::weight(T::TREXWeight::send_trex_data())]** macro is used to identify the resources a call will be needing. These are called Transactional weights. **Weights** are the mechanism used to manage the time it takes to validate a block. Generally speaking, this comes from limiting the storage **I/O** and **computation**.

So, the sole purpose of this function is to make changes in the blockchain state and fire an event to let everyone know about the changes by submitting a transaction.

### Create a subxt API
We will be creating a rust app to submit this extrinsic. So, to create a rust app follow the following steps in the terminal. First, you need to get your system ready. So install some packages to begin.

#### 1.create an application app for rust.

#### 2.add dependences like below:
``` rust
[dependencies]
subxt = "0.18.1"
async-std = { version = "1.9.0", features = ["attributes", "tokio1"] }
sp-keyring = "6.0.0"
env_logger = "0.9.0"
futures = "0.3.13"
codec = { package = "parity-scale-codec", version = "3.0.0", default-features = false, features = ["derive", "full", "bit-vec"] }
hex = "0.4.3"
```

#### 3.Install subxt-cli:
``` rust
cargo install subxt-cli
``` 

#### 4.Save the encoded metadata to a file:
``` rust
subxt metadata -f bytes > metadata.scale
``` 

#### 5.Generating the runtime API from the downloaded metadata
Declare a module and decorate it with the subxt attribute which points at the downloaded metadata for the target runtime:
``` rust
#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod trex { }
```
Important: runtime_metadata_path resolves to a path relative to the directory where your crate's Cargo.toml resides (CARGO_MANIFEST_DIR), not relative to the source file.

#### 6.Initializing the API client
``` rust
use subxt::{ClientBuilder, DefaultConfig, DefaultExtra};

let api = ClientBuilder::new()
    .set_url("wss://rpc.polkadot.io:443")
    .build()
    .await?
    .to_runtime_api::<trex::RuntimeApi<DefaultConfig, DefaultExtra<DefaultConfig>>>();
``` 
	
The *RuntimeApi* type is generated by the *subxt* macro from the supplied metadata. This can be parameterized with user supplied implementations for the **Config** and **Extra** types, if the default implementations differ from the target chain.

#### 7.Submitting Extrinsics
Submit an extrinsic, returning success once the transaction is validated and accepted into the pool:
``` rust
use sp_keyring::AccountKeyring;
use subxt::PairSigner;

let signer = PairSigner::new(AccountKeyring::Alice.pair());
let acount_id = AccountKeyring::Alice.to_account_id().into();

let str = "second vec u8 message2".as_bytes();
let hash = api
        .tx()
        .trex_module()
        .send_trex_data(acount_id,str.to_vec(),1023)
        .sign_and_submit(&signer)
        .await?;
``` 

For more advanced usage, which can wait for block inclusion and return any events triggered by the extrinsic.
