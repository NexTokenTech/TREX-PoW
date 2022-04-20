//! Capsule primitive constants and types.

#![cfg_attr(not(feature = "std"), no_std)]
pub type Difficulty = u128;

/// Block interval, in seconds, the network will tune its next_target for.
pub const BLOCK_TIME_SEC: usize = 60;
/// Block time interval in milliseconds.
pub const BLOCK_TIME_MILLISEC: usize = BLOCK_TIME_SEC * 1000;
/// Slot duration inverval in milliseconds
pub const SLOT_DURATION:u64 = BLOCK_TIME_MILLISEC as u64;

/// Nominal height for standard time intervals, hour is 60 blocks
pub const HOUR_HEIGHT: usize = 3600 / BLOCK_TIME_SEC;
/// A day is 1440 blocks
pub const DAY_HEIGHT: usize = 24 * HOUR_HEIGHT;
/// A week is 10_080 blocks
pub const WEEK_HEIGHT: usize = 7 * DAY_HEIGHT;
/// A year is 524_160 blocks
pub const YEAR_HEIGHT: usize = 52 * WEEK_HEIGHT;

/// Number of blocks used to calculate difficulty adjustments
pub const DIFFICULTY_ADJUST_WINDOW: usize = HOUR_HEIGHT;
/// Clamp factor to use for difficulty adjustment
/// Limit value to within this factor of goal
pub const CLAMP_FACTOR: u128 = 1;
/// Dampening factor to use for difficulty adjustment
pub const DIFFICULTY_DAMP_FACTOR: u128 = 3;
/// Minimum difficulty, enforced in diff re-target
/// avoids getting stuck when trying to increase difficulty subject to dampening
pub const MIN_DIFFICULTY: u128 = 32;
/// Maximum difficulty.
pub const MAX_DIFFICULTY: u128 = 256_u128;//u128::MAX;

/// Value of 1 CAP.
pub const DOLLARS: u128 = 1_000_000_000_000;
/// Value of cents relative to CAP.
pub const CENTS: u128 = DOLLARS / 100;
/// Value of millicents relative to CAP.
pub const MILLICENTS: u128 = CENTS / 1_000;
/// Value of microcents relative to CAP.
pub const MICROCENTS: u128 = MILLICENTS / 1_000;

pub const fn deposit(items: u32, bytes: u32) -> u128 {
	items as u128 * 2 * DOLLARS + (bytes as u128) * 10 * MILLICENTS
}

/// Block number of one hour.
pub const HOURS: u32 = 60;
/// Block number of one day.
pub const DAYS: u32 = 24 * HOURS;