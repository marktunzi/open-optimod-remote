use orban_protocol::{document::Document, message::Message, session::Session};
use std::{io::Read, net::SocketAddr, time::Duration};
use tokio::time::Instant;
use zeroize::Zeroizing;
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Diagnostic failed: {error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return Err("Usage: optimod-diagnose IP:6201 [seconds] [--password-stdin]".into());
    }
    let address: SocketAddr = args[0].parse().map_err(|_| "Expected IP:port")?;
    let seconds = args
        .get(1)
        .filter(|s| !s.starts_with('-'))
        .map(|s| s.parse::<u64>())
        .transpose()
        .map_err(|_| "Invalid duration")?
        .unwrap_or(10)
        .clamp(1, 3600);
    let code = Zeroizing::new(if args.iter().any(|s| s == "--password-stdin") {
        let mut input = String::new();
        std::io::stdin()
            .take(128)
            .read_to_string(&mut input)
            .map_err(|e| e.to_string())?;
        input.trim_end().to_owned()
    } else {
        rpassword::prompt_password("Access code: ").map_err(|e| e.to_string())?
    });
    let mut session = Session::connect(address, &code, Duration::from_secs(4)).await?;
    println!(
        "Connected: {} (access {})",
        session.info.firmware, session.info.access_level
    );
    let result = diagnose(&mut session, seconds).await;
    let disconnect = session.disconnect().await;
    result?;
    disconnect
}
async fn diagnose(session: &mut Session, seconds: u64) -> Result<(), String> {
    session.send(239, b"").await?;
    session.send(240, b"").await?;
    session.send(250, b"").await?;
    let mut counts = Vec::new();
    loop {
        match session.next_event().await? {
            Message::File { kind, data, .. } if kind == 229 || kind == 233 => {
                let plain = Zeroizing::new(session.decrypt_file(&data)?);
                let document = Document::parse(&plain)?;
                counts.push((kind, document.fields.len()));
            }
            Message::Other { kind: 251, .. } => break,
            _ => {}
        }
    }
    counts.sort();
    if counts.len() != 2 {
        return Err("Missing configuration documents".into());
    }
    println!(
        "Configuration: {} processing fields, {} system fields",
        counts[0].1, counts[1].1
    );
    session.send(222, b"").await?;
    let started = Instant::now();
    let mut frames = [0u64; 2];
    let mut changed = [0u64; 2];
    let mut previous: [Option<Vec<u8>>; 2] = [None, None];
    let mut max_cycle = Duration::ZERO;
    while started.elapsed() < Duration::from_secs(seconds) {
        let cycle = Instant::now();
        session.send(219, b"").await?;
        session.send(250, b"").await?;
        loop {
            match session.next_event().await? {
                Message::Meters { bank, values } => {
                    let i = usize::from(bank - 1);
                    frames[i] += 1;
                    if previous[i].as_ref().is_some_and(|old| *old != values) {
                        changed[i] += 1;
                    }
                    previous[i] = Some(values);
                }
                Message::Other { kind: 251, .. } => break,
                _ => {}
            }
        }
        max_cycle = max_cycle.max(cycle.elapsed());
        tokio::time::sleep_until(cycle + Duration::from_millis(100)).await;
    }
    session.send(223, b"").await?;
    println!(
        "Meters: {} / {} frames; {} / {} changed frames; slowest poll {} ms; duration {:.1}s",
        frames[0],
        frames[1],
        changed[0],
        changed[1],
        max_cycle.as_millis(),
        started.elapsed().as_secs_f32()
    );
    if frames[0] == 0 {
        return Err("No live meters received".into());
    }
    Ok(())
}
