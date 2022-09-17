#![warn(clippy::pedantic)]

//! This code is made for educational purposes for the [Rustlab] 2022 edition.
//!
//! The aim of this crate is to provide both some basic functionalities and the _solution_ to the
//! exercise, in order to prevent developers to write boilerplate code and, if needed, to check a
//! possible implementation.
//!
//! [Rustlab]: https://rustlab.it/

pub mod database;
pub mod leaky_bucket;

pub use leaky_bucket::LeakyBucket;

/// The HTTP header which represents leaky bucket points.
pub const BUCKET_POINTS_HEADER: &str = "x-bucket-points";

/// The HTTP header which represents leaky bucket capacity.
pub const BUCKET_CAPACITY_HEADER: &str = "x-bucket-capacity";

/// The HTTP header which represents leaky bucket leak-per-second.
pub const BUCKET_LEAK_PER_SECOND_HEADER: &str = "x-bucket-leak-per-second";
