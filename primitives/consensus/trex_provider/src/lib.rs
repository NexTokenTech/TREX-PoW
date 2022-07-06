// Copyright (C) 2019-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Inherents for trex

use sp_inherents::{Error, InherentData, InherentIdentifier};

use sp_std::result::Result;

/// The trex inherent identifier.
pub const INHERENT_IDENTIFIER: InherentIdentifier = *b"trexdev0";

/// The type of the BABE inherent.
pub type InherentType = String;

/// Provides the custom value inherent data for Trex.
// TODO: Remove in the future. https://github.com/paritytech/substrate/issues/8029
#[cfg(feature = "std")]
pub struct InherentDataProvider {
	custom_value: InherentType,
}

#[cfg(feature = "std")]
impl InherentDataProvider {
	/// Create `Self` while using the default value to get the String.
	pub fn from_default_value() -> Self {
		Self { custom_value: "hello".to_string() }
	}
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl sp_inherents::InherentDataProvider for InherentDataProvider {
	fn provide_inherent_data(&self, inherent_data: &mut InherentData) -> Result<(), Error> {
		inherent_data.put_data(INHERENT_IDENTIFIER, &self.custom_value)
	}

	async fn try_handle_error(
		&self,
		_: &InherentIdentifier,
		_: &[u8],
	) -> Option<Result<(), Error>> {
		// There is no error anymore
		None
	}
}
