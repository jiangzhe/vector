//! Future types
//!
use super::controller::Controller;
use futures_core::ready;
use pin_project::pin_project;
use std::time::Instant;
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::OwnedSemaphorePermit;

/// Future for the `AutoConcurrencyLimit` service.
#[pin_project]
#[derive(Debug)]
pub(crate) struct ResponseFuture<T> {
    #[pin]
    inner: T,
    // Keep this around so that it is dropped when the future completes
    _permit: OwnedSemaphorePermit,
    controller: Arc<Controller>,
    start: Instant,
}

impl<T> ResponseFuture<T> {
    pub(super) fn new(
        inner: T,
        _permit: OwnedSemaphorePermit,
        controller: Arc<Controller>,
    ) -> ResponseFuture<T> {
        ResponseFuture {
            inner,
            _permit,
            controller,
            start: Instant::now(),
        }
    }
}

impl<F, T, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<T, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(ready!(self.project().inner.poll(cx)))
    }
}
