//! Drives the client against an in-process fake broker on localhost.

use std::time::Duration;

use mqtt::transport::{self, ConnectError, ConnectionError, Options};
use mqtt::wire::{self, Incoming};
use smol::io::{AsyncReadExt, AsyncWriteExt};
use smol::net::{TcpListener, TcpStream};

const CONNECT: u8 = 1;
const SUBSCRIBE: u8 = 8;
const PINGREQ: u8 = 12;

struct Broker {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl Broker {
    async fn next(&mut self) -> Incoming {
        loop {
            if let Some((p, used)) = wire::decode(&self.buf).unwrap() {
                self.buf.drain(..used);
                return p;
            }
            let mut chunk = [0u8; 4096];
            let n = self.stream.read(&mut chunk).await.unwrap();
            assert!(n > 0, "client closed the connection");
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    async fn send(&mut self, bytes: &[u8]) {
        self.stream.write_all(bytes).await.unwrap();
    }
}

async fn listen() -> (TcpListener, Options) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let opts = Options {
        host: "127.0.0.1".into(),
        port,
        client_id: "test".into(),
        keep_alive: Duration::from_secs(60),
        username: None,
        password: None,
        max_packet_size: 1024 * 1024,
    };
    (listener, opts)
}

async fn accept(listener: &TcpListener, connack_code: u8) -> Broker {
    let (stream, _) = listener.accept().await.unwrap();
    let mut b = Broker {
        stream,
        buf: Vec::new(),
    };
    assert_eq!(
        b.next().await,
        Incoming::Other {
            packet_type: CONNECT
        }
    );
    b.send(&[0x20, 2, 0, connack_code]).await;
    b
}

#[test]
fn connects_subscribes_and_exchanges_publishes() {
    smol::block_on(async {
        let (listener, opts) = listen().await;
        let broker = smol::spawn(async move {
            let mut b = accept(&listener, 0).await;
            assert_eq!(
                b.next().await,
                Incoming::Other {
                    packet_type: SUBSCRIBE
                }
            );
            // Command from HA, QoS 1: the client must acknowledge it.
            b.send(&wire::publish("lumehub/device/set", b"{}", false, Some(42)))
                .await;
            assert_eq!(b.next().await, Incoming::PubAck { pid: 42 });
            // State published by the client.
            match b.next().await {
                Incoming::Publish {
                    topic,
                    payload,
                    pid,
                } => {
                    assert_eq!(topic, "lumehub/device/state");
                    assert_eq!(payload, b"on");
                    assert!(pid.is_some(), "client publishes with QoS 1");
                    b.send(&[0x40, 2, 0, pid.unwrap() as u8]).await;
                }
                other => panic!("expected publish, got {other:?}"),
            }
        });

        let (client, mut conn) = transport::connect(&opts).await.unwrap();
        client.subscribe(&["lumehub/device/set"]).await.unwrap();
        let msg = conn.next_publish().await.unwrap();
        assert_eq!(msg.topic, "lumehub/device/set");
        assert_eq!(msg.payload, b"{}");
        client
            .publish("lumehub/device/state", true, b"on")
            .await
            .unwrap();
        broker.await;
    });
}

#[test]
fn refused_connack_is_reported() {
    smol::block_on(async {
        let (listener, opts) = listen().await;
        let broker = smol::spawn(async move {
            let _b = accept(&listener, 5).await;
            smol::Timer::after(Duration::from_millis(200)).await;
        });
        let err = transport::connect(&opts).await.err().expect("must fail");
        assert!(matches!(err, ConnectError::Refused(5)), "{err}");
        broker.await;
    });
}

#[test]
fn idle_client_pings_and_detects_dead_broker() {
    smol::block_on(async {
        let (listener, mut opts) = listen().await;
        opts.keep_alive = Duration::from_millis(400);
        let broker = smol::spawn(async move {
            let mut b = accept(&listener, 0).await;
            // Never answer the ping: the client must give up on its own.
            assert_eq!(
                b.next().await,
                Incoming::Other {
                    packet_type: PINGREQ
                }
            );
            smol::Timer::after(Duration::from_secs(2)).await;
        });

        let (_client, mut conn) = transport::connect(&opts).await.unwrap();
        let started = std::time::Instant::now();
        let err = conn.next_publish().await.expect_err("must time out");
        assert!(matches!(err, ConnectionError::Timeout), "{err}");
        assert!(started.elapsed() < Duration::from_secs(2));
        broker.await;
    });
}

#[test]
fn oversized_packet_is_rejected() {
    smol::block_on(async {
        let (listener, mut opts) = listen().await;
        opts.max_packet_size = 64;
        let broker = smol::spawn(async move {
            let mut b = accept(&listener, 0).await;
            b.send(&wire::publish("t", &[0u8; 200], false, None)).await;
            smol::Timer::after(Duration::from_millis(500)).await;
        });

        let (_client, mut conn) = transport::connect(&opts).await.unwrap();
        let err = conn.next_publish().await.expect_err("must reject");
        assert!(matches!(err, ConnectionError::TooLarge(_)), "{err}");
        broker.await;
    });
}

#[test]
fn disconnected_handle_fails_fast() {
    let client = transport::MqttClient::disconnected();
    let res = smol::block_on(client.publish("t", false, b"x"));
    assert!(res.is_err());
}
