use orban_protocol::{
    adapter::DeviceModel,
    control::Scope,
    terminal::{read_snapshot, read_snapshot_for_model},
};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
#[tokio::test]
async fn terminal_snapshot_ends_at_document_marker_and_normalizes_line_endings() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        s.write_all(b"Orban Optimod 5700i V 3.0.1.20\n\r")
            .await
            .unwrap();
        let mut command = vec![];
        loop {
            let b = s.read_u8().await.unwrap();
            command.push(b);
            if b == b'\n' {
                break;
            }
        }
        assert_eq!(command, b"AS [9876]??\r\n");
        for chunk in b"OptimodVersion=<5700.51>\n\rPreset Name=<SYSTEM PRESET> size=1\n\rC:<CONTRAST>Int:2;D:2;\n\rEnd Preset<end>\n\r".chunks(7){s.write_all(chunk).await.unwrap();}
    });
    let result = read_snapshot(address, "9876", Scope::System, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(result.fields["CONTRAST"].index, 2);
    worker.await.unwrap();
}

#[tokio::test]
async fn read_only_snapshot_supports_the_official_5500i_terminal_dialect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream
            .write_all(b"Orban Optimod 5500i V 3.1.0.2\n\r")
            .await
            .unwrap();
        let mut command = vec![];
        loop {
            let byte = stream.read_u8().await.unwrap();
            command.push(byte);
            if byte == b'\n' {
                break;
            }
        }
        assert_eq!(command, b"AP [9876]??\r\n");
        stream.write_all(b"OptimodVersion=<5500.50>\r\nPreset Name=<ROCK> size=1\r\nC:<AGC>String:<On>;D:1;\r\nEnd Preset<end>\r\n").await.unwrap();
    });
    let result = read_snapshot_for_model(
        address,
        "9876",
        Scope::Processing,
        Duration::from_secs(2),
        DeviceModel::Optimod5500i,
    )
    .await
    .unwrap();
    assert_eq!(result.name, "ROCK");
    worker.await.unwrap();
}

#[tokio::test]
async fn later_pc_remote_models_keep_their_own_terminal_identity_and_document_family() {
    let cases = [
        (
            DeviceModel::Optimod6300,
            "Orban Optimod 6300 V 4.1\r\n",
            "6300.50",
        ),
        (
            DeviceModel::Optimod8600,
            "Orban Optimod 8600 V 4.5\r\n",
            "8600.40",
        ),
        (
            DeviceModel::Optimod8700i,
            "Orban Optimod 8700i V 1.5\r\n",
            "8700.51",
        ),
        (
            DeviceModel::Optimod9300,
            "Orban Optimod 9300 V 2.1\r\n",
            "9300.30",
        ),
        (
            DeviceModel::Optimod9400,
            "Orban Optimod 9400 V 2.0\r\n",
            "9400.30",
        ),
    ];
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        for (_, banner, version) in cases {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write_all(banner.as_bytes()).await.unwrap();
            let mut command = Vec::new();
            loop {
                let byte = stream.read_u8().await.unwrap();
                command.push(byte);
                if byte == b'\n' {
                    break;
                }
            }
            assert_eq!(command, b"AP [9876]??\r\n");
            let document = format!(
                "OptimodVersion=<{version}>\r\nPreset Name=<DEMO> size=1\r\nC:<AGC>String:<On>;D:1;\r\nEnd Preset<end>\r\n"
            );
            stream.write_all(document.as_bytes()).await.unwrap();
        }
    });
    for (model, _, _) in cases {
        let result = read_snapshot_for_model(
            address,
            "9876",
            Scope::Processing,
            Duration::from_secs(2),
            model,
        )
        .await
        .unwrap();
        assert_eq!(result.name, "DEMO");
    }
    worker.await.unwrap();
}

