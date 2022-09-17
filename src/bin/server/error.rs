#![warn(clippy::pedantic)]

//! Common error handling.

use std::fmt::{self, Display};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::BucketInfo;

/// An error type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The [`LeakyBucket`] does not have enough free capacity for a specific request.
    ///
    /// [`LeakyBucket`]: `workshop_rustlab_2022::leaky_bucket::LeakyBucket`
    NotEnoughCapacity {
        /// The points of the request.
        request: u16,

        /// The current points in the leaky bucket.
        points: u16,

        /// The capacity of the leaky bucket.
        capacity: u16,

        /// The _leak_ of the bucket for each second.
        ///
        /// Every second the bucket is going to empty by this value.
        leak_per_second: u8,
    },
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotEnoughCapacity {
                request,
                points,
                capacity,
                leak_per_second,
            } => {
                let available = capacity - points;
                write!(
                    f,
                    "Not enough capacity. Requested {request} points, available \
                     {available}/{capacity} points (leak: {leak_per_second}/s)"
                )
            }
        }
    }
}

impl IntoResponse for Error {
    /// Convert `Error` into an axum [`Response`].
    ///
    /// [`Response`]: axum::response::Response
    fn into_response(self) -> Response {
        match self {
            error @ Error::NotEnoughCapacity {
                points,
                capacity,
                leak_per_second,
                ..
            } => {
                let bucket_info = BucketInfo {
                    points,
                    capacity,
                    leak_per_second,
                };
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    bucket_info,
                    error.to_string(),
                )
                    .into_response()
            }
        }
    }
}
