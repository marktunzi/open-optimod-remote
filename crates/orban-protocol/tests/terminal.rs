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
            .write_all(b"Orban Optimod 5500i V 3.1\r\n")
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
        stream.write_all(b"OptimodVersion=<8300.10>\r\nPreset Name=<ROCK> size=1\r\nC:<AGC>String:<On>;D:1;\r\nEnd Preset<end>\r\n").await.unwrap();
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
