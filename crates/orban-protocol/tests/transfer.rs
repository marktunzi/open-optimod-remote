//! `transfer`: a scripted 6300 (4.1.1.19 opcodes) keeps the preset files it is sent (231, named) and lists them (242 -> 231 files).
use orban_protocol::{archive, framing::Decoder, message, recall::{FileLink, RecallError}, session::Session, transfer};
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

async fn unit(files: Vec<(String, String)>) -> (std::net::SocketAddr, Arc<Mutex<Vec<(String, String)>>>, Arc<Mutex<Vec<u16>>>) {
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    let store = Arc::new(Mutex::new(files));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (st, log) = (store.clone(), seen.clone());
    tokio::spawn(async move {
        let (mut s, _) = server.accept().await.unwrap();
        let mut login = [0; 14];
        s.read_exact(&mut login).await.unwrap();
        s.write_all(b"connect ok\n0\n6300 V 4.1.1.19\n4711\n").await.unwrap();
        let mut decoder = Decoder::default();
        let mut buf = [0; 16384];
        loop {
            let n = match s.read(&mut buf).await { Ok(0) | Err(_) => return, Ok(n) => n };
            for payload in decoder.feed(&buf[..n]).unwrap() {
                let kind = std::str::from_utf8(&payload[..8]).unwrap().trim().parse::<u16>().unwrap() + 204;
                log.lock().unwrap().push(kind);
                let mut reply = Vec::new();
                match kind {
                    242 => for (n, d) in st.lock().unwrap().iter() { reply.extend(file(231, n, d)); },
                    238 => reply = file(229, "OnAir.orb63onair", &doc("A", 14)),
                    231 => if let message::Message::File { name, data, .. } = message::decode(&payload).unwrap() {
                        let plain = String::from_utf8(archive::decrypt(&data, CODE).unwrap()).unwrap();
                        st.lock().unwrap().push((name, plain));
                    },
                    249 | 250 => reply = message::encode(250, b"").unwrap(),
                    _ => {}
                }
                if !reply.is_empty() { s.write_all(&reply).await.unwrap(); }
            }
        }
    });
    (addr, store, seen)
}

#[tokio::test]
async fn a_new_preset_is_sent_once_and_listed_by_the_processor() {
    let (addr, store, seen) = unit(vec![]).await;
    let mut session = Session::connect(addr, CODE, Duration::from_secs(2)).await.unwrap();
    let mut link = FileLink::new(&mut session, CODE);
    transfer::transfer(&mut link, "MORNING", &doc("MORNING", 9), 0, Duration::from_secs(3)).await.unwrap();
    assert_eq!(store.lock().unwrap().iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["MORNING.orb63user"]);
    assert_eq!(seen.lock().unwrap().iter().filter(|k| **k == 231).count(), 1);
    assert!(!seen.lock().unwrap().contains(&229));                  // (the on-air processing is not touched)
}

#[tokio::test]
async fn nothing_is_overwritten_and_nothing_foreign_is_sent() {
    let (addr, _store, seen) = unit(vec![("MORNING.orb63user".into(), doc("MORNING", 9))]).await;
    let mut session = Session::connect(addr, CODE, Duration::from_secs(2)).await.unwrap();
    let mut link = FileLink::new(&mut session, CODE);
    let r = transfer::transfer(&mut link, "MORNING", &doc("MORNING", 3), 0, Duration::from_secs(3)).await;
    assert!(matches!(r, Err(RecallError::Refused(ref e)) if e.contains("already")), "{r:?}");
    let foreign = doc("EVENING", 3).replace("6300.41", "8700.51");
    let r = transfer::transfer(&mut link, "EVENING", &foreign, 0, Duration::from_secs(3)).await;
    assert!(matches!(r, Err(RecallError::Refused(_))), "{r:?}");
    let r = transfer::transfer(&mut link, "modif X", &doc("modif X", 3), 0, Duration::from_secs(3)).await;
    assert!(matches!(r, Err(RecallError::Refused(_))), "{r:?}");
    assert!(!seen.lock().unwrap().contains(&231));
}
