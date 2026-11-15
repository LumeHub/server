use std::time::Duration;

use smol::future::FutureExt;
use smol::io::AsyncReadExt;
use smol::net::TcpStream;

use super::{MqttClient, Options};
use crate::wire::{self, Incoming, WireError};

/// An application message received from the broker.
#[derive(Debug, Clone)]
pub struct Message {
    pub topic: String,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub enum ConnectionError {
    Io(std::io::Error),
    Closed,
    /// Nothing heard from the broker for 1.5x the keepalive interval.
    Timeout,
    TooLarge(usize),
    Protocol(WireError),
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Closed => write!(f, "broker closed the connection"),
            Self::Timeout => write!(f, "broker stopped responding"),
            Self::TooLarge(n) => write!(f, "incoming packet of {n} bytes exceeds limit"),
            Self::Protocol(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ConnectionError {}

/// Read half of a connection. Dropping it stops the writer task too.
pub struct Connection {
    stream: TcpStream,
    buf: Vec<u8>,
    read_timeout: Duration,
    max_packet_size: usize,
    client: Option<MqttClient>,
    _writer: Option<smol::Task<()>>,
}

impl Connection {
    pub(super) fn new(stream: TcpStream, opts: &Options) -> Self {
        Self {
            stream,
            buf: Vec::new(),
            read_timeout: opts.keep_alive + opts.keep_alive / 2,
            max_packet_size: opts.max_packet_size,
            client: None,
            _writer: None,
        }
    }

    pub(super) fn attach(&mut self, client: MqttClient, writer: smol::Task<()>) {
        self.client = Some(client);
        self._writer = Some(writer);
    }

    /// Wait for the next application message, acknowledging QoS 1 deliveries
    /// and absorbing control packets (PINGRESP, PUBACK, SUBACK) on the way.
    pub async fn next_publish(&mut self) -> Result<Message, ConnectionError> {
        loop {
            if let Incoming::Publish {
                topic,
                payload,
                pid,
            } = self.next_packet().await?
            {
                if let (Some(pid), Some(client)) = (pid, &self.client) {
                    client
                        .send(wire::puback(pid))
                        .await
                        .map_err(|_| ConnectionError::Closed)?;
                }
                return Ok(Message { topic, payload });
            }
        }
    }

    pub(super) async fn next_packet(&mut self) -> Result<Incoming, ConnectionError> {
        loop {
            if let Some(len) = wire::frame_len(&self.buf).map_err(ConnectionError::Protocol)?
                && len > self.max_packet_size
            {
                return Err(ConnectionError::TooLarge(len));
            }
            if let Some((packet, used)) =
                wire::decode(&self.buf).map_err(ConnectionError::Protocol)?
            {
                self.buf.drain(..used);
                return Ok(packet);
            }
            self.fill().await?;
        }
    }

    async fn fill(&mut self) -> Result<(), ConnectionError> {
        let mut chunk = [0u8; 4096];
        let read = async {
            self.stream
                .read(&mut chunk)
                .await
                .map_err(ConnectionError::Io)
        };
        let timeout = async {
            smol::Timer::after(self.read_timeout).await;
            Err(ConnectionError::Timeout)
        };
        let n = read.or(timeout).await?;
        if n == 0 {
            return Err(ConnectionError::Closed);
        }
        self.buf.extend_from_slice(&chunk[..n]);
        Ok(())
    }
}
