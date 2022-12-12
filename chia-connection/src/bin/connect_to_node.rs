use chia_protocol::ChiaProtocolMessage;
use chia_protocol::Handshake;
use chia_protocol::NodeType;
use chia_protocol::Streamable;
use chia_protocol::ProtocolMessageTypes;
use chia_protocol::Bytes32;
use chia_protocol::wallet_protocol::*;
use futures_util::{SinkExt, StreamExt};
use native_tls::{Certificate, Identity, Protocol, TlsConnector};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{connect_async_tls_with_config, Connector};
use std::io::Cursor;
use hex::FromHex;

//use rcgen::KeyPair;

const CHIA_CA: &'static str = "\
-----BEGIN CERTIFICATE-----
MIIDKTCCAhGgAwIBAgIUXIpxI5MoZQ65/vhc7DK/d5ymoMUwDQYJKoZIhvcNAQEL
BQAwRDENMAsGA1UECgwEQ2hpYTEQMA4GA1UEAwwHQ2hpYSBDQTEhMB8GA1UECwwY
T3JnYW5pYyBGYXJtaW5nIERpdmlzaW9uMB4XDTIxMDEyMzA4NTEwNloXDTMxMDEy
MTA4NTEwNlowRDENMAsGA1UECgwEQ2hpYTEQMA4GA1UEAwwHQ2hpYSBDQTEhMB8G
A1UECwwYT3JnYW5pYyBGYXJtaW5nIERpdmlzaW9uMIIBIjANBgkqhkiG9w0BAQEF
AAOCAQ8AMIIBCgKCAQEAzz/L219Zjb5CIKnUkpd2julGC+j3E97KUiuOalCH9wdq
gpJi9nBqLccwPCSFXFew6CNBIBM+CW2jT3UVwgzjdXJ7pgtu8gWj0NQ6NqSLiXV2
WbpZovfrVh3x7Z4bjPgI3ouWjyehUfmK1GPIld4BfUSQtPlUJ53+XT32GRizUy+b
0CcJ84jp1XvyZAMajYnclFRNNJSw9WXtTlMUu+Z1M4K7c4ZPwEqgEnCgRc0TCaXj
180vo7mCHJQoDiNSCRATwfH+kWxOOK/nePkq2t4mPSFaX8xAS4yILISIOWYn7sNg
dy9D6gGNFo2SZ0FR3x9hjUjYEV3cPqg3BmNE3DDynQIDAQABoxMwETAPBgNVHRMB
Af8EBTADAQH/MA0GCSqGSIb3DQEBCwUAA4IBAQAEugnFQjzHhS0eeCqUwOHmP3ww
/rXPkKF+bJ6uiQgXZl+B5W3m3zaKimJeyatmuN+5ST1gUET+boMhbA/7grXAsRsk
SFTHG0T9CWfPiuimVmGCzoxLGpWDMJcHZncpQZ72dcy3h7mjWS+U59uyRVHeiprE
hvSyoNSYmfvh7vplRKS1wYeA119LL5fRXvOQNW6pSsts17auu38HWQGagSIAd1UP
5zEvDS1HgvaU1E09hlHzlpdSdNkAx7si0DMzxKHUg9oXeRZedt6kcfyEmryd52Mj
1r1R9mf4iMIUv1zc2sHVc1omxnCw9+7U4GMWLtL5OgyJyfNyoxk3tC+D3KNU
-----END CERTIFICATE-----
";

const WALLET_CERT: &'static str = "";

// converted from our native format with:
// openssl pkcs8 -topk8 -inform PEM -outform PEM -in .../public_wallet.key -out wallet_key.pem -nocrypt
const WALLET_KEY: &'static str = "";

fn make_msg<T: Streamable + ChiaProtocolMessage>(msg: T, id: u16) -> Vec<u8> {
    let mut buf = Vec::<u8>::new();
    msg.stream(&mut buf);
    let mut ret = Vec::<u8>::new();
    chia_protocol::Message {
        msg_type: <T as ChiaProtocolMessage>::msg_type(),
        id: Some(id),
        data: buf.into(),
    }
    .stream(&mut ret);
    ret
}
/*
struct Connection {
    map request IDs to futures to be notified of the response
}
*/

macro_rules! handle {
    ($msg: ident, $data:ident, $( $name:ident),*)  => {
        match $msg.msg_type.try_into() {
            Err(e) => {
                println!("{:?}", e);
            },
            $(Ok(ProtocolMessageTypes::$name) => {
                let m = $name::parse(&mut $data);
                println!("{m:?}");
            },)*
            Ok(_) => {
                println!("unsupported message {:?}", $msg.msg_type);
            },
        }
    }
}

