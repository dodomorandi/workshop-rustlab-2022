use std::{
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{ready, stream::FusedStream, FutureExt, Stream};
use pin_project::pin_project;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use tokio::time::{sleep, Instant, Sleep};
use tracing::{debug, warn};
use workshop_rustlab_2022::{
    database::{
        calc_query_cost, ServerField, ServerQuery, DEFAULT_PAGE_SIZE, LEAK_PER_SECOND,
        MAX_BUCKET_CAPACITY,
    },
    LeakyBucket,
};

#[pin_project(project = MyStreamProj)]
pub(crate) struct MyStream<T> {
    query: ServerQuery,
    port: Option<u16>,
    client: Client,
    query_cost: u16,
    bucket: Option<LeakyBucket>,
    last_call: Option<Instant>,
    #[pin]
    inner: Inner<T>,
}

type RequestFuture<T> = impl Future<Output = ResultWithBucket<T>>;
type ResultWithBucket<T> = (Result<Option<(T, bool)>, RequestError>, Option<LeakyBucket>);

#[pin_project(project = InnerProj)]
// We still need this for our "exercise" (in production you would need to perform benchmarks),
// because this is completely bonkers: `tokio::time::Sleep` is currently 640 bytes.
//
// This is a bit unfortunate because the internal `StateCell` has an `AtomicWaker` padded and
// aligned to 128 bytes, which not only creates a huge memory overhead for each waker (without
// padding, it is 24 bytes), but it also lead to a 119 bytes padding for each `StateCell`, wasting
// a **huge** amount of space. If you want to have fun optimizing Tokio, I think that there is some
// work to do :)
#[allow(clippy::large_enum_variant)]
enum Inner<T> {
    Empty,
    Request(#[pin] RequestFuture<T>),
    Sleep(#[pin] Sleep),
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
        loop {
            let mut this = self.as_mut().project();
            match this.inner.as_mut().project() {
                InnerProj::Empty => match this.request_next_page() {
                    Ok(request_fut) => this.inner.set(Inner::Request(request_fut)),
                    Err(sleep) => {
                        let mut this = self.as_mut().project();
                        let sleep = this.set_sleep(sleep);
                        ready!(sleep.poll(cx));
                        this.inner.set(Inner::Empty);
                    }
                },
                InnerProj::Request(fut) => {
                    let result = ready!(fut.poll(cx));
                    break self.handle_request_result(result, cx);
                }
                InnerProj::Sleep(sleep) => {
                    ready!(sleep.poll(cx));
                    match this.request_next_page() {
                        Ok(fut) => this.inner.set(Inner::Request(fut)),
                        Err(sleep) => {
                            let mut this = self.as_mut().project();
                            this.inner.set(Inner::Sleep(sleep));
                        }
                    }
                }
                InnerProj::Done => break Poll::Ready(None),
            }
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
        }
    }
}

impl<T: 'static> MyStreamProj<'_, T>
where
    T: for<'de> Deserialize<'de> + 'static,
{
    #[inline]
    fn request_next_page(&mut self) -> Result<RequestFuture<T>, Sleep> {
        match get_wait_time_for_request(self.bucket, *self.query_cost) {
            Some(wait_time) => Err(sleep(wait_time)),
            None => {
                *self.last_call = Some(Instant::now());
                let request = self.query.create_request(*self.port);
                self.query.page = Some(self.query.page.map(|page| page + 1).unwrap_or(1));
                let bucket = self.bucket.take();
                let client = self.client.clone();

                let future = request_next_page(
                    request,
                    bucket,
                    *self.query_cost,
                    usize::from(self.query.page_size.unwrap_or(DEFAULT_PAGE_SIZE)),
                    client,
                )
                .boxed();

                Ok(future)
            }
        }
    }
}

impl<T> MyStreamProj<'_, T> {
    fn set_sleep(&mut self, sleep: Sleep) -> Pin<&mut Sleep> {
        self.inner.set(Inner::Sleep(sleep));
        match self.inner.as_mut().project() {
            InnerProj::Sleep(sleep) => sleep,
            _ => unreachable!(),
        }
    }

    fn set_request(&mut self, request: RequestFuture<T>) -> Pin<&mut RequestFuture<T>> {
        self.inner.set(Inner::Request(request));
        match self.inner.as_mut().project() {
            InnerProj::Request(request) => request,
            _ => unreachable!(),
        }
    }
}

impl<T> MyStream<T>
where
    T: for<'de> Deserialize<'de> + 'static,
{
    fn handle_request_result(
        mut self: Pin<&mut Self>,
        result: ResultWithBucket<T>,
        cx: &mut Context,
    ) -> Poll<Option<<Self as Stream>::Item>> {
        let (result, bucket) = result;
        let mut this = self.as_mut().project();
        *this.bucket = bucket;

        match result {
            Ok(Some((out, has_another_page))) => {
                let inner = if has_another_page {
                    match this.request_next_page() {
                        Ok(request) => Inner::Request(request),
                        Err(sleep) => Inner::Sleep(sleep),
                    }
                } else {
                    Inner::Done
                };

                this.inner.set(inner);
                Poll::Ready(Some(Ok(out)))
            }
            Ok(None) => {
                this.inner.set(Inner::Done);
                Poll::Ready(None)
            }
            Err(RequestError::TooManyRequests { wait_time }) => {
                let sleep = this.set_sleep(sleep(wait_time));
                ready!(sleep.poll(cx));

                loop {
                    match this.request_next_page() {
                        Ok(request_future) => {
                            let request_future = this.set_request(request_future);
                            let response = ready!(request_future.poll(cx));
                            // Man, you're fast!
                            break self.handle_request_result(response, cx);
                        }
                        Err(sleep) => {
                            warn!(
                                "Still unable to perform a request after sleeping. Sleeping again."
                            );
                            let sleep = this.set_sleep(sleep);
                            ready!(sleep.poll(cx));
                        }
                    }
                }
            }
            Err(RequestError::Other(err)) => {
                self.project().inner.set(Inner::Done);
                Poll::Ready(Some(Err(err)))
            }
        }
    }
}

fn get_wait_time_for_request(bucket: &Option<LeakyBucket>, query_cost: u16) -> Option<Duration> {
    bucket.as_ref().and_then(|bucket| {
        let result = bucket.add(query_cost);
        if let Ok(new_points) = result.as_ref() {
            debug!(
                "Added {query_cost} points to bucket, reached {new_points}/{}",
                bucket.capacity()
            );
        }
        result.err().map(|_| {
            let wait_time = bucket.wait_time_to_use(query_cost);
            debug!(
                "Cannot add {query_cost} points to bucket, need to wait {} milliseconds",
                wait_time.as_millis()
            );
            wait_time
        })
    })
}

async fn request_next_page<T>(
    request: reqwest::Request,
    mut bucket: Option<LeakyBucket>,
    query_cost: u16,
    page_size: usize,
    client: Client,
) -> (Result<Option<(T, bool)>, RequestError>, Option<LeakyBucket>)
where
    T: for<'de> Deserialize<'de> + 'static,
{
    let response = match client.execute(request).await {
        Ok(ok) => ok,
        Err(err) => return (Err(err.into()), bucket),
    };

    match LeakyBucket::try_from(response.headers()) {
        Ok(new_bucket) => {
            debug!(
                "Got new bucket from server with {}/{} points",
                new_bucket.points(),
                new_bucket.capacity()
            );

            if let Some(bucket) = bucket.as_ref() {
                let cur_points = bucket.points();
                let new_points = new_bucket.points();
                if cur_points != new_points {
                    warn!("Expected a leaky bucket with {cur_points} points, server has {new_points} points");
                }
            }

            bucket = Some(new_bucket)
        }
        Err(err) => {
            warn!(
                "Cannot get bucket data from response: {err}. Trying to set a reasonable status."
            );
            bucket
                .get_or_insert_with(|| LeakyBucket::empty(MAX_BUCKET_CAPACITY, LEAK_PER_SECOND))
                .saturating_add(query_cost);
        }
    }

    match response.error_for_status() {
        Ok(response) => {
            let content = match response.text().await {
                Ok(ok) => ok,
                Err(err) => return (Err(err.into()), bucket),
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
                        if obj.is_empty() {
                            HasData::False
                        } else {
                            HasData::True {
                                has_another_page: obj.len() == page_size,
                            }
                        }
                    }
                    Value::Array(arr) => {
                        if arr.is_empty() {
                            HasData::False
                        } else {
                            HasData::True {
                                has_another_page: arr.len() == page_size,
                            }
                        }
                    }
                    _ => Default::default(),
                })
                .unwrap_or_default();

            match has_data {
                HasData::False => (Ok(None), bucket),
                HasData::True { has_another_page } => {
                    let response = serde_json::from_str(&content)
                        .map_err(RequestError::from)
                        .map(|response| Some((response, has_another_page)));

                    (response, bucket)
                }
            }
        }
        Err(err) => match err.status() {
            Some(StatusCode::TOO_MANY_REQUESTS) => {
                let wait_time =
                    get_wait_time_for_request(&bucket, query_cost).unwrap_or_else(|| {
                        // Rough estimation
                        Duration::from_secs((query_cost / u16::from(LEAK_PER_SECOND) + 1).into())
                    });
                warn!(
                    "Performed too many requests, waiting {} milliseconds",
                    wait_time.as_millis()
                );
                (Err(RequestError::TooManyRequests { wait_time }), bucket)
            }
            _ => (Err(Error::Reqwest(err).into()), bucket),
        },
    }
}

impl<T> FusedStream for MyStream<T>
where
    T: for<'de> Deserialize<'de> + 'static,
{
    fn is_terminated(&self) -> bool {
        matches!(self.inner, Inner::Done)
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

enum RequestError {
    TooManyRequests { wait_time: Duration },
    Other(Error),
}

impl From<Error> for RequestError {
    fn from(error: Error) -> Self {
        Self::Other(error)
    }
}

impl From<reqwest::Error> for RequestError {
    fn from(error: reqwest::Error) -> Self {
        Self::Other(Error::Reqwest(error))
    }
}

impl From<serde_json::Error> for RequestError {
    fn from(error: serde_json::Error) -> Self {
        Self::Other(Error::SerdeJson(error))
    }
}
