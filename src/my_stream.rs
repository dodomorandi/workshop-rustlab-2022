use std::{
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{future::BoxFuture, ready, FutureExt, Stream};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use tokio::time::Instant;
use tracing::{debug, warn};
use workshop_rustlab_2022::{
    database::{
        calc_query_cost, ServerField, ServerQuery, DEFAULT_PAGE_SIZE, LEAK_PER_SECOND,
        MAX_BUCKET_CAPACITY,
    },
    LeakyBucket,
};

pub(crate) struct MyStream<T> {
    query: ServerQuery,
    port: Option<u16>,
    client: Client,
    query_cost: u16,
    bucket: Option<LeakyBucket>,
    last_call: Option<Instant>,
    inner: Inner,
    _marker: PhantomData<fn() -> T>,
}

type HeadRequestFuture = BoxFuture<'static, Result<reqwest::Response, reqwest::Error>>;
type BodyRequestFuture = BoxFuture<'static, Result<String, reqwest::Error>>;

enum Inner {
    Empty,
    HeadRequest(HeadRequestFuture),
    BodyRequest(BodyRequestFuture),
    Done,
}

#[derive(Debug)]
pub(crate) enum Error {
    Reqwest(reqwest::Error),
    SerdeJson(serde_json::Error),
}

impl<T> Stream for MyStream<T>
where
    T: for<'de> Deserialize<'de> + 'static,
{
    type Item = Result<T, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        match &mut this.inner {
            Inner::Empty => {
                let mut reqwest_fut = this.request_next_page();
                match reqwest_fut.as_mut().poll(cx) {
                    Poll::Ready(head_data) => this.handle_head_data_result(head_data, cx),
                    Poll::Pending => {
                        this.inner = Inner::HeadRequest(reqwest_fut);
                        Poll::Pending
                    }
                }
            }
            Inner::HeadRequest(fut) => {
                let head_data = ready!(fut.as_mut().poll(cx));
                this.handle_head_data_result(head_data, cx)
            }
            Inner::BodyRequest(fut) => {
                let content_data = ready!(fut.as_mut().poll(cx));
                this.handle_content_data_result(content_data)
            }
            Inner::Done => Poll::Ready(None),
        }
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
            bucket: None,
            last_call: None,
            inner: Inner::Empty,
            _marker: PhantomData,
        }
    }

    #[inline]
    fn request_next_page(&mut self) -> HeadRequestFuture {
        self.last_call = Some(Instant::now());
        let future = self
            .client
            .execute(self.query.create_request(self.port))
            .boxed();
        self.query.page = Some(self.query.page.map(|page| page + 1).unwrap_or(1));
        future
    }
}

impl<T> MyStream<T>
where
    T: for<'de> Deserialize<'de> + 'static,
{
    fn handle_head_data_result(
        &mut self,
        response: Result<reqwest::Response, reqwest::Error>,
        cx: &mut Context,
    ) -> Poll<Option<<Self as Stream>::Item>> {
        let response = match response {
            Ok(response) => response,
            Err(err) => {
                self.inner = Inner::Done;
                return Poll::Ready(Some(Err(err.into())));
            }
        };

        match LeakyBucket::try_from(response.headers()) {
            Ok(bucket) => {
                debug!(
                    "Got new bucket from server with {}/{} points",
                    bucket.points(),
                    bucket.capacity()
                );

                if let Some(cur_bucket) = self.bucket.as_ref() {
                    let cur_points = cur_bucket.points();
                    let new_points = bucket.points();
                    if cur_points != new_points {
                        warn!("Expected a leaky bucket with {cur_points} points, server has {new_points} points");
                    }
                }
                self.bucket = Some(bucket);
            }
            Err(err) => {
                warn!("Cannot get bucket data from response: {err}. Trying to set a reasonable status.");
                self.bucket
                    .get_or_insert_with(|| LeakyBucket::empty(MAX_BUCKET_CAPACITY, LEAK_PER_SECOND))
                    .saturating_add(self.query_cost);
            }
        }

        match response.error_for_status() {
            Ok(response) => {
                let mut fut = Box::pin(response.text());
                match fut.as_mut().poll(cx) {
                    Poll::Ready(content) => self.handle_content_data_result(content),
                    Poll::Pending => {
                        self.inner = Inner::BodyRequest(fut);
                        Poll::Pending
                    }
                }
            }
            Err(err) => match err.status() {
                Some(StatusCode::TOO_MANY_REQUESTS) => {
                    todo!("create a sleep future and poll it")
                }
                _ => {
                    self.inner = Inner::Done;
                    Poll::Ready(Some(Err(Error::Reqwest(err))))
                }
            },
        }
    }

    fn handle_content_data_result(
        &mut self,
        content: Result<String, reqwest::Error>,
    ) -> Poll<Option<<MyStream<T> as Stream>::Item>> {
        let content = match content {
            Ok(content) => content,
            Err(err) => {
                self.inner = Inner::Done;
                return Poll::Ready(Some(Err(err.into())));
            }
        };

        enum HasData {
            False,
            True { has_another_page: bool },
        }

        impl Default for HasData {
            fn default() -> Self {
                Self::True {
                    has_another_page: true,
                }
            }
        }

        let has_data = serde_json::from_str(&content)
            .map(|json| match json {
                Value::Object(obj) => {
                    obj.is_empty()
                        .then_some(HasData::False)
                        .unwrap_or_else(|| HasData::True {
                            has_another_page: obj.len()
                                == usize::from(self.query.page_size.unwrap_or(DEFAULT_PAGE_SIZE)),
                        })
                }
                Value::Array(arr) => {
                    arr.is_empty()
                        .then_some(HasData::False)
                        .unwrap_or_else(|| HasData::True {
                            has_another_page: arr.len()
                                == usize::from(self.query.page_size.unwrap_or(DEFAULT_PAGE_SIZE)),
                        })
                }
                _ => Default::default(),
            })
            .unwrap_or_default();

        let response = match has_data {
            HasData::False => None,
            HasData::True { has_another_page } => {
                let response = serde_json::from_str(&content).map_err(Error::from);
                self.inner = if has_another_page {
                    Inner::HeadRequest(self.request_next_page())
                } else {
                    Inner::Done
                };

                Some(response)
            }
        };

        Poll::Ready(response)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reqwest(err) => write!(f, "Reqwest error: {err}"),
            Self::SerdeJson(err) => write!(f, "Json error: {err}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::Reqwest(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::SerdeJson(error)
    }
}
