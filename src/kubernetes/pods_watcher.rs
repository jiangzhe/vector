use super::watch_state::ResourceVersionState;
use super::{client::Client, stream as k8s_stream};
use async_stream::try_stream;
use futures::{
    pin_mut,
    stream::{Stream, StreamExt},
};
use http02::StatusCode;
use hyper13::Error as BodyError;
use k8s_openapi::{
    api::core::v1::Pod, apimachinery::pkg::apis::meta::v1::WatchEvent, WatchOptional, WatchResponse,
};
use std::time::Duration;
use tokio::time::delay_for;
// use snafu::Snafu;

pub struct PodsWatcher {
    client: Client,
    field_selector: Option<String>,
    label_selector: Option<String>,
    resource_version: ResourceVersionState,
    pause_between_requests: Duration,
}

impl PodsWatcher {
    pub fn new(
        client: Client,
        field_selector: Option<String>,
        label_selector: Option<String>,
        pause_between_requests: Duration,
    ) -> Self {
        let resource_version = ResourceVersionState::new();
        Self {
            client,
            label_selector,
            field_selector,
            resource_version,
            pause_between_requests,
        }
    }
}

impl PodsWatcher {
    pub fn watch(&mut self) -> impl Stream<Item = Result<WatchEvent<Pod>, crate::Error>> + '_ {
        try_stream! {
            loop {
                let stream = self.issue_request().await?;
                pin_mut!(stream);
                while let Some(item) = stream.next().await {
                    // Any error here is considered critical, do not attemt
                    // to retry and just quit.
                    let item = match item?;

                    let item = match item {
                        WatchResponse::Ok(item) => item,
                        WatchResponse::Other(item) => Err("got invalid response from k8s")?,
                    };

                    self.resource_version.update(&item);

                    yield item;
                }

                // For the next pause duration we won't get any updates.
                // This is better than flooding k8s api server with requests.
                delay_for(self.pause_between_requests).await;
            }
        }
    }

    async fn issue_request(
        &mut self,
    ) -> crate::Result<impl Stream<Item = Result<WatchResponse<Pod>, k8s_stream::Error<BodyError>>>>
    {
        let watch_options = WatchOptional {
            field_selector: self.field_selector.as_ref().map(|s| s.as_str()),
            label_selector: self.label_selector.as_ref().map(|s| s.as_str()),
            pretty: None,
            resource_version: self.resource_version.get(),
            timeout_seconds: None,
            allow_watch_bookmarks: Some(true),
        };

        let (request, _) = Pod::watch_pod_for_all_namespaces(watch_options)?;
        trace!(message = "Request prepared", ?request);

        let response = self.client.send(request).await?;
        trace!(message = "Got response", ?response);
        if response.status() != StatusCode::OK {
            Err("watch request failed")?;
        }

        let body = response.into_body();
        Ok(k8s_stream::body::<_, WatchResponse<Pod>>(body))
    }
}

// /// Errors that can occur while watching.
// #[derive(Debug, Snafu)]
// pub enum Error
// {
//     /// Server .
//     #[snafu(display("reading the data chunk failed"))]
//     WatchRequest {
//         /// The error we got while reading.
//         source: ReadError,
//     },

// }
