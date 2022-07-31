#![warn(clippy::pedantic)]

use std::{cell::Cell, time::Duration};

use tokio::time::Instant;

/// A simple [leaky bucket] implementation.
///
/// [leaky bucket]: https://en.wikipedia.org/wiki/Leaky_bucket
#[derive(Debug)]
pub struct LeakyBucket {
    max_capacity: u16,
    leak_per_second: u8,
    last_capacity: Cell<u16>,
    last_time: Cell<LastTime>,
}

#[derive(Clone, Copy, Debug)]
struct LastTime {
    instant: Instant,
    remainder_nanos: u32,
}

impl LeakyBucket {
    /// Creates an empty leaky bucket.
    ///
    /// Remember that an _empty_ bucket has its _capacity_ equal to the
    /// [`max_capacity`].
    ///
    /// [`max_capacity`]: Self::max_capacity.
    #[inline]
    #[must_use]
    pub fn empty(max_capacity: u16, leak_per_second: u8) -> Self {
        Self::with_capacity(max_capacity, max_capacity, leak_per_second)
    }

    /// Creates a leaky bucket given a capacity.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` exceed `max_capacity`.
    #[must_use]
    pub fn with_capacity(capacity: u16, max_capacity: u16, leak_per_second: u8) -> Self {
        assert!(
            capacity <= max_capacity,
            "Capacity cannot exceed max_capacity"
        );

        Self {
            max_capacity,
            leak_per_second,
            last_capacity: Cell::new(capacity),
            last_time: Cell::new(LastTime {
                instant: Instant::now(),
                remainder_nanos: 0,
            }),
        }
    }

    /// Returns the maximum capacity of the bucket.
    pub const fn max_capacity(&self) -> u16 {
        self.max_capacity
    }

    /// Returns the current capacity of the bucket.
    ///
    /// The internal state is updated depending on the last time the leaky bucket is _actively_
    /// used.
    pub fn capacity(&self) -> u16 {
        let now = Instant::now();

        let LastTime {
            instant,
            remainder_nanos,
        } = self.last_time.get();
        let delta = now - instant + Duration::from_nanos(remainder_nanos.into());
        let delta_secs = u16::try_from(delta.as_secs()).unwrap_or(u16::MAX);
        let leak = delta_secs.saturating_mul(u16::from(self.leak_per_second));

        let capacity = self
            .last_capacity
            .get()
            .saturating_add(leak)
            .min(self.max_capacity);
        let remainder_nanos = delta.subsec_nanos();

        self.last_capacity.set(capacity);
        self.last_time.set(LastTime {
            instant: now,
            remainder_nanos,
        });
        capacity
    }
}

#[cfg(test)]
mod tests {
    use tokio::time::sleep;

    use super::*;

    #[test]
    fn creation() {
        let bucket = LeakyBucket::with_capacity(5, 10, 2);
        assert_eq!(bucket.max_capacity, 10);
        assert_eq!(bucket.last_capacity, Cell::new(5));
        assert_eq!(bucket.leak_per_second, 2);
        assert_eq!(bucket.last_time.get().remainder_nanos, 0);

        let bucket = LeakyBucket::empty(10, 2);
        assert_eq!(bucket.max_capacity, 10);
        assert_eq!(bucket.last_capacity, Cell::new(10));
        assert_eq!(bucket.leak_per_second, 2);
        assert_eq!(bucket.last_time.get().remainder_nanos, 0);
    }

    #[tokio::test]
    async fn stable_empty() {
        let bucket = LeakyBucket::with_capacity(5, 5, 1);
        assert_eq!(bucket.last_capacity.get(), 5);
        assert_eq!(bucket.capacity(), 5);

        sleep(Duration::from_millis(1500)).await;
        assert_eq!(bucket.capacity(), 5);
    }

    #[tokio::test]
    async fn leaking() {
        let bucket = LeakyBucket::with_capacity(0, 5, 1);
        assert_eq!(bucket.capacity(), 0);

        sleep(Duration::from_millis(1500)).await;
        assert_eq!(bucket.capacity(), 1);

        sleep(Duration::from_millis(500)).await;
        assert_eq!(bucket.capacity(), 2);

        sleep(Duration::from_millis(2000)).await;
        assert_eq!(bucket.capacity(), 4);

        sleep(Duration::from_millis(2000)).await;
        assert_eq!(bucket.capacity(), 5);
    }
}
