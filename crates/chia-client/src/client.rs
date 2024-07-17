use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use chia_protocol::{
    Handshake, Message, NodeType, ProtocolMessageTypes, RequestPeers, RespondPeers,
};
use chia_traits::Streamable;
use dns_lookup::lookup_host;
use futures_util::{stream::FuturesUnordered, StreamExt};
use native_tls::TlsConnector;
use rand::{seq::SliceRandom, thread_rng};
use semver::Version;
use tokio::{
    sync::{mpsc, Mutex, Semaphore},
    time::timeout,
};

use crate::{Error, Event, Network, Peer, PeerId, Result};

/// A client that can connect to many different peers on the network.
#[derive(Debug, Clone)]
pub struct Client(Arc<ClientInner>);

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

#[derive(Debug)]
struct ClientInner {
    peers: Arc<Mutex<HashMap<PeerId, Peer>>>,
    message_sender: Arc<Mutex<mpsc::Sender<Event>>>,
    options: ClientOptions,
    tls_connector: TlsConnector,
    connection_lock: Semaphore,
}

impl Client {
    pub fn new(
        tls_connector: TlsConnector,
        options: ClientOptions,
    ) -> (Self, mpsc::Receiver<Event>) {
        let (sender, receiver) = mpsc::channel(32);

        let client = Self(Arc::new(ClientInner {
            peers: Arc::new(Mutex::new(HashMap::new())),
            message_sender: Arc::new(Mutex::new(sender)),
            options,
            tls_connector,
            connection_lock: Semaphore::new(1),
        }));

        (client, receiver)
    }

    pub async fn len(&self) -> usize {
        self.0.peers.lock().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.0.peers.lock().await.is_empty()
    }

    pub async fn peer_ids(&self) -> Vec<PeerId> {
        self.0.peers.lock().await.keys().copied().collect()
    }

    pub async fn peers(&self) -> Vec<Peer> {
        self.0.peers.lock().await.values().cloned().collect()
    }

    pub async fn peer(&self, peer_id: PeerId) -> Option<Peer> {
        self.0.peers.lock().await.get(&peer_id).cloned()
    }

    pub async fn remove_peer(&self, peer_id: PeerId) -> Option<Peer> {
        self.0.peers.lock().await.remove(&peer_id)
    }

    pub async fn clear(&self) {
        self.0.peers.lock().await.clear();
    }

    pub async fn find_peers(&self, prefer_introducers: bool) {
        let _permit = self
            .0
            .connection_lock
            .acquire()
            .await
            .expect("the semaphore should not be closed");

        if self.len().await >= self.0.options.target_peers {
            return;
        }

        // If we don't have any peers, try to connect to DNS introducers.
        if self.is_empty().await || prefer_introducers {
            self.connect_dns().await;

            // If we still don't have any peers, we can't do anything.
            if self.is_empty().await {
                return;
            }
        }

        if self.len().await >= self.0.options.target_peers {
            return;
        }

        if self.is_empty().await {
            log::error!("No peers connected after DNS lookups");
            return;
        }

        let peer_lock = self.0.peers.lock().await;
        let peers = peer_lock.clone();
        drop(peer_lock);

        for (peer_id, peer) in peers {
            if self.len().await >= self.0.options.target_peers {
                break;
            }

            // Request new peers from the peer.
            let Ok(Ok(response)): std::result::Result<Result<RespondPeers>, _> = timeout(
                self.0.options.request_peers_timeout,
                peer.request_infallible(RequestPeers::new()),
            )
            .await
            else {
                log::info!("Failed to request peers from {}", peer.ip_addr());
                self.remove_peer(peer_id).await;
                continue;
            };

            log::info!("Requested peers from {}", peer.ip_addr());

            let mut ips = HashSet::new();

            for item in response.peer_list {
                // If we can't parse the IP address, skip it.
                let Ok(ip_addr) = IpAddr::from_str(&item.host) else {
                    log::debug!("Failed to parse IP address {}", item.host);
                    continue;
                };

                ips.insert((ip_addr, item.port));
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

                self.connect_peers(next_peers).await;
            }
        }
    }

