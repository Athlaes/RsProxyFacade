use super::*;
use crate::commands::ProxyModeCommand;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn failed_candidate_falls_back_to_the_next_proxy_before_direct() {
    let available = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = available.local_addr().unwrap();
    let unavailable = tokio::net::TcpSocket::new_v4().unwrap();
    unavailable.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let proxy = |port| ResolvedDestination::HttpProxy {
        host: "127.0.0.1".into(),
        port,
    };
    let routes = vec![
        proxy(unavailable.local_addr().unwrap().port()),
        proxy(address.port()),
        ResolvedDestination::Direct,
    ];
    let (route, stream) = ProxyServer::connect_routes(routes, "127.0.0.1", 0, "127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    assert!(matches!(route, ResolvedDestination::HttpProxy { port, .. } if port == address.port()));
    assert_eq!(stream.peer_addr().unwrap(), address);
}

async fn headers(stream: &mut TcpStream) -> String {
    timeout(Duration::from_secs(3), async {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).await.unwrap();
            bytes.push(byte[0]);
            assert!(bytes.len() < 16384);
        }
        String::from_utf8(bytes).unwrap()
    })
    .await
    .expect("HTTP headers timed out")
}

async fn start(mode: ProxyModeCommand) -> ProxyServer {
    ProxyServer::new(ProxyServerCommand {
        host: "127.0.0.1".into(),
        port: 0,
        proxy_mode: mode,
    })
    .await
    .unwrap()
}

fn manual(address: SocketAddr, bypass: &str) -> ProxyModeCommand {
    ProxyModeCommand::Manual {
        host: address.ip().to_string(),
        port: address.port(),
        bypass: bypass.into(),
    }
}

async fn exchange(server: &ProxyServer, request: &str) -> String {
    let mut stream = TcpStream::connect(server.runtime.listener).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    headers(&mut stream).await
}

