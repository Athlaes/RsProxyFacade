use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::{Duration, timeout},
};

fn config(host: &str, port: u16) -> ProxyConfig {
    ProxyConfig {
        host: host.into(),
        port,
        proxy_mode: ProxyMode::Direct,
    }
}

async fn headers(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        bytes.push(stream.read_u8().await.unwrap());
    }
    String::from_utf8(bytes).unwrap()
}

#[tokio::test]
async fn blocks_http_and_connect_to_listener_including_dns_aliases() {
    timeout(Duration::from_secs(10), async {
        for bind in ["127.0.0.1", "0.0.0.0"] {
            let mut proxy = ProxyServer::new(config(bind, 0)).await.unwrap();
            let port = proxy.config.port;
            for host in ["127.0.0.1", "localhost"] {
                for line in [
                    format!("GET http://{host}:{port}/ HTTP/1.1"),
                    format!("CONNECT {host}:{port} HTTP/1.1"),
                ] {
                    let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
                    client
                        .write_all(format!("{line}\r\nHost: {host}:{port}\r\n\r\n").as_bytes())
                        .await
                        .unwrap();
                    assert!(headers(&mut client).await.starts_with("HTTP/1.1 403"));
                }
            }
            proxy.shutdown().await;
        }
        assert!(ProxyServer::is_listener(
            "[::1]:8080".parse().unwrap(),
            "[::]:8080".parse().unwrap()
        ));
        assert!(ProxyServer::is_listener(
            "[::ffff:127.0.0.1]:8080".parse().unwrap(),
            "127.0.0.1:8080".parse().unwrap()
        ));
        assert!(!ProxyServer::is_listener(
            "127.0.0.1:8081".parse().unwrap(),
            "127.0.0.1:8080".parse().unwrap()
        ));
    })
    .await
    .expect("Loop prevention test timed out");
}

#[tokio::test]
async fn stop_closes_tunnel_and_http_exchange_before_rebinding() {
    timeout(Duration::from_secs(10), async {
        let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = origin.local_addr().unwrap();
        let mut proxy = ProxyServer::new(config("127.0.0.1", 0)).await.unwrap();
        let port = proxy.config.port;

        let mut tunnel_client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        tunnel_client
            .write_all(format!("CONNECT {address} HTTP/1.1\r\nHost: {address}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        assert!(headers(&mut tunnel_client).await.starts_with("HTTP/1.1 200"));
        let (mut tunnel_origin, _) = origin.accept().await.unwrap();
        tunnel_client.write_all(b"ping").await.unwrap();
        let mut bytes = [0; 4];
        tunnel_origin.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"ping");

        let mut http_client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        http_client
            .write_all(format!("GET http://{address}/ HTTP/1.1\r\nHost: {address}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let (mut http_origin, _) = origin.accept().await.unwrap();
        headers(&mut http_origin).await;
        // The origin deliberately leaves its HTTP response pending.
        proxy.shutdown().await;
        assert!(matches!(proxy.proxy_status(), ExecutionStatus::Fail));
        for stream in [
            &mut tunnel_client,
            &mut tunnel_origin,
            &mut http_client,
            &mut http_origin,
        ] {
            assert!(matches!(stream.read(&mut bytes).await, Ok(0) | Err(_)));
        }
        proxy.shutdown().await; // Waiting for termination is idempotent.
        let mut restarted = ProxyServer::new(config("127.0.0.1", port)).await.unwrap();
        assert!(matches!(restarted.proxy_status(), ExecutionStatus::Success));
        restarted.shutdown().await;
    })
    .await
    .expect("Stop/restart test timed out");
}
