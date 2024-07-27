use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    ops::Deref,
    str::FromStr,
    sync::{Arc, Weak},
    time::Duration,
};

use chia_protocol::{Handshake, Message, NodeType, ProtocolMessageTypes, RespondPeers};
use chia_traits::Streamable;
use dns_lookup::lookup_host;
use futures_util::{stream::FuturesUnordered, StreamExt};
use native_tls::TlsConnector;
use rand::{seq::SliceRandom, thread_rng};
use semver::Version;
use tokio::{
    sync::{mpsc, Mutex},
    time::timeout,
};

use crate::{Error, Event, Network, Peer, PeerId, Result};

#[derive(Debug, Clone)]
pub struct ClientOptions {
    /// The network to connect to. By default, this is mainnet.
    pub network: Network,
    /// The type of service that this client represents.
    pub node_type: NodeType,
    /// The capabilities that this client supports.
    pub capabilities: Vec<(u16, String)>,
    /// The minimum protocol version that this client supports.
    /// If the protocol version of peers are lower than this, they will be disconnected.
    pub protocol_version: Version,
    /// The software version of this client.
    /// This is not important for the handshake, but is sent to the peer for informational purposes.
    pub software_version: String,
    /// The ideal number of peers that should be connected at any given time.
    pub target_peers: usize,
    /// The maximum number of concurrent connections that can be initiated at once.
    pub connection_concurrency: usize,
    /// How long to wait when trying to connect to a peer.
    pub connection_timeout: Duration,
    /// How long to wait for a handshake response from a peer before disconnecting.
    pub handshake_timeout: Duration,
    /// How long to wait for a response to a request for peers.
    pub request_peers_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct Client(Arc<ClientInner>);

impl Deref for Client {
    type Target = Mutex<ClientState>;

    fn deref(&self) -> &Self::Target {
        &self.0.state
    }
}

#[derive(Debug)]
struct ClientInner {
    state: Arc<Mutex<ClientState>>,
    options: ClientOptions,
    tls_connector: TlsConnector,
}

/// A client that can connect to many different peers on the network.
#[derive(Debug)]
pub struct ClientState {
    peers: HashMap<PeerId, Peer>,
    sender: mpsc::Sender<Event>,
    banned_peers: HashSet<IpAddr>,
    trusted_peers: HashSet<IpAddr>,
}

impl Client {
    pub fn new(
        tls_connector: TlsConnector,
        options: ClientOptions,
    ) -> (Self, mpsc::Receiver<Event>) {
        let (sender, receiver) = mpsc::channel(32);

        let state = ClientState {
            peers: HashMap::new(),
            sender,
            banned_peers: HashSet::new(),
            trusted_peers: HashSet::new(),
        };

        let client = Self(Arc::new(ClientInner {
            state: Arc::new(Mutex::new(state)),
            options,
            tls_connector,
        }));

        (client, receiver)
    }

    pub async fn discover_peers(&self, prefer_introducers: bool) {
        if self.lock().await.peers.len() >= self.0.options.target_peers {
            return;
        }

        // If we don't have any peers, try to connect to DNS introducers.
        if self.lock().await.peers.is_empty() || prefer_introducers {
            self.discover_peers_with_dns().await;

            // If we still don't have any peers, we can't do anything.
            if self.lock().await.peers.is_empty() {
                return;
            }
        }

        if self.lock().await.peers.len() >= self.0.options.target_peers {
            return;
        }

        if self.lock().await.peers.is_empty() {
            log::error!("No peers connected after DNS lookups");
            return;
        }

        for (peer_id, peer) in self.lock().await.peers.clone() {
            if self.lock().await.peers.len() >= self.0.options.target_peers {
                break;
            }

            // Request new peers from the peer.
            let Ok(Ok(response)): std::result::Result<Result<RespondPeers>, _> =
                timeout(self.0.options.request_peers_timeout, peer.request_peers()).await
            else {
                log::info!("Failed to request peers from {}", peer.socket_addr());
                self.lock().await.disconnect_peer(&peer_id);
                continue;
            };

            log::info!("Requested peers from {}", peer.socket_addr());

            let mut ips = HashSet::new();

            for item in response.peer_list {
                // If we can't parse the IP address, skip it.
                let Ok(ip_addr) = IpAddr::from_str(&item.host) else {
                    log::debug!("Failed to parse IP address {}", item.host);
                    continue;
                };
                ips.insert(SocketAddr::new(ip_addr, item.port));
            }

            // Keep connecting peers until the peer list is exhausted,
            // then move on to the next peer to request from.
            let mut iter = ips.into_iter();

            loop {
                let next_peers: Vec<_> = iter
                    .by_ref()
                    .take(self.0.options.connection_concurrency)
                    .collect();

                if next_peers.is_empty() {
                    break;
                }

                self.connect_peers(&next_peers).await;
            }
        }
    }

