use std::{fmt, net::IpAddr, sync::Arc};

use chia_protocol::{ChiaProtocolMessage, Message};
use chia_traits::Streamable;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use native_tls::TlsConnector;
use sha2::{digest::FixedOutput, Digest, Sha256};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot, Mutex},
    task::JoinHandle,
};
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream};

use crate::{request_map::RequestMap, Error, Response, Result};

type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Sink = SplitSink<WebSocket, tungstenite::Message>;
type Stream = SplitStream<WebSocket>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerId([u8; 32]);

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

#[derive(Debug, Clone)]
pub struct Peer(Arc<PeerInner>);

#[derive(Debug)]
struct PeerInner {
    sink: Mutex<Sink>,
    inbound_handle: JoinHandle<Result<()>>,
    requests: Arc<RequestMap>,
    peer_id: PeerId,
    ip_addr: IpAddr,
}

impl Peer {
    pub async fn connect(
        ip: IpAddr,
        port: u16,
        tls_connector: TlsConnector,
    ) -> Result<(Self, mpsc::Receiver<Message>)> {
        let uri = if ip.is_ipv4() {
            format!("wss://{ip}:{port}/ws")
        } else {
            format!("wss://[{ip}]:{port}/ws")
        };
        Self::connect_addr(&uri, tls_connector).await
    }

    pub async fn connect_addr(
        uri: &str,
        tls_connector: TlsConnector,
    ) -> Result<(Self, mpsc::Receiver<Message>)> {
        let (ws, _) = tokio_tungstenite::connect_async_tls_with_config(
            uri,
            None,
            false,
            Some(Connector::NativeTls(tls_connector)),
        )
        .await?;
        Self::from_websocket(ws)
    }

    pub fn from_websocket(ws: WebSocket) -> Result<(Self, mpsc::Receiver<Message>)> {
        let (addr, cert) = match ws.get_ref() {
            MaybeTlsStream::NativeTls(tls) => {
                let tls_stream = tls.get_ref();
                let tcp_stream = tls_stream.get_ref().get_ref();
                (tcp_stream.peer_addr()?, tls_stream.peer_certificate()?)
            }
            _ => return Err(Error::MissingCertificate),
        };

        let Some(cert) = cert else {
            return Err(Error::MissingCertificate);
        };

        let mut hasher = Sha256::new();
        hasher.update(cert.to_der()?);

        let peer_id = PeerId(hasher.finalize_fixed().into());
        let (sink, stream) = ws.split();
        let (sender, receiver) = mpsc::channel(32);
        let requests = Arc::new(RequestMap::new());

        let inbound_handle =
            tokio::spawn(handle_inbound_messages(stream, sender, requests.clone()));

        let peer = Self(Arc::new(PeerInner {
            sink: Mutex::new(sink),
            inbound_handle,
            requests,
            peer_id,
            ip_addr: addr.ip(),
        }));

        Ok((peer, receiver))
    }

    pub fn peer_id(&self) -> PeerId {
        self.0.peer_id
    }

    pub fn ip_addr(&self) -> IpAddr {
        self.0.ip_addr
    }

    pub async fn send<T>(&self, body: T) -> Result<()>
    where
        T: Streamable + ChiaProtocolMessage,
    {
        let message = Message::new(T::msg_type(), None, body.to_bytes()?.into())
            .to_bytes()?
            .into();

        self.0.sink.lock().await.send(message).await?;

        Ok(())
    }

    pub async fn request_fallible<T, E, B>(&self, body: B) -> Result<Response<T, E>>
    where
        T: Streamable + ChiaProtocolMessage,
        E: Streamable + ChiaProtocolMessage,
        B: Streamable + ChiaProtocolMessage,
    {
        let message = self.request_raw(body).await?;
        if message.msg_type != T::msg_type() && message.msg_type != E::msg_type() {
            return Err(Error::InvalidResponse(
                vec![T::msg_type(), E::msg_type()],
                message.msg_type,
            ));
        }
        if message.msg_type == T::msg_type() {
            Ok(Response::Success(T::from_bytes(&message.data)?))
        } else {
            Ok(Response::Rejection(E::from_bytes(&message.data)?))
        }
    }

    pub async fn request_infallible<T, B>(&self, body: B) -> Result<T>
    where
        T: Streamable + ChiaProtocolMessage,
        B: Streamable + ChiaProtocolMessage,
    {
        let message = self.request_raw(body).await?;
        if message.msg_type != T::msg_type() {
            return Err(Error::InvalidResponse(
                vec![T::msg_type()],
                message.msg_type,
            ));
        }
        Ok(T::from_bytes(&message.data)?)
    }

    pub async fn request_raw<T>(&self, body: T) -> Result<Message>
    where
        T: Streamable + ChiaProtocolMessage,
    {
        let (sender, receiver) = oneshot::channel();

        let message = Message {
            msg_type: T::msg_type(),
            id: Some(self.0.requests.insert(sender).await),
            data: body.to_bytes()?.into(),
        }
        .to_bytes()?
        .into();

        self.0.sink.lock().await.send(message).await?;
        Ok(receiver.await?)
    }
}

impl Drop for PeerInner {
    fn drop(&mut self) {
        self.inbound_handle.abort();
    }
}

async fn handle_inbound_messages(
    mut stream: Stream,
    sender: mpsc::Sender<Message>,
    requests: Arc<RequestMap>,
) -> Result<()> {
    while let Some(message) = stream.next().await {
        let message = Message::from_bytes(&message?.into_data())?;

        let Some(id) = message.id else {
            sender.send(message).await.map_err(|error| {
                log::debug!("Failed to send peer message event: {error}");
                Error::EventNotSent
            })?;
            continue;
        };

        let Some(request) = requests.remove(id).await else {
            log::warn!(
                "Received {:?} message with untracked id {id}",
                message.msg_type
            );
            return Err(Error::UnexpectedMessage(message.msg_type));
        };

        request.send(message);
    }
    Ok(())
}
