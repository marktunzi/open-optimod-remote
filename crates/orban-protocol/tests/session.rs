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

/// The unit answers `busy` while it still closes a previous session (seen on
/// the virtual 5700i with original firmware, 20 October 2026) and `in use`
/// in the same place; the original client names both to the operator.
#[tokio::test]
async fn busy_and_in_use_logins_are_named_for_the_operator() {
    for (answer, expected) in [
        ("busy", "The processor is busy. Try again in a few seconds."),
        ("in use", "The processor is in use."),
    ] {
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let worker = tokio::spawn(async move {
            let (mut s, _) = server.accept().await.unwrap();
            let mut b = [0; 14];
            s.read_exact(&mut b).await.unwrap();
            s.write_all(format!("{answer}\n").as_bytes()).await.unwrap();
        });
        let result = Session::connect(address, "9876", Duration::from_secs(1)).await;
        assert_eq!(result.err().as_deref(), Some(expected));
        worker.await.unwrap();
    }
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

#[tokio::test]
async fn a_malformed_message_does_not_lose_the_frames_after_it() {
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        let (mut stream, _) = server.accept().await.unwrap();
        let mut request = vec![0; 14];
        stream.read_exact(&mut request).await.unwrap();
        let mut reply = b"connect ok\n0\n5700i V 3.0.1.20\n12345\n".to_vec();
        // Meter bank 9 does not exist on the 5700i; the 251 behind it in the
        // same segment must still arrive.
        reply.extend(message::encode(228, b"\x09112\n").unwrap());
        reply.extend(message::encode(251, b"").unwrap());
        stream.write_all(&reply).await.unwrap();
        let mut end = [0; 21];
        let _ = stream.read_exact(&mut end).await;
    });
    let mut session = Session::connect(address, "9876", Duration::from_secs(2))
        .await
        .unwrap();
    assert!(session.next_event().await.is_err());
    assert!(!session.is_broken());
    assert_eq!(
        session.next_event().await.unwrap(),
        Message::Other {
            kind: 251,
            data: Vec::new()
        }
    );
    session.disconnect().await.unwrap();
    worker.await.unwrap();
}

#[tokio::test]
async fn disconnect_reads_until_the_unit_closes_and_never_waits_long() {
    for unit_closes in [true, false] {
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let worker = tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            let mut request = vec![0; 14];
            stream.read_exact(&mut request).await.unwrap();
            stream
                .write_all(b"connect ok\n0\n5700i V 3.0.1.20\n12345\n")
                .await
                .unwrap();
            let mut end = [0; 21];
            stream.read_exact(&mut end).await.unwrap();
            assert_eq!(end.to_vec(), message::encode(218, b"").unwrap());
            // Replies already in flight when the disconnect arrives.
            for _ in 0..20 {
                stream
                    .write_all(&message::encode(220, b"").unwrap())
                    .await
                    .unwrap();
            }
            let mut rest = Vec::new();
            stream.read_to_end(&mut rest).await.unwrap();
            assert!(rest.is_empty());
            if !unit_closes {
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        });
        let session = Session::connect(address, "9876", Duration::from_secs(2))
            .await
            .unwrap();
        let started = std::time::Instant::now();
        session.disconnect().await.unwrap();
        assert!(started.elapsed() < Duration::from_millis(1500));
        if unit_closes {
            worker.await.unwrap();
        } else {
            worker.abort();
        }
    }
}

#[tokio::test]
async fn an_unfinished_login_is_closed_with_a_reset() {
    // The 5700i keeps a half-closed session open and then answers no new
    // login; a reset frees it at once.
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        let (mut stream, _) = server.accept().await.unwrap();
        let mut request = vec![0; 14];
        stream.read_exact(&mut request).await.unwrap();
        let mut rest = [0; 16];
        stream.read(&mut rest).await
    });
    let result = Session::connect(address, "9876", Duration::from_millis(300)).await;
    assert!(matches!(result, Err(error) if error.contains("Login timed out")));
    let closed = worker.await.unwrap();
    assert_eq!(
        closed.map_err(|error| error.kind()),
        Err(std::io::ErrorKind::ConnectionReset)
    );
}

/// Issue #1: a 5500 must connect whether or not its login answer ends with a
/// session line, and its first boundary is the 8700HD generation's F9h.
#[tokio::test]
async fn a_5500_logs_in_with_or_without_a_session_line_and_asks_f9h() {
    for answer in [
        &b"connect ok\n0\n5500 V 1.2.8.24\n"[..],
        &b"connect ok\n0\n5500 V 1.2.8.24\n4711\n"[..],
    ] {
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let worker = tokio::spawn(async move {
            let (mut s, _) = server.accept().await.unwrap();
            let mut b = [0; 14];
            s.read_exact(&mut b).await.unwrap();
            s.write_all(answer).await.unwrap();
            let mut request = [0; 21];
            s.read_exact(&mut request).await.unwrap();
            assert_eq!(request.to_vec(), message::encode(249, b"").unwrap());
            s.write_all(&message::encode(250, b"").unwrap())
                .await
                .unwrap();
            let mut rest = Vec::new();
            let _ = s.read_to_end(&mut rest).await;
        });
        let mut session = Session::connect(address, "9876", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(session.info.model, DeviceModel::Optimod5500);
        let boundary = session.adapter().wire.boundary;
        assert_eq!(boundary, (249, 250));
        session.send(boundary.0, b"").await.unwrap();
        assert_eq!(
            session.next_event().await.unwrap(),
            Message::Other {
                kind: 250,
                data: Vec::new()
            }
        );
        session.disconnect().await.unwrap();
        worker.await.unwrap();
    }
}

/// The 5500 firmware's request switch is the 5500i's with every opcode one
/// lower and no network file; nothing of the 5500i's wire may leak into it.
#[test]
fn the_5500_wire_is_the_5500i_wire_one_opcode_lower() {
    use orban_protocol::adapter::{WIRE_5500, WIRE_5500I};
    let (old, new) = (WIRE_5500, WIRE_5500I);
    let (a, b) = (old.tool_files.unwrap(), new.tool_files.unwrap());
    assert_eq!(old.boundary, (249, 250));
    assert_eq!(old.on_air_request, 238);
    assert_eq!(old.setup_request, Some((239, 232)));
    assert_eq!(old.user_presets_request, Some((242, 231)));
    assert_eq!(old.preset_file_delete, Some(236));
    assert_eq!(
        (a.automation_request, a.automation_file, a.passcodes_request),
        (241, 234, 243)
    );
    assert_eq!(
        (
            b.automation_request - 1,
            b.automation_file - 1,
            b.passcodes_request - 1
        ),
        (a.automation_request, a.automation_file, a.passcodes_request)
    );
    assert_eq!(
        new.preset_file_delete.map(|d| d - 1),
        old.preset_file_delete
    );
    assert_eq!(old.backup_requests, [(239, 232), (241, 234)]);
    assert_eq!(old.restore_files, [("Automation.orb55autom", 234)]);
    assert_eq!(old.network_request, None);
    assert_eq!(old.setup_presets, None);
    assert_eq!(old.file_end, Some(250));
}
