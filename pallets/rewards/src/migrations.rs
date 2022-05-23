// use crate::{BalanceOf, Config, Mints, RewardChanges};
use crate::pallet::{BalanceOf, Config, Mints, RewardChanges};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{Perbill, RuntimeDebug};
use sp_std::{collections::btree_map::BTreeMap, prelude::*};

/// A value placed in storage that represents the current version of the Scheduler storage.
/// This value is used by the `on_runtime_upgrade` logic to determine whether we run
/// storage migration logic.
#[derive(Encode, Decode, TypeInfo, Clone, Copy, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum StorageVersion {
	V0 = 0,
	V1 = 1,
}

impl Default for StorageVersion {
	fn default() -> Self {
		StorageVersion::V0
	}
}

impl StorageVersion {
	pub fn migrate<T: Config>(self) -> StorageVersion {
		match self {
			StorageVersion::V0 => migrate_v0_to_v1::<T>(),
			StorageVersion::V1 => (),
		}

		StorageVersion::V1
	}
}

struct __CurveV0;
impl frame_support::traits::StorageInstance for __CurveV0 {
	fn pallet_prefix() -> &'static str {
		"Rewards"
	}
	const STORAGE_PREFIX: &'static str = "Curve";
}

#[allow(type_alias_bounds)]
type CurveV0<T: Config> = frame_support::storage::types::StorageValue<
	__CurveV0,
	Vec<CurvePointV0<T::BlockNumber, BalanceOf<T>>>,
>;

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CurvePointV0<BlockNumber, Balance> {
	start: BlockNumber,
	reward: Balance,
	taxation: Perbill,
}

struct __AdditionalRewardsV0;
impl frame_support::traits::StorageInstance for __AdditionalRewardsV0 {
	fn pallet_prefix() -> &'static str {
		"Rewards"
	}
	const STORAGE_PREFIX: &'static str = "AdditionalRewards";
}

#[allow(type_alias_bounds)]
type AdditionalRewardsV0<T: Config> = frame_support::storage::types::StorageValue<
	__AdditionalRewardsV0,
	Vec<(T::AccountId, BalanceOf<T>)>,
>;

fn migrate_v0_to_v1<T: Config>() {
	let curve = CurveV0::<T>::take().unwrap_or_default();
	let additional_rewards = AdditionalRewardsV0::<T>::take().unwrap_or_default();

	let mut reward_changes = BTreeMap::new();
	for point in curve {
		reward_changes.insert(point.start, point.reward);
	}
	RewardChanges::<T>::put(reward_changes);

	let mut mints = BTreeMap::new();
	for (destination, additional_reward) in additional_rewards {
		mints.insert(destination, additional_reward);
	}
	Mints::<T>::put(mints);
}
