use std::io::Cursor;
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

use crate::{Error, Result};

type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Requests = Arc<Mutex<HashMap<u16, oneshot::Sender<Message>>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEvent {
    Handshake(Handshake),
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
                    let bytes = message.into_data();
                    let cursor = &mut Cursor::new(bytes.as_slice());

                    // Parse the message.
                    let Ok(message) = Message::parse(cursor) else {
                        continue;
                    };

                    if let Some(id) = message.id {
                        // Send response through oneshot channel if present.
                        if let Some(request) = requests_clone.lock().await.remove(&id) {
                            request.send(message).ok();
                        }
                    } else {
                        match message.msg_type {
                            ProtocolMessageTypes::CoinStateUpdate => {
                                let cursor = &mut Cursor::new(message.data.as_ref());
                                if let Ok(body) = CoinStateUpdate::parse(cursor) {
                                    event_sender.send(PeerEvent::CoinStateUpdate(body)).ok();
                                }
                            }
                            ProtocolMessageTypes::NewPeakWallet => {
                                let cursor = &mut Cursor::new(message.data.as_ref());
                                if let Ok(body) = NewPeakWallet::parse(cursor) {
                                    event_sender.send(PeerEvent::NewPeakWallet(body)).ok();
                                }
                            }
                            _ => {}
                        }
                    }
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
            data: {
                let mut body_bytes = Vec::new();
                body.stream(&mut body_bytes)?;
                body_bytes.into()
            },
        };

        // Convert message to bytes.
        let mut bytes = Vec::new();
        message.stream(&mut bytes)?;

        // Send the message through the websocket.
        let mut sink = self.sink.lock().await;
        sink.send(bytes.into()).await?;

        Ok(())
    }

    pub async fn request<T, R>(&self, body: T) -> Result<R>
    where
        T: Streamable + ChiaProtocolMessage,
        R: Streamable + ChiaProtocolMessage,
    {
        // Get the current nonce.
        let mut nonce = self.nonce.lock().await;
        let message_id = *nonce;

        // Create the message.
        let message = Message {
            msg_type: T::msg_type(),
            id: Some(message_id),
            data: {
                let mut body_bytes = Vec::new();
                body.stream(&mut body_bytes)?;
                body_bytes.into()
            },
        };

        // Convert message to bytes.
        let mut bytes = Vec::new();
        message.stream(&mut bytes)?;

        // Create a saved oneshot channel to receive the response.
        let (sender, receiver) = oneshot::channel::<Message>();
        self.requests.lock().await.insert(message_id, sender);

        // Send the message.
        if let Err(error) = self.sink.lock().await.send(bytes.into()).await {
            // Prevent memory leak.
            self.requests.lock().await.remove(&message_id);

            return Err(error.into());
        }

        // Increment the nonce and drop its lock.
        *nonce += 1;
        drop(nonce);

        // Wait for the response.
        let response = receiver.await;

        // Remove the one shot channel.
        self.requests.lock().await.remove(&message_id);

        match response {
            Ok(message) => {
                let expected_type = R::msg_type();
                let found_type = message.msg_type;

                if found_type != expected_type {
                    return Err(Error::InvalidResponse(Some(message)));
                }

                match R::parse(&mut Cursor::new(message.data.as_ref())) {
                    Ok(response) => Ok(response),
                    _ => Err(Error::InvalidResponse(Some(message))),
                }
            }
            _ => Err(Error::InvalidResponse(None)),
        }
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
