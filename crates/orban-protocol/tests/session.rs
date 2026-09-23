use orban_protocol::{
    adapter::{DeviceModel, SkinId},
    message::{self, Message},
    session::Session,
};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
#[tokio::test]
async fn fragmented_login_and_coalesced_events_survive_tcp_boundaries() {
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        let (mut stream, _) = server.accept().await.unwrap();
        let mut request = vec![0; 14];
        stream.read_exact(&mut request).await.unwrap();
        assert_eq!(
            request,
            orban_protocol::auth::login_request("9876").unwrap()
        );
        let mut reply = b"connect ok\n0\n5700i V 3.0.1.20\n12345\n".to_vec();
        reply.extend(message::encode(220, b"").unwrap());
        reply.extend(message::encode(247, b"clock").unwrap());
        for part in reply.chunks(3) {
            stream.write_all(part).await.unwrap();
        }
        let mut end = [0; 21];
        stream.read_exact(&mut end).await.unwrap();
        assert_eq!(end.to_vec(), message::encode(218, b"").unwrap());
    });
    let mut session = Session::connect(address, "9876", Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(session.info.access_level, 0);
    assert_eq!(session.next_event().await.unwrap(), Message::Heartbeat);
    assert_eq!(
        session.next_event().await.unwrap(),
        Message::Other {
            kind: 247,
            data: b"clock".to_vec()
        }
    );
    session.disconnect().await.unwrap();
    worker.await.unwrap();
}
#[tokio::test]
async fn rejected_login_does_not_wait_for_three_more_lines() {
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        let (mut s, _) = server.accept().await.unwrap();
        let mut b = [0; 14];
        s.read_exact(&mut b).await.unwrap();
        s.write_all(b"password failed\n").await.unwrap();
    });
    let result = Session::connect(address, "9876", Duration::from_secs(1)).await;
    assert!(matches!(result,Err(e) if e.contains("password failed")));
    worker.await.unwrap();
}

#[tokio::test]
async fn model_is_detected_from_the_official_banner_and_a_wrong_hint_fails_closed() {
    async fn serve(banner: &'static str) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let worker = tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            let mut request = vec![0; 14];
            stream.read_exact(&mut request).await.unwrap();
            stream
                .write_all(format!("connect ok\n0\n{banner}\n12345\n").as_bytes())
                .await
                .unwrap();
            let mut close = [0; 21];
            if stream.read_exact(&mut close).await.is_ok() {
                assert_eq!(close.to_vec(), message::encode(218, b"").unwrap());
            }
        });
        (address, worker)
    }

    let (address, worker) = serve("5500i V 3.1.0").await;
    let session =
        Session::connect_for_model(address, "9876", Duration::from_secs(2), DeviceModel::Auto)
            .await
            .unwrap();
    assert_eq!(session.info.model, DeviceModel::Optimod5500i);
    assert_eq!(session.info.skin, SkinId::Optimod5500i);
    assert!(!session.info.capabilities.parameter_writes);
    session.disconnect().await.unwrap();
    worker.await.unwrap();

    let (address, worker) = serve("5500i V 3.1.0").await;
    let result = Session::connect_for_model(
        address,
        "9876",
        Duration::from_secs(2),
        DeviceModel::Optimod8500,
    )
    .await;
    assert!(matches!(result, Err(error) if error.contains("does not match")));
    worker.await.unwrap();
}
