//! One independent session per processor.
//!
//! Every saved connection (or an address without one) gets its own `App`:
//! snapshot, profile, meter stream, command queue, pending operation,
//! reconnect and activity counters, all run by its own owner task. The app's
//! windows reach a processor under `/api/d/{device}/…`; the unprefixed routes
//! stay the browser's single session. A processor is held by at most one
//! session at a time, so two tabs never open two connections to it.
use crate::{App, Snapshot, api_error};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Request, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use serde_json::{Value as JsonValue, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

/// What every session of the service shares: which processor each one holds,
/// and the 8400's one meter port.
#[derive(Default)]
pub(crate) struct Peers {
    /// Session key → the processor address it is connected, connecting or
    /// reconnecting to.
    claims: Mutex<HashMap<String, String>>,
    pub(crate) meters_8400: crate::o8400::MeterListener,
}

/// A session's own name among its peers.
#[derive(Clone, Default)]
pub(crate) struct Peer {
    pub(crate) key: Arc<str>,
    pub(crate) peers: Arc<Peers>,
}

/// A processor as the sessions hold it: its address and PC Remote port (two
/// sites behind one NAT address differ by port).
fn address(host: &str, port: u16) -> String {
    format!("{}:{port}", host.trim().to_ascii_lowercase())
}

impl Peer {
    /// Holds `host` for this session before it connects. Another session
    /// that holds it keeps it: one processor, one session.
    pub(crate) fn claim(&self, host: &str, port: u16) -> Result<(), String> {
        let wanted = address(host, port);
        let mut claims = self
            .peers
            .claims
            .lock()
            .map_err(|_| "Session list unavailable")?;
        if claims
            .iter()
            .any(|(key, held)| **key != *self.key && *held == wanted)
        {
            return Err(format!(
                "{} is already open in another tab of this app. Switch to that tab instead of connecting twice.",
                host.trim()
            ));
        }
        claims.insert(self.key.to_string(), wanted);
        Ok(())
    }

    /// The processor this session holds after a turn of its owner: the one it
    /// is connected or reconnecting to, or none.
    pub(crate) fn hold(&self, held: Option<(&str, u16)>) {
        let Ok(mut claims) = self.peers.claims.lock() else {
            return;
        };
        match held {
            Some((host, port)) => {
                claims.insert(self.key.to_string(), address(host, port));
            }
            None => {
                claims.remove(&*self.key);
            }
        }
    }
}

/// A session key from a request path: a saved connection's id or
/// `host:<address>` for a connection without one.
fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 200
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:[]".contains(c))
}

struct Unit {
    app: App,
    routes: Router,
}

/// Every session of the service, by key.
#[derive(Clone)]
pub(crate) struct Hub {
    units: Arc<Mutex<HashMap<String, Unit>>>,
    /// The browser's session (unprefixed routes); new sessions share its
    /// connection book and peers.
    base: App,
    template: Router<App>,
}

impl Hub {
    pub(crate) fn new(base: App, template: Router<App>) -> Self {
        Self {
            units: Arc::default(),
            base,
            template,
        }
    }

    /// The session of `key`, started on first use with its own owner task.
    fn unit(&self, key: &str) -> Result<(App, Router), String> {
        let mut units = self.units.lock().map_err(|_| "Session list unavailable")?;
        if let Some(unit) = units.get(key) {
            return Ok((unit.app.clone(), unit.routes.clone()));
        }
        let app = crate::start_session(&self.base, key);
        let routes = self.template.clone().with_state(app.clone());
        units.insert(
            key.to_owned(),
            Unit {
                app: app.clone(),
                routes: routes.clone(),
            },
        );
        Ok((app, routes))
    }

    fn apps(&self) -> Vec<(String, App)> {
        self.units
            .lock()
            .map(|units| {
                units
                    .iter()
                    .map(|(k, u)| (k.clone(), u.app.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// `/api/d/{device}/{*rest}`: the request goes to that processor's own
/// session, as `/api/{rest}`.
pub(crate) async fn device_request(
    State(hub): State<Hub>,
    Path((key, rest)): Path<(String, String)>,
    mut request: Request,
) -> Response {
    if !valid_key(&key) {
        return api_error("Unknown processor").into_response();
    }
    let (_, routes) = match hub.unit(&key) {
        Ok(unit) => unit,
        Err(error) => return api_error(error).into_response(),
    };
    let query = request
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let Ok(uri) = format!("/api/{rest}{query}").parse::<Uri>() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    *request.uri_mut() = uri;
    match routes.oneshot(request.map(Body::new)).await {
        Ok(response) => response,
        Err(never) => match never {},
    }
}

/// The summary of one session the app's windows follow: who it is, whether
/// it is connected, and the revision that changes with every document.
fn summary(key: &str, state: &Snapshot) -> JsonValue {
    json!({
        "key": key,
        "connected": state.connected,
        "host": state.host,
        "firmware": state.firmware,
        "device_id": state.device_id,
        "session_id": state.session_id,
        "adapter_id": state.adapter_id,
        "model": state.model,
        "capabilities": state.capabilities,
        "revision": state.revision,
        "reconnect": state.reconnect,
        "error": state.error,
    })
}

/// `GET /api/sessions`: every processor session of the app, connected or not.
/// One read for all of them, so a window count never adds polling.
pub(crate) async fn sessions(State(hub): State<Hub>) -> Json<JsonValue> {
    let mut list = Vec::new();
    for (key, app) in hub.apps() {
        list.push(summary(&key, &*app.state.read().await));
    }
    list.sort_by(|a, b| a["key"].as_str().cmp(&b["key"].as_str()));
    Json(json!({ "sessions": list }))
}

/// `POST /api/disconnect-all`: ends every session (the app quits). Each
/// session disconnects on its own; one that fails never stops the others.
pub(crate) async fn disconnect_all(State(hub): State<Hub>) -> Json<JsonValue> {
    let mut sessions = hub.apps();
    sessions.push((String::new(), hub.base.clone()));
    let mut failures = Vec::new();
    let results = futures_join(sessions.into_iter().map(|(key, app)| async move {
        let connected = {
            let state = app.state.read().await;
            state.connected || state.reconnect.is_some()
        };
        if !connected {
            return (key, Ok(()));
        }
        // A disconnect also drops a pending automatic reconnect.
        (
            key,
            crate::dispatch(app, crate::Command::Disconnect)
                .await
                .map(|_| ()),
        )
    }))
    .await;
    for (key, result) in results {
        if let Err((_, Json(error))) = result {
            failures.push(json!({"key": key, "error": error["error"]}));
        }
    }
    Json(json!({ "ok": failures.is_empty(), "failures": failures }))
}

async fn futures_join<F>(futures: impl Iterator<Item = F>) -> Vec<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let handles: Vec<_> = futures.map(tokio::spawn).collect();
    let mut out = Vec::with_capacity(handles.len());
    for handle in handles {
        if let Ok(value) = handle.await {
            out.push(value);
        }
    }
    out
}
