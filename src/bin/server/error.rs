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
    },
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotEnoughCapacity {
                request,
                points,
                capacity,
            } => {
                let available = capacity - points;
                write!(
                f,
                "Not enough capacity. Requested {request} points, available {available}/{capacity} points"
            )
            }
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            error @ Error::NotEnoughCapacity {
                points, capacity, ..
            } => {
                let bucket_info = BucketInfo { points, capacity };
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
