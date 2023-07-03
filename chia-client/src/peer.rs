use std::{collections::HashMap, io::Cursor, sync::Arc};

use chia_protocol::{ChiaProtocolMessage, Handshake, Message, NodeType, Streamable};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::{
    net::TcpStream,
    sync::{broadcast, mpsc, oneshot, Mutex, RwLock},
    task::JoinHandle,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{PeerEvent, PeerMessage, RequestError, SendError};

type PeerSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct Peer {
    event_sender: broadcast::Sender<PeerEvent>,
    message_sender: mpsc::Sender<PeerMessage>,
    requests: Arc<Mutex<HashMap<u16, oneshot::Sender<Message>>>>,
    request_nonce: RwLock<u16>,
    outbound_handler: JoinHandle<()>,
    inbound_handler: JoinHandle<()>,
}

impl Peer {
    pub async fn new(ws: PeerSocket) -> Self {
        let (sink, stream) = ws.split();
        let (event_sender, _) = broadcast::channel(32);
        let (message_sender, message_receiver) = mpsc::channel(32);
        let requests = Arc::new(Mutex::new(HashMap::<u16, oneshot::Sender<Message>>::new()));

        let outbound_handler = tokio::spawn(Self::outbound_handler(message_receiver, sink));
        let inbound_handler = tokio::spawn(Self::inbound_handler(Arc::clone(&requests), stream));

        Self {
            event_sender,
            message_sender,
            requests,
            request_nonce: RwLock::new(0),
            outbound_handler,
            inbound_handler,
        }
    }

    async fn outbound_handler(
        mut receiver: mpsc::Receiver<PeerMessage>,
        mut sink: SplitSink<PeerSocket, tungstenite::Message>,
    ) {
        while let Some(message) = receiver.recv().await {
            match message {
                PeerMessage::Protocol(message) => {
                    let mut bytes = Vec::new();
                    if message.stream(&mut bytes).is_ok() {
                        sink.send(bytes.into()).await.ok();
                    }
                }
                PeerMessage::Close => {
                    sink.close().await.ok();
                    break;
                }
            }
        }
    }

    async fn inbound_handler(
        requests: Arc<Mutex<HashMap<u16, oneshot::Sender<Message>>>>,
        mut stream: SplitStream<PeerSocket>,
    ) {
        while let Some(next) = stream.next().await {
            match next {
                Ok(ws_message) => match Message::parse(&mut Cursor::new(&ws_message.into_data())) {
                    Ok(message) => {
                        if let Some(id) = message.id {
                            if let Some(request) = requests.lock().await.remove(&id) {
                                request.send(message).ok();
                            }
                        }
                    }
                    Err(_error) => {} // TODO: Handle protocol errors
                },
                Err(_error) => {} // TODO: Handle protocol errors
            }
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PeerEvent> {
        self.event_sender.subscribe()
    }

    pub async fn perform_handshake(&self, network_id: String) -> Result<(), SendError> {
        let handshake = Handshake {
            network_id,
            protocol_version: "0.0.34".to_string(),
            software_version: "0.0.0".to_string(),
            server_port: 0,
            node_type: NodeType::Wallet,
            capabilities: vec![
                (1, "1".to_string()),
                (2, "1".to_string()),
                (3, "1".to_string()),
            ],
        };
        self.send(handshake).await
    }

    pub async fn send<T>(&self, body: T) -> Result<(), SendError>
    where
        T: Streamable + ChiaProtocolMessage,
    {
        let mut body_bytes = Vec::new();

        body.stream(&mut body_bytes)
            .map_err(|error| SendError::StreamError {
                reason: error.to_string(),
            })?;

        let message = Message {
            msg_type: T::msg_type(),
            id: None,
            data: body_bytes.into(),
        };

        self.message_sender
            .send(PeerMessage::Protocol(message))
            .await
            .map_err(|error| SendError::SocketError {
                reason: error.to_string(),
            })?;

        Ok(())
    }

    pub async fn request<T, R>(&self, body: T) -> Result<R, RequestError>
    where
        T: Streamable + ChiaProtocolMessage,
        R: Streamable + ChiaProtocolMessage,
    {
        let mut body_bytes = Vec::new();

        body.stream(&mut body_bytes)
            .map_err(|error| RequestError::StreamError {
                reason: error.to_string(),
            })?;

        let id = *self.request_nonce.read().await;

        let message = Message {
            msg_type: T::msg_type(),
            id: Some(id),
            data: body_bytes.into(),
        };

        *self.request_nonce.write().await += 1;

        self.message_sender
            .send(PeerMessage::Protocol(message))
            .await
            .map_err(|error| RequestError::SocketError {
                reason: error.to_string(),
            })?;

        let (sender, receiver) = oneshot::channel::<Message>();

        self.requests.lock().await.insert(id, sender);

        match receiver.await {
            Err(error) => {
                self.requests.lock().await.remove(&id);

                Err(RequestError::ResponseError {
                    message: None,
                    reason: error.to_string(),
                })
            }
            Ok(message) => {
                if message.msg_type != R::msg_type() {
                    return Err(RequestError::ResponseError {
                        message: Some(message),
                        reason: "invalid response message type".to_string(),
                    });
                }

                R::parse(&mut Cursor::new(message.data.as_ref())).map_err(|error| {
                    RequestError::ParseError {
                        message,
                        reason: error.to_string(),
                    }
                })
            }
        }
    }

    pub async fn close(&self) -> Result<(), mpsc::error::SendError<PeerMessage>> {
        self.message_sender.send(PeerMessage::Close).await
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.outbound_handler.abort();
        self.inbound_handler.abort();
    }
}
