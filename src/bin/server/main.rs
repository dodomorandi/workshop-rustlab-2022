mod database;
mod error;

use std::sync::Arc;

use axum::{
    extract::Query,
    response::{IntoResponse, Response},
    routing::get,
    Extension, Json, Router,
};
use database::PartialEntry;
use error::Error;
use tokio::{
    join,
    sync::{
        mpsc::{channel, Receiver, Sender},
        oneshot,
    },
};
use workshop_rustlab_2022::{
    database::{
        calc_query_cost, Entry, ServerQuery, DEFAULT_PAGE_SIZE, LEAK_PER_SECOND,
        MAX_BUCKET_CAPACITY,
    },
    LeakyBucket,
};

const RAW_DATABASE: &str = include_str!("../../../assets/database.json");

struct AppStateInner {
    sender: Sender<Message>,
}

type AppState = Arc<AppStateInner>;

const BUFFER_SIZE: usize = 32;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let database: Vec<Entry> =
        serde_json::from_str(RAW_DATABASE).expect("unable to parse JSON database");

    let (sender, receiver) = channel(BUFFER_SIZE);
    let app_state = AppStateInner { sender };

    let app = Router::new()
        .route("/", get(root))
        .layer(Extension(Arc::new(app_state)));

    let axum_future =
        axum::Server::bind(&"127.0.0.1:8080".parse().unwrap()).serve(app.into_make_service());

    let handler_future = handler(&database, receiver);

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
        replier: oneshot::Sender<Result<Response, error::Error>>,
    },
}

async fn handler(database: &[Entry], mut receiver: Receiver<Message>) {
    let bucket = LeakyBucket::empty(MAX_BUCKET_CAPACITY, LEAK_PER_SECOND);

    while let Some(message) = receiver.recv().await {
        match message {
            Message::Query { query, replier } => {
                let cost = calc_query_cost(&query);
                if bucket.add(cost).is_err() {
                    replier
                        .send(Err(Error::NotEnoughCapacity {
                            request: cost,
                            available: bucket.available(),
                        }))
                        .unwrap();
                    continue;
                }

                let entries: Vec<_> = database
                    .chunks(query.page_size.unwrap_or(DEFAULT_PAGE_SIZE).into())
                    .nth(query.page.unwrap_or(0))
                    .map(|page| {
                        page.iter()
                            .map(|entry| {
                                query
                                    .fields
                                    .as_ref()
                                    .map(|fields| {
                                        PartialEntry::from_entry_with_fields(entry, fields)
                                    })
                                    .unwrap_or_else(|| PartialEntry::from(entry))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let response = Json(entries).into_response();

                replier.send(Ok(response)).unwrap();
            }
        }
    }
}