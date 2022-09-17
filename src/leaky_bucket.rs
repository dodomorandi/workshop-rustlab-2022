#![warn(clippy::pedantic)]

//! A simple [leaky bucket] implementation.
//!
//! [leaky bucket]: https://en.wikipedia.org/wiki/Leaky_bucket

use std::{
    cell::Cell,
    fmt::{self, Display},
    time::Duration,
};

use tokio::time::Instant;

use crate::{BUCKET_CAPACITY_HEADER, BUCKET_LEAK_PER_SECOND_HEADER, BUCKET_POINTS_HEADER};

/// A simple [leaky bucket] implementation.
///
/// [leaky bucket]: https://en.wikipedia.org/wiki/Leaky_bucket
#[derive(Debug)]
pub struct LeakyBucket {
    capacity: u16,
    leak_per_second: u8,
    last_points: Cell<u16>,
    last_time: Cell<LastTime>,
}

/// Last time a request has been performed.
///
/// This keeps the remainder in nanoseconds in order to do not lose the time passed since the
/// beginning of the _current second_ across different requests.
#[derive(Clone, Copy, Debug)]
struct LastTime {
    instant: Instant,
    remainder_nanos: u32,
}

impl LeakyBucket {
    /// Creates an empty leaky bucket.
    #[inline]
    #[must_use]
    pub fn empty(max_capacity: u16, leak_per_second: u8) -> Self {
        Self::with_points(0, max_capacity, leak_per_second)
    }

    /// Creates a leaky bucket given the number of stored _points_.
    ///
    /// # Panics
    ///
    /// Panics if `points` exceed `capacity`.
    #[must_use]
    pub fn with_points(points: u16, capacity: u16, leak_per_second: u8) -> Self {
        assert!(points <= capacity, "Points cannot exceed capacity");

        Self {
            capacity,
            leak_per_second,
            last_points: Cell::new(points),
            last_time: Cell::new(LastTime {
                instant: Instant::now(),
                remainder_nanos: 0,
            }),
        }
    }

    /// Returns the capacity of the bucket.
    pub const fn capacity(&self) -> u16 {
        self.capacity
    }

    /// Returns the leak per second of the bucket.
    pub const fn leak_per_second(&self) -> u8 {
        self.leak_per_second
    }

    /// Returns the current points stored in the bucket.
    ///
    /// The internal state is updated depending on the last time the leaky bucket is _actively_
    /// used.
    pub fn points(&self) -> u16 {
        let now = Instant::now();

        let LastTime {
            instant,
            remainder_nanos,
        } = self.last_time.get();
        let delta = now - instant + Duration::from_nanos(remainder_nanos.into());
        let delta_secs = u16::try_from(delta.as_secs()).unwrap_or(u16::MAX);
        let leak = delta_secs.saturating_mul(u16::from(self.leak_per_second));

        let points = self.last_points.get().saturating_sub(leak);
        let remainder_nanos = delta.subsec_nanos();

        self.last_points.set(points);
        self.last_time.set(LastTime {
            instant: now,
            remainder_nanos,
        });
        points
    }

    /// Adds some points to the buckets.
    ///
    /// Returns the new amount of points in the bucket or an error if the capacity would be
    /// exceeded.
    ///
    /// # Errors
    ///
    /// If the capacity would be exceeded while adding the points, the actual points are left
    /// unchanged and an error is returned.
    pub fn add(&self, points: u16) -> Result<u16, MaxCapacityError> {
        let cur_points = self.points();
        let points = cur_points.saturating_add(points);
        if points <= self.capacity {
            self.last_points.set(points);
            Ok(points)
        } else {
            Err(MaxCapacityError(cur_points))
        }
    }

    /// Add some points to the bucket, saturating to its capacity.
    ///
    /// This will always succeeds, because not all points are necessarily added. This behavior makes
    /// the function useful to _artificially_ add points to the bucket, but it should not be used
    /// in order to run an operation that must fail if the bucket is too full.
    pub fn saturating_add(&self, points: u16) -> u16 {
        let points = self.points().saturating_add(points).min(self.capacity);
        self.last_points.set(points);
        points
    }

    /// Returns the number of available points.
    pub fn available(&self) -> u16 {
        self.capacity - self.points()
    }

    /// Calculates the waiting time in order to add some points to the bucket.
    ///
    /// This can be useful to evaluate when it is possible to perform a request to a service using
    /// the leaky bucket algorithm. In this way a failing request can be avoided using a likely
    /// estimation from local data.
    pub fn wait_time_to_use(&self, points: u16) -> Duration {
        let cur_points = self.points();
        let available = self.capacity - cur_points;
        match points.checked_sub(available) {
            None => Duration::ZERO,
            Some(to_restore) => {
                let seconds_needed = to_restore / u16::from(self.leak_per_second)
                    + u16::from(to_restore % u16::from(self.leak_per_second) != 0);

                Duration::from_secs(seconds_needed.into())
                    - Duration::from_nanos(self.last_time.get().remainder_nanos.into())
            }
        }
    }
}

