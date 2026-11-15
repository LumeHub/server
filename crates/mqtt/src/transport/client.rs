use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use smol::channel::{Receiver, Sender};
use smol::future::FutureExt;
use smol::io::AsyncWriteExt;
use smol::net::TcpStream;

use crate::wire;

/// Cloneable handle for sending to the broker. All frames go through one
/// writer task, so handlers can publish while the connection is reading.
#[derive(Clone)]
pub struct MqttClient {
    tx: Sender<Vec<u8>>,
    next_pid: Arc<AtomicU16>,
}

#[derive(Debug)]
pub struct ClientError;

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mqtt connection closed")
    }
}

impl std::error::Error for ClientError {}

impl MqttClient {
    /// A handle whose connection is already gone; every call fails.
    /// Useful for tests that only check side effects outside MQTT.
    pub fn disconnected() -> Self {
        let (tx, _) = smol::channel::bounded(1);
        Self {
            tx,
            next_pid: Arc::new(AtomicU16::new(1)),
        }
    }

    /// Publish with QoS 1.
    pub async fn publish(
        &self,
        topic: &str,
        retain: bool,
        payload: impl AsRef<[u8]>,
    ) -> Result<(), ClientError> {
        let frame = wire::publish(topic, payload.as_ref(), retain, Some(self.pid()));
        self.send(frame).await
    }

    /// Subscribe to all `topics` with QoS 1 in a single SUBSCRIBE.
    pub async fn subscribe(&self, topics: &[&str]) -> Result<(), ClientError> {
        self.send(wire::subscribe(self.pid(), topics, 1)).await
    }

    /// Ask the broker to close the session cleanly.
    pub async fn disconnect(&self) -> Result<(), ClientError> {
        self.send(wire::disconnect()).await
    }

    pub(super) async fn send(&self, frame: Vec<u8>) -> Result<(), ClientError> {
        self.tx.send(frame).await.map_err(|_| ClientError)
    }

    fn pid(&self) -> u16 {
        // Packet ids are 1..=65535; 0 is reserved.
        loop {
            let pid = self.next_pid.fetch_add(1, Ordering::Relaxed);
            if pid != 0 {
                return pid;
            }
        }
    }

    /// Start the task that owns the socket's write half. It sends PINGREQ
    /// whenever nothing was written for half the keepalive interval, and
    /// stops (closing the channel) on the first write error.
    pub(super) fn spawn_writer(
        mut stream: TcpStream,
        keep_alive: Duration,
        capacity: usize,
    ) -> (Self, smol::Task<()>) {
        let (tx, rx) = smol::channel::bounded(capacity);
        let task = smol::spawn(async move {
            let idle = keep_alive / 2;
            while let Some(frame) = next_frame(&rx, idle).await {
                if stream.write_all(&frame).await.is_err() {
                    break;
                }
            }
        });
        let client = Self {
            tx,
            next_pid: Arc::new(AtomicU16::new(1)),
        };
        (client, task)
    }
}

/// Next frame to write: a queued one, or a PINGREQ after `idle`. `None` once
/// every handle is dropped.
async fn next_frame(rx: &Receiver<Vec<u8>>, idle: Duration) -> Option<Vec<u8>> {
    let queued = async { rx.recv().await.ok() };
    let ping = async {
        smol::Timer::after(idle).await;
        Some(wire::pingreq())
    };
    queued.or(ping).await
}
