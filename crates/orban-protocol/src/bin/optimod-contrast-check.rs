//! Explicit hardware integration check. Temporarily changes ONLY display contrast.
use orban_protocol::{
    control::Scope,
    document::{Document, Field, Value},
    message::Message,
    profile::Profile,
    session::Session,
    terminal::read_snapshot,
};
use std::{io::Read, net::SocketAddr, time::Duration};
use zeroize::Zeroizing;
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Check failed: {e}");
        std::process::exit(1);
    }
}
async fn barrier(s: &mut Session) -> Result<(), String> {
    s.send(219, b"").await?;
    s.send(250, b"").await?;
    loop {
        if matches!(s.next_event().await?, Message::Other { kind: 251, .. }) {
            return Ok(());
        }
    }
}
async fn snapshots(address: SocketAddr, code: &str) -> Result<(Document, Document), String> {
    let system = read_snapshot(address, code, Scope::System, Duration::from_secs(4))
        .await
        .map_err(|e| format!("System read: {e}"))?;
    let processing = read_snapshot(address, code, Scope::Processing, Duration::from_secs(4))
        .await
        .map_err(|e| format!("Processing read: {e}"))?;
    Ok((system, processing))
}
async fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 || args[1] != "--temporarily-change-display-contrast" {
        return Err("Usage: optimod-contrast-check IP:6201 --temporarily-change-display-contrast [--password-stdin]".into());
    }
    let address: SocketAddr = args[0].parse().map_err(|_| "Expected IP:port")?;
    let terminal = SocketAddr::new(address.ip(), 23);
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
    let mut s = Session::connect(address, &code, Duration::from_secs(4)).await?;
    let result = check(&mut s, terminal, &code).await;
    let disconnect = s.disconnect().await;
    result?;
    disconnect
}
async fn check(s: &mut Session, address: SocketAddr, code: &str) -> Result<(), String> {
    if s.info.firmware != "5700i V 3.0.1.20" {
        return Err("Hardware check supports firmware 3.0.1.20 only".into());
    }
    let before = snapshots(address, code).await?;
    let old = before
        .0
        .fields
        .get("CONTRAST")
        .ok_or("Missing contrast")?
        .clone();
    if old.value != Value::Int(old.index as i32) || old.index > 3 {
        return Err("Unexpected contrast representation".into());
    }
    let index = if old.index > 0 { old.index - 1 } else { 1 };
    let next = Field {
        index,
        value: Profile::embedded()?.value(Scope::System, "CONTRAST", index)?,
    };
    println!(
        "Baseline verified: {} system + {} processing fields; contrast {} -> {} -> {}",
        before.0.fields.len(),
        before.1.fields.len(),
        old.index,
        index,
        old.index
    );
    // Always attempt restoration, including when a sent write has unknown outcome.
    let changed = async {
        s.change(Scope::System, "CONTRAST", &next).await?;
        barrier(s).await?;
        let mut during = snapshots(address, code).await?;
        if during.0.fields.get("CONTRAST") != Some(&next) {
            return Err("Contrast change was not confirmed".into());
        }
        during.0.fields.insert("CONTRAST".into(), old.clone());
        if during.0.fields != before.0.fields || during.1.fields != before.1.fields {
            return Err("Unexpected additional field changes".into());
        }
        println!("Confirmed: only display contrast changed.");
        Ok::<(), String>(())
    }
    .await;
    let restored = async {
        s.change(Scope::System, "CONTRAST", &old).await?;
        barrier(s).await?;
        let after = snapshots(address, code).await?;
        if after.0.fields != before.0.fields || after.1.fields != before.1.fields {
            return Err(
                "RESTORATION NOT CONFIRMED: inspect display contrast and device status".into(),
            );
        }
        println!(
            "Restored: all {} fields match the baseline.",
            after.0.fields.len() + after.1.fields.len()
        );
        Ok::<(), String>(())
    }
    .await;
    restored?;
    changed
}