/// An error representing an operation that would make the points exceed the capacity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MaxCapacityError(
    /// Points in the bucket.
    pub u16,
);

impl Display for MaxCapacityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Cannot add to leaky bucket with {self.0} points, capacity exceeded")
    }
}

impl TryFrom<&reqwest::header::HeaderMap> for LeakyBucket {
    type Error = FromHeaderError;

    fn try_from(headers: &reqwest::header::HeaderMap) -> Result<Self, Self::Error> {
        let points = headers
            .get(BUCKET_POINTS_HEADER)
            .ok_or(FromHeaderError::NoPoints)?;
        let capacity = headers
            .get(BUCKET_CAPACITY_HEADER)
            .ok_or(FromHeaderError::NoCapacity)?;
        let leak_per_second = headers
            .get(BUCKET_LEAK_PER_SECOND_HEADER)
            .ok_or(FromHeaderError::NoLeakPerSecond)?;

        let points = points
            .to_str()
            .ok()
            .and_then(|points| points.parse().ok())
            .ok_or(FromHeaderError::InvalidPoints)?;
        let capacity = capacity
            .to_str()
            .ok()
            .and_then(|capacity| capacity.parse().ok())
            .ok_or(FromHeaderError::InvalidCapacity)?;
        let leak_per_second = leak_per_second
            .to_str()
            .ok()
            .and_then(|leak_per_second| leak_per_second.parse().ok())
            .ok_or(FromHeaderError::InvalidLeakPerSecond)?;

        Ok(Self::with_points(points, capacity, leak_per_second))
    }
}

/// The possible errors when trying to convert a [`HeaderMap`] to a [`LeakyBucket`]
///
/// [`HeaderMap`]: `reqwest::header::HeaderMap`
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum FromHeaderError {
    /// Missing _points_ header
    NoPoints,

    /// Missing _capacity_ header
    NoCapacity,

    /// Missing _leak per second_ header
    NoLeakPerSecond,

    /// Invalid _points_ header
    InvalidPoints,

    /// Invalid _capacity_ header
    InvalidCapacity,

    /// Invalid _leak per second_ header
    InvalidLeakPerSecond,
}

impl fmt::Display for FromHeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            FromHeaderError::NoPoints => "no point header",
            FromHeaderError::NoCapacity => "no capacity header",
            FromHeaderError::NoLeakPerSecond => "no leak per second header",
            FromHeaderError::InvalidPoints => "invalid point header",
            FromHeaderError::InvalidCapacity => "invalid point capacity",
            FromHeaderError::InvalidLeakPerSecond => "invalid leak per second header",
        };

        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use tokio::time::sleep;

    use super::*;

    #[test]
    fn creation() {
        let bucket = LeakyBucket::with_points(5, 10, 2);
        assert_eq!(bucket.capacity, 10);
        assert_eq!(bucket.last_points, Cell::new(5));
        assert_eq!(bucket.leak_per_second, 2);
        assert_eq!(bucket.last_time.get().remainder_nanos, 0);

        let bucket = LeakyBucket::empty(10, 2);
        assert_eq!(bucket.capacity, 10);
        assert_eq!(bucket.last_points, Cell::new(0));
        assert_eq!(bucket.leak_per_second, 2);
        assert_eq!(bucket.last_time.get().remainder_nanos, 0);
    }

    #[tokio::test]
    async fn stable_empty() {
        let bucket = LeakyBucket::empty(5, 1);
        assert_eq!(bucket.last_points.get(), 0);
        assert_eq!(bucket.points(), 0);

        sleep(Duration::from_millis(1500)).await;
        assert_eq!(bucket.points(), 0);
    }

    #[tokio::test]
    async fn leaking() {
        let bucket = LeakyBucket::with_points(5, 5, 1);
        assert_eq!(bucket.points(), 5);

        sleep(Duration::from_millis(1500)).await;
        assert_eq!(bucket.points(), 4);

        sleep(Duration::from_millis(500)).await;
        assert_eq!(bucket.points(), 3);

        sleep(Duration::from_millis(2000)).await;
        assert_eq!(bucket.points(), 1);

        sleep(Duration::from_millis(2000)).await;
        assert_eq!(bucket.points(), 0);
    }

    #[tokio::test]
    async fn add_points() {
        let bucket = LeakyBucket::empty(10, 1);
        assert_eq!(bucket.add(7), Ok(7));
        assert_eq!(bucket.points(), 7);
        assert!(bucket.add(4).is_err());
        assert_eq!(bucket.points(), 7);

        sleep(Duration::from_secs(1)).await;
        assert_eq!(bucket.add(4), Ok(10));
        assert_eq!(bucket.points(), 10);
    }

    #[test]
    fn saturating_add_points() {
        let bucket = LeakyBucket::empty(10, 1);
        assert_eq!(bucket.saturating_add(7), 7);
        assert_eq!(bucket.points(), 7);
        assert_eq!(bucket.saturating_add(4), 10);
        assert_eq!(bucket.points(), 10);
        assert_eq!(bucket.saturating_add(4), 10);
        assert_eq!(bucket.points(), 10);
    }
}