    async fn connect_dns(&self) {
        log::info!("Requesting peers from DNS introducer");

        let mut ips = Vec::new();

        for dns_introducer in &self.0.options.network.dns_introducers {
            // If a DNS introducer lookup fails, we just skip it.
            let Ok(result) = lookup_host(dns_introducer) else {
                log::warn!("Failed to lookup DNS introducer `{dns_introducer}`");
                continue;
            };
            ips.extend(result);
        }

        // Shuffle the list of IPs so that we don't always connect to the same ones.
        // This also prevents bias towards IPv4 or IPv6.
        ips.as_mut_slice().shuffle(&mut thread_rng());

        // Keep track of where we are in the peer list.
        let mut cursor = 0;

        while self.len().await < self.0.options.target_peers {
            // If we've reached the end of the list of IPs, stop early.
            if cursor >= ips.len() {
                break;
            }

            // Get the remaining peers we can connect to, up to the concurrency limit.
            let peers_to_try = &ips[cursor
                ..ips
                    .len()
                    .min(cursor + self.0.options.connection_concurrency)];

            // Increment the cursor by the number of peers we're trying to connect to.
            cursor += peers_to_try.len();

            self.connect_peers(
                peers_to_try
                    .iter()
                    .map(|ip| (*ip, self.0.options.network.default_port))
                    .collect(),
            )
            .await;
        }
    }

    async fn connect_peers(&self, potential_ips: Vec<(IpAddr, u16)>) {
        let peer_lock = self.0.peers.lock().await;
        let peers = peer_lock.clone();
        drop(peer_lock);

        // Add the connections and wait for them to complete.
        let mut connections = FuturesUnordered::new();

        for (ip, port) in potential_ips {
            if peers.iter().any(|(_, peer)| peer.ip_addr() == ip) {
                continue;
            }

            connections.push(async move {
                self.connect_peer(ip, port)
                    .await
                    .map_err(|error| (ip, port, error))
            });
        }

        while let Some(result) = connections.next().await {
            if self.len().await >= self.0.options.target_peers {
                break;
            }

            let (peer, mut receiver) = match result {
                Ok(result) => result,
                Err((ip, port, error)) => {
                    log::warn!(
                        "{error} for peer {}",
                        if ip.is_ipv4() {
                            format!("{ip}:{port}")
                        } else {
                            format!("[{ip}]:{port}")
                        }
                    );
                    continue;
                }
            };

            let ip = peer.ip_addr();
            let peer_id = peer.peer_id();
            self.0.peers.lock().await.insert(peer_id, peer);

            let message_sender = self.0.message_sender.clone();
            let peer_map = self.0.peers.clone();

            // Spawn a task to propagate messages from the peer.
            tokio::spawn(async move {
                while let Some(message) = receiver.recv().await {
                    if let Err(error) = message_sender
                        .lock()
                        .await
                        .send(Event::Message(peer_id, message))
                        .await
                    {
                        log::warn!("Failed to send client message event: {error}");
                        break;
                    }
                }

                peer_map.lock().await.remove(&peer_id);

                if let Err(error) = message_sender
                    .lock()
                    .await
                    .send(Event::ConnectionClosed(peer_id))
                    .await
                {
                    log::warn!("Failed to send client connection closed event: {error}");
                }

                log::info!("Peer {ip} disconnected");
            });

            log::info!("Connected to peer {ip}");
        }
    }

    async fn connect_peer(&self, ip: IpAddr, port: u16) -> Result<(Peer, mpsc::Receiver<Message>)> {
        log::debug!("Connecting to peer {ip}");

        let (peer, mut receiver) = timeout(
            self.0.options.connection_timeout,
            Peer::connect(ip, port, self.0.tls_connector.clone()),
        )
        .await
        .map_err(Error::ConnectionTimeout)??;

        let options = &self.0.options;

        peer.send(Handshake {
            network_id: options.network.network_id.clone(),
            protocol_version: options.protocol_version.to_string(),
            software_version: options.software_version.clone(),
            server_port: 0,
            node_type: options.node_type,
            capabilities: options.capabilities.clone(),
        })
        .await?;

        let Some(message) = timeout(options.handshake_timeout, receiver.recv())
            .await
            .map_err(Error::HandshakeTimeout)?
        else {
            return Err(Error::ExpectedHandshake);
        };

        if message.msg_type != ProtocolMessageTypes::Handshake {
            return Err(Error::ExpectedHandshake);
        };

        let handshake = Handshake::from_bytes(&message.data)?;

        if handshake.network_id != options.network.network_id {
            return Err(Error::WrongNetworkId(handshake.network_id));
        }

        let Ok(protocol_version) = Version::parse(&handshake.protocol_version) else {
            return Err(Error::InvalidProtocolVersion(handshake.protocol_version));
        };

        if protocol_version < options.protocol_version {
            return Err(Error::OutdatedProtocolVersion(
                protocol_version,
                options.protocol_version.clone(),
            ));
        }

        Ok((peer, receiver))
    }
}