    pub async fn discover_peers_with_dns(&self) -> HashMap<SocketAddr, PeerId> {
        let mut socket_addrs: Vec<SocketAddr> = self.dns_lookup().into_iter().collect();

        // Shuffle the list of IPs so that we don't always connect to the same ones.
        // This also prevents bias towards IPv4 or IPv6.
        socket_addrs.as_mut_slice().shuffle(&mut thread_rng());

        self.connect_peers_batched(&socket_addrs).await
    }

    pub fn dns_lookup(&self) -> HashSet<SocketAddr> {
        let mut socket_addrs = HashSet::new();

        for dns_introducer in &self.0.options.network.dns_introducers {
            log::debug!("Performing DNS lookup of {dns_introducer}.");

            // If a DNS introducer lookup fails, we just skip it.
            let Ok(result) = lookup_host(dns_introducer) else {
                log::warn!("Failed to lookup DNS introducer `{dns_introducer}`");
                continue;
            };

            socket_addrs.extend(
                result
                    .into_iter()
                    .map(|ip| SocketAddr::new(ip, self.0.options.network.default_port)),
            );
        }

        log::info!(
            "Found a total of {} IPs from DNS introducers.",
            socket_addrs.len()
        );

        socket_addrs
    }

    pub async fn connect_peers_batched(
        &self,
        socket_addrs: &[SocketAddr],
    ) -> HashMap<SocketAddr, PeerId> {
        let mut peer_ids = HashMap::new();
        let mut cursor = 0;

        while self.lock().await.peers.len() < self.0.options.target_peers {
            if cursor >= socket_addrs.len() {
                break;
            }

            // Get the remaining peers we can connect to, up to the concurrency limit.
            let new_addrs = &socket_addrs[cursor
                ..socket_addrs
                    .len()
                    .min(cursor + self.0.options.connection_concurrency)];

            // Increment the cursor by the number of peers we're trying to connect to.
            cursor += new_addrs.len();

            peer_ids.extend(self.connect_peers(new_addrs).await);
        }

        peer_ids
    }

    pub async fn connect_peers(&self, socket_addrs: &[SocketAddr]) -> HashMap<SocketAddr, PeerId> {
        let mut connections = FuturesUnordered::new();

        let state = self.lock().await;

        for &socket_addr in socket_addrs {
            // Skip peers which we are already connected to.
            if state
                .peers
                .iter()
                .any(|(_, peer)| peer.socket_addr().ip() == socket_addr.ip())
            {
                continue;
            }

            // Add the next connection to the queue.
            connections.push(async move {
                let result = self.connect_peer(socket_addr).await;
                (socket_addr, result)
            });
        }

        // Prevent a deadlock and allow the connections to resolve.
        drop(state);

        let mut peer_ids = HashMap::new();

        while let Some((socket_addr, result)) = connections.next().await {
            match result {
                Err(error) => {
                    log::warn!("Failed to connect to peer {socket_addr} with error: {error}",);
                }
                Ok(peer_id) => {
                    peer_ids.insert(socket_addr, peer_id);
                    log::info!("Connected to peer {socket_addr}");
                }
            }
        }

        peer_ids
    }

