use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Stream;

pub(crate) struct MyStream<T> {
    _marker: PhantomData<fn() -> T>,
}

pub(crate) enum Error {}

impl<T> Stream for MyStream<T> {
    type Item = Result<T, Error>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Option<Self::Item>> {
        todo!()
    }
}
