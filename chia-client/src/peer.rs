use std::{collections::HashMap, sync::Arc};

use chia_protocol::{
    ChiaProtocolMessage, CoinStateUpdate, Handshake, Message, NewPeakWallet, NodeType,
    ProtocolMessageTypes,
};
use chia_traits::Streamable;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, oneshot, Mutex};
use tokio::{net::TcpStream, task::JoinHandle};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::Message as WsMessage;

use crate::utils::stream;
use crate::{Error, Result};

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
    nonce: Mutex<u16>,
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
            nonce: Mutex::new(0),
        }
    }

    async fn handle_inbound(
        message: WsMessage,
        requests: &Requests,
        event_sender: &broadcast::Sender<PeerEvent>,
    ) -> Result<()> {
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

        events!(CoinStateUpdate, NewPeakWallet);

        Ok(())
    }

    pub async fn perform_handshake(&self, network_id: String, node_type: NodeType) -> Result<()> {
        let handshake = Handshake {
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
        self.send(handshake).await
    }

    pub async fn send<T>(&self, body: T) -> Result<()>
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

    pub async fn request<T, R>(&self, body: T) -> Result<R>
    where
        T: Streamable + ChiaProtocolMessage,
        R: Streamable + ChiaProtocolMessage,
    {
        // Get the current nonce and increment.
        let message_id = {
            let mut nonce = self.nonce.lock().await;
            let id = *nonce;
            *nonce += 1;
            id
        };

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
        response
            .map(|message| {
                let expected_type = R::msg_type();
                let found_type = message.msg_type;

                if found_type != expected_type {
                    return Err(Error::InvalidResponse(message));
                }

                R::from_bytes(message.data.as_ref()).or(Err(Error::InvalidResponse(message)))
            })
            .unwrap_or(Err(Error::MissingResponse))
    }

    pub fn receiver(&self) -> &broadcast::Receiver<PeerEvent> {
        &self.event_receiver
    }

    pub fn receiver_mut(&mut self) -> &mut broadcast::Receiver<PeerEvent> {
        &mut self.event_receiver
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.inbound_task.abort();
    }
}