fn handle_incoming(msg: chia_protocol::Message) {
    let mut data = Cursor::<&[u8]>::new(msg.data.as_ref());
    handle!(msg, data, RequestPuzzleSolution,
        RespondPuzzleSolution,
        RejectPuzzleSolution,
        SendTransaction,
        TransactionAck,
        NewPeakWallet,
        RequestBlockHeader,
        RespondBlockHeader,
        RejectHeaderRequest,
        RequestRemovals,
        RespondRemovals,
        RejectRemovalsRequest,
        RequestAdditions,
        RespondAdditions,
        RejectAdditionsRequest,
        RespondBlockHeaders,
        RejectBlockHeaders,
        RequestBlockHeaders,
        RequestHeaderBlocks,
        RejectHeaderBlocks,
        RespondHeaderBlocks,
        RegisterForPhUpdates,
        RespondToPhUpdates,
        RegisterForCoinUpdates,
        RespondToCoinUpdates,
        CoinStateUpdate,
        RequestChildren,
        RespondChildren,
        RequestSesInfo,
        RespondSesInfo,
        RequestFeeEstimates,
        RespondFeeEstimates,
        Handshake);
}

#[tokio::main]
async fn main() {
    let url = url::Url::parse("wss://192.168.2.51:8444/ws").unwrap();

    //    let (stdin_tx, stdin_rx) = futures_channel::mpsc::unbounded();
    //    tokio::spawn(read_stdin(stdin_tx));

    // TODO: generate the wallet certificate rather than using a hard coded one

    // For python implementation, see ssl_context_for_client() in
    // chia/server/server.py in chia-blockchain
    let connector = TlsConnector::builder()
        .disable_built_in_roots(true)
        .add_root_certificate(
            Certificate::from_pem(CHIA_CA.as_bytes())
                .expect("internal error, failed to parse Chia CA"),
        )
        // the chia node use the empty string as hostname
        .danger_accept_invalid_hostnames(true)
        .use_sni(false)
        .identity(
            Identity::from_pkcs8(WALLET_CERT.as_bytes(), WALLET_KEY.as_bytes())
                .expect("failed to parse wallet certs"),
        )
        .min_protocol_version(Some(Protocol::Tlsv12))
        .build()
        .expect("internal error, failed to configure SSL context");

    println!("connecting to: {url}");
    let (mut ws_stream, _) =
        connect_async_tls_with_config(url, None, Some(Connector::NativeTls(connector)))
            .await
            .expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");

    let (mut write, mut read) = ws_stream.split();

    let msg = Message::Binary(make_msg(Handshake {
        network_id: "mainnet".into(),
        protocol_version: "0.0.34".into(),
        software_version: "0.0.1".into(),
        server_port: 0,
        node_type: NodeType::Wallet,
        capabilities: Vec::<(u16, String)>::new(),
    }, 0));
    println!("sending {:?}", msg);
    write.send(msg).await;

    match read.next().await.unwrap() {
        Ok(Message::Binary(buf)) => {
            let mut data = Cursor::<&[u8]>::new(&buf);
            let msg = chia_protocol::Message::parse(&mut data).expect("invalid protocol message");
            handle_incoming(msg);
        },
        _ => { panic!("unexpected response"); }
    };

    let msg = Message::Binary(make_msg(RegisterForPhUpdates{
        puzzle_hashes: vec![Bytes32::from(Vec::from_hex("4BC6435B409BCBABE53870DAE0F03755F6AABB4594C5915EC983ACF12A5D1FBA").unwrap())],
        min_height: 0,
    }, 1));

    println!("sending {:?}", msg);
    write.send(msg).await;

    let mut msg_id = 0xffff;
    loop {
        match read.next().await.unwrap() {
            Ok(Message::Binary(buf)) => {
                let mut data = Cursor::<&[u8]>::new(&buf);
                let msg = chia_protocol::Message::parse(&mut data).expect("invalid protocol message");
                if let Some(id) = msg.id {
                    println!("id: {}", id);
                    msg_id = id;
                }
                handle_incoming(msg);
            },
            _ => { panic!("unexpected response"); }
        };
        if msg_id == 1 {
            break;
        }
    }
    /*

        write.

        let stdin_to_ws = stdin_rx.map(Ok).forward(write);
        let ws_to_stdout = {
            read.for_each(|message| async {
                let data = message.unwrap().into_data();
                tokio::io::stdout().write_all(&data).await.unwrap();
            })
        };
        pin_mut!(stdin_to_ws, ws_to_stdout);
        future::select(stdin_to_ws, ws_to_stdout).await;
    */
}
