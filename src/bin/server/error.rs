#![warn(clippy::pedantic)]

use std::fmt::{self, Display};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::BucketInfo;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    NotEnoughCapacity {
        request: u16,
        points: u16,
        capacity: u16,
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
