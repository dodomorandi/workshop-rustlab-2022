use std::{
    fmt,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Stream;
use reqwest::Client;
use workshop_rustlab_2022::database::{calc_query_cost, ServerField, ServerQuery};

pub(crate) struct MyStream<T> {
    query: ServerQuery,
    port: Option<u16>,
    client: Client,
    query_cost: u16,
    _marker: PhantomData<fn() -> T>,
}

#[derive(Debug)]
pub(crate) enum Error {
    Reqwest(reqwest::Error),
}

impl<T> Stream for MyStream<T> {
    type Item = Result<T, Error>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Option<Self::Item>> {
        todo!()
    }
}

impl<T> MyStream<T> {
    pub(crate) fn new(
        fields: std::collections::HashSet<ServerField>,
        port: Option<u16>,
        client: Client,
    ) -> Self {
        let query = ServerQuery {
            fields,
            ..Default::default()
        };

        let query_cost = calc_query_cost(&query);

        Self {
            query,
            port,
            client,
            query_cost,
            _marker: PhantomData,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reqwest(err) => write!(f, "Reqwest error: {err}"),
        }
    }
}

impl std::error::Error for Error {}
