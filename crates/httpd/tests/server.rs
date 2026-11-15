//! Talks raw HTTP/1.1 over TCP to the real server loop.

use std::sync::Arc;
use std::time::Duration;

use httpd::{Request, Response, Router};
use smol::io::{AsyncReadExt, AsyncWriteExt};
use smol::net::{TcpListener, TcpStream};
use smol::stream::StreamExt;

async fn start() -> std::net::SocketAddr {
    let mut r = Router::new();
    r.get("/hello", (), |_, _| async { Response::text(200, "hi") });
    r.post("/echo", (), |_, req: Request| async move {
        Response::new(200).with_body(req.body().to_vec(), "application/octet-stream")
    });
    r.get("/events", (), |_, _| async {
        let chunks = smol::stream::iter(vec![b"data: 1\n\n".to_vec(), b"data: 2\n\n".to_vec()]);
        Response::event_stream(chunks)
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    smol::spawn(httpd::serve(listener, Arc::new(r))).detach();
    addr
}

/// Read until the peer closes or `until` appears.
async fn read_until(stream: &mut TcpStream, until: &str) -> String {
    let mut out = Vec::new();
    let mut buf = [0u8; 1024];
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !String::from_utf8_lossy(&out).contains(until) {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out; got {:?}",
            String::from_utf8_lossy(&out)
        );
        let n = stream.read(&mut buf).await.unwrap();
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }
    String::from_utf8(out).unwrap()
}

#[test]
fn serves_get_with_content_length() {
    smol::block_on(async {
        let addr = start().await;
        let mut s = TcpStream::connect(addr).await.unwrap();
        s.write_all(b"GET /hello HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let resp = read_until(&mut s, "\u{0}").await;
        assert!(resp.starts_with("HTTP/1.1 200 OK\r\n"), "{resp}");
        assert!(resp.contains("content-length: 2\r\n"), "{resp}");
        assert!(resp.ends_with("\r\n\r\nhi"), "{resp}");
    });
}

#[test]
fn keeps_connection_alive_between_requests() {
    smol::block_on(async {
        let addr = start().await;
        let mut s = TcpStream::connect(addr).await.unwrap();
        s.write_all(b"GET /hello HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        let first = read_until(&mut s, "hi").await;
        assert!(first.starts_with("HTTP/1.1 200"), "{first}");
        s.write_all(b"GET /missing HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        let second = read_until(&mut s, "\r\n\r\n").await;
        assert!(second.starts_with("HTTP/1.1 404"), "{second}");
    });
}

#[test]
fn reads_body_and_honours_expect_continue() {
    smol::block_on(async {
        let addr = start().await;
        let mut s = TcpStream::connect(addr).await.unwrap();
        s.write_all(
            b"POST /echo HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\nExpect: 100-continue\r\n\r\n",
        )
        .await
        .unwrap();
        let interim = read_until(&mut s, "\r\n\r\n").await;
        assert!(interim.starts_with("HTTP/1.1 100 Continue"), "{interim}");
        s.write_all(b"hello").await.unwrap();
        let resp = read_until(&mut s, "hello").await;
        assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
    });
}

#[test]
fn body_split_across_packets_is_reassembled() {
    smol::block_on(async {
        let addr = start().await;
        let mut s = TcpStream::connect(addr).await.unwrap();
        s.write_all(b"POST /echo HTTP/1.1\r\nContent-Length: 6\r\n\r\nab")
            .await
            .unwrap();
        smol::Timer::after(Duration::from_millis(50)).await;
        s.write_all(b"cdef").await.unwrap();
        let resp = read_until(&mut s, "abcdef").await;
        assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
    });
}

#[test]
fn streams_server_sent_events_chunked() {
    smol::block_on(async {
        let addr = start().await;
        let mut s = TcpStream::connect(addr).await.unwrap();
        s.write_all(b"GET /events HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        let resp = read_until(&mut s, "0\r\n\r\n").await;
        assert!(
            resp.contains("content-type: text/event-stream\r\n"),
            "{resp}"
        );
        assert!(resp.contains("transfer-encoding: chunked\r\n"), "{resp}");
        assert!(
            resp.contains("9\r\ndata: 1\n\n\r\n9\r\ndata: 2\n\n\r\n"),
            "{resp}"
        );
    });
}

#[test]
fn malformed_request_gets_400_and_close() {
    smol::block_on(async {
        let addr = start().await;
        let mut s = TcpStream::connect(addr).await.unwrap();
        s.write_all(b"NOT HTTP\r\n\r\n").await.unwrap();
        let resp = read_until(&mut s, "\u{0}").await;
        assert!(resp.starts_with("HTTP/1.1 400"), "{resp}");
    });
}

#[test]
fn oversized_body_is_rejected_with_413() {
    smol::block_on(async {
        let addr = start().await;
        let mut s = TcpStream::connect(addr).await.unwrap();
        s.write_all(b"POST /echo HTTP/1.1\r\nContent-Length: 999999999\r\n\r\n")
            .await
            .unwrap();
        let resp = read_until(&mut s, "\u{0}").await;
        assert!(resp.starts_with("HTTP/1.1 413"), "{resp}");
    });
}

#[test]
fn many_concurrent_clients_are_served() {
    smol::block_on(async {
        let addr = start().await;
        let tasks: Vec<_> = (0..20)
            .map(|_| {
                smol::spawn(async move {
                    let mut s = TcpStream::connect(addr).await.unwrap();
                    s.write_all(b"GET /hello HTTP/1.1\r\nConnection: close\r\n\r\n")
                        .await
                        .unwrap();
                    read_until(&mut s, "\u{0}").await
                })
            })
            .collect();
        let mut stream = smol::stream::iter(tasks).then(|t| t);
        while let Some(resp) = stream.next().await {
            assert!(resp.ends_with("hi"), "{resp}");
        }
    });
}
