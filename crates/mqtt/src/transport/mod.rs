//! Small MQTT 3.1.1 client on smol: connect + CONNACK, subscribe, QoS 1
//! publish, keepalive and dead-broker detection. Reconnecting is the
//! caller's job: on any error, drop the pair and call [`connect`] again.
//!
//! Like the rumqttc setup it replaces, sessions are clean and unacknowledged
//! publishes are not replayed after a reconnect; the caller republishes its
//! retained state on every new connection instead.

mod client;
mod connection;

pub use client::{ClientError, MqttClient};
pub use connection::{Connection, ConnectionError, Message};

use std::time::Duration;

use smol::future::FutureExt;
use smol::io::AsyncWriteExt;
use smol::net::TcpStream;

use crate::wire::{self, Incoming};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Outgoing frames buffered before `publish` starts waiting on the socket.
const OUTGOING_CAPACITY: usize = 256;

#[derive(Debug, Clone)]
pub struct Options {
    pub host: String,
    pub port: u16,
    pub client_id: String,
    pub keep_alive: Duration,
    pub username: Option<String>,
    pub password: Option<String>,
    /// Incoming frames larger than this are a protocol error.
    pub max_packet_size: usize,
}

#[derive(Debug)]
pub enum ConnectError {
    Io(std::io::Error),
    Timeout,
    /// Broker answered CONNACK with this non-zero return code.
    Refused(u8),
    Protocol(String),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Timeout => write!(f, "timed out waiting for broker"),
            Self::Refused(code) => write!(f, "broker refused connection (code {code})"),
            Self::Protocol(e) => write!(f, "protocol error: {e}"),
        }
    }
}

impl std::error::Error for ConnectError {}

/// Open a TCP connection, perform the CONNECT/CONNACK handshake and start
/// the writer task. The returned client handle and connection share it.
pub async fn connect(opts: &Options) -> Result<(MqttClient, Connection), ConnectError> {
    let handshake = async {
        let mut stream = TcpStream::connect((opts.host.as_str(), opts.port))
            .await
            .map_err(ConnectError::Io)?;
        stream.set_nodelay(true).map_err(ConnectError::Io)?;
        let keep_alive_secs = opts.keep_alive.as_secs().clamp(1, u16::MAX as u64) as u16;
        let packet = wire::connect(
            &opts.client_id,
            keep_alive_secs,
            true,
            opts.username.as_deref(),
            opts.password.as_deref(),
        );
        stream.write_all(&packet).await.map_err(ConnectError::Io)?;

        let mut conn = Connection::new(stream.clone(), opts);
        match conn.next_packet().await {
            Ok(Incoming::ConnAck { code: 0 }) => Ok((stream, conn)),
            Ok(Incoming::ConnAck { code }) => Err(ConnectError::Refused(code)),
            Ok(other) => Err(ConnectError::Protocol(format!(
                "expected CONNACK, got {other:?}"
            ))),
            Err(e) => Err(ConnectError::Protocol(e.to_string())),
        }
    };
    let timeout = async {
        smol::Timer::after(CONNECT_TIMEOUT).await;
        Err(ConnectError::Timeout)
    };
    let (stream, mut conn) = handshake.or(timeout).await?;

    let (client, writer) = MqttClient::spawn_writer(stream, opts.keep_alive, OUTGOING_CAPACITY);
    conn.attach(client.clone(), writer);
    Ok((client, conn))
}
