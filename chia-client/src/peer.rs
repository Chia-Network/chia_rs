use std::{collections::HashMap, io::Cursor, sync::Arc};

use chia_protocol::{Handshake, Message, NodeType, ProtocolMessageTypes, Streamable};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot, Mutex},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{PeerMessage, RequestError, SendError};

type PeerSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct Peer {
    sender: mpsc::Sender<PeerMessage>,
    requests: Arc<Mutex<HashMap<u16, oneshot::Sender<Message>>>>,
    request_nonce: u16,
}

impl Peer {
    pub async fn new(ws: PeerSocket) -> Self {
        let (sink, stream) = ws.split();
        let (sender, receiver) = mpsc::channel(32);
        let requests = Arc::new(Mutex::new(HashMap::<u16, oneshot::Sender<Message>>::new()));

        tokio::spawn(Self::outbound_handler(receiver, sink));
        tokio::spawn(Self::inbound_handler(Arc::clone(&requests), stream));

        Self {
            sender,
            requests,
            request_nonce: 0,
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

    pub async fn perform_handshake(&mut self, network_id: String) -> Result<(), SendError> {
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
        self.send(ProtocolMessageTypes::Handshake, handshake).await
    }

    async fn send<T>(&self, request_type: ProtocolMessageTypes, body: T) -> Result<(), SendError>
    where
        T: Streamable,
    {
        let mut body_bytes = Vec::new();

        body.stream(&mut body_bytes)
            .map_err(|error| SendError::StreamError {
                reason: error.to_string(),
            })?;

        let message = Message {
            msg_type: request_type,
            id: None,
            data: body_bytes.into(),
        };

        self.sender
            .send(PeerMessage::Protocol(message))
            .await
            .map_err(|error| SendError::SocketError {
                reason: error.to_string(),
            })?;

        Ok(())
    }

    pub async fn request<T, R>(
        &mut self,
        request_type: ProtocolMessageTypes,
        response_type: ProtocolMessageTypes,
        body: T,
    ) -> Result<R, RequestError>
    where
        T: Streamable,
        R: Streamable,
    {
        let mut body_bytes = Vec::new();

        body.stream(&mut body_bytes)
            .map_err(|error| RequestError::StreamError {
                reason: error.to_string(),
            })?;

        let id = self.request_nonce;

        let message = Message {
            msg_type: request_type,
            id: Some(id),
            data: body_bytes.into(),
        };

        self.request_nonce += 1;

        self.sender
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
                if message.msg_type != response_type {
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
        self.sender.send(PeerMessage::Close).await
    }
}
