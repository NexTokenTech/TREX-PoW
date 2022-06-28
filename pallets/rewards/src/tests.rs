// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Kulupu.
//
// Copyright (c) 2020 Wei Tang.
// Copyright (c) 2020 Shawn Tabrizi.
//
// Kulupu is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Kulupu is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Kulupu. If not, see <http://www.gnu.org/licenses/>.

//! Tests for Rewards Pallet

use crate::{mock::*, *};
use frame_support::{
	assert_noop, assert_ok,
	error::BadOrigin,
	traits::{OnFinalize, OnInitialize},
};
use frame_system::Event;
use pallet_balances::Error as BalancesError;
use sp_runtime::{testing::DigestItem, Digest};

// Get the last event from System
fn last_event() -> mock::Event {
	System::events().pop().expect("Event expected").event
}

/// Run until a particular block.
fn run_to_block(n: u64, author: u64) {
	while System::block_number() < n {
		Rewards::on_finalize(System::block_number());
		Balances::on_finalize(System::block_number());

		let current_block = System::block_number() + 1;
		let parent_hash = System::parent_hash();
		let pre_digest = DigestItem::PreRuntime(sp_consensus_pow::POW_ENGINE_ID, author.encode());
		System::initialize(&current_block, &parent_hash, &Digest { logs: vec![pre_digest] });
		System::set_block_number(current_block);

		Balances::on_initialize(System::block_number());
		Rewards::on_initialize(System::block_number());
	}
}

#[test]
fn genesis_config_works() {
	new_test_ext(1).execute_with(|| {
		assert_eq!(Author::<Test>::get(), Some(1));
		assert_eq!(Reward::<Test>::get(), 60);
		assert_eq!(Balances::free_balance(1), 0);
		assert_eq!(Balances::free_balance(2), 0);
		assert_eq!(System::block_number(), 1);
	});
}

#[test]
fn set_reward_works() {
	new_test_ext(1).execute_with(|| {
		// Fails with bad origin
		assert_noop!(Rewards::set_schedule(Origin::signed(1), 42, Default::default()), BadOrigin);
		// Successful
		assert_ok!(Rewards::set_schedule(Origin::root(), 42, Default::default()));
		assert_eq!(Reward::<Test>::get(), 42);
		assert_eq!(last_event(), Event::ScheduleSet.into());
		// Fails when too low
		assert_noop!(
			Rewards::set_schedule(Origin::root(), 0, Default::default()),
			Error::<Test>::RewardTooLow
		);
	});
}

#[test]
fn set_author_works() {
	new_test_ext(1).execute_with(|| {
		assert_eq!(Author::<Test>::get(), Some(1));
	});
}

#[test]
fn reward_payment_works() {
	new_test_ext(1).execute_with(|| {
		// Next block
		run_to_block(2, 2);
		// User gets reward
		assert_eq!(Balances::free_balance(1), 60);

		// Set new reward
		assert_ok!(Rewards::set_schedule(Origin::root(), 42, Default::default()));
		run_to_block(3, 1);
		assert_eq!(Balances::free_balance(2), 42);
	});
}

fn test_curve() -> Vec<(u64, u128)> {
	vec![(50, 20), (40, 25), (20, 50), (10, 100)]
}

#[test]
fn curve_works() {
	new_test_ext(1).execute_with(|| {
		// Set reward curve
		assert_ok!(Rewards::set_schedule(Origin::root(), 60, test_curve()));
		assert_eq!(last_event(), mock::Event::Rewards(crate::Event::<Test>::ScheduleSet));
		// Check current reward
		assert_eq!(Rewards::reward(), 60);
		run_to_block(9, 1);
		assert_eq!(Rewards::reward(), 60);
		run_to_block(10, 1);
		// Update successful
		assert_eq!(Rewards::reward(), 100);
		// Success reported
		assert_eq!(last_event(), mock::Event::Rewards(crate::Event::<Test>::RewardChanged(100)));
		run_to_block(20, 1);
		assert_eq!(Rewards::reward(), 50);
		// No change, not part of the curve
		run_to_block(30, 1);
		assert_eq!(Rewards::reward(), 50);
		run_to_block(40, 1);
		assert_eq!(Rewards::reward(), 25);
		run_to_block(50, 1);
		assert_eq!(Rewards::reward(), 20);
		// Curve is finished and is empty
		assert_eq!(RewardChanges::<Test>::get(), Default::default());
		// Everything works fine past the curve definition
		run_to_block(100, 1);
		assert_eq!(Rewards::reward(), 20);
	});
}
