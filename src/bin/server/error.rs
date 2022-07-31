use std::fmt::{self, Display};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    NotEnoughCapacity { request: u16, available: u16 },
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Error::*;

        match self {
            NotEnoughCapacity { request, available } => write!(
                f,
                "Not enough capacity. Requested {request} points, available {available} points"
            ),
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        use Error::*;

        let response = match self {
            err @ NotEnoughCapacity { .. } => (StatusCode::TOO_MANY_REQUESTS, err.to_string()),
        };

        response.into_response()
    }
}
