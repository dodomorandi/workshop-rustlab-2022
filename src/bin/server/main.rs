#![warn(clippy::pedantic)]

mod database;
mod error;

use std::{convert::Infallible, sync::Arc};

use axum::{
    extract::Query,
    http::{header::HeaderName, HeaderValue},
    response::{IntoResponse, IntoResponseParts, Response, ResponseParts},
    routing::get,
    Extension, Json, Router,
};
use database::PartialEntry;
use error::Error;
use rand::{thread_rng, Rng};
use tokio::{
    join,
    sync::{
        mpsc::{channel, Receiver, Sender},
        oneshot,
    },
};
use tower_http::trace::TraceLayer;
use tracing::info;
use workshop_rustlab_2022::{
    database::{
        calc_query_cost, Entry, ServerQuery, DEFAULT_PAGE_SIZE, LEAK_PER_SECOND,
        MAX_BUCKET_CAPACITY,
    },
    leaky_bucket::MaxCapacityError,
    LeakyBucket, BUCKET_CAPACITY_HEADER, BUCKET_POINTS_HEADER,
};

const RAW_DATABASE: &str = include_str!("../../../assets/database.json");

struct AppStateInner {
    sender: Sender<Message>,
}

type AppState = Arc<AppStateInner>;

const BUFFER_SIZE: usize = 32;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt::init();

    let database: Vec<Entry> =
        serde_json::from_str(RAW_DATABASE).expect("unable to parse JSON database");

    let (sender, receiver) = channel(BUFFER_SIZE);
    let app_state = AppStateInner { sender };

    let app = Router::new()
        .route("/", get(root))
        .layer(Extension(Arc::new(app_state)))
        .layer(TraceLayer::new_for_http());

    let axum_future =
        axum::Server::bind(&"127.0.0.1:8080".parse().unwrap()).serve(app.into_make_service());

    let handler_future = handler(&database, receiver);

    info!("Listening on 127.0.0.1:8080");
    let (axum_result, ()) = join!(axum_future, handler_future);
    axum_result.unwrap();
}

async fn root(
    Query(params): Query<ServerQuery>,
    Extension(state): Extension<AppState>,
) -> impl IntoResponse {
    let (replier, receiver) = oneshot::channel();
    state
        .sender
        .send(Message::Query {
            query: params,
            replier,
        })
        .await
        .unwrap();

    receiver.await.unwrap()
}

#[derive(Debug)]
enum Message {
    Query {
        query: ServerQuery,
        replier: oneshot::Sender<Result<(BucketInfo, Response), error::Error>>,
    },
}

#[derive(Debug)]
struct BucketInfo {
    points: u16,
    capacity: u16,
}

impl IntoResponseParts for BucketInfo {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        res.headers_mut().extend(
            [
                (BUCKET_POINTS_HEADER, self.points),
                (BUCKET_CAPACITY_HEADER, self.capacity),
            ]
            .into_iter()
            .map(|(header, value)| {
                (
                    Some(HeaderName::from_static(header)),
                    HeaderValue::from(value),
                )
            }),
        );
        Ok(res)
    }
}

async fn handler(database: &[Entry], mut receiver: Receiver<Message>) {
    const SPORADIC_POINTS_PROBABILITY: f64 = 0.15;
    const SPORADIC_POINTS_MAX: u16 = 4;

    let bucket = LeakyBucket::empty(MAX_BUCKET_CAPACITY, LEAK_PER_SECOND);
    let mut rng = thread_rng();

    while let Some(message) = receiver.recv().await {
        if rng.gen_bool(SPORADIC_POINTS_PROBABILITY) {
            bucket.saturating_add(rng.gen_range(1..=SPORADIC_POINTS_MAX));
        }

        match message {
            Message::Query { query, replier } => {
                let cost = calc_query_cost(&query);
                let capacity = bucket.capacity();
                let bucket_points = match bucket.add(cost) {
                    Ok(points) => points,
                    Err(MaxCapacityError(points)) => {
                        let error = Error::NotEnoughCapacity {
                            request: cost,
                            points,
                            capacity,
                        };
                        replier.send(Err(error)).unwrap();
                        continue;
                    }
                };

                let entries: Vec<_> = database
                    .chunks(query.page_size.unwrap_or(DEFAULT_PAGE_SIZE).into())
                    .nth(query.page.unwrap_or(0))
                    .map(|page| {
                        page.iter()
                            .map(|entry| {
                                query.fields.as_ref().map_or_else(
                                    || PartialEntry::from(entry),
                                    |fields| PartialEntry::from_entry_with_fields(entry, fields),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let response = Json(entries).into_response();
                let bucket_info = BucketInfo {
                    points: bucket_points,
                    capacity,
                };

                replier.send(Ok((bucket_info, response))).unwrap();
            }
        }
    }
}
