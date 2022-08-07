#![warn(clippy::pedantic)]

use std::{
    cell::Cell,
    fmt::{self, Display},
    time::Duration,
};

use tokio::time::Instant;

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
    /// This will always succeds, because not all points are necessarily added. This behavior makes
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
