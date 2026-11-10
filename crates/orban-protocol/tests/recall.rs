//! `recall`: a scripted 6300 (file-sync, PC Remote 4.1.1.19 opcodes) - a user
//! preset goes back as the on-air file and the readback confirms it; a unit
//! that never takes it ends Uncertain after exactly one upload.
use orban_protocol::{
    archive, framing::Decoder, message, presets::PresetKind,
    recall::{self, FileLink, RecallError}, session::Session,
};
use std::{sync::{Arc, Mutex}, time::Duration};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};

const CODE: &str = "1234";
fn doc(name: &str, drive: i32) -> String {
    format!("OptimodVersion=<6300.41>\r\nPreset Name=<{name}> size=3\r\nPreset File Name= <{name}.orb63user>\r\nC:<AGC DRIVE>Int:{drive};D:24;\r\nEnd Preset<end>\r\n")
}
fn file(kind: u16, name: &str, plain: &str) -> Vec<u8> {
    let sealed = archive::encrypt(plain.as_bytes(), CODE, [7; 16]).unwrap();
    let mut body = format!("0\n0\n{name}\n{}\n", sealed.len()).into_bytes();
    body.extend(sealed);
    message::encode(kind, &body).unwrap()
}

/// `takes`: whether the unit puts an uploaded document on air
async fn unit(takes: bool) -> (std::net::SocketAddr, Arc<Mutex<Vec<u16>>>) {
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        let (mut s, _) = server.accept().await.unwrap();
        let mut login = [0; 14];
        s.read_exact(&mut login).await.unwrap();
        s.write_all(b"connect ok\n0\n6300 V 4.1.1.19\n4711\n").await.unwrap();
        let mut on_air = doc("A", 14);
        let mut decoder = Decoder::default();
        let mut buf = [0; 8192];
        loop {
            let n = match s.read(&mut buf).await { Ok(0) | Err(_) => return, Ok(n) => n };
            for payload in decoder.feed(&buf[..n]).unwrap() {
                let kind = std::str::from_utf8(&payload[..8]).unwrap().trim().parse::<u16>().unwrap() + 204;
                log.lock().unwrap().push(kind);
                let reply: Vec<u8> = match kind {
                    242 => file(231, "B.orb63user", &doc("B", 9)),
                    238 => file(229, "OnAir.orb63onair", &on_air),
                    229 => {
                        if takes {
                            let m = message::decode(&payload).unwrap();
                            if let message::Message::File { data, .. } = m {
                                let plain = archive::decrypt(&data, CODE).unwrap();
                                on_air = String::from_utf8(plain).unwrap().replace('\n', "\r\n").replace("\r\r\n", "\r\n");
                            }
                        }
                        Vec::new()
                    }
                    249 | 250 => message::encode(250, b"").unwrap(),
                    _ => Vec::new(),
                };
                if !reply.is_empty() { s.write_all(&reply).await.unwrap(); }
            }
        }
    });
    (addr, seen)
}

#[tokio::test]
async fn a_user_preset_is_recalled_and_confirmed_by_its_readback() {
    let (addr, seen) = unit(true).await;
    let mut session = Session::connect(addr, CODE, Duration::from_secs(2)).await.unwrap();
    assert_eq!(session.info.adapter_id, "pc-remote-6300-4.1.1.19");
    let mut link = FileLink::new(&mut session, CODE);
    assert_eq!(link.on_air().await.unwrap().name, "A");
    let confirmed = recall::recall(&mut link, None, "B", &PresetKind::User).await.unwrap();
    assert_eq!(confirmed.name, "B");
    assert_eq!(seen.lock().unwrap().iter().filter(|k| **k == 229).count(), 1);
}

#[tokio::test]
async fn an_unconfirmed_recall_is_uncertain_and_never_sent_twice() {
    let (addr, seen) = unit(false).await;
    let mut session = Session::connect(addr, CODE, Duration::from_secs(2)).await.unwrap();
    let mut link = FileLink::new(&mut session, CODE);
    let r = recall::recall(&mut link, None, "B", &PresetKind::User).await;
    assert!(matches!(r, Err(RecallError::Uncertain(ref e)) if e.contains("reports A on air")), "{r:?}");
    assert_eq!(seen.lock().unwrap().iter().filter(|k| **k == 229).count(), 1);
}

#[tokio::test]
async fn nothing_is_sent_for_a_preset_the_unit_does_not_list() {
    let (addr, seen) = unit(true).await;
    let mut session = Session::connect(addr, CODE, Duration::from_secs(2)).await.unwrap();
    let mut link = FileLink::new(&mut session, CODE);
    let r = recall::recall(&mut link, None, "C", &PresetKind::User).await;
    assert!(matches!(r, Err(RecallError::Refused(_))));
    assert!(!seen.lock().unwrap().contains(&229));
}

#[test]
fn an_older_document_version_is_confirmed_by_name_only() {
    let model = orban_protocol::adapter::DeviceModel::Optimod6300;
    let sent = recall::file_document(doc("B", 9).replace("6300.41", "6300.40").as_bytes(), model).unwrap();
    let read = recall::file_document(doc("B", 11).as_bytes(), model).unwrap();
    assert!(recall::confirms("B", Some(&sent), &read));
    let same_version = recall::file_document(doc("B", 9).as_bytes(), model).unwrap();
    assert!(!recall::confirms("B", Some(&same_version), &read));
    assert!(!recall::confirms("A", None, &read));
}

/// The 2300 has no status port: a port given for it does not make the
/// firmware's own RP the route; its factory file goes up instead.
#[tokio::test]
async fn a_model_without_a_status_port_ignores_a_given_terminal_address() {
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut s, _) = server.accept().await.unwrap();
        let mut login = [0; 14];
        s.read_exact(&mut login).await.unwrap();
        s.write_all(b"connect ok\n0\n2300 V 2.0.3.85\n").await.unwrap();
        let mut buf = [0; 64];
        while s.read(&mut buf).await.map(|n| n > 0).unwrap_or(false) {}
    });
    let session = Session::connect(addr, CODE, Duration::from_secs(3)).await.unwrap();
    assert_eq!(recall::route(&session, &PresetKind::Factory, true), Ok(recall::Route::UploadFactory));
    assert_eq!(recall::route(&session, &PresetKind::Factory, false), Ok(recall::Route::UploadFactory));
}