#[tokio::test]
async fn http_routes_use_correct_target_and_strip_proxy_credentials() {
    for direct in [true, false] {
        let peer = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = peer.local_addr().unwrap();
        let mut server = start(if direct {
            ProxyModeCommand::Direct
        } else {
            manual(address, "")
        })
        .await;
        let target = if direct {
            address.to_string()
        } else {
            "origin.invalid".into()
        };
        let expected = if direct {
            "GET /page?q=1 HTTP/1.1".to_string()
        } else {
            format!("GET http://{target}/page?q=1 HTTP/1.1")
        };
        let fixture = tokio::spawn(async move {
            let (mut stream, _) = peer.accept().await.unwrap();
            let request = headers(&mut stream).await;
            assert!(request.starts_with(&expected), "{request}");
            assert!(!request.to_ascii_lowercase().contains("proxy-authorization"));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer origin-token")
            );
            stream
                .write_all(
                    b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=origin\r\nContent-Length: 0\r\n\r\n",
                )
                .await
                .unwrap();
        });
        let response = exchange(&server, &format!("GET http://{target}/page?q=1 HTTP/1.1\r\nHost: {target}\r\nProxy-Authorization: Basic browser-secret\r\nAuthorization: Bearer origin-token\r\n\r\n")).await;
        assert!(response.starts_with("HTTP/1.1 401"));
        assert!(response.to_ascii_lowercase().contains("www-authenticate"));
        fixture.await.unwrap();
        server.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn manual_bypass_connects_directly() {
    let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = origin.local_addr().unwrap();
    let mut server = start(manual(upstream.local_addr().unwrap(), "127.0.0.1")).await;
    let fixture = tokio::spawn(async move {
        let (mut stream, _) = origin.accept().await.unwrap();
        assert!(headers(&mut stream).await.starts_with("GET / HTTP/1.1"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
    });
    assert!(
        exchange(
            &server,
            &format!("GET http://{address}/ HTTP/1.1\r\nHost: {address}\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 200")
    );
    assert!(timeout(Duration::from_millis(50), upstream.accept()).await.is_err());
    fixture.await.unwrap();
    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn upstream_407_is_sanitized_for_http_and_connect() {
    for connect in [false, true] {
        let peer = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = peer.local_addr().unwrap();
        let mut server = start(manual(address, "")).await;
        let fixture = tokio::spawn(async move {
            let (mut stream, _) = peer.accept().await.unwrap();
            let request = headers(&mut stream).await;
            if connect {
                assert!(request.starts_with("CONNECT origin.invalid:443 HTTP/1.1"));
            }
            assert!(!request.to_ascii_lowercase().contains("proxy-authorization"));
            stream.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Negotiate secret-token\r\nContent-Length: 6\r\n\r\nsecret").await.unwrap();
        });
        let line = if connect {
            "CONNECT origin.invalid:443"
        } else {
            "GET http://origin.invalid/"
        };
        let response = exchange(
            &server,
            &format!("{line} HTTP/1.1\r\nHost: origin.invalid\r\nProxy-Authorization: Basic browser-secret\r\n\r\n"),
        )
        .await;
        assert!(response.starts_with("HTTP/1.1 502"), "{response}");
        assert!(!response.to_ascii_lowercase().contains("proxy-authenticate"));
        fixture.await.unwrap();
        server.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn connect_waits_for_upstream_and_preserves_buffered_bytes() {
    let peer = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut server = start(manual(peer.local_addr().unwrap(), "")).await;
    let (received_tx, received_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let fixture = tokio::spawn(async move {
        let (mut stream, _) = peer.accept().await.unwrap();
        assert!(
            headers(&mut stream)
                .await
                .starts_with("CONNECT origin.invalid:443 HTTP/1.1")
        );
        received_tx.send(()).unwrap();
        release_rx.await.unwrap();
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\nhello")
            .await
            .unwrap();
        let mut bytes = [0; 4];
        stream.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"ping");
        stream.write_all(b"pong").await.unwrap();
    });
    let mut client = TcpStream::connect(server.runtime.listener).await.unwrap();
    client
        .write_all(b"CONNECT origin.invalid:443 HTTP/1.1\r\nHost: origin.invalid:443\r\n\r\n")
        .await
        .unwrap();
    timeout(Duration::from_secs(3), received_rx).await.unwrap().unwrap();
    assert!(timeout(Duration::from_millis(50), client.read_u8()).await.is_err());
    release_tx.send(()).unwrap();
    assert!(headers(&mut client).await.starts_with("HTTP/1.1 200"));
    let mut hello = [0; 5];
    timeout(Duration::from_secs(3), client.read_exact(&mut hello))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&hello, b"hello");
    client.write_all(b"ping").await.unwrap();
    let mut pong = [0; 4];
    timeout(Duration::from_secs(3), client.read_exact(&mut pong))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&pong, b"pong");
    fixture.await.unwrap();
    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn direct_connect_and_failed_upstream() {
    let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = origin.local_addr().unwrap();
    let mut server = start(ProxyModeCommand::Direct).await;
    let fixture = tokio::spawn(async move {
        let (mut stream, _) = origin.accept().await.unwrap();
        stream.write_all(b"hello").await.unwrap();
    });
    let mut client = TcpStream::connect(server.runtime.listener).await.unwrap();
    client
        .write_all(format!("CONNECT {address} HTTP/1.1\r\nHost: {address}\r\n\r\n").as_bytes())
        .await
        .unwrap();
    assert!(headers(&mut client).await.starts_with("HTTP/1.1 200"));
    let mut bytes = [0; 5];
    timeout(Duration::from_secs(3), client.read_exact(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&bytes, b"hello");
    fixture.await.unwrap();
    server.shutdown().await.unwrap();

    let peer = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut server = start(manual(peer.local_addr().unwrap(), "")).await;
    let fixture = tokio::spawn(async move {
        let (mut stream, _) = peer.accept().await.unwrap();
        headers(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
    });
    assert!(
        exchange(
            &server,
            "CONNECT origin.invalid:443 HTTP/1.1\r\nHost: origin.invalid:443\r\n\r\n"
        )
        .await
        .starts_with("HTTP/1.1 502")
    );
    fixture.await.unwrap();
    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn early_407_stops_upload_without_retry() {
    let peer = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut server = start(manual(peer.local_addr().unwrap(), "")).await;
    let fixture = tokio::spawn(async move {
        let (mut stream, _) = peer.accept().await.unwrap();
        assert!(
            headers(&mut stream)
                .await
                .starts_with("POST http://origin.invalid/upload HTTP/1.1")
        );
        stream.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=proxy\r\nContent-Length: 0\r\n\r\n").await.unwrap();
        let mut byte = [0];
        let result = timeout(Duration::from_secs(3), stream.read(&mut byte))
            .await
            .expect("Upstream connection was not closed");
        assert!(matches!(result, Ok(0)) || result.is_err());
        assert!(timeout(Duration::from_millis(50), peer.accept()).await.is_err());
    });
    assert!(
        exchange(
            &server,
            "POST http://origin.invalid/upload HTTP/1.1\r\nHost: origin.invalid\r\nContent-Length: 4096\r\n\r\n"
        )
        .await
        .starts_with("HTTP/1.1 502")
    );
    fixture.await.unwrap();
    server.shutdown().await.unwrap();
}
