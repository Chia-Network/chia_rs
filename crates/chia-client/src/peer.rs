use std::sync::atomic::{AtomicU16, Ordering};
use std::{collections::HashMap, sync::Arc};

use chia_protocol::*;
use chia_traits::Streamable;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, oneshot, Mutex};
use tokio::{net::TcpStream, task::JoinHandle};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::Message as WsMessage;

use crate::utils::stream;
use crate::Error;

type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Requests = Arc<Mutex<HashMap<u16, oneshot::Sender<Message>>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEvent {
    CoinStateUpdate(CoinStateUpdate),
    NewPeakWallet(NewPeakWallet),
}

pub struct Peer {
    sink: Mutex<SplitSink<WebSocket, tungstenite::Message>>,
    inbound_task: JoinHandle<()>,
    event_receiver: broadcast::Receiver<PeerEvent>,
    requests: Requests,

    // TODO: This does not currently prevent multiple requests with the same id at the same time.
    // If one of them is still running while all other ids are being iterated through.
    nonce: AtomicU16,
}

impl Peer {
    pub fn new(ws: WebSocket) -> Self {
        let (sink, mut stream) = ws.split();
        let (event_sender, event_receiver) = broadcast::channel(32);

        let requests = Requests::default();
        let requests_clone = Arc::clone(&requests);

        let inbound_task = tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                if let Ok(message) = message {
                    Self::handle_inbound(message, &requests_clone, &event_sender)
                        .await
                        .ok();
                }
            }
        });

        Self {
            sink: Mutex::new(sink),
            inbound_task,
            event_receiver,
            requests,
            nonce: AtomicU16::new(0),
        }
    }

    pub async fn send_handshake(
        &self,
        network_id: String,
        node_type: NodeType,
    ) -> Result<(), Error<()>> {
        let body = Handshake {
            network_id,
            protocol_version: "0.0.34".to_string(),
            software_version: "0.0.0".to_string(),
            server_port: 0,
            node_type,
            capabilities: vec![
                (1, "1".to_string()),
                (2, "1".to_string()),
                (3, "1".to_string()),
            ],
        };
        self.send(body).await
    }

    pub async fn request_puzzle_and_solution(
        &self,
        coin_id: Bytes32,
        height: u32,
    ) -> Result<PuzzleSolutionResponse, Error<RejectPuzzleSolution>> {
        let body = RequestPuzzleSolution {
            coin_name: coin_id,
            height,
        };
        let response: RespondPuzzleSolution = self.request_or_reject(body).await?;
        Ok(response.response)
    }

    pub async fn send_transaction(
        &self,
        spend_bundle: SpendBundle,
    ) -> Result<TransactionAck, Error<()>> {
        let body = SendTransaction {
            transaction: spend_bundle,
        };
        self.request(body).await
    }

    pub async fn request_block_header(
        &self,
        height: u32,
    ) -> Result<HeaderBlock, Error<RejectHeaderRequest>> {
        let body = RequestBlockHeader { height };
        let response: RespondBlockHeader = self.request_or_reject(body).await?;
        Ok(response.header_block)
    }

    pub async fn request_block_headers(
        &self,
        start_height: u32,
        end_height: u32,
        return_filter: bool,
    ) -> Result<Vec<HeaderBlock>, Error<()>> {
        let body = RequestBlockHeaders {
            start_height,
            end_height,
            return_filter,
        };
        let response: RespondBlockHeaders =
            self.request_or_reject(body)
                .await
                .map_err(|error: Error<RejectBlockHeaders>| match error {
                    Error::Rejection(_rejection) => Error::Rejection(()),
                    Error::Chia(error) => Error::Chia(error),
                    Error::WebSocket(error) => Error::WebSocket(error),
                    Error::InvalidResponse(error) => Error::InvalidResponse(error),
                    Error::MissingResponse => Error::MissingResponse,
                })?;
        Ok(response.header_blocks)
    }

    pub async fn request_removals(
        &self,
        height: u32,
        header_hash: Bytes32,
        coin_ids: Option<Vec<Bytes32>>,
    ) -> Result<RespondRemovals, Error<RejectRemovalsRequest>> {
        let body = RequestRemovals {
            height,
            header_hash,
            coin_names: coin_ids,
        };
        self.request_or_reject(body).await
    }

    pub async fn request_additions(
        &self,
        height: u32,
        header_hash: Option<Bytes32>,
        puzzle_hashes: Option<Vec<Bytes32>>,
    ) -> Result<RespondAdditions, Error<RejectAdditionsRequest>> {
        let body = RequestAdditions {
            height,
            header_hash,
            puzzle_hashes,
        };
        self.request_or_reject(body).await
    }

    pub async fn register_for_ph_updates(
        &self,
        puzzle_hashes: Vec<Bytes32>,
        min_height: u32,
    ) -> Result<Vec<CoinState>, Error<()>> {
        let body = RegisterForPhUpdates {
            puzzle_hashes,
            min_height,
        };
        let response: RespondToPhUpdates = self.request(body).await?;
        Ok(response.coin_states)
    }

    pub async fn register_for_coin_updates(
        &self,
        coin_ids: Vec<Bytes32>,
        min_height: u32,
    ) -> Result<Vec<CoinState>, Error<()>> {
        let body = RegisterForCoinUpdates {
            coin_ids,
            min_height,
        };
        let response: RespondToCoinUpdates = self.request(body).await?;
        Ok(response.coin_states)
    }

    pub async fn request_children(&self, coin_id: Bytes32) -> Result<Vec<CoinState>, Error<()>> {
        let body = RequestChildren { coin_name: coin_id };
        let response: RespondChildren = self.request(body).await?;
        Ok(response.coin_states)
    }

    pub async fn request_ses_info(
        &self,
        start_height: u32,
        end_height: u32,
    ) -> Result<RespondSesInfo, Error<()>> {
        let body = RequestSesInfo {
            start_height,
            end_height,
        };
        self.request(body).await
    }

    pub async fn request_fee_estimates(
        &self,
        time_targets: Vec<u64>,
    ) -> Result<FeeEstimateGroup, Error<()>> {
        let body = RequestFeeEstimates { time_targets };
        let response: RespondFeeEstimates = self.request(body).await?;
        Ok(response.estimates)
    }

    pub async fn send<T>(&self, body: T) -> Result<(), Error<()>>
    where
        T: Streamable + ChiaProtocolMessage,
    {
        // Create the message.
        let message = Message {
            msg_type: T::msg_type(),
            id: None,
            data: stream(&body)?.into(),
        };

        // Send the message through the websocket.
        let mut sink = self.sink.lock().await;
        sink.send(stream(&message)?.into()).await?;

        Ok(())
    }

    pub async fn request_or_reject<T, R, B>(&self, body: B) -> Result<T, Error<R>>
    where
        T: Streamable + ChiaProtocolMessage,
        R: Streamable + ChiaProtocolMessage,
        B: Streamable + ChiaProtocolMessage,
    {
        let message = self.request_raw(body).await?;
        let data = message.data.as_ref();

        if message.msg_type == T::msg_type() {
            T::from_bytes(data).or(Err(Error::InvalidResponse(message)))
        } else if message.msg_type == R::msg_type() {
            let rejection = R::from_bytes(data).or(Err(Error::InvalidResponse(message)))?;
            Err(Error::Rejection(rejection))
        } else {
            Err(Error::InvalidResponse(message))
        }
    }

    pub async fn request<Response, T>(&self, body: T) -> Result<Response, Error<()>>
    where
        Response: Streamable + ChiaProtocolMessage,
        T: Streamable + ChiaProtocolMessage,
    {
        let message = self.request_raw(body).await?;
        let data = message.data.as_ref();

        if message.msg_type == Response::msg_type() {
            Response::from_bytes(data).or(Err(Error::InvalidResponse(message)))
        } else {
            Err(Error::InvalidResponse(message))
        }
    }

    pub async fn request_raw<T, R>(&self, body: T) -> Result<Message, Error<R>>
    where
        T: Streamable + ChiaProtocolMessage,
    {
        // Get the current nonce and increment.
        let message_id = self.nonce.fetch_add(1, Ordering::SeqCst);

        // Create the message.
        let message = Message {
            msg_type: T::msg_type(),
            id: Some(message_id),
            data: stream(&body)?.into(),
        };

        // Create a saved oneshot channel to receive the response.
        let (sender, receiver) = oneshot::channel::<Message>();
        self.requests.lock().await.insert(message_id, sender);

        // Send the message.
        let bytes = match stream(&message) {
            Ok(bytes) => bytes.into(),
            Err(error) => {
                self.requests.lock().await.remove(&message_id);
                return Err(error.into());
            }
        };
        let send_result = self.sink.lock().await.send(bytes).await;

        if let Err(error) = send_result {
            self.requests.lock().await.remove(&message_id);
            return Err(error.into());
        }

        // Wait for the response.
        let response = receiver.await;

        // Remove the one shot channel.
        self.requests.lock().await.remove(&message_id);

        // Handle the response, if present.
        response.or(Err(Error::MissingResponse))
    }

    pub fn receiver(&self) -> &broadcast::Receiver<PeerEvent> {
        &self.event_receiver
    }

    pub fn receiver_mut(&mut self) -> &mut broadcast::Receiver<PeerEvent> {
        &mut self.event_receiver
    }

    async fn handle_inbound(
        message: WsMessage,
        requests: &Requests,
        event_sender: &broadcast::Sender<PeerEvent>,
    ) -> Result<(), Error<()>> {
        // Parse the message.
        let message = Message::from_bytes(message.into_data().as_ref())?;

        if let Some(id) = message.id {
            // Send response through oneshot channel if present.
            if let Some(request) = requests.lock().await.remove(&id) {
                request.send(message).ok();
            }
            return Ok(());
        }

        macro_rules! events {
            ( $( $event:ident ),+ $(,)? ) => {
                match message.msg_type {
                    $( ProtocolMessageTypes::$event => {
                        event_sender
                            .send(PeerEvent::$event($event::from_bytes(message.data.as_ref())?))
                            .ok();
                    } )+
                    _ => {}
                }
            };
        }

        // TODO: Handle unexpected messages.
        events!(CoinStateUpdate, NewPeakWallet);

        Ok(())
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.inbound_task.abort();
    }
}
