#![feature(type_alias_impl_trait)]

use std::future::ready;

use futures_util::{future, stream, TryStreamExt};
use reqwest::Client;
use serde::Deserialize;
use workshop_rustlab_2022::database::{self, GeoPoint2d};

use crate::my_stream::MyStream;

mod my_stream;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use database::ServerField::*;

    tracing_subscriber::fmt::init();

    let stream = MyStream::<Vec<MyEntry>>::new(
        [Name, GeoPoint2d, Numeromoderno].into_iter().collect(),
        Some(8080),
        Client::new(),
    );

    stream
        .map_ok(|entries| stream::iter(entries.into_iter().map(Ok)))
        .try_flatten()
        .try_filter(|data| ready(data.name != "-"))
        .try_for_each(|data| {
            println!("{}: {:#?}", data.name, data.geo_point_2d);
            future::ok::<_, my_stream::Error>(())
        })
        .await?;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct MyEntry {
    pub geo_point_2d: GeoPoint2d,
    pub name: String,
    pub numeromoderno: String,
}
