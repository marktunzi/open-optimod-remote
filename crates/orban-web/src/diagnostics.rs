//! Bounded, redacted record of connection and session events.
//!
//! Field reports need the failing stage, model, firmware and port, never the
//! access code, a preset's content or the processor's address. Every detail is
//! redacted on entry, so an export of this log is safe to attach to an issue.
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const CAPACITY: usize = 256;
const DETAIL_LIMIT: usize = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Ok,
    Failed,
    Info,
}

#[derive(Clone, Debug, Serialize)]
pub struct Event {
    /// Milliseconds since the Unix epoch.
    pub at_ms: u64,
    pub stage: &'static str,
    pub outcome: Outcome,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

#[derive(Default)]
pub struct Log {
    events: Mutex<VecDeque<Event>>,
}

impl Log {
    pub fn record(
        &self,
        stage: &'static str,
        outcome: Outcome,
        detail: impl AsRef<str>,
        elapsed: Option<Duration>,
    ) {
        let event = Event {
            at_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_millis() as u64),
            stage,
            outcome,
            detail: redact(detail.as_ref()),
            elapsed_ms: elapsed.map(|elapsed| elapsed.as_millis() as u64),
        };
        let Ok(mut events) = self.events.lock() else {
            return;
        };
        if events.len() == CAPACITY {
            events.pop_front();
        }
        events.push_back(event);
    }

    pub fn snapshot(&self) -> Vec<Event> {
        self.events
            .lock()
            .map(|events| events.iter().cloned().collect())
            .unwrap_or_default()
    }
}

/// Removes bracketed text (terminal commands carry the access code as
/// `[CODE]`) and IPv4 addresses other than firmware versions, and bounds the
/// length.
pub fn redact(text: &str) -> String {
    let mut unbracketed = String::with_capacity(text.len());
    let mut depth = 0_usize;
    for character in text.chars() {
        match character {
            '[' => {
                if depth == 0 {
                    unbracketed.push_str("[redacted]");
                }
                depth += 1;
            }
            ']' if depth > 0 => depth -= 1,
            _ if depth > 0 => {}
            _ => unbracketed.push(character),
        }
    }
    let mut redacted = String::with_capacity(unbracketed.len());
    let mut rest = unbracketed.as_str();
    while let Some(start) = rest.find(|c: char| c.is_ascii_digit()) {
        redacted.push_str(&rest[..start]);
        let candidate = &rest[start..];
        let end = candidate
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(candidate.len());
        let token = candidate[..end].trim_end_matches('.');
        // Firmware banners such as `5500 V 1.2.8.24` have the shape of an address.
        if is_ipv4(token) && !redacted.ends_with("V ") {
            redacted.push_str("<address>");
        } else {
            redacted.push_str(token);
        }
        rest = &candidate[token.len()..];
    }
    redacted.push_str(rest);
    if redacted.chars().count() > DETAIL_LIMIT {
        redacted = redacted.chars().take(DETAIL_LIMIT).collect::<String>() + "…";
    }
    redacted
}

fn is_ipv4(token: &str) -> bool {
    let parts = token.split('.').collect::<Vec<_>>();
    parts.len() == 4
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.len() <= 3 && part.parse::<u8>().is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_removes_codes_and_addresses_but_keeps_versions_and_ports() {
        assert_eq!(
            redact("AP [1234]?? to 192.168.1.20 port 6201, 5500 V 1.2.8.24, 8300.10"),
            "AP [redacted]?? to <address> port 6201, 5500 V 1.2.8.24, 8300.10"
        );
        assert_eq!(redact("nested [a [b] c] end"), "nested [redacted] end");
        assert_eq!(redact("unclosed [secret"), "unclosed [redacted]");
        assert_eq!(redact(&"x".repeat(400)).chars().count(), DETAIL_LIMIT + 1);
    }

    #[test]
    fn the_log_keeps_only_the_most_recent_events() {
        let log = Log::default();
        for _ in 0..CAPACITY + 10 {
            log.record("test", Outcome::Info, "event", None);
        }
        assert_eq!(log.snapshot().len(), CAPACITY);
    }
}