#[tokio::test]
async fn preset_catalog_has_a_document_boundary_and_recall_requires_acknowledgement() {
    use orban_protocol::terminal::{read_presets, recall_preset};
    use std::time::Duration;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        for (expected, prefix) in [
            (
                "LP [9876]\r\nAP [9876]??\r\n",
                "EXAMPLE factory\r\nMy Preset user\r\n",
            ),
            ("RP EXAMPLE[9876]\r\nAP [9876]??\r\n", "ON AIR: EXAMPLE\r\n"),
            ("RP EXAMPLE[9876]\r\nAP [9876]??\r\n", ""),
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream
                .write_all(b"Orban Optimod 5700i 3.0.1.20\n\r")
                .await
                .unwrap();
            let mut got = vec![0; expected.len()];
            stream.read_exact(&mut got).await.unwrap();
            assert_eq!(got, expected.as_bytes());
            let text = format!(
                "{prefix}OptimodVersion=<5700.3.0>\r\nPreset Name=<EXAMPLE> size=7\r\nC:<AGC>String:<On>;D:1;\r\nEnd Preset<end>\r\n"
            );
            for chunk in text.as_bytes().chunks(17) {
                stream.write_all(chunk).await.unwrap();
            }
            let mut discard = Vec::new();
            stream.read_to_end(&mut discard).await.unwrap();
        }
    });
    let (presets, doc) = read_presets(address, "9876", Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(presets.len(), 2);
    assert_eq!(doc.name, "EXAMPLE");
    assert_eq!(
        recall_preset(address, "9876", "EXAMPLE", Duration::from_secs(2))
            .await
            .unwrap()
            .name,
        "EXAMPLE"
    );
    assert!(
        recall_preset(address, "9876", "EXAMPLE", Duration::from_secs(2))
            .await
            .is_err()
    );
    server.await.unwrap();
}

/// A banner, then whatever `reply` holds, then silence; returns what the
/// unit received after the banner (the command and the normal close).
async fn silent_after(
    reply: &'static [u8],
) -> (std::net::SocketAddr, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream
            .write_all(b"Orban Optimod 5700i V 3.0.1.20\n\r")
            .await
            .unwrap();
        stream.write_all(reply).await.unwrap();
        let mut received = Vec::new();
        let _ =
            tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut received)).await;
        received
    });
    (address, worker)
}

#[tokio::test]
async fn a_late_read_names_its_stage_and_does_not_claim_a_write() {
    let (address, worker) = silent_after(b"OptimodVersion=<5700.51>\r\n").await;
    let error = read_snapshot(address, "9876", Scope::System, Duration::from_millis(400))
        .await
        .unwrap_err();
    assert!(
        error.starts_with(&format!("The status port (TCP {})", address.port())),
        "{error}"
    );
    assert!(
        error.contains("did not finish its reply within 0.4 s (26 bytes received)"),
        "{error}"
    );
    assert!(error.ends_with("nothing was changed"), "{error}");
    assert!(!error.contains("applied"), "{error}");
    // The session still ends as the unit expects, not with a reset.
    let received = worker.await.unwrap();
    assert!(received.ends_with(b"disconnect\r\n"), "{received:?}");
}

#[tokio::test]
async fn a_port_without_a_banner_is_reported_before_anything_is_sent() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut received = Vec::new();
        let _ =
            tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut received)).await;
        received
    });
    let error = read_snapshot(
        address,
        "9876",
        Scope::Processing,
        Duration::from_millis(300),
    )
    .await
    .unwrap_err();
    assert!(
        error.contains("sent no terminal banner within 0.3 s; nothing was sent"),
        "{error}"
    );
    assert!(!String::from_utf8_lossy(&worker.await.unwrap()).contains("AP ["));
}

#[tokio::test]
async fn a_port_that_refuses_the_connection_is_unreachable() {
    let address = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap();
    // The listener is dropped: nothing accepts on this port any more.
    let error = read_snapshot(address, "9876", Scope::System, Duration::from_secs(2))
        .await
        .unwrap_err();
    assert!(orban_protocol::terminal::is_unreachable(&error), "{error}");
    assert!(
        error.starts_with(&format!(
            "The status port (TCP {}) could not be reached",
            address.port()
        )),
        "{error}"
    );
}

#[tokio::test]
async fn a_port_that_closes_at_once_says_so() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = listener.accept().await.unwrap();
    });
    let error = read_snapshot(address, "9876", Scope::System, Duration::from_secs(2))
        .await
        .unwrap_err();
    assert_eq!(
        error,
        format!(
            "The status port (TCP {}): closed the connection without a terminal banner",
            address.port()
        )
    );
    assert!(!orban_protocol::terminal::is_unreachable(&error));
}

#[tokio::test]
async fn a_late_recall_reply_keeps_its_unknown_outcome() {
    let (address, _worker) = silent_after(b"").await;
    let error = orban_protocol::terminal::recall_preset_for_model(
        address,
        "9876",
        "ROCK",
        Duration::from_millis(300),
        DeviceModel::Optimod5700i,
    )
    .await
    .unwrap_err();
    assert!(
        error.ends_with("the recall may have been applied"),
        "{error}"
    );
}
