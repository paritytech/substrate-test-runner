use futures::compat::Future01CompatExt;
use futures::{
	channel::mpsc,
	compat::{Compat01As03, Sink01CompatExt, Stream01CompatExt},
	sink::SinkExt,
	stream::StreamExt,
};
use futures01::sync::mpsc as mpsc01;
use jsonrpc_core::MetaIoHandler;
use jsonrpsee::{
	common::{Request, Response},
	transport::TransportClient,
};
pub use sc_service::{
	config::{DatabaseConfig, KeystoreConfig},
	Error as ServiceError,
};
use std::{future::Future, pin::Pin, sync::Arc};

/// Error thrown by the client.
#[derive(Debug, derive_more::Display, derive_more::From, derive_more::Error)]
pub enum SubxtClientError {
	/// Failed to parse json rpc message.
	#[display(fmt = "{}", _0)]
	Json(serde_json::Error),
	/// Channel closed.
	#[display(fmt = "{}", _0)]
	Mpsc(mpsc::SendError),
}

/// Client for an embedded substrate node.
pub struct SubxtClient {
	request_sink: mpsc::Sender<String>,
	response_stream: Compat01As03<mpsc01::Receiver<String>>,
}

impl SubxtClient {
	/// Create a new client.
	pub fn new(rpc: Arc<MetaIoHandler<sc_rpc::Metadata>>) -> Self {
		let (request_sink, mut request_stream) = mpsc::channel::<String>(4);
		let (response_sink, response_stream) = mpsc01::channel::<String>(4);

		let session = response_sink.clone();

		tokio::task::spawn(async move {
			while let Some(request) = request_stream.next().await {
				let rpc = rpc.clone();
				let mut response_sink = response_sink.clone().sink_compat();

				let response = rpc.handle_request(&request, session.clone().into()).compat().await;
				if let Ok(Some(response)) = response {
					response_sink.send(response).await.ok();
				}
			}
		});

		Self {
			request_sink,
			response_stream: response_stream.compat(),
		}
	}
}

impl TransportClient for SubxtClient {
	type Error = SubxtClientError;

	fn send_request<'a>(
		&'a mut self,
		request: Request,
	) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>> {
		Box::pin(async move {
			let request = serde_json::to_string(&request)?;
			self.request_sink.send(request).await?;
			Ok(())
		})
	}

	fn next_response<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = Result<Response, Self::Error>> + Send + 'a>> {
		Box::pin(async move {
			let response = self
				.response_stream
				.next()
				.await
				.expect("channel shouldn't close")
				.unwrap();
			Ok(serde_json::from_str(&response)?)
		})
	}
}

impl From<SubxtClient> for jsonrpsee::Client {
	fn from(client: SubxtClient) -> Self {
		let client = jsonrpsee::raw::RawClient::new(client);
		jsonrpsee::Client::new(client)
	}
}