    pub async fn connect_peer(&self, socket_addr: SocketAddr) -> Result<PeerId> {
        log::debug!("Connecting to peer {socket_addr}");

        let (peer, mut receiver) = timeout(
            self.0.options.connection_timeout,
            Peer::connect(socket_addr, self.0.tls_connector.clone()),
        )
        .await
        .map_err(Error::ConnectionTimeout)??;

        peer.send(Handshake {
            network_id: self.0.options.network.network_id.clone(),
            protocol_version: self.0.options.protocol_version.to_string(),
            software_version: self.0.options.software_version.clone(),
            server_port: 0,
            node_type: self.0.options.node_type,
            capabilities: self.0.options.capabilities.clone(),
        })
        .await?;

        let Some(message) = timeout(self.0.options.handshake_timeout, receiver.recv())
            .await
            .map_err(Error::HandshakeTimeout)?
        else {
            return Err(Error::ExpectedHandshake);
        };

        if message.msg_type != ProtocolMessageTypes::Handshake {
            return Err(Error::ExpectedHandshake);
        };

        let handshake = Handshake::from_bytes(&message.data)?;

        if handshake.network_id != self.0.options.network.network_id {
            return Err(Error::WrongNetworkId(handshake.network_id));
        }

        let Ok(protocol_version) = Version::parse(&handshake.protocol_version) else {
            return Err(Error::InvalidProtocolVersion(handshake.protocol_version));
        };

        if protocol_version < self.0.options.protocol_version {
            return Err(Error::OutdatedProtocolVersion(
                protocol_version,
                self.0.options.protocol_version.clone(),
            ));
        }

        self.insert_peer(peer, receiver).await
    }

    pub async fn insert_peer(
        &self,
        peer: Peer,
        receiver: mpsc::Receiver<Message>,
    ) -> Result<PeerId> {
        let mut state = self.lock().await;
        state.peers.insert(peer.peer_id(), peer.clone());
        state.sender.send(Event::Connected(peer.peer_id())).await?;

        // Spawn a task to propagate messages from the peer.
        // We downgrade the client to avoid a cycle and allow it to be dropped.
        tokio::spawn(handle_peer_connection(
            Arc::downgrade(&self.0.state),
            peer.peer_id(),
            peer.socket_addr(),
            receiver,
        ));

        Ok(peer.peer_id())
    }
}

impl ClientState {
    pub fn peers(&self) -> &HashMap<PeerId, Peer> {
        &self.peers
    }

    pub fn disconnect_peer(&mut self, peer_id: &PeerId) {
        self.peers.remove(peer_id);
    }

    pub fn disconnect_all(&mut self) {
        self.peers.clear();
    }

    pub fn banned_peers(&self) -> &HashSet<IpAddr> {
        &self.banned_peers
    }

    pub fn is_banned(&self, ip_addr: &IpAddr) -> bool {
        self.banned_peers.contains(ip_addr)
    }

    pub fn ban_peer(&mut self, ip_addr: IpAddr) {
        self.banned_peers.insert(ip_addr);
    }

    pub fn unban_peer(&mut self, ip_addr: &IpAddr) {
        self.banned_peers.remove(ip_addr);
    }

    pub fn trusted_peers(&self) -> &HashSet<IpAddr> {
        &self.trusted_peers
    }

    pub fn is_trusted(&self, ip_addr: &IpAddr) -> bool {
        self.trusted_peers.contains(ip_addr)
    }

    pub fn trust_peer(&mut self, ip_addr: IpAddr) {
        self.trusted_peers.insert(ip_addr);
    }

    pub fn untrust_peer(&mut self, ip_addr: &IpAddr) {
        self.trusted_peers.remove(ip_addr);
    }
}

async fn handle_peer_connection(
    state: Weak<Mutex<ClientState>>,
    peer_id: PeerId,
    socket_addr: SocketAddr,
    mut receiver: mpsc::Receiver<Message>,
) {
    while let Some(message) = receiver.recv().await {
        // If the client has been dropped, we should gracefully end the task.
        let Some(state) = state.upgrade() else {
            return;
        };
        let state = state.lock().await;

        // Close the connection if an error occurs.
        if let Err(error) = state.sender.send(Event::Message(peer_id, message)).await {
            log::warn!("Failed to send client message event: {error}");
            break;
        }
    }

    // If the client has been dropped, we should gracefully end the task.
    let Some(state) = state.upgrade() else {
        return;
    };
    let mut state = state.lock().await;

    state.peers.remove(&peer_id);

    log::info!("Peer {socket_addr} disconnected");

    if let Err(error) = state.sender.send(Event::Disconnected(socket_addr)).await {
        log::warn!("Failed to send client connection closed event: {error}");
    }
}
