#![warn(missing_docs)]
// This line of compiling attributes is important to enable WASM support.
// This line means that if the crate does not have feature "std", then, do not use std lib.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
#[doc(hidden)]
pub use serde;

/// Consensus engine unique ID.
pub type ConsensusEngineId = [u8; 4];
pub mod generic;