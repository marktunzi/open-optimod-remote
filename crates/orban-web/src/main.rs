mod automation;
mod devices;
mod o8400;
mod security;
mod sessions;
use axum::{
    Json, Router,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{
        Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use orban_protocol::{
    adapter::{Capabilities, DeviceModel, Evidence, SkinId},
    control::Scope,
    document::{Document, Field, Value},
    message::Message,
    presets::{Preset, PresetKind},
    profile::Profile,
    session::Session,
    terminal::read_snapshot_for_model,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    ffi::OsStr,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{RwLock, mpsc, oneshot, watch},
    time::{MissedTickBehavior, interval, sleep},
};
use tokio_stream::{StreamExt, wrappers::WatchStream};
use tower_http::services::ServeDir;
use zeroize::Zeroizing;
/// The loopback port of the service. `OPEN_OPTIMOD_PORT` moves a
/// development or test instance next to an installed app, which keeps 5701.
static SERVICE_PORT: std::sync::LazyLock<u16> =
    std::sync::LazyLock::new(|| service_port(std::env::var("OPEN_OPTIMOD_PORT").ok().as_deref()));
static ORIGIN: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| format!("http://127.0.0.1:{}", *SERVICE_PORT));
static HOST: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| format!("127.0.0.1:{}", *SERVICE_PORT));
fn service_port(value: Option<&str>) -> u16 {
    value
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|port| *port >= 1024)
        .unwrap_or(5701)
}
const METER_POLL_PERIOD: Duration = Duration::from_millis(50);
/// A response boundary unanswered this long means the processor stopped
/// serving the session. Slow or failed ticks before that are not a lost
/// connection: the original PC Remote waits about 42 s without any byte.
const UNANSWERED_LIMIT: Duration = Duration::from_secs(20);
/// Reads of the on-air archive after a push, and the wait before each: the
/// unit saves the archive file a moment after the change.
const ARCHIVE_READS: u8 = 5;
const ARCHIVE_RETRY: Duration = Duration::from_secs(2);
/// The 8700HD saves its on-air file 4 to 7 s after a change (original
/// firmware 1.0.2.161, 21 October 2026); a readback waits this long for it.
const FILE_READBACK_LIMIT: Duration = Duration::from_secs(15);
const FILE_READBACK_INTERVAL: Duration = Duration::from_secs(1);
/// How often the active input is read again while a fallback lets the
/// firmware change it on its own.
const INPUT_STATUS_PERIOD: Duration = Duration::from_secs(10);
/// PC Remote login including the TCP connection, and each wait for a reply
/// on the session (such as a response boundary).
const SESSION_DEADLINE: Duration = Duration::from_secs(4);
/// A login the unit answers with `busy` (it is still closing the previous
/// session) is tried once more after this wait; nothing was sent yet.
const BUSY_RETRY: Duration = Duration::from_secs(1);
/// Terminal reads after login: the system document, then the preset list
/// with the processing document in one exchange.
const SYSTEM_READ_DEADLINE: Duration = Duration::from_secs(4);
const CATALOG_READ_DEADLINE: Duration = Duration::from_secs(6);
/// Recall: one RP/AP exchange plus at most one confirming AP read.
const RECALL_DEADLINE: Duration = Duration::from_secs(10);
const RECALL_CONFIRM_DEADLINE: Duration = Duration::from_secs(6);
fn should_open_default_browser(value: Option<&OsStr>) -> bool {
    value != Some(OsStr::new("1"))
}
fn bundled_asset_candidates(executable: &std::path::Path) -> [std::path::PathBuf; 2] {
    let directory = executable.parent().unwrap_or(executable);
    [directory.join("../ui"), directory.join("../Resources/ui")]
}
#[derive(Clone, Serialize, Default)]
struct Snapshot {
    connected: bool,
    firmware: String,
    host: String,
    processing: Option<Document>,
    system: Option<Document>,
    error: Option<String>,
    revision: u64,
    write_enabled: bool,
    /// What the session may change at its access level (the client's
    /// Security levels); `write_enabled` is its Modify right.
    access: Option<orban_protocol::access::Access>,
    device_id: Option<String>,
    session_id: Option<String>,
    presets: Vec<orban_protocol::presets::Preset>,
    preset_base_name: Option<String>,
    modified_fields: BTreeSet<String>,
    less_more_available: bool,
    model: DeviceModel,
    skin_id: Option<SkinId>,
    adapter_id: Option<String>,
    capabilities: Option<Capabilities>,
    /// Hardware-verified or statically derived profile, for the status display.
    evidence: Option<Evidence>,
    /// Set while the PC Remote session is connected but the current settings
    /// or presets could not be read from the status (terminal) port.
    sync_error: Option<String>,
    /// Set while the app waits to reconnect after an unexpected session loss.
    reconnect: Option<ReconnectStatus>,
    /// The status text the unit sends after the PC Remote login (opcode
    /// 205): what the original client shows as its Diagnostics.
    diagnostics: Option<String>,
    /// System fields shown but never written: those of the unit's network
    /// file, which the original client changes as a file.
    read_only_fields: BTreeSet<String>,
}

#[derive(Clone, Serialize)]
struct ReconnectStatus {
    /// The attempt that comes next, counted from 1.
    attempt: u32,
    attempts: u32,
    /// Wall-clock time of that attempt, in milliseconds since the Unix epoch.
    next_attempt_at_ms: u64,
    /// True while that attempt is running.
    in_progress: bool,
    /// Why the session was lost, or why the last attempt failed.
    reason: String,
}
#[derive(Clone)]
struct App {
    state: Arc<RwLock<Snapshot>>,
    meters: watch::Sender<JsonValue>,
    commands: mpsc::Sender<Command>,
    /// Profile of the active session for the interface: its own profile when the
    /// adapter has one, otherwise the 5700i reference used for read-only display.
    profile: Arc<std::sync::RwLock<Arc<Profile>>>,
    book: Arc<std::sync::Mutex<devices::Book>>,
    /// What the owner did on the processor, so the app's windows can show
    /// that they share one session instead of each opening their own.
    activity: Arc<Activity>,
    /// This session's name among the app's other processor sessions.
    peer: sessions::Peer,
}
/// Counters of processor traffic since the service started. A window that
/// opens, closes or reloads must not move any of them except meter readers.
#[derive(Default)]
struct Activity {
    /// Logins started, by hand or by an automatic reconnect.
    session_opens: AtomicU64,
    /// Meter requests sent by the one meter poll (PC Remote, opcode 219).
    meter_polls: AtomicU64,
    /// Live meter frames received from the processor.
    meter_frames: AtomicU64,
    /// Commands that can change the processor: edits, recalls, preset
    /// files, preset uploads and setup restores.
    device_writes: AtomicU64,
}
impl Activity {
    fn count(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}
type Reply = oneshot::Sender<Result<(), String>>;
enum Command {
    Connect(Connect, Reply),
    Disconnect(Reply),
    Change(Change, Reply),
    Refresh(Reply),
    Recall(Recall, Reply),
    ApplyPreset(ApplyPreset, Reply),
    PresetFile(PresetFile, Reply),
    RestoreSetup(RestoreSetup, Reply),
    BackupFiles(oneshot::Sender<Result<Vec<BackupFile>, String>>),
    /// The unit's automation (235) or passcode (230) file, decrypted.
    ReadUnitFile(u16, oneshot::Sender<Result<Option<String>, String>>),
    WriteUnitFile(UnitFileWrite, Reply),
    RestorePresetFiles(RestorePresetFiles, Reply),
    SetupPresets(oneshot::Sender<Result<Vec<String>, String>>),
    SetupPreset(SetupPresetAction, Reply),
    CancelReconnect(Reply),
}
/// A file of the original PC Remote's backup: its name on the unit and its
/// decrypted text, as the client's plain-text backup stores it.
#[derive(Serialize, Deserialize)]
struct BackupFile {
    name: String,
    document: String,
    /// Restore only: the file's modification time, milliseconds since 1970.
    #[serde(default)]
    modified: Option<u64>,
}
/// A unit file the original PC Remote replaces whole: the automation
/// events (235, named) or the passcodes (230, without a name).
struct UnitFileWrite {
    kind: u16,
    document: Zeroizing<String>,
    expected_host: String,
    expected_session: String,
}
/// The client's Open Setup (`open`) or Save Setup As (`save_as`).
#[derive(Deserialize)]
struct SetupPresetAction {
    action: String,
    name: String,
    confirmed: bool,
    expected_host: String,
    expected_session: String,
}
#[derive(Deserialize)]
struct RestorePresetFiles {
    files: Vec<BackupFile>,
    confirmed: bool,
    expected_host: String,
    expected_session: String,
}
#[derive(Deserialize)]
struct Connect {
    host: String,
    code: String,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default = "default_terminal")]
    terminal_port: u16,
    #[serde(default)]
    model: DeviceModel,
    #[serde(skip)]
    device_id: Option<String>,
}
fn default_port() -> u16 {
    6201
}
fn default_terminal() -> u16 {
    23
}
#[derive(Deserialize)]
struct Change {
    scope: Scope,
    name: String,
    index: u32,
    expected: Field,
    requested: Option<Value>,
    expected_host: String,
    expected_session: String,
    expected_preset: Option<String>,
}
#[derive(Deserialize)]
struct Recall {
    name: String,
    expected_name: String,
    confirmed: bool,
    expected_host: String,
    expected_session: String,
}
/// Save, delete or rename a user preset on the processor.
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum PresetFileAction {
    /// Save the on-air processing under a new name.
    Save { name: String },
    /// Delete a user preset that is not on air.
    Delete { name: String },
    /// Save the unmodified on-air user preset under a new name, then delete the old one.
    Rename { name: String, new_name: String },
}
#[derive(Deserialize)]
struct PresetFile {
    #[serde(flatten)]
    action: PresetFileAction,
    expected_name: String,
    confirmed: bool,
    expected_host: String,
    expected_session: String,
}
/// A system (AS) document from a backup, restored field by field.
#[derive(Deserialize)]
struct RestoreSetup {
    document: String,
    confirmed: bool,
    expected_host: String,
    expected_session: String,
}
#[derive(Deserialize)]
struct SetupDocument {
    document: String,
}
#[derive(Deserialize)]
struct ApplyPreset {
    document: String,
    expected_name: String,
    confirmed: bool,
    expected_host: String,
    expected_session: String,
}

#[derive(Debug)]
struct OperationError {
    message: String,
}

impl OperationError {
    fn safe(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn uncertain(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
/// Response boundaries (250) sent on the PC Remote session and not yet
/// answered (251). Every 250 gets exactly one 251, so a late answer to a
/// timed-out tick can never confirm a later write.
#[derive(Default)]
struct Boundaries {
    outstanding: usize,
    /// When the unit last had nothing left to answer. Measured from there, a
    /// lost 251 that keeps the count from reaching zero still ends the session.
    since: Option<tokio::time::Instant>,
}

impl Boundaries {
    fn sent(&mut self) {
        if self.outstanding == 0 {
            self.since = Some(tokio::time::Instant::now());
        }
        self.outstanding += 1;
    }
    /// Records one 251; true when no boundary is outstanding any more.
    fn answered(&mut self) -> bool {
        self.outstanding = self.outstanding.saturating_sub(1);
        if self.outstanding == 0 {
            self.since = None;
        }
        self.outstanding == 0
    }
    fn unanswered_for(&self) -> Duration {
        self.since.map_or(Duration::ZERO, |since| since.elapsed())
    }
    /// The 5700i drops whole frames when its send buffer is full, a 251 among
    /// them. Once it has answered later requests, the missing 251 will never
    /// come and is forgotten.
    fn dropped(&mut self) {
        self.outstanding = 0;
        self.since = None;
    }
}

/// The encrypted on-air file the unit pushed: whole, or the part that
/// arrived before the unit cut the frame.
enum Pushed {
    Whole(Vec<u8>),
    Start(Vec<u8>),
}

struct Device {
    session: Session,
    boundaries: Boundaries,
    /// Set when a poll during other work found the PC Remote session lost.
    lost: Option<String>,
    /// The on-air document the unit pushed; see `on_air_pushed`.
    pushed: Option<Pushed>,
    /// The start of the pushed document that a later archive must match.
    expected: Option<Document>,
    /// An on-air archive (opcode 239) is on its way, and the one received.
    archive_requested: bool,
    archive: Option<Vec<u8>>,
    /// The next read of the on-air archive after one that failed, and how
    /// many reads are left.
    archive_retry: Option<(tokio::time::Instant, u8)>,
    /// The opcode of the files a request is waiting for, and those received.
    wanted_files: Option<u16>,
    files: Vec<(String, Vec<u8>)>,
    /// The documents of the user presets read over the PC Remote session,
    /// by preset name; a Recall sends one of them back (file-sync models).
    user_presets: BTreeMap<String, Zeroizing<Vec<u8>>>,
    terminal: SocketAddr,
    code: Zeroizing<String>,
    processing_baseline: Option<Document>,
    /// The exact-banner profile writes are validated against. `None` = read-only.
    profile: Option<Arc<Profile>>,
    /// When the active input (`ACTUAL A OR D`) is read again; see `input_status`.
    input_status: Option<InputStatus>,
    /// What the last synchronization could not read over the status port,
    /// as shown to the user. Changes wait for a complete one: their
    /// confirmation is read on that port.
    sync_gap: Option<String>,
    /// The status port did not answer this session's synchronization, and
    /// the model can be run as its original PC Remote does: settings,
    /// presets, confirmations and Recall all over the PC Remote session.
    status_port_down: bool,
}

/// The next read of the active input, and whether it follows a change of the
/// input choice (then the read after it waits for the fallback's silence delay).
#[derive(Clone, Copy)]
struct InputStatus {
    at: tokio::time::Instant,
    after_change: bool,
}
/// A new processor session named `key`: its own state, meters, commands,
/// profile and owner task, sharing the connection book and peers of `base`.
fn start_session(base: &App, key: &str) -> App {
    let (tx, rx) = mpsc::channel(8);
    let (meters, _) = watch::channel(json!({"live":false}));
    let app = App {
        activity: Arc::default(),
        peer: sessions::Peer {
            key: key.into(),
            peers: base.peer.peers.clone(),
        },
        state: Arc::new(RwLock::new(Snapshot::default())),
        meters,
        commands: tx,
        profile: shared_profile(reference_profile()),
        book: base.book.clone(),
    };
    tokio::spawn(owner(app.clone(), rx));
    app
}
fn reference_profile() -> Arc<Profile> {
    Arc::new(Profile::embedded().expect("Valid bundled profile"))
}
fn shared_profile(profile: Arc<Profile>) -> Arc<std::sync::RwLock<Arc<Profile>>> {
    Arc::new(std::sync::RwLock::new(profile))
}
#[tokio::main]
async fn main() {
    let profile = shared_profile(reference_profile());
    let (tx, rx) = mpsc::channel(8);
    let (meters, _) = watch::channel(json!({"live":false}));
    let app = App {
        activity: Arc::default(),
        peer: sessions::Peer::default(),
        state: Arc::new(RwLock::new(Snapshot::default())),
        meters,
        commands: tx,
        profile,
        book: Arc::new(std::sync::Mutex::new(
            devices::Book::open(
                devices::data_file().expect("Application data directory"),
                Box::new(devices::FileVault::new(
                    devices::credential_file().expect("Application credential directory"),
                )),
            )
            .expect("Connection book can be read"),
        )),
    };
    tokio::spawn(owner(app.clone(), rx));
    let hub = sessions::Hub::new(app.clone(), device_routes());
    let bundled_assets = std::env::current_exe().ok().and_then(|executable| {
        bundled_asset_candidates(&executable)
            .into_iter()
            .find(|path| path.is_dir())
    });
    let bundled = bundled_assets.is_some();
    let assets = std::env::var_os("OPTIMOD_UI_DIR")
        .map(std::path::PathBuf::from)
        .or(bundled_assets)
        .unwrap_or_else(|| "packages/ui/dist".into());
    let sessions = Router::new()
        .route(
            "/api/d/{device}/{*rest}",
            axum::routing::any(sessions::device_request),
        )
        .route("/api/sessions", get(sessions::sessions))
        .route("/api/disconnect-all", post(sessions::disconnect_all))
        .with_state(hub);
    let routes = Router::new()
        .route("/api/health", get(|| async { "open-optimod-remote/0.2.0" }))
        .merge(device_routes().with_state(app))
        .merge(sessions)
        .fallback_service(ServeDir::new(assets))
        .layer(middleware::from_fn(local_origin));
    let listener = tokio::net::TcpListener::bind(HOST.as_str())
        .await
        .unwrap_or_else(|_| panic!("Local port {} available", *SERVICE_PORT));
    if bundled {
        if let Some(home) = std::env::var_os("HOME") {
            let cache = std::path::PathBuf::from(home).join("Library/Caches/OpenOptimodRemote");
            let _ = std::fs::create_dir_all(&cache);
            let _ = std::fs::write(cache.join("service.pid"), std::process::id().to_string());
        }
        if should_open_default_browser(std::env::var_os("OPEN_OPTIMOD_NO_BROWSER").as_deref()) {
            #[cfg(target_os = "macos")]
            tokio::spawn(async {
                tokio::time::sleep(Duration::from_millis(250)).await;
                let _ = std::process::Command::new("/usr/bin/open")
                    .arg(ORIGIN.as_str())
                    .spawn();
            });
        }
    }
    println!("Open Optimod Remote development build: {}", *ORIGIN);
    axum::serve(listener, routes).await.unwrap();
}
/// The routes of one processor session, the same under `/api/d/{device}/`
/// as unprefixed for the browser's session.
fn device_routes() -> Router<App> {
    Router::new()
        .route("/api/state", get(state))
        .route("/api/profile", get(profile_handler))
        .route("/api/meters", get(meters_handler))
        .route("/api/diagnostics", get(diagnostics_handler))
        .route("/api/connect", post(connect_handler))
        .route("/api/disconnect", post(disconnect_handler))
        .route("/api/reconnect/cancel", post(cancel_reconnect_handler))
        .route("/api/change", post(change_handler))
        .route("/api/refresh", post(refresh_handler))
        .route("/api/presets/recall", post(recall_handler))
        .route("/api/presets/current-file", get(preset_file_handler))
        .route("/api/presets/apply-file", post(apply_preset_handler))
        .route("/api/presets/manage", post(preset_file_action_handler))
        .route("/api/backup", get(backup_handler))
        .route("/api/backup/setup-plan", post(setup_plan_handler))
        .route("/api/backup/restore-setup", post(restore_setup_handler))
        .route("/api/backup/files", get(backup_files_handler))
        .route(
            "/api/setup-presets",
            get(setup_presets_handler).post(setup_preset_handler),
        )
        .route("/api/backup/encrypt", post(backup_encrypt_handler))
        .route("/api/backup/decrypt", post(backup_decrypt_handler))
        .route(
            "/api/automation",
            get(automation_handler).post(automation_write_handler),
        )
        .route(
            "/api/security",
            get(security_handler).post(security_write_handler),
        )
        .route(
            "/api/backup/restore-presets",
            post(restore_preset_files_handler),
        )
        .route("/api/devices", get(devices_list).post(devices_save))
        .route("/api/devices/delete", post(devices_delete))
        .route("/api/devices/groups", post(devices_groups))
        .route("/api/devices/code", post(devices_code))
        .route("/api/devices/connect", post(devices_connect))
}
async fn local_origin(req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    let origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    let valid_host = host == Some(HOST.as_str());
    let mutation =
        req.method() != axum::http::Method::GET && req.method() != axum::http::Method::HEAD;
    // A GET from an image or link on another site carries no Origin, but a
    // browser marks it cross-site. Some GETs read from the processor.
    let foreign_site = req
        .headers()
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|site| site != "same-origin" && site != "none");
    if !valid_host
        || foreign_site
        || (origin.is_some() && origin != Some(ORIGIN.as_str()))
        || (mutation && origin != Some(ORIGIN.as_str()))
    {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(axum::body::Body::from(
                "Open the app from its local address",
            ))
            .unwrap();
    }
    let mut response = next.run(req).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response.headers_mut().insert(header::CONTENT_SECURITY_POLICY,"default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'".parse().unwrap());
    response
}
async fn state(State(app): State<App>) -> Json<Snapshot> {
    Json(app.state.read().await.clone())
}
async fn profile_handler(State(app): State<App>) -> Json<Profile> {
    let active = app
        .profile
        .read()
        .map(|profile| profile.clone())
        .unwrap_or_else(|_| reference_profile());
    Json((*active).clone())
}
async fn meters_handler(
    State(app): State<App>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    Sse::new(
        WatchStream::new(app.meters.subscribe()).map(|v| Ok(Event::default().data(v.to_string()))),
    )
    .keep_alive(KeepAlive::default())
}
/// The shared session as the app's windows see it: which processor, and how
/// much traffic the one owner has had with it. Reading this sends nothing.
async fn diagnostics_handler(State(app): State<App>) -> Json<JsonValue> {
    let state = app.state.read().await;
    let activity = &app.activity;
    Json(json!({
        "device_id": state.device_id,
        "session_id": state.session_id,
        "connected": state.connected,
        "session_opens": activity.session_opens.load(Ordering::Relaxed),
        "meter_polls": activity.meter_polls.load(Ordering::Relaxed),
        "meter_frames": activity.meter_frames.load(Ordering::Relaxed),
        "device_writes": activity.device_writes.load(Ordering::Relaxed),
        "meter_readers": app.meters.receiver_count(),
    }))
}
type ApiResult = Result<Json<JsonValue>, (StatusCode, Json<JsonValue>)>;
fn api_error(e: impl ToString) -> (StatusCode, Json<JsonValue>) {
    (StatusCode::CONFLICT, Json(json!({"error":e.to_string()})))
}
async fn dispatch(app: App, build: impl FnOnce(Reply) -> Command) -> ApiResult {
    let (tx, rx) = oneshot::channel();
    app.commands
        .try_send(build(tx))
        .map_err(|_| api_error("Another command is still in progress"))?;
    rx.await.map_err(api_error)?.map_err(api_error)?;
    Ok(Json(json!({"ok":true})))
}
async fn connect_handler(State(app): State<App>, Json(body): Json<Connect>) -> ApiResult {
    dispatch(app, |r| Command::Connect(body, r)).await
}
async fn disconnect_handler(State(app): State<App>) -> ApiResult {
    dispatch(app, Command::Disconnect).await
}
async fn cancel_reconnect_handler(State(app): State<App>) -> ApiResult {
    dispatch(app, Command::CancelReconnect).await
}
async fn refresh_handler(State(app): State<App>) -> ApiResult {
    dispatch(app, Command::Refresh).await
}
async fn recall_handler(State(app): State<App>, Json(body): Json<Recall>) -> ApiResult {
    dispatch(app, |r| Command::Recall(body, r)).await
}
async fn preset_file_handler(State(app): State<App>) -> ApiResult {
    let _ = dispatch(app.clone(), Command::Refresh).await?;
    let state = app.state.read().await;
    let document = state
        .processing
        .as_ref()
        .ok_or_else(|| api_error("Connect to an Optimod before saving a preset file"))?;
    Ok(Json(
        json!({"name": document.name, "document": document.raw}),
    ))
}
async fn preset_file_action_handler(
    State(app): State<App>,
    Json(body): Json<PresetFile>,
) -> ApiResult {
    dispatch(app, |r| Command::PresetFile(body, r)).await
}
/// Refreshes both documents and the preset list, then returns them as one
/// backup. User presets other than the on-air one cannot be read without
/// recalling them, so only their names are included.
async fn backup_handler(State(app): State<App>) -> ApiResult {
    let _ = dispatch(app.clone(), Command::Refresh).await?;
    let state = app.state.read().await;
    let (Some(processing), Some(system)) = (&state.processing, &state.system) else {
        return Err(api_error("Connect to an Optimod before making a backup"));
    };
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    Ok(Json(json!({
        "format": "open-optimod-backup",
        "version": 1,
        "created": created,
        "model": state.model.label(),
        "adapter_id": state.adapter_id,
        "firmware": state.firmware,
        "processing": {"name": processing.name, "document": processing.raw},
        "system": {"document": system.raw},
        "presets": state.presets,
    })))
}
/// Lists which settings a restore would change and which it leaves alone,
/// without contacting the processor.
async fn setup_plan_handler(State(app): State<App>, Json(body): Json<SetupDocument>) -> ApiResult {
    let state = app.state.read().await;
    let current = state
        .system
        .as_ref()
        .ok_or_else(|| api_error("Connect to an Optimod before restoring settings"))?;
    let imported =
        Document::parse_for_model(body.document.as_bytes(), state.model).map_err(api_error)?;
    let profile = app
        .profile
        .read()
        .map_err(|_| api_error("Profile unavailable"))?
        .clone();
    let (changes, skipped) = setup_restore_plan(&profile, current, &imported);
    Ok(Json(json!({
        "changes": changes.iter().map(|(name, field)| json!({"name": name, "from": current.fields.get(name), "to": field})).collect::<Vec<_>>(),
        "skipped": skipped.iter().map(|(name, reason)| json!({"name": name, "reason": reason})).collect::<Vec<_>>(),
    })))
}
/// The original PC Remote's backup (models whose user presets are files on
/// the unit): the setup file and every user preset file, decrypted.
async fn backup_files_handler(State(app): State<App>) -> ApiResult {
    let (tx, rx) = oneshot::channel();
    app.commands
        .try_send(Command::BackupFiles(tx))
        .map_err(|_| api_error("Another command is still in progress"))?;
    let files = rx.await.map_err(api_error)?.map_err(api_error)?;
    let state = app.state.read().await;
    Ok(Json(json!({
        "model": state.model.label(),
        "adapter_id": state.adapter_id,
        "firmware": state.firmware,
        "files": files,
    })))
}
/// A backup file of the original client: plain text, or encrypted as the
/// unit's own files are (`archive`) with the session's access code or a
/// password the user chose (its "Choose backup file format" dialog).
#[derive(Deserialize, Serialize)]
struct CryptFile {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    document: Option<String>,
    /// The file's bytes, hexadecimal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    data: Option<String>,
}
#[derive(Deserialize)]
struct BackupCrypt {
    files: Vec<CryptFile>,
    /// None: the access code of the connected processor's saved connection.
    #[serde(default)]
    password: Option<String>,
}
fn plain_backup(bytes: &[u8]) -> bool {
    bytes.is_empty() || bytes.starts_with(b"OptimodVersion=<")
}
fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn from_hex(text: &str) -> Result<Vec<u8>, String> {
    if text.len() % 2 != 0 || text.len() > 2 * 65_535 {
        return Err("Invalid file data".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|_| "Invalid file data".into()))
        .collect()
}
/// The password of a backup: the one given, else the session's access code.
async fn backup_password(
    app: &App,
    password: Option<String>,
) -> Result<Zeroizing<String>, (StatusCode, Json<JsonValue>)> {
    if let Some(password) = password.filter(|p| !p.is_empty()) {
        if !password.is_ascii() || password.len() > 64 {
            return Err(api_error("Use a password of up to 64 plain characters"));
        }
        return Ok(Zeroizing::new(password));
    }
    let id = {
        let state = app.state.read().await;
        if !state.connected {
            return Err(api_error("Connect to the processor first"));
        }
        state
            .device_id
            .clone()
            .ok_or_else(|| api_error("Enter the backup password"))?
    };
    book_task(app, move |book| book.code(&id)).await
}
async fn backup_encrypt_handler(
    State(app): State<App>,
    Json(body): Json<BackupCrypt>,
) -> ApiResult {
    let code = backup_password(&app, body.password).await?;
    let mut files = Vec::new();
    for file in body.files {
        let plain = file
            .document
            .ok_or_else(|| api_error("Missing file text"))?;
        let iv = *uuid::Uuid::new_v4().as_bytes();
        let data = if plain.is_empty() {
            // An empty file is the IV alone, as the client writes an empty automation list.
            iv.to_vec()
        } else {
            orban_protocol::archive::encrypt(plain.as_bytes(), &code, iv).map_err(api_error)?
        };
        files.push(CryptFile {
            name: file.name,
            document: None,
            data: Some(to_hex(&data)),
        });
    }
    Ok(Json(json!({ "files": files })))
}
async fn backup_decrypt_handler(
    State(app): State<App>,
    Json(body): Json<BackupCrypt>,
) -> ApiResult {
    let mut code = None;
    let mut files = Vec::new();
    for file in body.files {
        let bytes = from_hex(file.data.as_deref().unwrap_or_default()).map_err(api_error)?;
        let plain = if plain_backup(&bytes) {
            bytes
        } else {
            if code.is_none() {
                code = Some(backup_password(&app, body.password.clone()).await?);
            }
            let plain = if bytes.len() == 16 {
                Vec::new()
            } else {
                orban_protocol::archive::decrypt(&bytes, code.as_ref().unwrap()).unwrap_or_default()
            };
            if !plain_backup(&plain) || (plain.is_empty() && bytes.len() != 16) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(
                        json!({"error": format!("{} is encrypted with another password", file.name), "needs_password": true}),
                    ),
                ));
            }
            plain
        };
        let document = String::from_utf8(plain)
            .map_err(|_| api_error(format!("{} is not a backup file", file.name)))?;
        files.push(CryptFile {
            name: file.name,
            document: Some(document),
            data: None,
        });
    }
    Ok(Json(json!({ "files": files })))
}
async fn read_unit_file(
    app: &App,
    request: u16,
) -> Result<Option<String>, (StatusCode, Json<JsonValue>)> {
    let (tx, rx) = oneshot::channel();
    app.commands
        .try_send(Command::ReadUnitFile(request, tx))
        .map_err(|_| api_error("Another command is still in progress"))?;
    rx.await.map_err(api_error)?.map_err(api_error)
}
async fn automation_handler(State(app): State<App>) -> ApiResult {
    let text = read_unit_file(&app, 235).await?;
    let events = text
        .as_deref()
        .map(automation::parse)
        .transpose()
        .map_err(api_error)?
        .unwrap_or_default();
    let (functions, _, _) = automation::flavour(app.state.read().await.model);
    Ok(Json(json!({"events": events, "functions": functions})))
}
#[derive(Deserialize)]
struct AutomationWrite {
    events: Vec<automation::Event>,
    confirmed: bool,
    expected_host: String,
    expected_session: String,
}
async fn automation_write_handler(
    State(app): State<App>,
    Json(body): Json<AutomationWrite>,
) -> ApiResult {
    if !body.confirmed {
        return Err(api_error("Confirm the change first"));
    }
    let model = app.state.read().await.model;
    let document = Zeroizing::new(automation::render(&body.events, model).map_err(api_error)?);
    dispatch(app, |r| {
        Command::WriteUnitFile(
            UnitFileWrite {
                kind: 235,
                document,
                expected_host: body.expected_host,
                expected_session: body.expected_session,
            },
            r,
        )
    })
    .await
}
async fn security_handler(State(app): State<App>) -> ApiResult {
    let text = read_unit_file(&app, 230).await?;
    let entries = text
        .as_deref()
        .map(security::parse)
        .transpose()
        .map_err(api_error)?
        .unwrap_or_default();
    let (levels, _, _) = security::flavour(app.state.read().await.model);
    Ok(Json(json!({"passcodes": entries, "levels": levels})))
}
#[derive(Deserialize)]
struct SecurityWrite {
    passcodes: Vec<security::Passcode>,
    confirmed: bool,
    expected_host: String,
    expected_session: String,
}
async fn security_write_handler(
    State(app): State<App>,
    Json(body): Json<SecurityWrite>,
) -> ApiResult {
    if !body.confirmed {
        return Err(api_error("Confirm the change first"));
    }
    let model = app.state.read().await.model;
    let document = Zeroizing::new(security::render(&body.passcodes, model).map_err(api_error)?);
    dispatch(app, |r| {
        Command::WriteUnitFile(
            UnitFileWrite {
                kind: 230,
                document,
                expected_host: body.expected_host,
                expected_session: body.expected_session,
            },
            r,
        )
    })
    .await
}
async fn setup_presets_handler(State(app): State<App>) -> ApiResult {
    let (tx, rx) = oneshot::channel();
    app.commands
        .try_send(Command::SetupPresets(tx))
        .map_err(|_| api_error("Another command is still in progress"))?;
    let names = rx.await.map_err(api_error)?.map_err(api_error)?;
    Ok(Json(json!({ "names": names })))
}
async fn setup_preset_handler(
    State(app): State<App>,
    Json(body): Json<SetupPresetAction>,
) -> ApiResult {
    dispatch(app, |r| Command::SetupPreset(body, r)).await
}
async fn restore_preset_files_handler(
    State(app): State<App>,
    Json(body): Json<RestorePresetFiles>,
) -> ApiResult {
    dispatch(app, |r| Command::RestorePresetFiles(body, r)).await
}
async fn restore_setup_handler(
    State(app): State<App>,
    Json(body): Json<RestoreSetup>,
) -> ApiResult {
    dispatch(app, |r| Command::RestoreSetup(body, r)).await
}
async fn apply_preset_handler(State(app): State<App>, Json(body): Json<ApplyPreset>) -> ApiResult {
    dispatch(app, |r| Command::ApplyPreset(body, r)).await
}
async fn change_handler(State(app): State<App>, Json(body): Json<Change>) -> ApiResult {
    dispatch(app, |r| Command::Change(body, r)).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceId {
    id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedCode {
    id: String,
    code: Option<String>,
}
async fn book_task<T: Send + 'static>(
    app: &App,
    work: impl FnOnce(&mut devices::Book) -> Result<T, String> + Send + 'static,
) -> Result<T, (StatusCode, Json<JsonValue>)> {
    let book = app.book.clone();
    tokio::task::spawn_blocking(move || {
        work(&mut *book.lock().map_err(|_| "Connection list is unavailable")?)
    })
    .await
    .map_err(api_error)?
    .map_err(api_error)
}
async fn devices_list(State(app): State<App>) -> ApiResult {
    let (devices, groups) = book_task(&app, |book| Ok((book.list(), book.groups()))).await?;
    Ok(Json(
        json!({"devices":devices,"groups":groups,"credential_storage":"local"}),
    ))
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum GroupEdit {
    Create { name: String },
    Rename { name: String, new_name: String },
    Delete { name: String },
}
/// New, Rename and Delete Group of the original client's connection list.
/// Deleting a group deletes its connections; the active one is disconnected.
async fn devices_groups(State(app): State<App>, Json(edit): Json<GroupEdit>) -> ApiResult {
    let removed = book_task(&app, move |book| match edit {
        GroupEdit::Create { name } => book.create_group(&name).map(|_| Vec::new()),
        GroupEdit::Rename { name, new_name } => {
            book.rename_group(&name, &new_name).map(|_| Vec::new())
        }
        GroupEdit::Delete { name } => book.delete_group(&name),
    })
    .await?;
    Ok(Json(json!({"ok":true,"removed":removed})))
}
async fn devices_save(State(app): State<App>, Json(edit): Json<devices::EditDevice>) -> ApiResult {
    let device = book_task(&app, move |book| book.save(edit)).await?;
    Ok(Json(json!({"device":device})))
}
async fn devices_delete(State(app): State<App>, Json(id): Json<DeviceId>) -> ApiResult {
    book_task(&app, move |book| book.delete(&id.id)).await?;
    Ok(Json(json!({"ok":true})))
}
async fn devices_code(State(app): State<App>, Json(body): Json<SavedCode>) -> ApiResult {
    let code = body.code.map(Zeroizing::new);
    book_task(&app, move |book| book.set_code(&body.id, code)).await?;
    Ok(Json(json!({"ok":true})))
}
async fn devices_connect(State(app): State<App>, Json(body): Json<SavedCode>) -> ApiResult {
    // A processor's own session connects only that saved connection.
    if !app.peer.key.is_empty() && *app.peer.key != body.id {
        return Err(api_error("This window belongs to another processor"));
    }
    let provided = body.code.map(Zeroizing::new);
    let connect = book_task(&app, move |book| {
        let device = book.find(&body.id)?;
        let code = match provided {
            Some(code) if !code.is_empty() => code,
            _ => book.code(&body.id)?,
        };
        Ok(Connect {
            host: device.host,
            port: device.port,
            terminal_port: device.terminal_port,
            model: device.model,
            code: code.to_string(),
            device_id: Some(device.id),
        })
    })
    .await?;
    dispatch(app, |r| Command::Connect(connect, r)).await
}
// The terminal and PC Remote sockets are independent. Keep polling the latter
// while a bounded terminal operation is in flight, without another writer actor.
async fn with_meter_polling<T>(
    device: &mut Device,
    app: &App,
    work: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    if !device.session.info.capabilities.live_meters {
        return work.await;
    }
    tokio::pin!(work);
    let mut tick = interval(METER_POLL_PERIOD);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            result = &mut work => return result,
            _ = tick.tick() => {
                // Meters are secondary to the terminal work: only a lost
                // session ends it, a slow tick does not.
                let polled = async {
                    device.session.send(219, b"").await?;
                    drain(device, app).await
                }
                .await;
                if let Err(error) = polled
                    && let Some(lost) = session_lost(device, &error)
                {
                    device.lost = Some(lost.clone());
                    return Err(lost);
                }
            }
        }
    }
}
async fn live_snapshot(device: &mut Device, app: &App, scope: Scope) -> Result<Document, String> {
    if uses_files(device) {
        return read_file_document(app, device, scope).await;
    }
    let address = device.terminal;
    let code = device.code.clone();
    with_meter_polling(
        device,
        app,
        read_snapshot_for_model(
            address,
            &code,
            scope,
            SYSTEM_READ_DEADLINE,
            device.session.info.model,
        ),
    )
    .await
}
fn processing_modifications(current: &Document, baseline: Option<&Document>) -> BTreeSet<String> {
    let Some(baseline) = baseline else {
        return BTreeSet::new();
    };
    current
        .fields
        .iter()
        .filter(|(name, field)| baseline.fields.get(*name) != Some(*field))
        .map(|(name, _)| name.clone())
        .collect()
}

fn refresh_processing_identity(
    processing: &Document,
    _presets: &[orban_protocol::presets::Preset],
    baseline: &mut Option<Document>,
) -> (Option<String>, BTreeSet<String>) {
    if baseline.is_none() {
        let mut initial = processing.clone();
        if let Some(name) = initial.name.strip_prefix("modif ") {
            initial.name = name.to_owned();
        }
        *baseline = Some(initial);
    }
    let base_name = baseline
        .as_ref()
        .map(|document| document.name.clone())
        .or_else(|| processing.name.strip_prefix("modif ").map(str::to_owned));
    (
        base_name,
        processing_modifications(processing, baseline.as_ref()),
    )
}

fn refreshed_less_more_availability(
    processing: &Document,
    previous_name: Option<&str>,
    previous_available: bool,
    presets: &[orban_protocol::presets::Preset],
) -> bool {
    if !processing.fields.contains_key("LESS MORE") {
        return false;
    }
    if presets.iter().any(|preset| {
        preset.name == processing.name
            && preset.kind == orban_protocol::presets::PresetKind::Factory
    }) {
        return true;
    }
    let same_modified_preset = processing.name.strip_prefix("modif ").is_some_and(|base| {
        previous_name == Some(processing.name.as_str()) || previous_name == Some(base)
    });
    previous_available && same_modified_preset
}

/// Why a failed tick ends the session, or `None` when the session stays: a
/// slow or malformed reply is not a lost connection.
fn session_lost(device: &Device, error: &str) -> Option<String> {
    if is_definite_disconnect(error) || device.session.is_broken() {
        Some(error.to_owned())
    } else if device.boundaries.unanswered_for() >= UNANSWERED_LIMIT {
        Some(format!(
            "The processor has not answered for {} seconds. {error}",
            UNANSWERED_LIMIT.as_secs()
        ))
    } else {
        None
    }
}

fn is_definite_disconnect(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("device disconnected")
        || lower.contains("connection reset")
        || lower.contains("broken pipe")
        || lower.contains("not connected")
}

/// Whether this session reads and confirms over the PC Remote session only:
/// a file-sync model always, a 5700i whose status port did not answer.
fn uses_files(device: &Device) -> bool {
    device.session.adapter().wire.file_sync || device.status_port_down
}

/// A model whose original PC Remote needs no status port: its setup file,
/// preset files and on-air file can be read over the PC Remote session.
fn can_use_files(device: &Device) -> bool {
    let wire = device.session.adapter().wire;
    wire.setup_request.is_some() && wire.user_presets_request.is_some()
}

/// Reads the current settings and presets from the status (terminal) port.
/// The system settings (AS) and the processing with the preset list (LP/AP)
/// are separate reads: one failing does not skip the other. When the status
/// port cannot give the processing, the on-air archive is read over the PC
/// Remote session instead (opcode 239), without the preset list. A port that
/// did not accept the connection is not tried a second time.
async fn synchronize(app: &App, device: &mut Device) -> Result<(), String> {
    if device.session.adapter().wire.file_sync {
        return synchronize_files(app, device).await;
    }
    if device.status_port_down {
        let synced = synchronize_files(app, device).await;
        device.sync_gap = synced.as_ref().err().map(|e| sync_failure(device, e));
        return synced;
    }
    // Until this read is complete, nothing may change the processor; a read
    // that stops early (a lost session) leaves the gap in place.
    device.sync_gap = Some("The synchronization has not finished".into());
    let capabilities = device.session.info.capabilities;
    let mut unavailable = Vec::new();
    let mut unreachable = None;
    let mut from_archive = false;
    if capabilities.parameter_reads {
        match live_snapshot(device, app, Scope::System).await {
            Ok(system) => {
                device.input_status = next_input_status(device, &system, false);
                let mut state = app.state.write().await;
                state.system = Some(system);
                state.revision += 1;
            }
            Err(error) if device.lost.is_some() => return Err(error),
            // As the original PC Remote: everything over the PC Remote session.
            Err(_) if can_use_files(device) => {
                device.status_port_down = true;
                let synced = synchronize_files(app, device).await;
                device.sync_gap = synced.as_ref().err().map(|e| sync_failure(device, e));
                return synced;
            }
            Err(error) => {
                if orban_protocol::terminal::is_unreachable(&error) {
                    unreachable = Some(error.clone());
                }
                unavailable.push(("System settings", error));
            }
        }
    }
    let processing = match &unreachable {
        Some(error) => Err(error.clone()),
        None => reload_processing(app, device).await,
    };
    match processing {
        Ok(()) => {}
        Err(error) if device.lost.is_some() => return Err(error),
        Err(error) => match read_on_air_archive(app, device).await {
            Ok(document) => {
                from_archive = true;
                let mut state = app.state.write().await;
                show_on_air(&mut state, device, document);
                if capabilities.preset_catalog {
                    // After an unreachable port, the system settings line already says why.
                    let why = if unreachable.is_some() {
                        String::new()
                    } else {
                        error
                    };
                    unavailable.push(("The preset list", why));
                }
            }
            Err(_) if device.lost.is_some() => {
                return Err(device.lost.clone().unwrap_or(error));
            }
            Err(archive) => {
                let mut state = app.state.write().await;
                state.processing = None;
                state.modified_fields.clear();
                state.revision += 1;
                unavailable.push((
                    "The processing",
                    format!(
                        "{error}. The on-air document could not be read over the PC Remote connection either: {archive}"
                    ),
                ));
            }
        },
    }
    if unavailable.is_empty() {
        device.sync_gap = None;
        let mut state = app.state.write().await;
        state.error = None;
        state.sync_error = None;
        return Ok(());
    }
    let mut error = sync_gap(&unavailable);
    if from_archive {
        error.push_str(" The processing was read over the PC Remote connection.");
    }
    device.sync_gap = Some(sync_failure(device, &error));
    Err(error)
}

/// What a partial synchronization left out, and why, in one sentence each.
fn sync_gap(unavailable: &[(&str, String)]) -> String {
    let mut text = String::new();
    for (what, why) in unavailable {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(what);
        if why.is_empty() {
            text.push_str(" is unavailable for the same reason.");
        } else {
            text.push_str(&format!(" could not be read: {why}."));
        }
    }
    text
}

/// Reads the on-air processing again (with the preset list where the model
/// has one: LP is followed by AP in the same exchange).
async fn reload_processing(app: &App, device: &mut Device) -> Result<(), String> {
    if uses_files(device) {
        return synchronize_files(app, device).await;
    }
    let capabilities = device.session.info.capabilities;
    if capabilities.preset_catalog {
        refresh_presets(app, device).await?;
    } else if capabilities.parameter_reads {
        let processing = live_snapshot(device, app, Scope::Processing).await?;
        let mut state = app.state.write().await;
        let (base_name, modified_fields) = refresh_processing_identity(
            &processing,
            &state.presets,
            &mut device.processing_baseline,
        );
        state.processing = Some(processing);
        state.preset_base_name = base_name;
        state.modified_fields = modified_fields;
        state.revision += 1;
    }
    Ok(())
}

/// The on-air processing document over the PC Remote session (opcode 239);
/// no terminal session is opened. While session meters are on, the 5700i
/// cuts every frame longer than 4096 bytes, this archive too; with meters
/// off (223) it arrives whole (virtual 5700i, original firmware, 5 of 5).
async fn read_on_air_archive(app: &App, device: &mut Device) -> Result<Document, String> {
    let plain = read_on_air_file(app, device).await?;
    if uses_files(device) {
        return file_document(&plain, device.session.info.model);
    }
    Document::parse_for_model(&plain, device.session.info.model)
}

/// The decrypted on-air file (opcode 229) the unit sends for its on-air request.
async fn read_on_air_file(app: &App, device: &mut Device) -> Result<Zeroizing<Vec<u8>>, String> {
    device.archive = None;
    device.archive_requested = true;
    let meters = device.session.info.capabilities.live_meters;
    let read = async {
        if meters {
            device.session.send(223, b"").await?;
        }
        let request = device.session.adapter().wire.on_air_request;
        device.session.send(request, b"").await?;
        drain(device, app).await
    }
    .await;
    device.archive_requested = false;
    let resumed = if meters {
        device.session.send(222, b"").await
    } else {
        Ok(())
    };
    read?;
    resumed?;
    let data = device
        .archive
        .take()
        .ok_or("The processor did not send its on-air document")?;
    Ok(Zeroizing::new(device.session.decrypt_file(&data)?))
}

/// A file of a file-sync model as a document. The 8700HD saves a preset
/// that a Recall sent with the LF line ends of that upload (the original
/// PC Remote's too); its own files end lines with CR LF.
fn file_document(plain: &[u8], model: DeviceModel) -> Result<Document, String> {
    if plain.windows(2).any(|w| w == b"\r\n") {
        return Document::parse_for_model(plain, model);
    }
    let text = Zeroizing::new(
        std::str::from_utf8(plain)
            .map_err(|_| "Unsupported document encoding")?
            .replace('\n', "\r\n"),
    );
    Document::parse_for_model(text.as_bytes(), model)
}

/// The files the unit sends in answer to `request` (opcode `reply`), read
/// before the response boundary with session meters off, as the original
/// PC Remote does around its file requests.
async fn read_files(
    app: &App,
    device: &mut Device,
    request: u16,
    reply: u16,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    device.files.clear();
    device.wanted_files = Some(reply);
    let meters = device.session.info.capabilities.live_meters;
    let read = async {
        if meters {
            device.session.send(223, b"").await?;
        }
        device.session.send(request, b"").await?;
        drain(device, app).await
    }
    .await;
    device.wanted_files = None;
    let resumed = if meters {
        device.session.send(222, b"").await
    } else {
        Ok(())
    };
    read?;
    resumed?;
    Ok(std::mem::take(&mut device.files))
}

/// The system settings (setup file) or the on-air processing (on-air file)
/// over the PC Remote session.
async fn read_file_document(
    app: &App,
    device: &mut Device,
    scope: Scope,
) -> Result<Document, String> {
    if scope == Scope::Processing {
        return read_on_air_archive(app, device).await;
    }
    let (request, reply) = device
        .session
        .adapter()
        .wire
        .setup_request
        .ok_or("The system settings cannot be read over this connection")?;
    let (_, data) = read_files(app, device, request, reply)
        .await?
        .into_iter()
        .next()
        .ok_or("The processor did not send its system settings")?;
    let plain = Zeroizing::new(device.session.decrypt_file(&data)?);
    file_document(&plain, device.session.info.model)
}

/// Reads `scope` until `accept` holds. A file-sync model saves its files a
/// few seconds after a change, so a read right after it can show the
/// previous state; the last document read is returned either way.
async fn read_back(
    app: &App,
    device: &mut Device,
    scope: Scope,
    accept: impl Fn(&Document) -> bool,
) -> Result<Document, String> {
    if !uses_files(device) {
        return live_snapshot(device, app, scope).await;
    }
    let end = tokio::time::Instant::now() + FILE_READBACK_LIMIT;
    loop {
        let read = read_file_document(app, device, scope).await;
        let last = tokio::time::Instant::now() + FILE_READBACK_INTERVAL > end;
        match read {
            Ok(document) if accept(&document) || last => return Ok(document),
            Err(error) if last || session_lost(device, &error).is_some() => return Err(error),
            _ => sleep(FILE_READBACK_INTERVAL).await,
        }
    }
}

/// The preset list of a file-sync model: the factory presets of its PC
/// Remote installer and the user presets whose files the unit sends. The
/// firmware's `modif` working copies are listed as unsaved.
async fn read_preset_files(app: &App, device: &mut Device) -> Result<Vec<Preset>, String> {
    let wire = device.session.adapter().wire;
    let mut presets: Vec<Preset> = wire
        .factory_presets
        .iter()
        .map(|name| Preset {
            name: (*name).to_owned(),
            kind: PresetKind::Factory,
        })
        .collect();
    device.user_presets.clear();
    let Some((request, reply)) = wire.user_presets_request else {
        return Ok(presets);
    };
    let model = device.session.info.model;
    for (name, data) in read_files(app, device, request, reply).await? {
        let plain = Zeroizing::new(device.session.decrypt_file(&data)?);
        // The 2300 sends no file names; its documents name their own file.
        let name = if name.is_empty() {
            preset_file_name(&plain).unwrap_or_default()
        } else {
            name
        };
        // The unit also sends its setup files, the firmware's working copies
        // of factory presets and files of other models.
        if !name.to_ascii_lowercase().ends_with(wire.user_extension) {
            continue;
        }
        // A file copied onto the unit from another model keeps its own
        // document version (seen: a 6300 preset renamed `.orb86user`); it is
        // not a preset of this processor.
        let Ok(document) = file_document(&plain, model) else {
            continue;
        };
        // The 8700HD sends its `modif` working copy twice.
        if device
            .user_presets
            .get(&document.name)
            .is_some_and(|known| **known == *plain)
        {
            continue;
        }
        // A user preset may carry a factory preset's name (seen on a 6300:
        // CRISP.orb63user next to the factory CRISP); it is listed apart.
        let listed = if presets.iter().any(|p| p.name == document.name) {
            format!("{} (user)", document.name)
        } else {
            document.name.clone()
        };
        if presets.iter().any(|p| p.name == listed) {
            return Err(format!("Ambiguous duplicate preset name {listed}"));
        }
        let kind = if document.name.to_ascii_lowercase().starts_with("modif ") {
            PresetKind::Unsaved
        } else {
            PresetKind::User
        };
        presets.push(Preset {
            name: listed.clone(),
            kind,
        });
        device.user_presets.insert(listed, plain);
    }
    Ok(presets)
}

/// The terminal list of a model whose `LP` lists user presets among the
/// factory presets (5700i): the unit's user preset files mark them as user
/// presets, and its `modif` working copies as unsaved.
async fn with_user_preset_files(
    app: &App,
    device: &mut Device,
    mut presets: Vec<Preset>,
) -> Result<Vec<Preset>, String> {
    if device.session.adapter().wire.user_presets_request.is_none() {
        return Ok(presets);
    }
    let files = read_preset_files(app, device).await?;
    for user in files.into_iter().filter(|p| p.kind != PresetKind::Factory) {
        // `LP` writes the firmware's working copies in capitals (`MODIF ...`).
        match presets
            .iter_mut()
            .find(|p| p.name.eq_ignore_ascii_case(&user.name))
        {
            Some(listed) => *listed = user,
            None => presets.push(user),
        }
    }
    Ok(presets)
}

/// The `Preset File Name` a document gives itself.
fn preset_file_name(plain: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(plain).ok()?;
    let line = text
        .lines()
        .find_map(|l| l.strip_prefix("Preset File Name= <"))?;
    Some(line.strip_suffix('>')?.to_owned())
}

/// Settings, presets and on-air processing of a file-sync model, read over
/// the PC Remote session without the status (terminal) port.
async fn synchronize_files(app: &App, device: &mut Device) -> Result<(), String> {
    let mut system = if device.session.info.capabilities.parameter_reads {
        Some(read_file_document(app, device, Scope::System).await?)
    } else {
        None
    };
    // The client's Network page shows the unit's network file.
    if let (Some(system), Some((request, reply))) = (
        system.as_mut(),
        device.session.adapter().wire.network_request,
    ) {
        let mut read_only = BTreeSet::new();
        for (_, data) in read_files(app, device, request, reply).await? {
            let plain = Zeroizing::new(device.session.decrypt_file(&data)?);
            let network = file_document(&plain, device.session.info.model)?;
            for (name, field) in network.fields {
                read_only.insert(name.clone());
                system.fields.entry(name).or_insert(field);
            }
        }
        app.state.write().await.read_only_fields = read_only;
    }
    // The status text of the client's Diagnostics answers 204 with the
    // access code, which the original client sends at every sync.
    if device.session.adapter().wire.tool_files.is_some()
        && app.state.read().await.diagnostics.is_none()
    {
        let code = device.code.clone();
        device.session.send(204, code.as_bytes()).await?;
        drain(device, app).await?;
    }
    let presets = read_preset_files(app, device).await?;
    let processing = read_on_air_archive(app, device).await?;
    let mut state = app.state.write().await;
    if system.is_some() {
        state.system = system;
    }
    let previous_name = state.processing.as_ref().map(|d| d.name.clone());
    let previous_less_more = state.less_more_available;
    let (base_name, modified_fields) =
        refresh_processing_identity(&processing, &presets, &mut device.processing_baseline);
    state.less_more_available = refreshed_less_more_availability(
        &processing,
        previous_name.as_deref(),
        previous_less_more,
        &presets,
    );
    state.presets = presets;
    state.processing = Some(processing);
    state.preset_base_name = base_name;
    state.modified_fields = modified_fields;
    state.error = None;
    state.sync_error = None;
    state.revision += 1;
    Ok(())
}

/// Puts `text` on air as the original PC Remote does: an unnamed on-air
/// file (229), sealed with the access code.
async fn upload_on_air(device: &mut Device, text: &str) -> Result<(), OperationError> {
    let iv = *uuid::Uuid::new_v4().as_bytes();
    let sealed = orban_protocol::archive::encrypt(text.as_bytes(), &device.code, iv)
        .map_err(OperationError::safe)?;
    // The 2300's upload has no name line at all.
    let mut body = if device.session.adapter().wire.file_names {
        format!("0\n0\n\n{}\n", sealed.len())
    } else {
        format!("0\n0\n{}\n", sealed.len())
    }
    .into_bytes();
    body.extend(sealed);
    device
        .session
        .send(229, &body)
        .await
        .map_err(OperationError::uncertain)?;
    if let Some(end) = device.session.adapter().wire.file_end {
        device
            .session
            .send(end, b"")
            .await
            .map_err(OperationError::uncertain)?;
        device.boundaries.sent();
    } else if device.status_port_down {
        // The 5700i answers the upload with a boundary answer of its own.
        device.boundaries.sent();
    }
    Ok(())
}

/// The factory file of `name` the original PC Remote would upload: the one
/// for the unit's pre-emphasis (system setting `PRE-E`, 50us or 75us).
async fn factory_file(device: &Device, app: &App, name: &str) -> Result<String, OperationError> {
    let files = device.session.adapter().wire.factory_files.unwrap_or("{}");
    let emphasis = app
        .state
        .read()
        .await
        .system
        .as_ref()
        .and_then(|system| system.fields.get("PRE-E"))
        .and_then(|field| match &field.value {
            Value::Choice(text) | Value::Text(text) => Some(text.clone()),
            _ => None,
        })
        .ok_or_else(|| {
            OperationError::safe("The pre-emphasis setting is not available; use Refresh first")
        })?;
    let files: JsonValue =
        serde_json::from_str(files).map_err(|e| OperationError::safe(e.to_string()))?;
    files[name][emphasis.as_str()]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| OperationError::safe(format!("No factory file for {name} at {emphasis}")))
}

/// Recall on a file-sync model, as the original PC Remote does it: a user
/// preset's document is sent back as an unnamed on-air file (229). A factory
/// preset is recalled by the firmware itself (terminal `RP`): PC Remote
/// converts its installer's factory file with its own tables, which this app
/// does not have. Either way the on-air file read over the PC Remote session
/// confirms the result.
async fn recall_with_files(
    device: &mut Device,
    app: &App,
    name: &str,
    kind: &PresetKind,
) -> Result<Document, OperationError> {
    let expected = match kind {
        PresetKind::User => {
            let plain = device
                .user_presets
                .get(name)
                .ok_or_else(|| OperationError::safe("Refresh the preset list first"))?
                .clone();
            // The original client sends the document with LF line ends.
            let text = String::from_utf8_lossy(&plain).replace("\r\n", "\n");
            upload_on_air(device, &text).await?;
            Some(file_document(&plain, device.session.info.model).map_err(OperationError::safe)?)
        }
        // Without a status port (a 5700i whose TCP 23 does not answer, the
        // 2300 that has none), a factory preset is recalled as the original
        // client does: by uploading the factory file of its installation
        // (captures, 29 October 2026). The client converts it to the
        // current document version first; the firmware converts the
        // unconverted file to the same on-air document.
        PresetKind::Factory
            if device.session.adapter().wire.factory_files.is_some()
                && (device.status_port_down || !device.session.adapter().wire.factory_recall) =>
        {
            let text = factory_file(device, app, name).await?;
            upload_on_air(device, &text).await?;
            Some(
                file_document(text.as_bytes(), device.session.info.model)
                    .map_err(OperationError::safe)?,
            )
        }
        PresetKind::Factory if !device.session.adapter().wire.factory_recall => {
            return Err(OperationError::safe(format!(
                "A factory preset cannot be recalled over the network on the {}: the original PC Remote converts its own copy of the factory file, and the unit has no terminal port. Recall it on the front panel, or recall a user preset.",
                device.session.info.model.label()
            )));
        }
        PresetKind::Factory => {
            orban_protocol::terminal::send_recall_for_model(
                device.terminal,
                &device.code,
                name,
                RECALL_DEADLINE,
                device.session.info.model,
            )
            .await
            .map_err(|e| {
                OperationError::uncertain(format!("The recall of {name} could not be sent. {e}"))
            })?;
            None
        }
        PresetKind::Unsaved => return Err(OperationError::safe("Select a saved preset")),
    };
    // On air, the unit reports the name inside the document.
    let on_air_name = expected
        .as_ref()
        .map_or(name, |expected| expected.name.as_str());
    let version = |document: &Document| document.raw.lines().next().map(str::to_owned);
    // A document of an older version is converted by the firmware when it
    // loads it (2300: user presets saved as 2300.10, run as 2300.20); then
    // only the name can confirm the Recall.
    let same_fields = |expected: &Document, document: &Document| {
        version(expected) != version(document) || expected.fields == document.fields
    };
    let confirmed = read_back(app, device, Scope::Processing, |document| {
        document.name == on_air_name
            && expected
                .as_ref()
                .is_none_or(|expected| same_fields(expected, document))
    })
    .await
    .map_err(|e| {
        OperationError::uncertain(format!(
            "The recall of {name} could not be confirmed; the connection remains active. {e}"
        ))
    })?;
    if confirmed.name != on_air_name
        || expected
            .as_ref()
            .is_some_and(|expected| !same_fields(expected, &confirmed))
    {
        return Err(OperationError::uncertain(format!(
            "The processor did not confirm the recall of {name}; it reports {} on air",
            confirmed.name
        )));
    }
    Ok(confirmed)
}

/// Whether `document` agrees with every field of `start`, the start of
/// another document.
fn begins_like(start: &Document, document: &Document) -> bool {
    start.name == document.name
        && start
            .fields
            .iter()
            .all(|(name, field)| document.fields.get(name) == Some(field))
}

/// Shows `processing` as the on-air document; another preset resets the
/// baseline that modifications are counted from.
fn show_on_air(state: &mut Snapshot, device: &mut Device, processing: Document) {
    let before = device.processing_baseline.as_ref().map(|d| d.name.clone());
    let now = processing
        .name
        .strip_prefix("modif ")
        .unwrap_or(processing.name.as_str())
        .to_owned();
    if before.as_deref() != Some(now.as_str()) {
        device.processing_baseline = None;
    }
    let (base_name, modified_fields) =
        refresh_processing_identity(&processing, &state.presets, &mut device.processing_baseline);
    state.processing = Some(processing);
    state.preset_base_name = base_name;
    state.modified_fields = modified_fields;
    // What the status port could not give at the last synchronization stays shown.
    state.sync_error = device.sync_gap.clone();
    state.revision += 1;
}

/// No processing document is shown: processing writes, Recall and preset
/// management are refused until one is read.
fn hide_on_air(state: &mut Snapshot, message: String) {
    state.processing = None;
    state.modified_fields.clear();
    state.sync_error = Some(message);
    state.revision += 1;
}

/// The unit pushed its on-air processing document (opcode 229) without a
/// request: after the front panel, the terminal, automation or the app's own
/// Recall changed it. With session meters on, the 5700i cuts that frame, but
/// the part that arrives decrypts to the preset name and the first fields.
/// A push that agrees with the shown document changes nothing (the app's own
/// Recall). Otherwise the shown document is withdrawn and the on-air archive
/// is read over the PC Remote session until it agrees with the push: the
/// unit saves that file a moment after the change, so a read right after it
/// can return the previous preset. The terminal is not used here: the change
/// may come from a terminal session that is still open.
async fn on_air_pushed(app: &App, device: &mut Device) {
    let Some(pushed) = device.pushed.take() else {
        return;
    };
    let model = device.session.info.model;
    let start = match pushed {
        Pushed::Whole(sealed) => {
            match device.session.decrypt_file(&sealed).and_then(|plain| {
                // A file-sync model pushes its document with LF line ends.
                if uses_files(device) {
                    file_document(&plain, model)
                } else {
                    Document::parse_for_model(&plain, model)
                }
            }) {
                Ok(processing) => {
                    device.expected = None;
                    device.archive_retry = None;
                    let mut state = app.state.write().await;
                    show_on_air(&mut state, device, processing);
                    return;
                }
                Err(_) => None,
            }
        }
        Pushed::Start(sealed) => orban_protocol::archive::decrypt_prefix(&sealed, &device.code)
            .and_then(|plain| Document::parse_prefix_for_model(&plain, model))
            .ok(),
    };
    let mut state = app.state.write().await;
    if let (Some(start), Some(shown)) = (&start, &state.processing)
        && begins_like(start, shown)
    {
        return;
    }
    device.expected = start;
    if device.session.adapter().on_air_archive {
        device.archive_retry = Some((tokio::time::Instant::now() + ARCHIVE_RETRY, ARCHIVE_READS));
        hide_on_air(
            &mut state,
            "The on-air processing was changed on the processor. Reading it again.".to_owned(),
        );
    } else {
        hide_on_air(
            &mut state,
            "The on-air processing was changed on the processor. Use Refresh to read it."
                .to_owned(),
        );
    }
}

/// One scheduled read of the on-air archive after a push.
async fn read_pushed_on_air(app: &App, device: &mut Device) {
    let Some((_, left)) = device.archive_retry.take() else {
        return;
    };
    let read = read_on_air_archive(app, device)
        .await
        .and_then(|processing| match &device.expected {
            Some(start) if !begins_like(start, &processing) => {
                Err("The processor has not saved its new on-air settings yet".to_owned())
            }
            _ => Ok(processing),
        });
    let mut state = app.state.write().await;
    match read {
        Ok(processing) => {
            device.expected = None;
            show_on_air(&mut state, device, processing);
        }
        Err(_) if left > 1 => {
            device.archive_retry = Some((tokio::time::Instant::now() + ARCHIVE_RETRY, left - 1));
        }
        Err(error) => {
            device.expected = None;
            hide_on_air(
                &mut state,
                format!("{error}. Current settings are unavailable until Refresh reads them."),
            );
        }
    }
}

fn sync_failure(device: &Device, error: &str) -> String {
    if uses_files(device) {
        return format!(
            "Connected, but the current settings and presets could not be read over the PC Remote connection. Use Refresh to try again. {error}"
        );
    }
    format!("Connected. {error} Use Refresh to try again.")
}
async fn drain(device: &mut Device, app: &App) -> Result<(), String> {
    let (boundary, answer) = device.session.adapter().wire.boundary;
    device.session.send(boundary, b"").await?;
    device.boundaries.sent();
    // One deadline for the whole wait: unrelated pushes (247 about every
    // 0.85 s) must not keep the owner here past it.
    let mut end = tokio::time::Instant::now() + device.session.deadline();
    // An answer to a request (not a timed 247 or clock push) shows the unit
    // is processing requests while this boundary is not answered.
    let mut answered_other = false;
    for _ in 0..4096 {
        let event = match tokio::time::timeout_at(end, device.session.next_event()).await {
            Ok(Ok(event)) => event,
            Ok(Err(error)) if error != "Receive timed out" => return Err(error),
            _ => {
                if answered_other {
                    device.boundaries.dropped();
                }
                return Err("Receive timed out".to_owned());
            }
        };
        answered_other |= matches!(
            event,
            Message::Meters { .. } | Message::Truncated { .. } | Message::File { .. }
        );
        // A unit with many preset files sends them one by one for longer
        // than one deadline; each file shows it is still answering.
        if matches!(event, Message::File { .. }) {
            end = tokio::time::Instant::now() + device.session.deadline();
        }
        match event {
            // Bank 1 carries the current values, bank 2 the firmware's peak
            // hold of the same channels; the original 5700i client draws a bar
            // from the first and its peak marker from the second. A record
            // keeps the last peaks until its own bank 2 arrives.
            Message::Meters { bank: 1, values } => {
                Activity::count(&app.activity.meter_frames);
                let peaks = app.meters.borrow().get("peaks").cloned().filter(|peaks| {
                    peaks
                        .as_array()
                        .is_some_and(|peaks| peaks.len() == values.len())
                });
                app.meters.send_replace(match peaks {
                    Some(peaks) => json!({"live":true,"bank":1,"values":values,"peaks":peaks}),
                    None => json!({"live":true,"bank":1,"values":values}),
                });
            }
            Message::Meters { bank: 2, values } => {
                app.meters.send_if_modified(|meter| {
                    let current = meter.get("values").and_then(|v| v.as_array()).map(Vec::len);
                    if meter["live"] != true || current != Some(values.len()) {
                        return false;
                    }
                    meter["peaks"] = json!(values);
                    true
                });
            }
            Message::Meters { .. } => {}
            Message::Other { kind: 205, data } => {
                let text = String::from_utf8_lossy(&data).replace('\r', "");
                app.state.write().await.diagnostics = Some(text);
            }
            Message::Other { kind, .. } if kind == answer && device.boundaries.answered() => {
                return Ok(());
            }
            Message::File { kind, name, data } if device.wanted_files == Some(kind) => {
                device.files.push((name, data));
            }
            // The requested archive is a named file; the push has no name.
            // Where files carry no names (2300), the answer to a request in
            // flight is the requested file.
            Message::File {
                kind: 229,
                name,
                data,
            } if device.archive_requested
                && (!name.is_empty() || !device.session.adapter().wire.file_names) =>
            {
                device.archive_requested = false;
                device.archive = Some(data);
            }
            // A requested archive that does not fit the unit's send buffer
            // arrives cut, like a push; it answers the request but is unusable.
            Message::Truncated { kind: 229, .. } if device.archive_requested => {
                device.archive_requested = false;
            }
            Message::File {
                kind: 229, data, ..
            } => {
                // Its end marker arrives as one more boundary answer.
                if device.session.adapter().wire.file_end.is_some() {
                    device.boundaries.sent();
                }
                device.pushed = Some(Pushed::Whole(data));
            }
            Message::Truncated {
                kind: 229, data, ..
            } => {
                let start = orban_protocol::message::file_prefix(&data)
                    .map_or_else(Vec::new, |(_, start)| start.to_vec());
                device.pushed = Some(Pushed::Start(start));
            }
            _ => {}
        }
    }
    Err("Too many device events before response boundary".into())
}
async fn make_device(body: Connect, app: &App) -> Result<Device, String> {
    let code = Zeroizing::new(body.code);
    let ip = body.host.parse().map_err(|_| "Enter an IP address")?;
    let address = SocketAddr::new(ip, body.port);
    let session =
        match Session::connect_for_model(address, &code, SESSION_DEADLINE, body.model).await {
            Err(error) if error.starts_with("The processor is busy") => {
                sleep(BUSY_RETRY).await;
                Session::connect_for_model(address, &code, SESSION_DEADLINE, body.model).await?
            }
            result => result?,
        };
    let profile = Profile::for_adapter(session.info.adapter_id)?.map(Arc::new);
    let mut device = Device {
        session,
        boundaries: Boundaries::default(),
        lost: None,
        pushed: None,
        expected: None,
        archive_requested: false,
        archive: None,
        archive_retry: None,
        wanted_files: None,
        files: Vec::new(),
        user_presets: BTreeMap::new(),
        terminal: SocketAddr::new(ip, body.terminal_port),
        code,
        processing_baseline: None,
        profile,
        input_status: None,
        sync_gap: None,
        status_port_down: false,
    };
    // The PC Remote login is the connection. Reading the current settings
    // over the status port is a separate step: when it fails, the session
    // stays connected without settings, and Refresh tries again.
    let synced = synchronize(app, &mut device).await;
    if let Some(lost) = device.lost.take() {
        let _ = device.session.disconnect().await;
        return Err(lost);
    }
    if device.session.info.capabilities.live_meters
        && let Err(error) = device.session.send(222, b"").await
    {
        let _ = device.session.disconnect().await;
        return Err(error);
    }
    if let Ok(mut active) = app.profile.write() {
        *active = device.profile.clone().unwrap_or_else(reference_profile);
    }
    let mut state = app.state.write().await;
    state.connected = true;
    let access = device.session.info.access();
    state.write_enabled = access.modify && device.session.info.capabilities.parameter_writes;
    state.access = Some(access);
    state.firmware = device.session.info.firmware.clone();
    state.model = device.session.info.model;
    state.skin_id = Some(device.session.info.skin);
    state.adapter_id = Some(device.session.info.adapter_id.into());
    state.capabilities = Some(device.session.info.capabilities);
    state.evidence = Some(device.session.info.evidence);
    state.host = body.host;
    state.device_id = body.device_id;
    state.session_id = Some(uuid::Uuid::new_v4().to_string());
    state.error = None;
    state.sync_error = synced.err().map(|error| sync_failure(&device, &error));
    Ok(device)
}
async fn refresh_presets(app: &App, device: &mut Device) -> Result<(), String> {
    if uses_files(device) {
        return synchronize_files(app, device).await;
    }
    let address = device.terminal;
    let code = device.code.clone();
    let (presets, processing) = with_meter_polling(
        device,
        app,
        orban_protocol::terminal::read_presets_for_model(
            address,
            &code,
            CATALOG_READ_DEADLINE,
            device.session.info.model,
        ),
    )
    .await?;
    let presets = with_user_preset_files(app, device, presets).await?;
    // The status text the original client shows as Diagnostics answers its
    // login message (204 with the access code); the app's login is the
    // terminal-style one, so it asks once here.
    if device.session.adapter().wire.preset_file_delete.is_some() {
        let code = device.code.clone();
        device.session.send(204, code.as_bytes()).await?;
        drain(device, app).await?;
    }
    let mut state = app.state.write().await;
    let previous_name = state
        .processing
        .as_ref()
        .map(|document| document.name.clone());
    let previous_less_more = state.less_more_available;
    let (base_name, modified_fields) =
        refresh_processing_identity(&processing, &presets, &mut device.processing_baseline);
    state.less_more_available = refreshed_less_more_availability(
        &processing,
        previous_name.as_deref(),
        previous_less_more,
        &presets,
    );
    state.presets = presets;
    state.processing = Some(processing);
    state.preset_base_name = base_name;
    state.modified_fields = modified_fields;
    state.revision += 1;
    Ok(())
}
async fn recall(device: &mut Device, app: &App, body: Recall) -> Result<(), OperationError> {
    require_synchronized(device)?;
    if !device.session.info.capabilities.preset_recall {
        return Err(OperationError::safe(format!(
            "Preset recall is not enabled for {}",
            device.session.info.model.label()
        )));
    }
    if !body.confirmed {
        return Err(OperationError::safe(
            "Confirm the on-air preset change first",
        ));
    }
    if !device.session.info.access().recall {
        return Err(OperationError::safe(
            "Preset recall is not enabled for this access level",
        ));
    }
    {
        let state = app.state.read().await;
        ensure_session(&state, &body.expected_session).map_err(OperationError::safe)?;
        if state.host != body.expected_host {
            return Err(OperationError::safe(
                "The active device changed. Review the connection first",
            ));
        }
        if state
            .processing
            .as_ref()
            .is_none_or(|d| d.name != body.expected_name)
        {
            return Err(OperationError::safe(
                "The on-air preset changed elsewhere. Review the current preset first",
            ));
        }
        if !state
            .presets
            .iter()
            .any(|p| p.name == body.name && p.kind != orban_protocol::presets::PresetKind::Unsaved)
        {
            return Err(OperationError::safe(
                "Select an available factory or user preset",
            ));
        }
    }
    if uses_files(device) {
        let kind = app
            .state
            .read()
            .await
            .presets
            .iter()
            .find(|p| p.name == body.name)
            .map(|p| p.kind.clone())
            .ok_or_else(|| OperationError::safe("Select an available factory or user preset"))?;
        let recalled = recall_with_files(device, app, &body.name, &kind).await?;
        // The confirmed document supersedes what the unit pushed meanwhile; a
        // push that arrived before the confirmation can be an earlier state.
        device.pushed = None;
        device.expected = None;
        device.archive_retry = None;
        let mut baseline = recalled.clone();
        if let Some(name) = baseline.name.strip_prefix("modif ") {
            baseline.name = name.to_owned();
        }
        device.processing_baseline = Some(baseline);
        let mut state = app.state.write().await;
        state.less_more_available =
            recalled.fields.contains_key("LESS MORE") && kind == PresetKind::Factory;
        state.processing = Some(recalled);
        state.preset_base_name = Some(body.name);
        state.modified_fields.clear();
        state.error = None;
        state.revision += 1;
        return Ok(());
    }
    let address = device.terminal;
    let code = device.code.clone();
    // Preset switching temporarily stalls the 5700i's PC Remote replies. Do
    // not overlap the RP/AP transaction with 50 ms meter polls: that made the
    // firmware queue work, visibly stutter, and sometimes trip the heartbeat.
    // The already loaded catalog and processing document provide the guards.
    let model = device.session.info.model;
    let recalled = match orban_protocol::terminal::recall_preset_for_model(
        address,
        &code,
        &body.name,
        RECALL_DEADLINE,
        model,
    )
    .await
    {
        Ok(document) => Ok(document),
        Err(first_error) => {
            // The RP command may have succeeded even if its combined response
            // was unusual. Resolve that exceptional case with one direct AP
            // read while meter polling remains paused.
            match read_snapshot_for_model(
                address,
                &code,
                Scope::Processing,
                RECALL_CONFIRM_DEADLINE,
                model,
            )
            .await
            {
                Ok(document) if document.name == body.name => Ok(document),
                Ok(_) => Err(OperationError::safe(format!(
                    "The processor did not recall {}. {first_error}",
                    body.name
                ))),
                Err(read_error) => Err(OperationError::safe(format!(
                    "Recall could not be confirmed; the connection remains active. {first_error}. {read_error}"
                ))),
            }
        }
    };
    // A preset can reset the device's meter subscription. Re-enable it once,
    // after the processing document is complete, then let the normal owner
    // tick resume the 50 ms cadence.
    let subscribed = device.session.send(222, b"").await;
    let recalled = recalled?;
    let mut baseline = recalled.clone();
    if let Some(name) = baseline.name.strip_prefix("modif ") {
        baseline.name = name.to_owned();
    }
    device.processing_baseline = Some(baseline);
    let mut state = app.state.write().await;
    state.less_more_available = recalled.fields.contains_key("LESS MORE")
        && state.presets.iter().any(|preset| {
            preset.name == body.name && preset.kind == orban_protocol::presets::PresetKind::Factory
        });
    state.processing = Some(recalled);
    state.preset_base_name = Some(body.name);
    state.modified_fields.clear();
    state.revision += 1;
    // The preset is confirmed on air either way; a failed send means the PC
    // Remote session itself is in trouble, which the next tick reports.
    state.error = subscribed
        .err()
        .map(|error| format!("Live meters could not be resumed after Recall. {error}"));
    Ok(())
}

/// Sends `SP` or `DP` once, then confirms the result with a fresh preset list.
/// There is never a retry: a list that does not show the expected change is
/// reported with the processor's actual list published.
async fn preset_file_step(
    device: &mut Device,
    app: &App,
    name: &str,
    store: bool,
) -> Result<(), OperationError> {
    let address = device.terminal;
    let code = device.code.clone();
    let model = device.session.info.model;
    let sent = if let Some(delete) = device.session.adapter().wire.preset_file_delete {
        preset_file_message(app, device, name, store, delete).await
    } else if store {
        orban_protocol::terminal::store_preset_for_model(
            address,
            &code,
            name,
            Duration::from_secs(10),
            model,
        )
        .await
        .map(|_| ())
    } else {
        orban_protocol::terminal::delete_preset_for_model(
            address,
            &code,
            name,
            Duration::from_secs(10),
            model,
        )
        .await
        .map(|_| ())
    };
    let _ = device.session.send(222, b"").await;
    // Read the list even after an error: the command may have taken effect.
    let listed = refresh_presets(app, device).await;
    let state = app.state.read().await;
    let mut present = state
        .presets
        .iter()
        .any(|p| p.name == name && p.kind == orban_protocol::presets::PresetKind::User);
    // A file upload also puts the saved preset on air without changes; a
    // Save that replaces a listed preset is confirmed by that alone.
    if store && device.session.adapter().wire.preset_file_delete.is_some() {
        present &= state.processing.as_ref().is_some_and(|d| d.name == name);
    }
    drop(state);
    let verb = if store { "save" } else { "delete" };
    match (sent, listed) {
        (Ok(_), Ok(())) if present == store => Ok(()),
        (Ok(_), Ok(())) => Err(OperationError::uncertain(format!(
            "The processor did not {verb} {name}: the preset list does not show the change"
        ))),
        (Err(e), Ok(())) if present == store => Err(OperationError::uncertain(format!(
            "The preset list shows that {name} was {verb}d, but the reply was unusual: {e}"
        ))),
        (Err(e), Ok(())) => Err(OperationError::safe(format!(
            "Could not {verb} {name}. {e}"
        ))),
        (_, Err(e)) => Err(OperationError::uncertain(format!(
            "The result of {verb} {name} could not be confirmed. Refresh the preset list. {e}"
        ))),
    }
}

/// The original PC Remote's Save / Save As (store) or Delete over the PC
/// Remote session: the on-air file with `Preset Name` and `Preset File Name`
/// set to `name`, uploaded as opcode 229 without a file name, or opcode
/// `delete` with the preset's file name and a newline.
async fn preset_file_message(
    app: &App,
    device: &mut Device,
    name: &str,
    store: bool,
    delete: u16,
) -> Result<(), String> {
    let extension = device.session.adapter().wire.user_extension;
    if !store {
        device
            .session
            .send(delete, format!("{name}{extension}\n").as_bytes())
            .await?;
        return Ok(());
    }
    // The terminal `AP` document, not the on-air file: the unit writes that
    // file a few seconds after a change, so a Save right after a change
    // would store the previous value (seen 28 October 2026).
    let model = device.session.info.model;
    let text = if device.status_port_down {
        on_air_text_as_shown(app, device).await?
    } else {
        let address = device.terminal;
        let code = device.code.clone();
        with_meter_polling(
            device,
            app,
            orban_protocol::terminal::read_processing_text_for_model(
                address,
                &code,
                SYSTEM_READ_DEADLINE,
                model,
            ),
        )
        .await?
    };
    let on_air = Document::parse_for_model(text.as_bytes(), model)?;
    let shown = app.state.read().await.processing.clone();
    let changed = match &shown {
        None => Some("the on-air preset".to_owned()),
        // Right after a write `AP` still gives the name without `modif `.
        Some(shown)
            if shown.name.strip_prefix("modif ").unwrap_or(&shown.name)
                != on_air.name.strip_prefix("modif ").unwrap_or(&on_air.name) =>
        {
            Some(format!("the name ({})", on_air.name))
        }
        Some(shown) => on_air
            .fields
            .iter()
            .find(|(field, value)| shown.fields.get(*field) != Some(value))
            .map(|(field, _)| field.clone()),
    };
    if let Some(changed) = changed {
        return Err(format!(
            "The on-air processing changed elsewhere ({changed}). Review it first"
        ));
    }
    let document = Zeroizing::new(renamed_preset_file(&text, name, extension)?);
    let iv = *uuid::Uuid::new_v4().as_bytes();
    let sealed = orban_protocol::archive::encrypt(document.as_bytes(), &device.code, iv)?;
    let mut body = format!("0\n0\n\n{}\n", sealed.len()).into_bytes();
    body.extend(sealed);
    device.session.send(229, &body).await?;
    // The unit answers the upload with a boundary answer (FBh) of its own.
    device.boundaries.sent();
    Ok(())
}

/// Without the status port, the on-air file once the unit has saved what
/// is shown (it writes that file a few seconds after a change). The last
/// file read is returned either way; the caller compares it.
async fn on_air_text_as_shown(app: &App, device: &mut Device) -> Result<Zeroizing<String>, String> {
    let model = device.session.info.model;
    let end = tokio::time::Instant::now() + FILE_READBACK_LIMIT;
    loop {
        let plain = read_on_air_file(app, device).await?;
        let text = Zeroizing::new(
            String::from_utf8(plain.to_vec()).map_err(|_| "Unsupported document encoding")?,
        );
        let last = tokio::time::Instant::now() + FILE_READBACK_INTERVAL > end;
        let shown = app.state.read().await.processing.clone();
        let saved = Document::parse_for_model(text.as_bytes(), model)
            .ok()
            .zip(shown)
            .is_some_and(|(on_air, shown)| on_air.fields == shown.fields);
        if saved || last {
            return Ok(text);
        }
        sleep(FILE_READBACK_INTERVAL).await;
    }
}

/// An on-air file as the original 5700i PC Remote saves it under `name`: LF
/// line ends, `Preset Name=<name>` (keeping the size after it) and
/// `Preset File Name= <name + extension>`; every other line is unchanged.
fn renamed_preset_file(text: &str, name: &str, extension: &str) -> Result<String, String> {
    let mut named = false;
    let mut filed = false;
    let lines: Vec<String> = text
        .replace("\r\n", "\n")
        .split('\n')
        .map(|line| {
            if !named && let Some(rest) = line.strip_prefix("Preset Name=<") {
                named = true;
                let after = rest.split_once('>').map_or("", |(_, after)| after);
                format!("Preset Name=<{name}>{after}")
            } else if !filed && line.starts_with("Preset File Name=") {
                filed = true;
                format!("Preset File Name= <{name}{extension}>")
            } else {
                line.to_owned()
            }
        })
        .collect();
    if !named || !filed {
        return Err("The on-air document has no preset name".into());
    }
    Ok(lines.join("\n"))
}

/// The files the original PC Remote's Backup saves: the setup file (240)
/// and every file the unit sends for its user presets (243), decrypted.
async fn backup_files(device: &mut Device, app: &App) -> Result<Vec<BackupFile>, String> {
    if !device.session.info.access().memory {
        return Err("Backup is not available for this access level".into());
    }
    let wire = device.session.adapter().wire;
    let Some((presets, preset_reply)) = wire.user_presets_request else {
        return Err(format!(
            "The {} keeps no preset files to back up",
            device.session.info.model.label()
        ));
    };
    let mut files: Vec<BackupFile> = Vec::new();
    let requests = wire
        .backup_requests
        .iter()
        .copied()
        .chain([(presets, preset_reply)]);
    for (request, reply) in requests {
        for (name, data) in read_files(app, device, request, reply).await? {
            let lower = name.to_ascii_lowercase();
            if reply == preset_reply
                && !wire.backup_kept.is_empty()
                && !wire.backup_kept.iter().any(|e| lower.ends_with(e))
            {
                continue;
            }
            let plain = device.session.decrypt_file(&data)?;
            let document = String::from_utf8(plain).map_err(|_| "Unsupported document encoding")?;
            // Automation and network files are no processing documents.
            let automation = wire.tool_files.map(|t| t.automation_file);
            let network = wire.network_request.map(|(_, answer)| answer);
            if Some(reply) != automation && Some(reply) != network {
                file_document(document.as_bytes(), device.session.info.model)?;
            }
            // The unit sends its working copies twice.
            if !files.iter().any(|f| f.name.eq_ignore_ascii_case(&name)) {
                files.push(BackupFile {
                    name,
                    document,
                    modified: None,
                });
            }
        }
    }
    Ok(files)
}

/// The unit's automation file (`kind` 235) or passcode file (230), with the
/// model's own request and answer opcodes (the 5500 answers EAh for its
/// automation). A unit without automation events sends nothing.
async fn read_unit_file_from(
    device: &mut Device,
    app: &App,
    reply: u16,
) -> Result<Option<String>, String> {
    let Some(tools) = device.session.adapter().wire.tool_files else {
        return Err(format!(
            "Not available for {}",
            device.session.info.model.label()
        ));
    };
    if reply == 235 && !automation::available(device.session.info.model) {
        return Err(format!(
            "Automation is not available for the {} in this version",
            device.session.info.model.label()
        ));
    }
    // The passcodes are for a full-access session only, as their write is.
    let access = device.session.info.access();
    if (reply == 230 && !access.security) || (reply == 235 && !access.automation) {
        return Err("Security is not available for this access level".into());
    }
    let (request, answer) = match reply {
        235 => (tools.automation_request, tools.automation_file),
        230 => (tools.passcodes_request, 230),
        _ => return Err("Unsupported file request".into()),
    };
    if let Some(prelude) = tools.prelude {
        device.session.send(prelude, b"").await?;
    }
    let files = read_files(app, device, request, answer).await?;
    let Some((_, data)) = files.into_iter().next() else {
        return Ok(None);
    };
    let plain = device.session.decrypt_file(&data)?;
    String::from_utf8(plain)
        .map(Some)
        .map_err(|_| "Unsupported document encoding".into())
}

/// Every change to a processor needs a profile that may write: an adapter
/// without verified writes (for example the original 5500, never tested on
/// its firmware) stays read-only for files, Setup and restores too.
fn require_writes(device: &Device) -> Result<(), OperationError> {
    if device.session.info.capabilities.parameter_writes {
        Ok(())
    } else {
        Err(OperationError::safe(format!(
            "Changes are not available for the {} in this version",
            device.session.info.model.label()
        )))
    }
}

/// Nothing is sent that changes the processor while the last synchronization
/// is incomplete: what it left out (system settings, the preset list, the
/// status port each change is confirmed on) is exactly what a change would
/// be checked against. Recall, presets, files, Setup and restores alike.
fn require_synchronized(device: &Device) -> Result<(), OperationError> {
    if device.sync_gap.is_none() {
        return Ok(());
    }
    Err(OperationError::safe(format!(
        "Changes wait until the status port (TCP {}) answers: the processor confirms them there. Use Refresh first",
        device.terminal.port()
    )))
}

/// Replaces the unit's automation or passcode file as the original client
/// does, then reads it back and requires it to be the document sent.
async fn write_unit_file(
    device: &mut Device,
    app: &App,
    body: UnitFileWrite,
) -> Result<(), OperationError> {
    require_synchronized(device)?;
    require_writes(device)?;
    let Some(tools) = device.session.adapter().wire.tool_files else {
        return Err(OperationError::safe(format!(
            "Not available for {}",
            device.session.info.model.label()
        )));
    };
    if body.kind == 235 && !automation::available(device.session.info.model) {
        return Err(OperationError::safe(format!(
            "Automation is not available for the {} in this version",
            device.session.info.model.label()
        )));
    }
    let access = device.session.info.access();
    if !(if body.kind == 230 {
        access.security
    } else {
        access.automation
    }) {
        return Err(OperationError::safe(
            "Changes are not enabled for this access level",
        ));
    }
    {
        let state = app.state.read().await;
        ensure_session(&state, &body.expected_session).map_err(OperationError::safe)?;
        if state.host != body.expected_host {
            return Err(OperationError::safe(
                "The active device changed. Review the connection first",
            ));
        }
    }
    // The session's own passcode must stay, with full access, or the app
    // (and the original client with the same code) loses this unit.
    if body.kind == 230 {
        let passcodes = security::parse(&body.document).map_err(OperationError::safe)?;
        if !passcodes.iter().any(|p| {
            p.code.eq_ignore_ascii_case(&device.code)
                && p.level == security::flavour(device.session.info.model).0[0]
        }) {
            return Err(OperationError::safe(
                "Keep the passcode this connection uses, with access to all screens",
            ));
        }
    }
    // With no events left the original client deletes the automation file.
    let name = (body.kind == 235).then_some(tools.automation_name);
    if body.document.is_empty() {
        let name = name.ok_or_else(|| OperationError::safe("Nothing to save"))?;
        let delete = device
            .session
            .adapter()
            .wire
            .preset_file_delete
            .ok_or_else(|| OperationError::safe("Nothing to save"))?;
        device
            .session
            .send(delete, format!("{name}\n").as_bytes())
            .await
            .map_err(OperationError::uncertain)?;
        let end = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            match read_unit_file_from(device, app, body.kind).await {
                Ok(None) => return Ok(()),
                _ if tokio::time::Instant::now() + Duration::from_millis(500) > end => {
                    return Err(OperationError::uncertain(
                        "The processor still holds automation events. Open the window again to see what it holds",
                    ));
                }
                _ => sleep(Duration::from_millis(500)).await,
            }
        }
    }
    let iv = *uuid::Uuid::new_v4().as_bytes();
    let sealed = orban_protocol::archive::encrypt(body.document.as_bytes(), &device.code, iv)
        .map_err(OperationError::safe)?;
    let header = match name {
        Some(name) => {
            let millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or_default();
            let filetime = millis * 10_000 + 116_444_736_000_000_000;
            format!(
                "{}\n{}\n{name}\n{}\n",
                filetime & 0xffff_ffff,
                filetime >> 32,
                sealed.len()
            )
        }
        None => format!("0\n0\n\n{}\n", sealed.len()),
    };
    let mut message = header.into_bytes();
    message.extend(sealed);
    let opcode = if body.kind == 235 {
        tools.automation_file
    } else {
        body.kind
    };
    device
        .session
        .send(opcode, &message)
        .await
        .map_err(OperationError::uncertain)?;
    // Unlike a 229 or 231 upload, these get no boundary answer of their own.
    // The unit takes the file a moment later: read until it matches.
    let normalize = |text: &str| text.replace("\r\n", "\n");
    let end = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let read = read_unit_file_from(device, app, body.kind).await;
        let last = tokio::time::Instant::now() + Duration::from_millis(500) > end;
        match read {
            Ok(Some(text)) if normalize(&text) == normalize(&body.document) => return Ok(()),
            Ok(_) if last => {
                return Err(OperationError::uncertain(
                    "The processor's file does not show the change. Open the window again to see what it holds",
                ));
            }
            Err(e) if last || session_lost(device, &e).is_some() => {
                return Err(OperationError::uncertain(format!(
                    "The change could not be confirmed. {e}"
                )));
            }
            _ => sleep(Duration::from_millis(500)).await,
        }
    }
}

/// The unit's setup presets (the client's System Presets), by their
/// document name, with the decrypted file.
async fn setup_presets(
    device: &mut Device,
    app: &App,
) -> Result<Vec<(String, Zeroizing<Vec<u8>>)>, String> {
    // The client enables Open Setup and Save Setup As with Setup (level 2 or lower).
    if !device.session.info.access().setup {
        return Err("System Setup is not enabled for this access level".into());
    }
    let wire = device.session.adapter().wire;
    let (Some(extension), Some((request, reply))) = (wire.setup_presets, wire.user_presets_request)
    else {
        return Err(format!(
            "Setup presets are not available for {}",
            device.session.info.model.label()
        ));
    };
    let mut presets: Vec<(String, Zeroizing<Vec<u8>>)> = Vec::new();
    for (name, data) in read_files(app, device, request, reply).await? {
        if !name.to_ascii_lowercase().ends_with(extension) {
            continue;
        }
        let plain = Zeroizing::new(device.session.decrypt_file(&data)?);
        let document = file_document(&plain, device.session.info.model)?;
        // The unit sends its working copy twice.
        if !presets.iter().any(|(known, _)| *known == document.name) {
            presets.push((document.name, plain));
        }
    }
    Ok(presets)
}

/// Open Setup puts a setup preset in force; Save Setup As stores the unit's
/// setup under a new name. As the original client: the document as an
/// unnamed E9h upload, then FBh. Open is confirmed by the unit's setup file,
/// Save As by its list of setup presets.
async fn setup_preset(
    device: &mut Device,
    app: &App,
    body: SetupPresetAction,
) -> Result<(), OperationError> {
    require_synchronized(device)?;
    use orban_protocol::presets::{max_store_name_length, validate_store_name};
    if !body.confirmed {
        return Err(OperationError::safe("Confirm the change first"));
    }
    require_writes(device)?;
    if !device.session.info.access().setup {
        return Err(OperationError::safe(
            "System Setup is not enabled for this access level",
        ));
    }
    {
        let state = app.state.read().await;
        ensure_session(&state, &body.expected_session).map_err(OperationError::safe)?;
        if state.host != body.expected_host {
            return Err(OperationError::safe(
                "The active device changed. Review the connection first",
            ));
        }
    }
    let model = device.session.info.model;
    let extension = device
        .session
        .adapter()
        .wire
        .setup_presets
        .unwrap_or_default();
    let presets = setup_presets(device, app)
        .await
        .map_err(OperationError::safe)?;
    let document = match body.action.as_str() {
        "open" => {
            let (_, plain) = presets
                .iter()
                .find(|(name, _)| *name == body.name)
                .ok_or_else(|| OperationError::safe("Choose a setup preset of this processor"))?;
            String::from_utf8_lossy(plain).replace("\r\n", "\n")
        }
        "save_as" => {
            validate_store_name(&body.name, max_store_name_length(model))
                .map_err(OperationError::safe)?;
            if presets
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(&body.name))
            {
                return Err(OperationError::safe(format!(
                    "A setup preset named {} already exists. Choose another name",
                    body.name
                )));
            }
            let current = read_file_document(app, device, Scope::System)
                .await
                .map_err(OperationError::safe)?;
            renamed_preset_file(&current.raw.replace("\r\n", "\n"), &body.name, extension)
                .map_err(OperationError::safe)?
        }
        _ => return Err(OperationError::safe("Unknown setup preset action")),
    };
    let iv = *uuid::Uuid::new_v4().as_bytes();
    let sealed = orban_protocol::archive::encrypt(document.as_bytes(), &device.code, iv)
        .map_err(OperationError::safe)?;
    let setup_reply = device
        .session
        .adapter()
        .wire
        .setup_request
        .map_or(233, |(_, reply)| reply);
    let mut message = format!("0\n0\n\n{}\n", sealed.len()).into_bytes();
    message.extend(sealed);
    device
        .session
        .send(setup_reply, &message)
        .await
        .map_err(OperationError::uncertain)?;
    if let Some(end) = device.session.adapter().wire.file_end {
        device
            .session
            .send(end, b"")
            .await
            .map_err(OperationError::uncertain)?;
        device.boundaries.sent();
    }
    let confirmed = if body.action == "open" {
        let wanted = file_document(document.as_bytes(), model).map_err(OperationError::safe)?;
        let system = read_file_document(app, device, Scope::System)
            .await
            .map_err(|e| {
                OperationError::uncertain(format!("The new setup could not be confirmed. {e}"))
            })?;
        // The active input is a status the unit reports, not a setting.
        let differs = wanted
            .fields
            .iter()
            .filter(|(name, _)| name.as_str() != "ACTUAL A OR D")
            .any(|(name, field)| system.fields.get(name) != Some(field));
        app.state.write().await.system = Some(system);
        !differs
    } else {
        setup_presets(device, app)
            .await
            .map_err(OperationError::uncertain)?
            .iter()
            .any(|(name, _)| *name == body.name)
    };
    if confirmed {
        app.state.write().await.revision += 1;
        Ok(())
    } else {
        Err(OperationError::uncertain(
            "The processor does not show the change. Open System Settings to see what it holds",
        ))
    }
}

/// The original PC Remote's Restore of user preset files: each file as
/// opcode 231 (or the model's own opcode for its automation and network
/// files) with its modification time (a Windows FILETIME, low and high
/// word), its name and the archive sealed with the access code, then 222.
async fn restore_preset_files(
    device: &mut Device,
    app: &App,
    body: RestorePresetFiles,
) -> Result<(), OperationError> {
    require_synchronized(device)?;
    require_writes(device)?;
    let model = device.session.info.model;
    let extension = device.session.adapter().wire.user_extension;
    if device.session.adapter().wire.preset_file_delete.is_none() {
        return Err(OperationError::safe(format!(
            "Restoring preset files is not available for {}",
            model.label()
        )));
    }
    if !body.confirmed {
        return Err(OperationError::safe("Confirm the restore first"));
    }
    if !device.session.info.access().memory {
        return Err(OperationError::safe(
            "Preset changes are not enabled for this access level",
        ));
    }
    {
        let state = app.state.read().await;
        ensure_session(&state, &body.expected_session).map_err(OperationError::safe)?;
        if state.host != body.expected_host {
            return Err(OperationError::safe(
                "The active device changed. Review the connection first",
            ));
        }
    }
    let mut names = Vec::new();
    let others = device.session.adapter().wire.restore_extensions;
    for file in &body.files {
        let lower = file.name.to_ascii_lowercase();
        let other = others
            .iter()
            .any(|e| lower.len() > e.len() && lower.ends_with(e));
        let valid_name = (other
            || (file.name.len() > extension.len() && lower.ends_with(extension)))
            && !file
                .name
                .contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|'])
            && file.name.is_ascii();
        if !valid_name {
            return Err(OperationError::safe(format!(
                "{} is not a user preset file of the {}",
                file.name,
                model.label()
            )));
        }
        if other {
            // Setup, automation and network files: the client checks no more.
            if !file.document.starts_with("OptimodVersion=<") {
                return Err(OperationError::safe(format!(
                    "{} is not an Optimod file",
                    file.name
                )));
            }
            continue;
        }
        let document = file_document(file.document.as_bytes(), model)
            .map_err(|e| OperationError::safe(format!("{}: {e}", file.name)))?;
        names.push(document.name);
    }
    for file in &body.files {
        // FILETIME: 100 ns since 1601.
        let millis = file.modified.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or_default()
        });
        let filetime = millis * 10_000 + 116_444_736_000_000_000;
        let iv = *uuid::Uuid::new_v4().as_bytes();
        let sealed = orban_protocol::archive::encrypt(file.document.as_bytes(), &device.code, iv)
            .map_err(OperationError::safe)?;
        let mut message = format!(
            "{}\n{}\n{}\n{}\n",
            filetime & 0xffff_ffff,
            filetime >> 32,
            file.name,
            sealed.len()
        )
        .into_bytes();
        message.extend(sealed);
        // The client sends some files with an opcode of their own (the
        // automation file as EBh on the 5500i, EAh on the 5500).
        let opcode = device
            .session
            .adapter()
            .wire
            .restore_files
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(&file.name))
            .map_or(231, |(_, opcode)| *opcode);
        device
            .session
            .send(opcode, &message)
            .await
            .map_err(OperationError::uncertain)?;
        // The unit answers each preset-file upload with a boundary answer
        // (FBh); automation and passcode uploads get none.
        if opcode == 231 {
            device.boundaries.sent();
        }
        let _ = device.session.send(222, b"").await;
    }
    let listed = refresh_presets(app, device).await;
    let state = app.state.read().await;
    let missing: Vec<&str> = names
        .iter()
        .filter(|name| !state.presets.iter().any(|p| &p.name == *name))
        .map(String::as_str)
        .collect();
    match listed {
        Ok(()) if missing.is_empty() => Ok(()),
        Ok(()) => Err(OperationError::uncertain(format!(
            "The processor's preset list does not show {}",
            missing.join(", ")
        ))),
        Err(e) => Err(OperationError::uncertain(format!(
            "The restore could not be confirmed. Refresh the preset list. {e}"
        ))),
    }
}

async fn manage_preset(
    device: &mut Device,
    app: &App,
    body: PresetFile,
) -> Result<(), OperationError> {
    use orban_protocol::presets::{PresetKind, max_store_name_length, validate_store_name};
    require_synchronized(device)?;
    let capabilities = device.session.info.capabilities;
    let model = device.session.info.model;
    let needs_store = !matches!(body.action, PresetFileAction::Delete { .. });
    let needs_delete = !matches!(body.action, PresetFileAction::Save { .. });
    if (needs_store && !capabilities.preset_store) || (needs_delete && !capabilities.preset_delete)
    {
        return Err(OperationError::safe(format!(
            "Saving, renaming and deleting presets on the processor is not available for {}",
            model.label()
        )));
    }
    if !body.confirmed {
        return Err(OperationError::safe("Confirm the preset change first"));
    }
    let access = device.session.info.access();
    if (needs_store && !access.presets) || (needs_delete && !access.delete) {
        return Err(OperationError::safe(
            "Preset changes are not enabled for this access level",
        ));
    }
    {
        let state = app.state.read().await;
        ensure_session(&state, &body.expected_session).map_err(OperationError::safe)?;
        if state.host != body.expected_host {
            return Err(OperationError::safe(
                "The active device changed. Review the connection first",
            ));
        }
        let on_air = state.processing.as_ref().map(|d| d.name.as_str());
        if on_air != Some(body.expected_name.as_str()) {
            return Err(OperationError::safe(
                "The on-air preset changed elsewhere. Review the current preset first",
            ));
        }
        let kind = |name: &str| {
            state
                .presets
                .iter()
                .find(|p| p.name == name)
                .map(|p| p.kind.clone())
        };
        let new_name = match &body.action {
            PresetFileAction::Save { name } | PresetFileAction::Rename { new_name: name, .. } => {
                Some(name)
            }
            PresetFileAction::Delete { .. } => None,
        };
        // The original 5700i client's Save stores the on-air preset under
        // its own name again, replacing that user preset.
        let saves_on_air = matches!(&body.action, PresetFileAction::Save { name }
            if device.session.adapter().wire.preset_file_delete.is_some()
                && kind(name) == Some(PresetKind::User)
                && on_air.is_some_and(|n| n == name || n.strip_prefix("modif ") == Some(name)));
        if let Some(name) = new_name.filter(|_| !saves_on_air) {
            validate_store_name(name, max_store_name_length(model))
                .map_err(OperationError::safe)?;
            match kind(name) {
                Some(PresetKind::Factory) => {
                    return Err(OperationError::safe(format!(
                        "A factory preset named {name} already exists"
                    )));
                }
                Some(_) => {
                    return Err(OperationError::safe(format!(
                        "A preset named {name} already exists. Choose another name"
                    )));
                }
                None => {}
            }
        }
        match &body.action {
            PresetFileAction::Delete { name } => {
                if kind(name) != Some(PresetKind::User) {
                    return Err(OperationError::safe("Only user presets can be deleted"));
                }
                if on_air.is_some_and(|n| n == name || n.strip_prefix("modif ") == Some(name)) {
                    return Err(OperationError::safe(format!(
                        "Cannot delete the on-air preset {name}"
                    )));
                }
            }
            PresetFileAction::Rename { name, .. } => {
                if kind(name) != Some(PresetKind::User) {
                    return Err(OperationError::safe("Only user presets can be renamed"));
                }
                // SP saves the on-air processing, so only the unmodified
                // on-air preset can be renamed without changing its content.
                if on_air != Some(name.as_str()) {
                    return Err(OperationError::safe(format!(
                        "Recall {name} without changes first; only the on-air preset can be renamed"
                    )));
                }
            }
            PresetFileAction::Save { .. } => {}
        }
    }
    match body.action {
        PresetFileAction::Save { name } => preset_file_step(device, app, &name, true).await,
        PresetFileAction::Delete { name } => preset_file_step(device, app, &name, false).await,
        PresetFileAction::Rename { name, new_name } => {
            preset_file_step(device, app, &new_name, true).await?;
            preset_file_step(device, app, &name, false)
                .await
                .map_err(|e| {
                    OperationError::uncertain(format!(
                        "{new_name} was saved, but {name} was not deleted. {}",
                        e.message
                    ))
                })
        }
    }?;
    let mut state = app.state.write().await;
    if let Some(document) = &state.processing {
        let base = document
            .name
            .strip_prefix("modif ")
            .unwrap_or(&document.name)
            .to_owned();
        state.preset_base_name = Some(base);
    }
    state.error = None;
    Ok(())
}

fn same_value_type(left: &Value, right: &Value) -> bool {
    matches!(
        (left, right),
        (Value::Int(_), Value::Int(_))
            | (Value::Cent(_), Value::Cent(_))
            | (Value::Choice(_), Value::Choice(_))
            | (Value::Text(_), Value::Text(_))
    )
}

fn validate_import_field(
    profile: &Profile,
    name: &str,
    current: &Field,
    requested: &Field,
) -> Result<(), String> {
    if !same_value_type(&current.value, &requested.value) {
        return Err(format!("Preset field {name} has a different value type"));
    }
    match &requested.value {
        Value::Text(text) => {
            if requested.index != current.index || text.len() > 255 {
                return Err(format!("Preset field {name} is not a supported text value"));
            }
        }
        _ => {
            if profile.value(Scope::Processing, name, requested.index)? != requested.value {
                return Err(format!(
                    "Preset field {name} does not match this processor's profile"
                ));
            }
        }
    }
    // Validate delimiters before any write reaches the device.
    orban_protocol::control::encode_change(Scope::Processing, name, requested)?;
    Ok(())
}

async fn apply_preset(
    device: &mut Device,
    app: &App,
    body: ApplyPreset,
) -> Result<(), OperationError> {
    require_synchronized(device)?;
    require_writes(device)?;
    let imported = Document::parse_for_model(body.document.as_bytes(), device.session.info.model)
        .map_err(OperationError::safe)?;
    let profile = device.profile.clone().ok_or_else(|| {
        OperationError::safe("This processor has no parameter profile; it is read-only")
    })?;
    let profile = profile.as_ref();
    let state = app.state.read().await;
    ensure_session(&state, &body.expected_session).map_err(OperationError::safe)?;
    if state.host != body.expected_host {
        return Err(OperationError::safe(
            "The active device changed. Review the connection first",
        ));
    }
    drop(state);
    if !body.confirmed {
        return Err(OperationError::safe(
            "Confirm the preset file before applying it",
        ));
    }
    if !device.session.info.access().modify {
        return Err(OperationError::safe(
            "Preset changes are not enabled for this access level",
        ));
    }

    let before = live_snapshot(device, app, Scope::Processing)
        .await
        .map_err(OperationError::safe)?;
    if before.name != body.expected_name {
        return Err(OperationError::safe(
            "The on-air preset changed. Review the preset file again",
        ));
    }
    for (name, requested) in &imported.fields {
        let current = before.fields.get(name).ok_or_else(|| {
            OperationError::safe(format!(
                "Preset field {name} is not present on this processor"
            ))
        })?;
        validate_import_field(profile, name, current, requested).map_err(OperationError::safe)?;
    }

    let current_coupled = before
        .fields
        .get("HD COUPLING")
        .is_some_and(|field| field.value == Value::Choice("FM->HD".into()));
    let target_coupled = imported
        .fields
        .get("HD COUPLING")
        .is_some_and(|field| field.value == Value::Choice("FM->HD".into()));
    let mut changes: Vec<_> = imported
        .fields
        .iter()
        .filter(|(name, requested)| {
            before.fields.get(*name) != Some(*requested)
                && !(target_coupled && hd_control_tracks_fm(name))
        })
        .map(|(name, field)| (name.clone(), field.clone()))
        .collect();
    changes.sort_by_key(|(name, _)| {
        if name == "HD COUPLING" && current_coupled && !target_coupled {
            0
        } else if name == "HD COUPLING" {
            2
        } else {
            1
        }
    });

    for (name, field) in &changes {
        device
            .session
            .change(Scope::Processing, name, field)
            .await
            .map_err(OperationError::uncertain)?;
        drain(device, app)
            .await
            .map_err(OperationError::uncertain)?;
    }

    let mut after = None;
    for attempt in 0..4 {
        let snapshot = read_back(app, device, Scope::Processing, |document| {
            imported
                .fields
                .iter()
                .all(|(name, expected)| document.fields.get(name) == Some(expected))
        })
        .await
        .map_err(OperationError::uncertain)?;
        let matches = imported
            .fields
            .iter()
            .all(|(name, expected)| snapshot.fields.get(name) == Some(expected));
        after = Some(snapshot);
        if matches {
            break;
        }
        if attempt < 3 {
            sleep(Duration::from_millis(180)).await;
        }
    }
    let mut after = after.expect("verification loop always produces a snapshot");
    // The unit names the on-air preset "modif …" after any processing write,
    // but its on-air document can still carry the old name (seen on the 5700i).
    if !changes.is_empty() && !after.name.starts_with("modif ") {
        after.name = format!("modif {}", after.name);
    }
    if !imported
        .fields
        .iter()
        .all(|(name, expected)| after.fields.get(name) == Some(expected))
    {
        return Err(OperationError::uncertain(
            "The processor did not confirm every setting in the preset file",
        ));
    }

    device.processing_baseline = Some(after.clone());
    let mut state = app.state.write().await;
    state.processing = Some(after);
    state.preset_base_name = Some(imported.name);
    state.modified_fields.clear();
    state.less_more_available = false;
    state.revision += 1;
    state.error = None;
    Ok(())
}

/// System fields a restore never writes: the network settings that carry this
/// connection, the running clock, and status values the processor reports.
fn setup_restore_exclusion(name: &str) -> Option<&'static str> {
    if name.starts_with("NETWORK ") || name.ends_with(" PORT") || name == "SHOW IP" {
        Some("network settings are left alone so the connection survives")
    } else if matches!(
        name,
        "SET HOUR" | "SET MINUTE" | "SET SECOND" | "SET DAY" | "SET MONTH" | "SET YEAR"
    ) {
        Some("the clock is not set back to the time of the backup")
    } else if name.starts_with("ACTUAL ") {
        Some("a status value reported by the processor")
    } else {
        None
    }
}

/// The system fields a restore writes, and those it skips with a reason.
type RestorePlan = (Vec<(String, Field)>, Vec<(String, String)>);

fn setup_restore_plan(profile: &Profile, current: &Document, imported: &Document) -> RestorePlan {
    let mut changes = Vec::new();
    let mut skipped = Vec::new();
    for (name, requested) in &imported.fields {
        let Some(present) = current.fields.get(name) else {
            skipped.push((name.clone(), "not present on this processor".into()));
            continue;
        };
        if present == requested {
            continue;
        }
        if let Some(reason) = setup_restore_exclusion(name) {
            skipped.push((name.clone(), reason.into()));
            continue;
        }
        let valid = same_value_type(&present.value, &requested.value)
            && match &requested.value {
                Value::Text(text) => requested.index == present.index && text.len() <= 255,
                _ => profile
                    .value(Scope::System, name, requested.index)
                    .is_ok_and(|value| value == requested.value),
            }
            && orban_protocol::control::encode_change(Scope::System, name, requested).is_ok();
        if valid {
            changes.push((name.clone(), requested.clone()));
        } else {
            skipped.push((
                name.clone(),
                "not writable with this processor's profile".into(),
            ));
        }
    }
    (changes, skipped)
}

/// Writes each planned system field once, then reads AS back (up to four
/// times, without replaying a write) and requires every written field to match.
async fn restore_setup(
    device: &mut Device,
    app: &App,
    body: RestoreSetup,
) -> Result<(), OperationError> {
    require_synchronized(device)?;
    if !device.session.info.capabilities.parameter_writes {
        return Err(OperationError::safe(format!(
            "Settings cannot be written on {}",
            device.session.info.model.label()
        )));
    }
    if !body.confirmed {
        return Err(OperationError::safe("Confirm the restore first"));
    }
    if !device.session.info.access().memory {
        return Err(OperationError::safe(
            "Setting changes are not enabled for this access level",
        ));
    }
    {
        let state = app.state.read().await;
        ensure_session(&state, &body.expected_session).map_err(OperationError::safe)?;
        if state.host != body.expected_host {
            return Err(OperationError::safe(
                "The active device changed. Review the connection first",
            ));
        }
    }
    let imported = Document::parse_for_model(body.document.as_bytes(), device.session.info.model)
        .map_err(OperationError::safe)?;
    let profile = device.profile.clone().ok_or_else(|| {
        OperationError::safe("This processor has no parameter profile; it is read-only")
    })?;
    let before = live_snapshot(device, app, Scope::System)
        .await
        .map_err(OperationError::safe)?;
    let (changes, _) = setup_restore_plan(&profile, &before, &imported);
    for (name, field) in &changes {
        device
            .session
            .change(Scope::System, name, field)
            .await
            .map_err(OperationError::uncertain)?;
        drain(device, app)
            .await
            .map_err(OperationError::uncertain)?;
    }
    let mut after = None;
    for attempt in 0..4 {
        let snapshot = read_back(app, device, Scope::System, |document| {
            changes
                .iter()
                .all(|(name, expected)| document.fields.get(name) == Some(expected))
        })
        .await
        .map_err(OperationError::uncertain)?;
        let matches = changes
            .iter()
            .all(|(name, expected)| snapshot.fields.get(name) == Some(expected));
        after = Some((snapshot, matches));
        if matches {
            break;
        }
        if attempt < 3 {
            sleep(Duration::from_millis(180)).await;
        }
    }
    let (after, matches) = after.expect("verification loop always produces a snapshot");
    let mut state = app.state.write().await;
    state.system = Some(after);
    state.revision += 1;
    if !matches {
        return Err(OperationError::uncertain(
            "The processor did not confirm every restored setting; the current values are shown",
        ));
    }
    state.error = None;
    Ok(())
}

async fn change(device: &mut Device, app: &App, body: Change) -> Result<(), OperationError> {
    require_synchronized(device)?;
    require_writes(device)?;
    let before = {
        let state = app.state.read().await;
        ensure_session(&state, &body.expected_session).map_err(OperationError::safe)?;
        if state.host != body.expected_host {
            return Err(OperationError::safe(
                "The active device changed. Review the connection first",
            ));
        }
        let document = match body.scope {
            Scope::System => state.system.as_ref(),
            Scope::Processing => state.processing.as_ref(),
        }
        .ok_or_else(|| OperationError::safe("The current settings are not available"))?;
        if body.scope == Scope::System && state.read_only_fields.contains(&body.name) {
            return Err(OperationError::safe(
                "This network setting is kept in the processor's network file and is not changed from here",
            ));
        }
        if body.scope == Scope::Processing
            && body.expected_preset.as_deref() != Some(document.name.as_str())
        {
            return Err(OperationError::safe(
                "The on-air preset changed. Review the current settings first",
            ));
        }
        if document.fields.get(&body.name) != Some(&body.expected) {
            return Err(OperationError::safe(
                "This setting changed elsewhere. Review the current value first",
            ));
        }
        document.clone()
    };
    let value = match (&body.requested, &body.expected.value) {
        (Some(Value::Text(text)), Value::Text(_))
            if body.index == body.expected.index && text.len() <= 255 =>
        {
            Value::Text(text.clone())
        }
        (Some(_), _) => return Err(OperationError::safe("Unsupported typed setting value")),
        (None, _) => {
            let profile = device.profile.as_ref().ok_or_else(|| {
                OperationError::safe("This processor has no parameter profile; it is read-only")
            })?;
            if profile
                .value(body.scope, &body.name, body.expected.index)
                .map_err(OperationError::safe)?
                != body.expected.value
            {
                return Err(OperationError::safe(
                    "The current value does not match the reference profile",
                ));
            }
            profile
                .value(body.scope, &body.name, body.index)
                .map_err(OperationError::safe)?
        }
    };
    if body.scope == Scope::Processing
        && hd_control_tracks_fm(&body.name)
        && before
            .fields
            .get("HD COUPLING")
            .is_some_and(|f| f.value == Value::Choice("FM->HD".into()))
    {
        return Err(OperationError::safe(
            "HD follows FM. Change HD coupling before editing HD independently",
        ));
    }
    let expected = Field {
        index: body.index,
        value,
    };
    orban_protocol::control::encode_change(body.scope, &body.name, &expected)
        .map_err(OperationError::safe)?;
    device
        .session
        .change(body.scope, &body.name, &expected)
        .await
        .map_err(OperationError::safe)?;
    // Statically derived adapters and statically derived fields of a verified
    // profile are confirmed by readback; hardware-verified fields keep the fast path.
    let verified_field = device.session.info.evidence != Evidence::Static
        && device.profile.as_ref().is_some_and(|profile| {
            matches!(expected.value, Value::Text(_)) || !profile.is_static(body.scope, &body.name)
        });
    let boundary = drain(device, app).await;
    let after = match boundary {
        Ok(()) if verified_field => {
            let mut after = before;
            after.fields.insert(body.name.clone(), expected.clone());
            if body.scope == Scope::Processing && !after.name.starts_with("modif ") {
                after.name = format!("modif {}", after.name);
            }
            after
        }
        boundary => {
            // A failed response boundary leaves the write outcome uncertain, and a
            // statically derived profile is not yet proven on hardware. Both use
            // the slower exact terminal readback. The 5700i pauses its meter
            // stream while producing a full AP/AS document.
            let snapshot = read_back(app, device, body.scope, |document| {
                document.fields.get(&body.name) == Some(&expected)
            })
            .await
                .map_err(|read_error| {
                    OperationError::safe(match &boundary {
                        Err(boundary_error) => format!(
                            "The processor did not confirm the change; the connection remains active. {boundary_error}. {read_error}"
                        ),
                        Ok(()) => format!(
                            "The change could not be read back; the connection remains active. {read_error}"
                        ),
                    })
                })?;
            if snapshot.fields.get(&body.name) != Some(&expected) {
                // Show what the processor actually holds, never the requested value.
                let mut state = app.state.write().await;
                match body.scope {
                    Scope::System => state.system = Some(snapshot),
                    Scope::Processing => state.processing = Some(snapshot),
                }
                state.revision += 1;
                return Err(OperationError::safe(
                    "The processor did not confirm the change",
                ));
            }
            snapshot
        }
    };
    if body.scope == Scope::System
        && decides_active_input(&body.name)
        && reads_live_input(device, &after)
    {
        device.input_status = Some(InputStatus {
            at: tokio::time::Instant::now(),
            after_change: true,
        });
    }
    let mut state = app.state.write().await;
    match body.scope {
        Scope::System => state.system = Some(after),
        Scope::Processing => {
            let (base_name, modified_fields) = refresh_processing_identity(
                &after,
                &state.presets,
                &mut device.processing_baseline,
            );
            state.processing = Some(after);
            state.preset_base_name = base_name;
            state.modified_fields = modified_fields;
            if body.name != "LESS MORE" {
                state.less_more_available = false;
            }
        }
    }
    state.revision += 1;
    state.error = None;
    Ok(())
}

/// System fields that decide which input the processor uses. The firmware
/// reports its choice only in `ACTUAL A OR D` of a live AS document: it
/// pushes nothing over the PC Remote session when the input changes, and its
/// setup file (opcode 240) keeps the value of its last save (virtual 5700i,
/// original firmware 3.0.1.20, 28 October 2026).
fn decides_active_input(name: &str) -> bool {
    matches!(
        name,
        "INPUT A OR D"
            | "ANALOG FALLBACK"
            | "DIGITAL FALLBACK"
            | "DI ANALOG FALLBACK"
            | "SILENCE THR"
            | "SILENCE DELAY"
    )
}

/// The active input can be read live: the model reports it and its system
/// document comes from the status port, not from a saved file.
fn reads_live_input(device: &Device, system: &Document) -> bool {
    !uses_files(device) && system.fields.contains_key("ACTUAL A OR D")
}

/// When to read the active input again. Without a fallback the processor
/// uses the chosen input, so nothing changes on its own. With one, it
/// switches once the input in use has been silent for `SILENCE DELAY`
/// seconds: right after a change wait for that delay, otherwise read it
/// every `INPUT_STATUS_PERIOD`.
fn next_input_status(
    device: &Device,
    system: &Document,
    after_change: bool,
) -> Option<InputStatus> {
    if !reads_live_input(device, system) {
        return None;
    }
    let enabled = |name: &str| {
        system
            .fields
            .get(name)
            .is_some_and(|field| field.value == Value::Choice("Yes".into()))
    };
    if !["ANALOG FALLBACK", "DIGITAL FALLBACK", "DI ANALOG FALLBACK"]
        .into_iter()
        .any(enabled)
    {
        return None;
    }
    let wait = match system.fields.get("SILENCE DELAY").map(|field| &field.value) {
        Some(Value::Int(seconds)) if after_change && *seconds > 0 => {
            Duration::from_secs(*seconds as u64 + 1).min(INPUT_STATUS_PERIOD)
        }
        _ => INPUT_STATUS_PERIOD,
    };
    Some(InputStatus {
        at: tokio::time::Instant::now() + wait,
        after_change: false,
    })
}

/// Reads the system document again for the active input and shows it when
/// anything differs. Only a live AS read is used (see `decides_active_input`).
async fn read_input_status(app: &App, device: &mut Device) {
    let Some(due) = device.input_status.take() else {
        return;
    };
    match live_snapshot(device, app, Scope::System).await {
        Ok(system) => {
            device.input_status = next_input_status(device, &system, due.after_change);
            let mut state = app.state.write().await;
            let changed = state
                .system
                .as_ref()
                .is_none_or(|shown| shown.name != system.name || shown.fields != system.fields);
            if changed {
                state.system = Some(system);
                state.revision += 1;
            }
        }
        Err(_) => {
            // The next attempt waits a full period; the shown value stays.
            device.input_status = Some(InputStatus {
                at: tokio::time::Instant::now() + INPUT_STATUS_PERIOD,
                after_change: false,
            });
        }
    }
}

fn hd_control_tracks_fm(name: &str) -> bool {
    name.starts_with("HD ") && !matches!(name, "HD COUPLING" | "HD DE ESS")
}

/// Automatic reconnection after an unexpected loss of the PC Remote session.
/// After a hard disconnect the reference 5700i answers no login while new
/// attempts keep arriving, and needs about two minutes without any; so the
/// first attempt waits two minutes and each failure doubles the wait.
#[derive(Clone, Copy)]
struct ReconnectPolicy {
    first: Duration,
    longest: Duration,
    attempts: u32,
}

const RECONNECT: ReconnectPolicy = ReconnectPolicy {
    first: Duration::from_secs(120),
    longest: Duration::from_secs(960),
    attempts: 6,
};

impl ReconnectPolicy {
    /// Wait before attempt `attempt` (counted from 1).
    fn delay(&self, attempt: u32) -> Duration {
        self.first
            .saturating_mul(1 << attempt.saturating_sub(1).min(16))
            .min(self.longest)
    }
}

/// The connection to restore: the same device, address, ports, model and code.
struct ReconnectTarget {
    host: String,
    port: u16,
    terminal_port: u16,
    model: DeviceModel,
    device_id: Option<String>,
    code: Zeroizing<String>,
    /// The saved connection held a code when the session was made: each
    /// attempt then uses the code saved now, and none if it was removed.
    saved_code: bool,
}

impl ReconnectTarget {
    fn from(connect: &Connect, app: &App) -> Self {
        let saved_code = connect.device_id.as_ref().is_some_and(|id| {
            app.book
                .lock()
                .is_ok_and(|book| book.find(id).is_ok_and(|saved| saved.has_code))
        });
        Self {
            host: connect.host.clone(),
            port: connect.port,
            terminal_port: connect.terminal_port,
            model: connect.model,
            device_id: connect.device_id.clone(),
            code: Zeroizing::new(connect.code.clone()),
            saved_code,
        }
    }
    fn connect(&self) -> Connect {
        Connect {
            host: self.host.clone(),
            code: self.code.to_string(),
            port: self.port,
            terminal_port: self.terminal_port,
            model: self.model,
            device_id: self.device_id.clone(),
        }
    }
    /// A saved connection that was edited or removed meanwhile is not
    /// reconnected with its old settings, and a removed code is not used.
    fn still_saved(&mut self, app: &App) -> bool {
        let Some(id) = &self.device_id else {
            return true;
        };
        let Ok(mut book) = app.book.lock() else {
            return false;
        };
        let unchanged = book.find(id).is_ok_and(|saved| {
            saved.host == self.host
                && saved.port == self.port
                && saved.terminal_port == self.terminal_port
                && saved.model == self.model
        });
        if !unchanged || !self.saved_code {
            return unchanged;
        }
        match book.code(id) {
            Ok(code) => {
                self.code = code;
                true
            }
            Err(_) => false,
        }
    }
}

struct PendingReconnect {
    target: ReconnectTarget,
    attempt: u32,
    at: tokio::time::Instant,
    reason: String,
}

fn unix_ms_after(delay: Duration) -> u64 {
    (std::time::SystemTime::now() + delay)
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}

async fn publish_reconnect(
    app: &App,
    policy: &ReconnectPolicy,
    pending: &PendingReconnect,
    in_progress: bool,
) {
    app.state.write().await.reconnect = Some(ReconnectStatus {
        attempt: pending.attempt,
        attempts: policy.attempts,
        next_attempt_at_ms: unix_ms_after(
            pending
                .at
                .saturating_duration_since(tokio::time::Instant::now()),
        ),
        in_progress,
        reason: pending.reason.clone(),
    });
}

async fn owner(app: App, rx: mpsc::Receiver<Command>) {
    owner_with(app, rx, RECONNECT).await
}

async fn owner_with(app: App, mut rx: mpsc::Receiver<Command>, policy: ReconnectPolicy) {
    let mut device: Option<Device> = None;
    // An 8400 has its own session; at most one of `device` and `unit` is set.
    let mut unit: Option<o8400::Unit> = None;
    let mut keepalive = interval(o8400::KEEPALIVE_PERIOD);
    keepalive.set_missed_tick_behavior(MissedTickBehavior::Delay);
    // What a lost session reconnects to; set by a successful connect.
    let mut target: Option<ReconnectTarget> = None;
    let mut pending: Option<PendingReconnect> = None;
    let mut tick = interval(METER_POLL_PERIOD);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    'owner: loop {
        'turn: {
            tokio::select! {
             command=rx.recv()=>{
              let Some(command)=command else{break 'owner};
              match command{
               Command::Connect(body,reply)=>{
                if device.is_some()||unit.is_some(){let _=reply.send(Err("Disconnect the current device first".into()));break 'turn;}
                // One processor, one session: another tab that holds it keeps it.
                if let Err(e)=app.peer.claim(&body.host,body.port){let _=reply.send(Err(e));break 'turn;}
                Activity::count(&app.activity.session_opens);
                // A manual connection replaces any pending automatic one.
                pending=None;
                target=None;
                *app.state.write().await = Snapshot::default();
                let remembered=ReconnectTarget::from(&body,&app);
                if o8400::wanted(&body) {
                 match o8400::connect(body,&app).await{Ok(u)=>{unit=Some(u);target=Some(remembered);let _=reply.send(Ok(()));},Err(e)=>{let _=reply.send(Err(e));}}
                 break 'turn;
                }
                match make_device(body,&app).await{Ok(d)=>{device=Some(d);target=Some(remembered);let _=reply.send(Ok(()));},Err(e)=>{let _=reply.send(Err(e));}}
               }
               Command::Disconnect(reply)=>{
                // A manual disconnect is never followed by an automatic reconnect.
                pending=None;
                target=None;
                let disconnecting=device.take();
                let disconnecting_unit=unit.take();
                let mut state=app.state.write().await;state.connected=false;state.session_id=None;state.error=None;state.reconnect=None;drop(state);app.meters.send_replace(json!({"live":false}));
                let result=if let Some(d)=disconnecting{d.session.disconnect().await}else if let Some(u)=disconnecting_unit{u.disconnect().await}else{Ok(())};
                let _=reply.send(result);
               }
               Command::Refresh(reply)=>{
                let result=if let Some(u)=&mut unit{
                 let result=o8400::synchronize(&app,u).await;
                 if let Err(e)=&result{app.state.write().await.sync_error=Some(format!("Settings could not be read: {e}"));}
                 result
                }else if let Some(d)=&mut device{
                 let result=synchronize(&app,d).await;
                 if let Err(e)=&result{app.state.write().await.sync_error=Some(sync_failure(d,e));}
                 result
                }else{Err("Not connected".into())};
                let _=reply.send(result);
               }
               Command::Recall(body,reply)=>{
                Activity::count(&app.activity.device_writes);
                let result=if let Some(u)=&mut unit{o8400::recall(u,&app,body).await}else if let Some(d)=&mut device{recall(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
                tick.reset();
               match result {
                Ok(())=>{let _=reply.send(Ok(()));}
                Err(e)=>{
                  let _=reply.send(Err(e.message));
                 }
                }
               }
               Command::Change(body,reply)=>{
                Activity::count(&app.activity.device_writes);
                let result=if let Some(u)=&mut unit{o8400::change(u,&app,body).await}else if let Some(d)=&mut device{change(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
                match result {
                 Ok(())=>{let _=reply.send(Ok(()));}
                Err(e)=>{
                  // Operation failures are logged; the heartbeat owns transport-loss detection.
                  let _=reply.send(Err(e.message));
                 }
                }
               }
               Command::PresetFile(body,reply)=>{
                Activity::count(&app.activity.device_writes);
                let result=if unit.is_some(){Err(OperationError::safe("Not available for the OPTIMOD-FM 8400 yet"))}else if let Some(d)=&mut device{manage_preset(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
                tick.reset();
                let _=reply.send(result.map_err(|e| e.message));
               }
               Command::RestoreSetup(body,reply)=>{
                Activity::count(&app.activity.device_writes);
                let result=if unit.is_some(){Err(OperationError::safe("Not available for the OPTIMOD-FM 8400 yet"))}else if let Some(d)=&mut device{restore_setup(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
                let _=reply.send(result.map_err(|e| e.message));
               }
               Command::BackupFiles(reply)=>{
                let result=if unit.is_some(){Err("Not available for the OPTIMOD-FM 8400 yet".to_owned())}else if let Some(d)=&mut device{backup_files(d,&app).await}else{Err("Not connected".to_owned())};
                tick.reset();
                let _=reply.send(result);
               }
               Command::ReadUnitFile(request,reply)=>{
                let result=if unit.is_some(){Err("Not available for the OPTIMOD-FM 8400 yet".to_owned())}else if let Some(d)=&mut device{read_unit_file_from(d,&app,request).await}else{Err("Not connected".to_owned())};
                tick.reset();
                let _=reply.send(result);
               }
               Command::WriteUnitFile(body,reply)=>{
                Activity::count(&app.activity.device_writes);
                let result=if unit.is_some(){Err(OperationError::safe("Not available for the OPTIMOD-FM 8400 yet"))}else if let Some(d)=&mut device{write_unit_file(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
                tick.reset();
                let _=reply.send(result.map_err(|e| e.message));
               }
               Command::SetupPresets(reply)=>{
                let result=if unit.is_some(){Err("Not available for the OPTIMOD-FM 8400 yet".to_owned())}else if let Some(d)=&mut device{setup_presets(d,&app).await.map(|files| files.into_iter().map(|(name,_)| name).collect())}else{Err("Not connected".to_owned())};
                tick.reset();
                let _=reply.send(result);
               }
               Command::SetupPreset(body,reply)=>{
                Activity::count(&app.activity.device_writes);
                let result=if unit.is_some(){Err(OperationError::safe("Not available for the OPTIMOD-FM 8400 yet"))}else if let Some(d)=&mut device{setup_preset(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
                tick.reset();
                let _=reply.send(result.map_err(|e| e.message));
               }
               Command::RestorePresetFiles(body,reply)=>{
                Activity::count(&app.activity.device_writes);
                let result=if unit.is_some(){Err(OperationError::safe("Not available for the OPTIMOD-FM 8400 yet"))}else if let Some(d)=&mut device{restore_preset_files(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
                tick.reset();
                let _=reply.send(result.map_err(|e| e.message));
               }
               Command::CancelReconnect(reply)=>{
                // A Cancel that waited behind a successful attempt has nothing
                // left to cancel; it must not turn off later reconnects.
                if pending.is_none(){let _=reply.send(Err("No reconnection is pending".into()));break 'turn;}
                pending=None;
                target=None;
                app.state.write().await.reconnect=None;
                let _=reply.send(Ok(()));
               }
               Command::ApplyPreset(body,reply)=>{
                Activity::count(&app.activity.device_writes);
                let result=if unit.is_some(){Err(OperationError::safe("Not available for the OPTIMOD-FM 8400 yet"))}else if let Some(d)=&mut device{apply_preset(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
                match result {
                Ok(())=>{let _=reply.send(Ok(()));}
                Err(e)=>{
                  let _=reply.send(Err(e.message));
                 }
                }
               }
              }
              // A poll during the command may have found the session lost.
              if let Some(lost)=device.as_mut().and_then(|d| d.lost.take()) {
                 end_session(&app,&mut device,lost.clone()).await;
                 pending=schedule_reconnect(&app,&policy,target.take(),lost).await;
              }
              if let Some(d)=device.as_mut().filter(|d| d.pushed.is_some()) {on_air_pushed(&app,d).await;}
             }
             _=keepalive.tick(),if unit.is_some()=>{
              let u=unit.as_mut().unwrap();
              if let Err(e)=o8400::keepalive(u).await {
                 let lost=format!("The PC Remote session was lost: {e}");
                 if let Some(u)=unit.take(){let _=u.disconnect().await;}
                 end_session(&app,&mut device,lost.clone()).await;
                 pending=schedule_reconnect(&app,&policy,target.take(),lost).await;
              }
             }
             _=tick.tick(),if device.as_ref().is_some_and(|d| d.session.info.capabilities.live_meters)=>{
              let d=device.as_mut().unwrap();Activity::count(&app.activity.meter_polls);let result=async{d.session.send(219,b"").await?;drain(d,&app).await}.await;
              if let Err(e)=result && let Some(lost)=session_lost(d,&e) {
                 end_session(&app,&mut device,lost.clone()).await;
                 pending=schedule_reconnect(&app,&policy,target.take(),lost).await;
              } else if d.pushed.is_some() {
                 on_air_pushed(&app,d).await;
              } else if d.archive_retry.is_some_and(|(at,_)| at<=tokio::time::Instant::now()) {
                 read_pushed_on_air(&app,d).await;
              } else if d.input_status.is_some_and(|due| due.at<=tokio::time::Instant::now()) {
                 read_input_status(&app,d).await;
              }
             }
             _=tokio::time::sleep_until(pending.as_ref().map_or_else(tokio::time::Instant::now,|p| p.at)),if pending.is_some()=>{
              let mut attempt=pending.take().unwrap();
              if !attempt.target.still_saved(&app) {
               let mut state=app.state.write().await;
               state.reconnect=None;
               state.error=Some("Automatic reconnection stopped: the saved connection or its access code was changed or removed".into());
               break 'turn;
              }
              publish_reconnect(&app,&policy,&attempt,true).await;
              // The same reset of the visible state as a manual connect, keeping the status.
              let status=app.state.read().await.reconnect.clone();
              *app.state.write().await=Snapshot{reconnect:status,..Snapshot::default()};
              let body=attempt.target.connect();
              Activity::count(&app.activity.session_opens);
              let made=if o8400::wanted(&body){o8400::connect(body,&app).await.map(|u|{unit=Some(u);})}else{make_device(body,&app).await.map(|d|{device=Some(d);})};
              match made {
               Ok(())=>{
                target=Some(attempt.target);
                app.state.write().await.reconnect=None;
               }
               Err(e)=>{
                if attempt.attempt>=policy.attempts || refuses_this_target(&e) {
                 let mut state=app.state.write().await;
                 state.reconnect=None;
                 state.error=Some(if refuses_this_target(&e) {
                  format!("Automatic reconnection stopped: the processor refused this connection. {e}")
                 } else {
                  format!("Automatic reconnection stopped after {} attempts. Connect again when the processor is reachable. {e}",policy.attempts)
                 });
                } else {
                 attempt.attempt+=1;
                 attempt.at=tokio::time::Instant::now()+policy.delay(attempt.attempt);
                 attempt.reason=e;
                 publish_reconnect(&app,&policy,&attempt,false).await;
                 pending=Some(attempt);
                }
               }
              }
             }
            }
        }
        // The processor this session holds until the next turn: connected,
        // or waiting to reconnect.
        let held = if device.is_some() || unit.is_some() {
            target.as_ref().map(|t| (t.host.as_str(), t.port))
        } else {
            pending
                .as_ref()
                .map(|p| (p.target.host.as_str(), p.target.port))
        };
        app.peer.hold(held);
    }
    if let Some(d) = device {
        let _ = d.session.disconnect().await;
    }
    if let Some(u) = unit {
        let _ = u.disconnect().await;
    }
}

/// A refusal that another attempt cannot change: the code or the processor
/// identity no longer match. Retrying would only repeat the login.
fn refuses_this_target(error: &str) -> bool {
    [
        "password failed",
        "no password set",
        "does not match saved model",
        "Unsupported device model",
    ]
    .iter()
    .any(|refusal| error.contains(refusal))
}

/// After an unexpected loss: wait, then reconnect to the same target. Nothing
/// that was pending on the lost session is replayed; a reconnect is a new
/// session that must synchronize before any control is enabled again.
async fn schedule_reconnect(
    app: &App,
    policy: &ReconnectPolicy,
    target: Option<ReconnectTarget>,
    reason: String,
) -> Option<PendingReconnect> {
    let pending = PendingReconnect {
        target: target?,
        attempt: 1,
        at: tokio::time::Instant::now() + policy.delay(1),
        reason,
    };
    publish_reconnect(app, policy, &pending, false).await;
    Some(pending)
}

async fn end_session(app: &App, device: &mut Option<Device>, error: String) {
    if let Some(d) = device.take() {
        let _ = d.session.disconnect().await;
    }
    let mut state = app.state.write().await;
    state.connected = false;
    state.session_id = None;
    state.diagnostics = None;
    state.read_only_fields.clear();
    state.error = Some(error);
    app.meters.send_replace(json!({"live":false}));
}

fn ensure_session(state: &Snapshot, expected: &str) -> Result<(), String> {
    if state.connected && state.session_id.as_deref() == Some(expected) {
        Ok(())
    } else {
        Err("The active device session changed. Review the current connection first".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The upload the original 5700i client sent for Save As "OOR SAVEAS 1"
    /// (28 October 2026): the on-air file with LF line ends and only the two
    /// name lines changed.
    #[test]
    fn a_5700i_save_renames_the_on_air_file_as_the_original_client() {
        let on_air = "OptimodVersion=<5700.51>\r\nPreset Name=<modif UFBS 15 09 2022> size=306\r\nFactory Preset Name=<LOUD-BIG>\r\nPreset Type= <MULTIBAND PRESET>\r\nPreset File Name= <modif UFBS 15 09 2022.orb57user>\r\nC:<AGC>String:<On>;D:1;\r\nEnd Preset<end>\r\n";
        assert_eq!(
            super::renamed_preset_file(on_air, "OOR SAVEAS 1", ".orb57user").unwrap(),
            "OptimodVersion=<5700.51>\nPreset Name=<OOR SAVEAS 1> size=306\nFactory Preset Name=<LOUD-BIG>\nPreset Type= <MULTIBAND PRESET>\nPreset File Name= <OOR SAVEAS 1.orb57user>\nC:<AGC>String:<On>;D:1;\nEnd Preset<end>\n"
        );
        assert!(
            super::renamed_preset_file("C:<AGC>String:<On>;D:1;\n", "X", ".orb57user").is_err()
        );
    }
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tower::ServiceExt;
    fn processing(name: &str, gain_index: u32) -> Document {
        Document {
            name: name.into(),
            fields: [(
                "GAIN".into(),
                Field {
                    index: gain_index,
                    value: Value::Int(gain_index as i32),
                },
            )]
            .into_iter()
            .collect(),
            raw: String::new(),
        }
    }

    fn processing_with_less_more(name: &str, gain_index: u32) -> Document {
        let mut document = processing(name, gain_index);
        document.fields.insert(
            "LESS MORE".into(),
            Field {
                index: 10,
                value: Value::Cent(500),
            },
        );
        document
    }

    #[test]
    fn saved_preset_becomes_the_field_comparison_baseline() {
        let current = processing("GREGG OPEN", 10);
        let presets = vec![orban_protocol::presets::Preset {
            name: "GREGG OPEN".into(),
            kind: orban_protocol::presets::PresetKind::Factory,
        }];
        let mut baseline = None;
        let (name, modified) = refresh_processing_identity(&current, &presets, &mut baseline);
        assert_eq!(name.as_deref(), Some("GREGG OPEN"));
        assert!(modified.is_empty());

        let edited = processing("modif GREGG OPEN", 11);
        let (name, modified) = refresh_processing_identity(&edited, &[], &mut baseline);
        assert_eq!(name.as_deref(), Some("GREGG OPEN"));
        assert_eq!(modified, BTreeSet::from(["GAIN".into()]));
    }

    #[test]
    fn returning_a_field_to_its_preset_value_clears_modified_state() {
        let baseline = processing("GREGG OPEN", 10);
        let restored = processing("modif GREGG OPEN", 10);
        assert!(processing_modifications(&restored, Some(&baseline)).is_empty());
    }

    #[test]
    fn an_already_modified_document_still_establishes_a_session_baseline() {
        let current = processing("modif GREGG OPEN", 10);
        let presets = vec![
            orban_protocol::presets::Preset {
                name: "GREGG OPEN".into(),
                kind: orban_protocol::presets::PresetKind::Factory,
            },
            orban_protocol::presets::Preset {
                name: "modif GREGG OPEN".into(),
                kind: orban_protocol::presets::PresetKind::Unsaved,
            },
        ];
        let mut baseline = None;
        let (name, modified) = refresh_processing_identity(&current, &presets, &mut baseline);
        assert_eq!(name.as_deref(), Some("GREGG OPEN"));
        assert!(modified.is_empty());

        let changed = processing("modif GREGG OPEN", 11);
        let (_, modified) = refresh_processing_identity(&changed, &presets, &mut baseline);
        assert_eq!(modified, BTreeSet::from(["GAIN".into()]));
    }

    #[test]
    fn device_readback_does_not_replace_the_loaded_preset_baseline() {
        let loaded = processing("GREGG OPEN", 10);
        let saved = orban_protocol::presets::Preset {
            name: "GREGG OPEN".into(),
            kind: orban_protocol::presets::PresetKind::User,
        };
        let unsaved = orban_protocol::presets::Preset {
            name: "modif GREGG OPEN".into(),
            kind: orban_protocol::presets::PresetKind::Unsaved,
        };
        let mut baseline = None;
        refresh_processing_identity(&loaded, std::slice::from_ref(&saved), &mut baseline);

        let changed = processing("GREGG OPEN", 11);
        let (name, modified) =
            refresh_processing_identity(&changed, &[saved, unsaved], &mut baseline);

        assert_eq!(name.as_deref(), Some("GREGG OPEN"));
        assert_eq!(modified, BTreeSet::from(["GAIN".into()]));
        assert_eq!(baseline.as_ref().unwrap().fields["GAIN"].index, 10);
    }
    #[test]
    fn mutation_guard_distinguishes_sessions_on_the_same_host() {
        let state = Snapshot {
            connected: true,
            host: "192.0.2.10".into(),
            session_id: Some("new-session".into()),
            ..Snapshot::default()
        };
        assert!(ensure_session(&state, "old-session").is_err());
        assert!(ensure_session(&state, "new-session").is_ok());
    }
    #[test]
    fn meter_poll_target_matches_observed_device_cadence() {
        assert_eq!(METER_POLL_PERIOD, Duration::from_millis(50));
    }
    #[tokio::test]
    async fn long_terminal_work_keeps_the_pc_remote_heartbeat_alive() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let polls = Arc::new(AtomicUsize::new(0));
        let server_polls = polls.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut login = Vec::new();
            loop {
                let byte = stream.read_u8().await.unwrap();
                login.push(byte);
                if byte == 0 {
                    break;
                }
            }
            stream
                .write_all(b"connect ok\n0\n5700i V 3.0.1.20\n123\n")
                .await
                .unwrap();
            let mut decoder = orban_protocol::framing::Decoder::default();
            let mut buffer = [0_u8; 4096];
            loop {
                let count = match stream.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                for frame in decoder.feed(&buffer[..count]).unwrap() {
                    match orban_protocol::message::decode(&frame).unwrap() {
                        Message::Other { kind: 219, .. } => {
                            server_polls.fetch_add(1, Ordering::SeqCst);
                            stream
                                .write_all(&orban_protocol::message::encode(220, b"").unwrap())
                                .await
                                .unwrap();
                        }
                        Message::Other { kind: 250, .. } => {
                            stream
                                .write_all(&orban_protocol::message::encode(251, b"").unwrap())
                                .await
                                .unwrap();
                        }
                        _ => {}
                    }
                }
            }
        });
        let session = Session::connect(address, "1234", Duration::from_secs(1))
            .await
            .unwrap();
        let (commands, _) = mpsc::channel(1);
        let (meters, _) = watch::channel(json!({"live":false}));
        let storage = tempfile::tempdir().unwrap();
        let app = App {
            activity: Arc::default(),
            peer: sessions::Peer::default(),
            state: Arc::new(RwLock::new(Snapshot::default())),
            meters,
            commands,
            profile: shared_profile(reference_profile()),
            book: Arc::new(std::sync::Mutex::new(
                devices::Book::open(
                    storage.path().join("connections.json"),
                    Box::new(devices::FileVault::new(
                        storage.path().join("credentials.json"),
                    )),
                )
                .unwrap(),
            )),
        };
        let mut device = Device {
            session,
            boundaries: Boundaries::default(),
            lost: None,
            pushed: None,
            expected: None,
            archive_requested: false,
            archive: None,
            archive_retry: None,
            wanted_files: None,
            files: Vec::new(),
            user_presets: BTreeMap::new(),
            terminal: "127.0.0.1:1".parse().unwrap(),
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: None,
            profile: Some(reference_profile()),
            input_status: None,
            sync_gap: None,
            status_port_down: false,
        };

        with_meter_polling(&mut device, &app, async {
            sleep(Duration::from_millis(225)).await;
            Ok::<_, String>(())
        })
        .await
        .unwrap();

        assert!(polls.load(Ordering::SeqCst) >= 4);
        server.abort();
    }

    #[tokio::test]
    async fn single_control_change_does_not_wait_for_a_terminal_snapshot() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let changes = Arc::new(AtomicUsize::new(0));
        let boundaries = Arc::new(AtomicUsize::new(0));
        let server_changes = changes.clone();
        let server_boundaries = boundaries.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            loop {
                if stream.read_u8().await.unwrap() == 0 {
                    break;
                }
            }
            stream
                .write_all(b"connect ok\n0\n5700i V 3.0.1.20\n123\n")
                .await
                .unwrap();
            let mut decoder = orban_protocol::framing::Decoder::default();
            let mut buffer = [0_u8; 4096];
            loop {
                let count = match stream.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                for frame in decoder.feed(&buffer[..count]).unwrap() {
                    match orban_protocol::message::decode(&frame).unwrap() {
                        Message::Other { kind: 227, .. } => {
                            server_changes.fetch_add(1, Ordering::SeqCst);
                        }
                        Message::Other { kind: 250, .. } => {
                            server_boundaries.fetch_add(1, Ordering::SeqCst);
                            stream
                                .write_all(&orban_protocol::message::encode(251, b"").unwrap())
                                .await
                                .unwrap();
                        }
                        _ => {}
                    }
                }
            }
        });
        let session = Session::connect(address, "1234", Duration::from_secs(1))
            .await
            .unwrap();
        let (commands, _) = mpsc::channel(1);
        let (meters, _) = watch::channel(json!({"live":false}));
        let storage = tempfile::tempdir().unwrap();
        let processing = Document {
            name: "TEST".into(),
            fields: [(
                "AGC DRIVE".into(),
                Field {
                    index: 20,
                    value: Value::Int(10),
                },
            )]
            .into_iter()
            .collect(),
            raw: String::new(),
        };
        let app = App {
            activity: Arc::default(),
            peer: sessions::Peer::default(),
            state: Arc::new(RwLock::new(Snapshot {
                connected: true,
                host: "127.0.0.1".into(),
                processing: Some(processing.clone()),
                write_enabled: true,
                session_id: Some("test-session".into()),
                ..Snapshot::default()
            })),
            meters,
            commands,
            profile: shared_profile(reference_profile()),
            book: Arc::new(std::sync::Mutex::new(
                devices::Book::open(
                    storage.path().join("connections.json"),
                    Box::new(devices::FileVault::new(
                        storage.path().join("credentials.json"),
                    )),
                )
                .unwrap(),
            )),
        };
        let mut device = Device {
            session,
            boundaries: Boundaries::default(),
            lost: None,
            pushed: None,
            expected: None,
            archive_requested: false,
            archive: None,
            archive_retry: None,
            wanted_files: None,
            files: Vec::new(),
            user_presets: BTreeMap::new(),
            terminal: "127.0.0.1:1".parse().unwrap(),
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: Some(processing),
            profile: Some(reference_profile()),
            input_status: None,
            sync_gap: None,
            status_port_down: false,
        };

        change(
            &mut device,
            &app,
            Change {
                scope: Scope::Processing,
                name: "AGC DRIVE".into(),
                index: 21,
                expected: Field {
                    index: 20,
                    value: Value::Int(10),
                },
                requested: None,
                expected_host: "127.0.0.1".into(),
                expected_session: "test-session".into(),
                expected_preset: Some("TEST".into()),
            },
        )
        .await
        .unwrap();

        let state = app.state.read().await;
        assert_eq!(
            state.processing.as_ref().unwrap().fields["AGC DRIVE"],
            Field {
                index: 21,
                value: Value::Int(11),
            }
        );
        assert!(state.connected);
        assert_eq!(changes.load(Ordering::SeqCst), 1);
        assert_eq!(boundaries.load(Ordering::SeqCst), 1);
        server.abort();
    }

    #[tokio::test]
    async fn recall_uses_cached_catalog_and_pauses_pc_remote_polling() {
        let pc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let pc_address = pc_listener.local_addr().unwrap();
        let polls = Arc::new(AtomicUsize::new(0));
        let subscriptions = Arc::new(AtomicUsize::new(0));
        let server_polls = polls.clone();
        let server_subscriptions = subscriptions.clone();
        let pc_server = tokio::spawn(async move {
            let (mut stream, _) = pc_listener.accept().await.unwrap();
            loop {
                if stream.read_u8().await.unwrap() == 0 {
                    break;
                }
            }
            stream
                .write_all(b"connect ok\n0\n5700i V 3.0.1.20\n123\n")
                .await
                .unwrap();
            let mut decoder = orban_protocol::framing::Decoder::default();
            let mut buffer = [0_u8; 4096];
            loop {
                let count = match stream.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                for frame in decoder.feed(&buffer[..count]).unwrap() {
                    match orban_protocol::message::decode(&frame).unwrap() {
                        Message::Other { kind: 219, .. } => {
                            server_polls.fetch_add(1, Ordering::SeqCst);
                            stream
                                .write_all(&orban_protocol::message::encode(220, b"").unwrap())
                                .await
                                .unwrap();
                        }
                        Message::Other { kind: 222, .. } => {
                            server_subscriptions.fetch_add(1, Ordering::SeqCst);
                        }
                        Message::Other { kind: 250, .. } => {
                            stream
                                .write_all(&orban_protocol::message::encode(251, b"").unwrap())
                                .await
                                .unwrap();
                        }
                        _ => {}
                    }
                }
            }
        });

        let terminal_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let terminal_address = terminal_listener.local_addr().unwrap();
        let terminal_connections = Arc::new(AtomicUsize::new(0));
        let server_connections = terminal_connections.clone();
        let terminal_server = tokio::spawn(async move {
            let (mut stream, _) = terminal_listener.accept().await.unwrap();
            server_connections.fetch_add(1, Ordering::SeqCst);
            stream
                .write_all(b"Orban Optimod 5700i test\r\n")
                .await
                .unwrap();
            let mut command = Vec::new();
            let mut byte = [0_u8; 1];
            while !command.ends_with(b"??\r\n") {
                stream.read_exact(&mut byte).await.unwrap();
                command.push(byte[0]);
            }
            sleep(Duration::from_millis(180)).await;
            stream
                .write_all(
                    b"ON AIR: TARGET\r\nOptimodVersion=<5700.51>\r\nPreset Name=<TARGET> size=1\r\nC:<AGC DRIVE>Int:10;D:20;\r\nEnd Preset<end>\r\n",
                )
                .await
                .unwrap();
        });

        let session = Session::connect(pc_address, "1234", Duration::from_secs(1))
            .await
            .unwrap();
        let (commands, _) = mpsc::channel(1);
        let (meters, _) = watch::channel(json!({"live":false}));
        let storage = tempfile::tempdir().unwrap();
        let current = processing("CURRENT", 20);
        let app = App {
            activity: Arc::default(),
            peer: sessions::Peer::default(),
            state: Arc::new(RwLock::new(Snapshot {
                connected: true,
                host: "127.0.0.1".into(),
                processing: Some(current.clone()),
                write_enabled: true,
                session_id: Some("test-session".into()),
                presets: vec![orban_protocol::presets::Preset {
                    name: "TARGET".into(),
                    kind: orban_protocol::presets::PresetKind::Factory,
                }],
                ..Snapshot::default()
            })),
            meters,
            commands,
            profile: shared_profile(reference_profile()),
            book: Arc::new(std::sync::Mutex::new(
                devices::Book::open(
                    storage.path().join("connections.json"),
                    Box::new(devices::FileVault::new(
                        storage.path().join("credentials.json"),
                    )),
                )
                .unwrap(),
            )),
        };
        let mut device = Device {
            session,
            boundaries: Boundaries::default(),
            lost: None,
            pushed: None,
            expected: None,
            archive_requested: false,
            archive: None,
            archive_retry: None,
            wanted_files: None,
            files: Vec::new(),
            user_presets: BTreeMap::new(),
            terminal: terminal_address,
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: Some(current),
            profile: Some(reference_profile()),
            input_status: None,
            sync_gap: None,
            status_port_down: false,
        };

        recall(
            &mut device,
            &app,
            Recall {
                name: "TARGET".into(),
                expected_name: "CURRENT".into(),
                confirmed: true,
                expected_host: "127.0.0.1".into(),
                expected_session: "test-session".into(),
            },
        )
        .await
        .unwrap();

        sleep(Duration::from_millis(20)).await;
        assert_eq!(terminal_connections.load(Ordering::SeqCst), 1);
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        assert_eq!(subscriptions.load(Ordering::SeqCst), 1);
        let state = app.state.read().await;
        assert!(state.connected);
        assert_eq!(state.processing.as_ref().unwrap().name, "TARGET");
        assert_eq!(state.preset_base_name.as_deref(), Some("TARGET"));
        terminal_server.abort();
        pc_server.abort();
    }

    /// A native client that gives up before the service finishes reports a
    /// failure for an operation that may still succeed, such as a connection
    /// or a Recall. Each client must wait longer than the service can take.
    #[test]
    fn native_clients_wait_longer_than_the_service_can_take() {
        fn timeouts(file: &str) -> (u64, u64) {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../native/macos")
                .join(file);
            let source = std::fs::read_to_string(path).unwrap();
            let value = |key: &str| -> u64 {
                let start = source.find(key).unwrap_or_else(|| panic!("{file}: {key}"));
                source[start + key.len()..]
                    .trim_start_matches([' ', '='])
                    .split(|c: char| !c.is_ascii_digit())
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap()
            };
            (
                value("timeoutIntervalForRequest"),
                value("timeoutIntervalForResource"),
            )
        }
        let connect = SESSION_DEADLINE
            + BUSY_RETRY
            + SESSION_DEADLINE
            + SYSTEM_READ_DEADLINE
            + CATALOG_READ_DEADLINE
            // The on-air archive over the PC Remote session, then the
            // normal close of each terminal session (500 ms at most).
            + SESSION_DEADLINE
            + Duration::from_secs(1);
        let recall = RECALL_DEADLINE + RECALL_CONFIRM_DEADLINE;
        let change = SESSION_DEADLINE + SYSTEM_READ_DEADLINE;
        for (file, budget) in [
            ("ConnectionsAPI.swift", connect),
            ("PresetsAPI.swift", recall),
            ("SystemSettingsAPI.swift", change),
        ] {
            let (request, resource) = timeouts(file);
            // The service sends nothing until it is done, so the request
            // (idle) timeout is the one that fires first.
            assert!(
                Duration::from_secs(request) >= budget + Duration::from_secs(5),
                "{file}: {request} s for a {budget:?} operation"
            );
            assert!(resource >= request, "{file}");
        }
    }

    /// How the synthetic unit's status (terminal) port behaves.
    #[derive(Clone, Copy, PartialEq)]
    enum Terminal {
        /// Accepts and closes at once, like a disabled or wrong port.
        Closed,
        /// Accepts, sends a banner and never answers.
        Stalled,
        /// Answers AS, AP and LP/AP with small valid documents.
        Answering,
        /// Answers LP/AP and AP, but never finishes AS.
        StallsSystem,
    }

    #[derive(Default)]
    struct UnitCounts {
        /// Logins still to refuse by closing without an answer.
        refuse: AtomicUsize,
        /// Logins still to answer with `busy`.
        busy: AtomicUsize,
        login_times: std::sync::Mutex<Vec<tokio::time::Instant>>,
        logins: AtomicUsize,
        subscriptions: AtomicUsize,
        disconnects: AtomicUsize,
        writes: AtomicUsize,
        /// Every other message that is not a read, poll or session message.
        other_kinds: std::sync::Mutex<Vec<u16>>,
        polls: AtomicUsize,
        terminal_sessions: AtomicUsize,
        unsubscriptions: AtomicUsize,
    }

    /// A synthetic 5700i that accepts any number of consecutive PC Remote
    /// sessions. `stall_once` delays the answer to one 250 boundary, as a
    /// temporary network problem would.
    struct FakeUnit {
        pc: std::net::SocketAddr,
        terminal: std::net::SocketAddr,
        terminal_mode: Arc<std::sync::Mutex<Terminal>>,
        counts: Arc<UnitCounts>,
        stall_once: Arc<std::sync::Mutex<Option<Duration>>>,
        /// Bumped to drop every open PC Remote session, like a lost link.
        drop_sessions: Arc<watch::Sender<u64>>,
        /// The preset the terminal reports on air.
        on_air: Arc<std::sync::Mutex<&'static str>>,
        /// Bumped to push the on-air document on every open session, cut at
        /// 4096 wire bytes as the reference 5700i does after a terminal RP.
        push_on_air: Arc<watch::Sender<u64>>,
        /// Whether opcode 239 is answered with the on-air archive.
        answers_archive: Arc<AtomicBool>,
        /// Whether the setup file (240) and preset files (243) are answered.
        answers_files: Arc<AtomicBool>,
        /// Archive reads still answered with the previous (CURRENT) file.
        archive_lag: Arc<AtomicUsize>,
    }

    impl FakeUnit {
        async fn start(terminal: Terminal) -> Self {
            let pc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let terminal_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let unit = Self {
                pc: pc_listener.local_addr().unwrap(),
                terminal: terminal_listener.local_addr().unwrap(),
                terminal_mode: Arc::new(std::sync::Mutex::new(terminal)),
                counts: Arc::default(),
                stall_once: Arc::default(),
                drop_sessions: Arc::new(watch::channel(0).0),
                on_air: Arc::new(std::sync::Mutex::new("CURRENT")),
                push_on_air: Arc::new(watch::channel(0).0),
                answers_archive: Arc::new(AtomicBool::new(true)),
                answers_files: Arc::new(AtomicBool::new(true)),
                archive_lag: Arc::default(),
            };
            let counts = unit.counts.clone();
            let stall = unit.stall_once.clone();
            let drops = unit.drop_sessions.clone();
            let pushes = unit.push_on_air.clone();
            let archive_on_air = unit.on_air.clone();
            let answers_archive = unit.answers_archive.clone();
            let answers_files = unit.answers_files.clone();
            let archive_lag = unit.archive_lag.clone();
            tokio::spawn(async move {
                while let Ok((stream, _)) = pc_listener.accept().await {
                    let counts = counts.clone();
                    let stall = stall.clone();
                    let archive_on_air = archive_on_air.clone();
                    let push_on_air = archive_on_air.clone();
                    let answers_archive = answers_archive.clone();
                    let answers_files = answers_files.clone();
                    let archive_lag = archive_lag.clone();
                    let mut dropped = drops.subscribe();
                    let mut pushed = pushes.subscribe();
                    tokio::spawn(async move {
                        let (mut reader, mut writer) = stream.into_split();
                        while reader.read_u8().await.is_ok_and(|byte| byte != 0) {}
                        counts
                            .login_times
                            .lock()
                            .unwrap()
                            .push(tokio::time::Instant::now());
                        if counts
                            .refuse
                            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                            .is_ok()
                        {
                            return;
                        }
                        if counts
                            .busy
                            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                            .is_ok()
                        {
                            let _ = writer.write_all(b"busy\n").await;
                            return;
                        }
                        counts.logins.fetch_add(1, Ordering::SeqCst);
                        if writer
                            .write_all(b"connect ok\n0\n5700i V 3.0.1.20\n123\n")
                            .await
                            .is_err()
                        {
                            return;
                        }
                        let mut decoder = orban_protocol::framing::Decoder::default();
                        let mut buffer = [0_u8; 4096];
                        loop {
                            let count = tokio::select! {
                                read = reader.read(&mut buffer) => match read {
                                    Ok(0) | Err(_) => return,
                                    Ok(count) => count,
                                },
                                _ = dropped.changed() => return,
                                _ = pushed.changed() => {
                                    let name = *push_on_air.lock().unwrap();
                                    let sealed = orban_protocol::archive::encrypt(
                                        on_air_text(name).as_bytes(),
                                        "1234",
                                        [5; 16],
                                    )
                                    .unwrap();
                                    let mut payload =
                                        format!("{:8}{:8}0\n0\n\n{}\n", 229 - 204, 6, sealed.len()).into_bytes();
                                    payload.extend(sealed);
                                    assert!(payload.len() > 4096);
                                    let frame = orban_protocol::framing::encode(&payload).unwrap();
                                    if writer.write_all(&frame[..4096]).await.is_err() {
                                        return;
                                    }
                                    continue;
                                }
                            };
                            for frame in decoder.feed(&buffer[..count]).unwrap() {
                                let reply = match orban_protocol::message::decode(&frame).unwrap() {
                                    Message::Other { kind: 219, .. } => {
                                        counts.polls.fetch_add(1, Ordering::SeqCst);
                                        let mut meter = vec![1];
                                        meter.extend(b"112\n");
                                        meter.extend([5; 112]);
                                        orban_protocol::message::encode(228, &meter).unwrap()
                                    }
                                    Message::Other { kind: 222, .. } => {
                                        counts.subscriptions.fetch_add(1, Ordering::SeqCst);
                                        continue;
                                    }
                                    Message::Other { kind: 223, .. } => {
                                        counts.unsubscriptions.fetch_add(1, Ordering::SeqCst);
                                        continue;
                                    }
                                    Message::Other { kind: 227, .. } => {
                                        counts.writes.fetch_add(1, Ordering::SeqCst);
                                        continue;
                                    }
                                    Message::Other { kind: 218, .. } => {
                                        counts.disconnects.fetch_add(1, Ordering::SeqCst);
                                        return;
                                    }
                                    Message::Other { kind: 239, .. } => {
                                        if !answers_archive.load(Ordering::SeqCst) {
                                            continue;
                                        }
                                        // A lagging unit still has the previous file saved.
                                        let name = if archive_lag
                                            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                                                n.checked_sub(1)
                                            })
                                            .is_ok()
                                        {
                                            "CURRENT"
                                        } else {
                                            *archive_on_air.lock().unwrap()
                                        };
                                        let text = on_air_text(name);
                                        let sealed = orban_protocol::archive::encrypt(
                                            text.as_bytes(),
                                            "1234",
                                            [3; 16],
                                        )
                                        .unwrap();
                                        let mut body =
                                            format!("0\n0\nOnAir.orb57onair\n{}\n", sealed.len())
                                                .into_bytes();
                                        body.extend(sealed);
                                        orban_protocol::message::encode(229, &body).unwrap()
                                    }
                                    Message::Other {
                                        kind: kind @ (240 | 243),
                                        ..
                                    } if answers_files.load(Ordering::SeqCst) => {
                                        let (reply, name, text) = if kind == 240 {
                                            (
                                                233,
                                                "Setup.orb57setup",
                                                document_text(
                                                    "SYSTEM PRESET",
                                                    &[("CONTRAST", "Int:3;D:3;")],
                                                ),
                                            )
                                        } else {
                                            (231, "MINE.ORB57USER", on_air_text("MINE"))
                                        };
                                        let sealed = orban_protocol::archive::encrypt(
                                            text.as_bytes(),
                                            "1234",
                                            [4; 16],
                                        )
                                        .unwrap();
                                        let mut body = format!("0\n0\n{name}\n{}\n", sealed.len())
                                            .into_bytes();
                                        body.extend(sealed);
                                        orban_protocol::message::encode(reply, &body).unwrap()
                                    }
                                    // Unanswered file reads are still reads.
                                    Message::Other {
                                        kind: 240 | 243, ..
                                    } => continue,
                                    Message::Other { kind: 250, .. } => {
                                        let delay = stall.lock().unwrap().take();
                                        if let Some(delay) = delay {
                                            sleep(delay).await;
                                        }
                                        orban_protocol::message::encode(251, b"").unwrap()
                                    }
                                    other => {
                                        if let Message::Other { kind, .. } = other {
                                            counts.other_kinds.lock().unwrap().push(kind);
                                        }
                                        continue;
                                    }
                                };
                                if writer.write_all(&reply).await.is_err() {
                                    return;
                                }
                            }
                        }
                    });
                }
            });
            let mode = unit.terminal_mode.clone();
            let on_air = unit.on_air.clone();
            let terminal_counts = unit.counts.clone();
            tokio::spawn(async move {
                while let Ok((mut stream, _)) = terminal_listener.accept().await {
                    terminal_counts
                        .terminal_sessions
                        .fetch_add(1, Ordering::SeqCst);
                    let mode = *mode.lock().unwrap();
                    let on_air = *on_air.lock().unwrap();
                    tokio::spawn(async move {
                        if mode == Terminal::Closed {
                            return;
                        }
                        let _ = stream.write_all(b"Orban Optimod 5700i test\r\n").await;
                        let mut request = Vec::new();
                        let mut byte = [0_u8; 1];
                        while !request.ends_with(b"??\r\n") {
                            if stream.read_exact(&mut byte).await.is_err() {
                                return;
                            }
                            request.push(byte[0]);
                        }
                        let request = String::from_utf8_lossy(&request);
                        if mode == Terminal::Stalled
                            || (mode == Terminal::StallsSystem && request.starts_with("AS "))
                        {
                            sleep(Duration::from_secs(60)).await;
                            return;
                        }
                        let mut reply = String::new();
                        if request.starts_with("LP ") {
                            reply.push_str("TARGET factory\r\nCURRENT user\r\n");
                        }
                        reply.push_str(&if request.starts_with("AS ") {
                            document_text("SYSTEM PRESET", &[("CONTRAST", "Int:3;D:3;")])
                        } else {
                            on_air_text(on_air)
                        });
                        let _ = stream.write_all(reply.as_bytes()).await;
                        let mut tail = [0_u8; 64];
                        let _ = stream.read(&mut tail).await;
                    });
                }
            });
            unit
        }

        fn connect(&self) -> Connect {
            Connect {
                host: "127.0.0.1".into(),
                code: "1234".into(),
                port: self.pc.port(),
                terminal_port: self.terminal.port(),
                model: DeviceModel::Auto,
                device_id: None,
            }
        }

        fn count(&self, field: fn(&UnitCounts) -> &AtomicUsize) -> usize {
            field(&self.counts).load(Ordering::SeqCst)
        }

        /// Recalls `preset` from outside the app: the terminal reports it on
        /// air and every open session gets the pushed document.
        fn recall_elsewhere(&self, preset: &'static str) {
            *self.on_air.lock().unwrap() = preset;
            self.push_on_air.send_modify(|generation| *generation += 1);
        }

        /// Ends every open session from the unit's side, like a lost link.
        fn drop_sessions(&self) {
            self.drop_sessions
                .send_modify(|generation| *generation += 1);
        }

        fn login_times(&self) -> Vec<tokio::time::Instant> {
            self.counts.login_times.lock().unwrap().clone()
        }
    }

    /// The real session owner, driven through its command channel.
    async fn running_owner() -> (App, tempfile::TempDir) {
        owner_with_policy(RECONNECT).await
    }

    async fn owner_with_policy(policy: ReconnectPolicy) -> (App, tempfile::TempDir) {
        let (mut app, storage) = test_app();
        let (commands, rx) = mpsc::channel(8);
        app.commands = commands;
        tokio::spawn(owner_with(app.clone(), rx, policy));
        (app, storage)
    }

    /// The real schedule shape with short waits.
    const FAST: ReconnectPolicy = ReconnectPolicy {
        first: Duration::from_millis(600),
        longest: Duration::from_millis(2400),
        attempts: 4,
    };

    async fn unit_counted(
        unit: &FakeUnit,
        field: fn(&UnitCounts) -> &AtomicUsize,
        n: usize,
    ) -> bool {
        for _ in 0..200 {
            if unit.count(field) == n {
                return true;
            }
            sleep(Duration::from_millis(10)).await;
        }
        false
    }

    async fn wait_until(app: &App, limit: Duration, done: impl Fn(&Snapshot) -> bool) -> bool {
        let end = tokio::time::Instant::now() + limit;
        while tokio::time::Instant::now() < end {
            if done(&*app.state.read().await) {
                return true;
            }
            sleep(Duration::from_millis(20)).await;
        }
        false
    }

    async fn command(app: &App, build: impl FnOnce(Reply) -> Command) -> Result<(), String> {
        let (reply, result) = oneshot::channel();
        app.commands.send(build(reply)).await.unwrap();
        result.await.unwrap()
    }

    fn change_request(session: &str) -> Change {
        Change {
            scope: Scope::Processing,
            name: "AGC DRIVE".into(),
            index: 21,
            expected: Field {
                index: 20,
                value: Value::Int(10),
            },
            requested: None,
            expected_host: "127.0.0.1".into(),
            expected_session: session.into(),
            expected_preset: Some("CURRENT".into()),
        }
    }

    #[tokio::test]
    async fn an_unexpected_loss_reconnects_after_the_wait_and_synchronizes_first() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = owner_with_policy(FAST).await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let first = app.state.read().await.session_id.clone().unwrap();
        // Let the unit take the first session's 222 before the link drops.
        assert!(unit_counted(&unit, |c| &c.subscriptions, 2).await);
        unit.drop_sessions();
        let lost = tokio::time::Instant::now();
        assert!(wait_until(&app, Duration::from_secs(1), |s| s.reconnect.is_some()).await);
        {
            let state = app.state.read().await;
            assert!(!state.connected);
            let status = state.reconnect.as_ref().unwrap();
            assert_eq!(
                (status.attempt, status.attempts, status.in_progress),
                (1, 4, false)
            );
            assert!(!status.reason.is_empty());
            assert_eq!(state.error.as_deref(), Some(status.reason.as_str()));
            let now_ms = unix_ms_after(Duration::ZERO);
            assert!((now_ms + 300..=now_ms + 700).contains(&status.next_attempt_at_ms));
        }
        assert!(wait_until(&app, Duration::from_secs(3), |s| s.connected).await);
        let logins = unit.login_times();
        assert_eq!(logins.len(), 2);
        assert!(logins[1] - lost >= FAST.first, "{:?}", logins[1] - lost);
        // Controls come back only with a synchronized new session and meters.
        let state = app.state.read().await.clone();
        assert!(state.reconnect.is_none() && state.sync_error.is_none() && state.error.is_none());
        assert_ne!(state.session_id.as_deref(), Some(first.as_str()));
        assert_eq!(state.processing.as_ref().unwrap().name, "CURRENT");
        // TARGET and CURRENT from `LP`, MINE from the unit's preset files.
        assert_eq!(state.presets.len(), 3);
        // The unit counts the new session's 222 as it arrives.
        assert!(unit_counted(&unit, |c| &c.subscriptions, 4).await);
        let polls = unit.count(|c| &c.polls);
        sleep(Duration::from_millis(300)).await;
        assert!(unit.count(|c| &c.polls) > polls && app.meters.borrow()["live"] == true);
        assert_eq!(unit.count(|c| &c.writes), 0);
    }

    #[tokio::test]
    async fn a_manual_disconnect_is_never_followed_by_a_reconnect() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = owner_with_policy(FAST).await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        command(&app, Command::Disconnect).await.unwrap();
        sleep(FAST.first * 3).await;
        assert_eq!(unit.login_times().len(), 1);
        assert!(app.state.read().await.reconnect.is_none());

        // Disconnecting while a reconnect is pending cancels it as well.
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        unit.drop_sessions();
        assert!(wait_until(&app, Duration::from_secs(1), |s| s.reconnect.is_some()).await);
        command(&app, Command::Disconnect).await.unwrap();
        sleep(FAST.first * 3).await;
        assert_eq!(unit.login_times().len(), 2);
        let state = app.state.read().await;
        assert!(!state.connected && state.reconnect.is_none());
    }

    #[tokio::test]
    async fn cancel_stops_a_pending_reconnect() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = owner_with_policy(FAST).await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        unit.drop_sessions();
        assert!(wait_until(&app, Duration::from_secs(1), |s| s.reconnect.is_some()).await);
        command(&app, Command::CancelReconnect).await.unwrap();
        sleep(FAST.first * 3).await;
        assert_eq!(unit.login_times().len(), 1);
        let state = app.state.read().await;
        assert!(!state.connected && state.reconnect.is_none());
        drop(state);
        // A manual connect afterwards works as usual.
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        assert!(app.state.read().await.connected);
    }

    /// Virtual 5700i under load: a login 1 s after the previous session
    /// closed was answered `busy` (5 of 100). One more login follows 1 s
    /// later; a second `busy` is reported.
    #[tokio::test]
    async fn a_busy_login_is_tried_once_more() {
        for (busy, connects) in [(1, true), (2, false)] {
            let unit = FakeUnit::start(Terminal::Answering).await;
            let (app, _storage) = running_owner().await;
            unit.counts.busy.store(busy, Ordering::SeqCst);
            let result = command(&app, |r| Command::Connect(unit.connect(), r)).await;
            assert_eq!(result.is_ok(), connects, "{result:?}");
            if !connects {
                assert!(result.unwrap_err().starts_with("The processor is busy"));
            }
            let logins = unit.login_times();
            assert_eq!(logins.len(), 2);
            assert!(logins[1] - logins[0] >= BUSY_RETRY);
        }
    }

    /// A Cancel that waited behind an attempt that succeeded finds nothing
    /// pending. It must not switch off the reconnect after a later loss.
    #[tokio::test]
    async fn a_late_cancel_keeps_later_reconnects() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = owner_with_policy(FAST).await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        assert!(unit_counted(&unit, |c| &c.subscriptions, 2).await);
        unit.drop_sessions();
        assert!(wait_until(&app, Duration::from_secs(1), |s| s.reconnect.is_some()).await);
        assert!(wait_until(&app, Duration::from_secs(3), |s| s.connected).await);
        assert_eq!(
            command(&app, Command::CancelReconnect).await.unwrap_err(),
            "No reconnection is pending"
        );
        assert!(unit_counted(&unit, |c| &c.subscriptions, 4).await);
        unit.drop_sessions();
        assert!(wait_until(&app, Duration::from_secs(1), |s| s.reconnect.is_some()).await);
        assert!(wait_until(&app, Duration::from_secs(3), |s| s.connected).await);
        assert_eq!(unit.login_times().len(), 3);
    }

    /// An access code removed from the saved connection while the app waits
    /// is not used for the next attempt.
    #[tokio::test]
    async fn a_removed_saved_code_stops_the_reconnect() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = owner_with_policy(FAST).await;
        let mut connect = unit.connect();
        let id = {
            let mut book = app.book.lock().unwrap();
            let saved = book
                .save(devices::EditDevice {
                    id: None,
                    name: "Test".into(),
                    host: connect.host.clone(),
                    port: connect.port,
                    terminal_port: connect.terminal_port,
                    model: DeviceModel::Auto,
                    group: None,
                })
                .unwrap();
            book.set_code(&saved.id, Some(Zeroizing::new("1234".into())))
                .unwrap();
            saved.id
        };
        connect.device_id = Some(id.clone());
        command(&app, |r| Command::Connect(connect, r))
            .await
            .unwrap();
        assert!(unit_counted(&unit, |c| &c.subscriptions, 2).await);
        unit.drop_sessions();
        assert!(wait_until(&app, Duration::from_secs(1), |s| s.reconnect.is_some()).await);
        app.book.lock().unwrap().set_code(&id, None).unwrap();
        assert!(wait_until(&app, Duration::from_secs(2), |s| s.reconnect.is_none()).await);
        let state = app.state.read().await;
        assert!(!state.connected);
        assert!(state.error.as_deref().unwrap().contains("access code"));
        assert_eq!(unit.login_times().len(), 1);
    }

    /// Security review: before the cut push was decoded the session died
    /// and was read again; now it survives, so the push itself must make the
    /// app read the on-air processing again.
    #[tokio::test]
    async fn a_recall_made_elsewhere_is_read_again() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let session = app.state.read().await.session_id.clone().unwrap();
        assert_eq!(
            app.state.read().await.processing.as_ref().unwrap().name,
            "CURRENT"
        );
        let terminal_sessions = unit.count(|c| &c.terminal_sessions);
        unit.recall_elsewhere("TARGET");
        // The shown document is withdrawn at once, then read again.
        assert!(wait_until(&app, Duration::from_secs(1), |s| s.processing.is_none()).await);
        assert!(
            wait_until(&app, Duration::from_secs(4), |s| s
                .processing
                .as_ref()
                .is_some_and(|d| d.name == "TARGET"))
            .await
        );
        let state = app.state.read().await;
        assert_eq!(state.session_id.as_deref(), Some(session.as_str()));
        assert_eq!(state.preset_base_name.as_deref(), Some("TARGET"));
        assert!(state.modified_fields.is_empty() && state.sync_error.is_none());
        drop(state);
        // Read on the PC Remote session: no terminal session was opened for
        // it, with meters off during the read and on again afterwards.
        assert_eq!(unit.count(|c| &c.terminal_sessions), terminal_sessions);
        assert_eq!(unit.count(|c| &c.unsubscriptions), 2);
        assert!(unit_counted(&unit, |c| &c.subscriptions, 3).await);
        // A write still addressed to the old preset is refused.
        let error = command(&app, |r| Command::Change(change_request(&session), r))
            .await
            .unwrap_err();
        assert!(error.contains("changed"), "{error}");
        assert_eq!(unit.count(|c| &c.writes), 0);
    }

    /// The push that follows the app's own Recall agrees with the document
    /// the Recall read: nothing is withdrawn or read again.
    #[tokio::test]
    async fn a_push_that_matches_the_shown_document_changes_nothing() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let revision = app.state.read().await.revision;
        unit.recall_elsewhere("CURRENT");
        sleep(Duration::from_millis(500)).await;
        let state = app.state.read().await;
        assert_eq!(state.processing.as_ref().unwrap().name, "CURRENT");
        assert_eq!(state.revision, revision);
        drop(state);
        sleep(ARCHIVE_RETRY + Duration::from_millis(500)).await;
        assert_eq!(unit.count(|c| &c.unsubscriptions), 1);
    }

    /// The unit saves the archive file a moment after the change: an archive
    /// that still holds the previous preset is not shown as the new one.
    #[tokio::test]
    async fn a_stale_archive_is_not_taken_for_the_pushed_document() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        unit.archive_lag.store(2, Ordering::SeqCst);
        unit.recall_elsewhere("TARGET");
        assert!(wait_until(&app, Duration::from_secs(1), |s| s.processing.is_none()).await);
        // Two stale reads are refused; the third, 6 s after the push, agrees.
        sleep(Duration::from_secs(5)).await;
        assert!(app.state.read().await.processing.is_none());
        assert!(
            wait_until(&app, Duration::from_secs(3), |s| s
                .processing
                .as_ref()
                .is_some_and(|d| d.name == "TARGET"))
            .await
        );
        assert_eq!(unit.count(|c| &c.unsubscriptions), 4);
    }

    /// If the new document cannot be read, the old one is not kept as if it
    /// were still on air.
    #[tokio::test]
    async fn an_unreadable_change_elsewhere_drops_the_old_document() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let session = app.state.read().await.session_id.clone().unwrap();
        unit.answers_archive.store(false, Ordering::SeqCst);
        unit.recall_elsewhere("TARGET");
        assert!(wait_until(&app, Duration::from_secs(3), |s| s.processing.is_none()).await);
        let state = app.state.read().await;
        assert!(state.connected && state.sync_error.is_some());
        drop(state);
        assert!(
            command(&app, |r| Command::Change(change_request(&session), r))
                .await
                .is_err()
        );
        assert_eq!(unit.count(|c| &c.writes), 0);
    }

    #[tokio::test]
    async fn failed_attempts_back_off_and_stop_at_the_limit() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = owner_with_policy(FAST).await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        unit.counts.refuse.store(100, Ordering::SeqCst);
        unit.drop_sessions();
        let lost = tokio::time::Instant::now();
        assert!(
            wait_until(&app, Duration::from_secs(12), |s| s.reconnect.is_none()
                && s.error.is_some())
            .await
        );
        let error = app.state.read().await.error.clone().unwrap();
        assert!(error.contains("stopped after 4 attempts"), "{error}");
        let mut previous = lost;
        let attempts: Vec<_> = unit.login_times()[1..].to_vec();
        assert_eq!(attempts.len(), 4, "one login per attempt");
        for (n, at) in attempts.iter().enumerate() {
            let wait = *at - previous;
            let expected = FAST.delay(n as u32 + 1);
            assert!(
                wait >= expected && wait < expected + Duration::from_millis(400),
                "attempt {}: {wait:?}",
                n + 1
            );
            previous = *at;
        }
        assert_eq!(
            [1, 2, 3, 4, 5].map(|n| FAST.delay(n).as_millis()),
            [600, 1200, 2400, 2400, 2400]
        );
        sleep(FAST.longest * 2).await;
        assert_eq!(unit.login_times().len(), 5, "no attempt after the limit");
        assert!(!app.state.read().await.connected);
    }

    #[tokio::test]
    async fn a_reconnect_succeeds_once_the_unit_answers_again() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = owner_with_policy(FAST).await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        unit.counts.refuse.store(2, Ordering::SeqCst);
        unit.drop_sessions();
        assert!(
            wait_until(&app, Duration::from_secs(3), |s| s
                .reconnect
                .as_ref()
                .is_some_and(|r| r.attempt == 3 && !r.in_progress))
            .await
        );
        assert!(
            !app.state
                .read()
                .await
                .reconnect
                .as_ref()
                .unwrap()
                .reason
                .is_empty()
        );
        assert!(wait_until(&app, Duration::from_secs(4), |s| s.connected).await);
        let state = app.state.read().await;
        assert!(state.reconnect.is_none() && state.sync_error.is_none());
        assert_eq!(unit.login_times().len(), 4);
    }

    #[tokio::test]
    async fn nothing_from_the_lost_session_is_replayed() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = owner_with_policy(FAST).await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let old = app.state.read().await.session_id.clone().unwrap();
        unit.drop_sessions();
        assert!(wait_until(&app, Duration::from_secs(1), |s| s.reconnect.is_some()).await);
        // A change while the app waits is refused, not queued.
        let waiting = command(&app, |r| Command::Change(change_request(&old), r)).await;
        assert_eq!(waiting.unwrap_err(), "Not connected");
        assert!(wait_until(&app, Duration::from_secs(3), |s| s.connected).await);
        sleep(Duration::from_millis(200)).await;
        assert_eq!(unit.count(|c| &c.writes), 0);
        // A request still carrying the old session is refused by the new one.
        let stale = command(&app, |r| Command::Change(change_request(&old), r)).await;
        assert!(stale.unwrap_err().contains("session changed"));
        assert_eq!(unit.count(|c| &c.writes), 0);
    }

    #[tokio::test]
    async fn status_port_failures_never_start_a_reconnect() {
        let unit = FakeUnit::start(Terminal::Closed).await;
        unit.answers_files.store(false, Ordering::SeqCst);
        let (app, _storage) = owner_with_policy(FAST).await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        assert!(command(&app, Command::Refresh).await.is_err());
        sleep(FAST.first * 3).await;
        let state = app.state.read().await;
        assert!(state.connected && state.sync_error.is_some() && state.reconnect.is_none());
        assert_eq!(unit.login_times().len(), 1);
    }

    #[test]
    fn refusals_that_cannot_change_end_the_reconnect() {
        assert!(refuses_this_target("password failed"));
        assert!(refuses_this_target(
            "Detected OPTIMOD 8700i does not match saved model OPTIMOD 5700i"
        ));
        assert!(!refuses_this_target("Login timed out"));
        assert!(!refuses_this_target("Connection timed out"));
        assert!(!refuses_this_target("Connection refused (os error 61)"));
        assert_eq!(RECONNECT.delay(1), Duration::from_secs(120));
        assert_eq!(RECONNECT.delay(2), Duration::from_secs(240));
        assert_eq!(RECONNECT.delay(5), Duration::from_secs(960));
    }

    /// Issue #18: a status port that takes no command does not limit the
    /// 5700i. Settings, presets and processing come over PC Remote, as with
    /// the original client, and the sync is complete.
    #[tokio::test]
    async fn a_closed_status_port_syncs_everything_over_pc_remote() {
        let unit = FakeUnit::start(Terminal::Closed).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        {
            let state = app.state.read().await;
            assert!(state.connected && state.error.is_none());
            assert_eq!(state.sync_error, None);
            assert_eq!(state.processing.as_ref().unwrap().name, "CURRENT");
            assert_eq!(state.system.as_ref().unwrap().name, "SYSTEM PRESET");
            let factory = state
                .presets
                .iter()
                .filter(|p| p.kind == PresetKind::Factory)
                .count();
            assert_eq!(factory, 46);
            assert!(
                state
                    .presets
                    .iter()
                    .any(|p| p.name == "MINE" && p.kind == PresetKind::User)
            );
        }
        assert_eq!(unit.count(|c| &c.terminal_sessions), 1);
        assert!(unit.count(|c| &c.subscriptions) >= 1);
        sleep(Duration::from_millis(300)).await;
        assert!(unit.count(|c| &c.polls) > 2, "meters keep running");
        assert_eq!(app.meters.borrow()["live"], true);
        // Refresh stays on PC Remote for this session.
        command(&app, Command::Refresh).await.unwrap();
        assert_eq!(app.state.read().await.sync_error, None);
        assert_eq!(unit.count(|c| &c.terminal_sessions), 1);
        assert_eq!(unit.count(|c| &c.writes), 0);
    }

    /// Issue #18 review: an incomplete synchronization refuses every command
    /// that changes the processor, not only parameter changes, and sends nothing.
    #[tokio::test]
    async fn an_incomplete_sync_sends_no_write_of_any_kind() {
        let unit = FakeUnit::start(Terminal::Closed).await;
        unit.answers_files.store(false, Ordering::SeqCst);
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let (session, host) = {
            let state = app.state.read().await;
            // Neither the status port nor the setup file answered: incomplete.
            assert!(state.connected && state.sync_error.is_some());
            (state.session_id.clone().unwrap(), state.host.clone())
        };
        let terminal_sessions = unit.count(|c| &c.terminal_sessions);
        let on_air = on_air_text("CURRENT");
        type Attempt = (&'static str, Box<dyn FnOnce(Reply) -> Command>);
        let attempts: Vec<Attempt> = vec![
            ("change", {
                let (session, host) = (session.clone(), host.clone());
                Box::new(move |r| {
                    let mut change = change_request(&session);
                    change.expected_host = host;
                    Command::Change(change, r)
                })
            }),
            ("recall", {
                let (session, host) = (session.clone(), host.clone());
                Box::new(move |r| {
                    Command::Recall(
                        Recall {
                            name: "TARGET".into(),
                            expected_name: "CURRENT".into(),
                            confirmed: true,
                            expected_host: host,
                            expected_session: session,
                        },
                        r,
                    )
                })
            }),
            ("save", {
                let (session, host) = (session.clone(), host.clone());
                Box::new(move |r| {
                    Command::PresetFile(
                        PresetFile {
                            action: PresetFileAction::Save { name: "NEW".into() },
                            expected_name: "CURRENT".into(),
                            confirmed: true,
                            expected_host: host,
                            expected_session: session,
                        },
                        r,
                    )
                })
            }),
            ("delete", {
                let (session, host) = (session.clone(), host.clone());
                Box::new(move |r| {
                    Command::PresetFile(
                        PresetFile {
                            action: PresetFileAction::Delete { name: "OLD".into() },
                            expected_name: "CURRENT".into(),
                            confirmed: true,
                            expected_host: host,
                            expected_session: session,
                        },
                        r,
                    )
                })
            }),
            ("apply file", {
                let (session, host, document) = (session.clone(), host.clone(), on_air.clone());
                Box::new(move |r| {
                    Command::ApplyPreset(
                        ApplyPreset {
                            document,
                            expected_name: "CURRENT".into(),
                            confirmed: true,
                            expected_host: host,
                            expected_session: session,
                        },
                        r,
                    )
                })
            }),
            ("restore setup", {
                let (session, host) = (session.clone(), host.clone());
                Box::new(move |r| {
                    Command::RestoreSetup(
                        RestoreSetup {
                            document: document_text("SYSTEM PRESET", &[("CONTRAST", "Int:3;D:3;")]),
                            confirmed: true,
                            expected_host: host,
                            expected_session: session,
                        },
                        r,
                    )
                })
            }),
            ("open setup", {
                let (session, host) = (session.clone(), host.clone());
                Box::new(move |r| {
                    Command::SetupPreset(
                        SetupPresetAction {
                            action: "open".into(),
                            name: "STUDIO".into(),
                            confirmed: true,
                            expected_host: host,
                            expected_session: session,
                        },
                        r,
                    )
                })
            }),
            ("save setup as", {
                let (session, host) = (session.clone(), host.clone());
                Box::new(move |r| {
                    Command::SetupPreset(
                        SetupPresetAction {
                            action: "save_as".into(),
                            name: "STUDIO".into(),
                            confirmed: true,
                            expected_host: host,
                            expected_session: session,
                        },
                        r,
                    )
                })
            }),
            ("restore presets", {
                let (session, host, document) = (session.clone(), host.clone(), on_air.clone());
                Box::new(move |r| {
                    Command::RestorePresetFiles(
                        RestorePresetFiles {
                            files: vec![BackupFile {
                                name: "NEW.orb57user".into(),
                                document,
                                modified: Some(0),
                            }],
                            confirmed: true,
                            expected_host: host,
                            expected_session: session,
                        },
                        r,
                    )
                })
            }),
            ("automation", {
                let (session, host) = (session.clone(), host.clone());
                Box::new(move |r| {
                    Command::WriteUnitFile(
                        UnitFileWrite {
                            kind: 235,
                            document: Zeroizing::new(String::new()),
                            expected_host: host,
                            expected_session: session,
                        },
                        r,
                    )
                })
            }),
            ("security", {
                let (session, host) = (session.clone(), host.clone());
                Box::new(move |r| {
                    Command::WriteUnitFile(
                        UnitFileWrite {
                            kind: 230,
                            document: Zeroizing::new("C:1234String:all access\n".into()),
                            expected_host: host,
                            expected_session: session,
                        },
                        r,
                    )
                })
            }),
        ];
        for (what, build) in attempts {
            let refused = command(&app, build).await.unwrap_err();
            assert!(refused.contains("status port (TCP"), "{what}: {refused}");
        }
        sleep(Duration::from_millis(200)).await;
        assert_eq!(unit.count(|c| &c.writes), 0);
        assert_eq!(*unit.counts.other_kinds.lock().unwrap(), Vec::<u16>::new());
        assert_eq!(unit.count(|c| &c.terminal_sessions), terminal_sessions);
        assert!(app.state.read().await.connected);
    }

    #[tokio::test]
    async fn a_silent_status_port_does_not_hold_the_connection_beyond_its_budget() {
        let unit = FakeUnit::start(Terminal::Stalled).await;
        let (app, _storage) = running_owner().await;
        let started = tokio::time::Instant::now();
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        // One deadline on the status port; then everything over PC Remote.
        let took = started.elapsed();
        assert!(
            took < SYSTEM_READ_DEADLINE + Duration::from_secs(2),
            "{took:?}"
        );
        let state = app.state.read().await;
        assert!(state.connected && state.processing.is_some() && state.system.is_some());
        assert_eq!(state.sync_error, None);
    }

    /// Issue #18: the physical 5700i's TCP 23 did not take the connection.
    /// The port is tried once; everything else comes over PC Remote.
    #[tokio::test]
    async fn an_unreachable_status_port_is_tried_once_and_pc_remote_gives_everything() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = running_owner().await;
        let mut connect = unit.connect();
        connect.terminal_port = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let started = tokio::time::Instant::now();
        command(&app, |r| Command::Connect(connect, r))
            .await
            .unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
        let state = app.state.read().await;
        assert!(state.connected && state.reconnect.is_none() && state.error.is_none());
        assert_eq!(state.sync_error, None);
        assert_eq!(state.processing.as_ref().unwrap().name, "CURRENT");
        assert_eq!(state.system.as_ref().unwrap().name, "SYSTEM PRESET");
        assert_eq!(state.presets.len(), 47);
    }

    /// A status port that does not finish `AS` in time hands the session to
    /// PC Remote after one deadline; nothing waits for it again.
    #[tokio::test]
    async fn a_late_system_read_hands_the_session_to_pc_remote() {
        let unit = FakeUnit::start(Terminal::StallsSystem).await;
        let (app, _storage) = running_owner().await;
        let started = tokio::time::Instant::now();
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        assert!(started.elapsed() < SYSTEM_READ_DEADLINE + Duration::from_secs(2));
        let state = app.state.read().await;
        assert_eq!(state.sync_error, None);
        assert_eq!(state.processing.as_ref().unwrap().name, "CURRENT");
        assert_eq!(state.system.as_ref().unwrap().name, "SYSTEM PRESET");
        assert_eq!(state.presets.len(), 47);
        assert_eq!(unit.count(|c| &c.terminal_sessions), 1);
    }

    /// Both processing structures reach the app from the on-air archive with
    /// the field that tells them apart, and only that one.
    #[tokio::test]
    async fn the_archive_fallback_carries_the_structure_on_air() {
        for (preset, present, absent) in [
            ("LOUD 5B", "B1 ATTACK", "2B DRIVE"),
            ("SOFT 2B", "2B DRIVE", "B1 ATTACK"),
        ] {
            let unit = FakeUnit::start(Terminal::Closed).await;
            *unit.on_air.lock().unwrap() = preset;
            let (app, _storage) = running_owner().await;
            command(&app, |r| Command::Connect(unit.connect(), r))
                .await
                .unwrap();
            let state = app.state.read().await;
            let processing = state.processing.as_ref().unwrap();
            assert_eq!(processing.name, preset);
            assert!(processing.fields.contains_key(present), "{preset}");
            assert!(!processing.fields.contains_key(absent), "{preset}");
        }
    }

    /// Without the status port and without the archive, no processing is
    /// shown at all, so no structure's pages can appear.
    #[tokio::test]
    async fn no_processing_is_shown_when_neither_read_succeeds() {
        let unit = FakeUnit::start(Terminal::Closed).await;
        unit.answers_archive.store(false, Ordering::SeqCst);
        unit.answers_files.store(false, Ordering::SeqCst);
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let state = app.state.read().await;
        assert!(state.connected && state.processing.is_none() && state.system.is_none());
        let sync = state.sync_error.as_deref().unwrap();
        assert!(
            sync.contains("could not be read over the PC Remote connection"),
            "{sync}"
        );
    }

    #[tokio::test]
    async fn a_hundred_connect_and_disconnect_cycles_leave_no_session_behind() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = running_owner().await;
        let mut sessions = BTreeSet::new();
        for cycle in 0..100 {
            command(&app, |r| Command::Connect(unit.connect(), r))
                .await
                .unwrap_or_else(|e| panic!("cycle {cycle}: {e}"));
            {
                let state = app.state.read().await;
                assert!(
                    state.connected && state.sync_error.is_none(),
                    "cycle {cycle}"
                );
                assert_eq!(state.processing.as_ref().unwrap().name, "CURRENT");
                assert!(sessions.insert(state.session_id.clone().unwrap()));
            }
            command(&app, Command::Disconnect).await.unwrap();
            assert!(!app.state.read().await.connected);
        }
        sleep(Duration::from_millis(100)).await;
        assert_eq!(unit.count(|c| &c.logins), 100);
        assert_eq!(unit.count(|c| &c.subscriptions), 200);
        assert_eq!(unit.count(|c| &c.disconnects), 100);
        assert_eq!(unit.count(|c| &c.writes), 0);
    }

    #[tokio::test]
    async fn a_temporary_stall_does_not_end_a_healthy_session() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        // Fourteen seconds without a response boundary: three read timeouts in
        // a row, which used to end the session.
        *unit.stall_once.lock().unwrap() = Some(Duration::from_secs(14));
        sleep(Duration::from_secs(15)).await;
        let polls = unit.count(|c| &c.polls);
        sleep(Duration::from_millis(500)).await;
        let state = app.state.read().await;
        assert!(state.connected, "{:?}", state.error);
        assert!(unit.count(|c| &c.polls) > polls + 3, "polling resumed");
        assert_eq!(unit.count(|c| &c.logins), 1, "no reconnect");
    }

    #[tokio::test]
    async fn a_closed_pc_remote_port_fails_the_connection_cleanly() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = closed.local_addr().unwrap().port();
        drop(closed);
        let (app, _storage) = running_owner().await;
        let mut connect = unit.connect();
        connect.port = port;
        assert!(
            command(&app, |r| Command::Connect(connect, r))
                .await
                .is_err()
        );
        assert!(!app.state.read().await.connected);
        // The same owner then connects normally on the configured ports.
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        assert!(app.state.read().await.connected);
    }

    /// A synthetic file-sync unit (8700HD 1.0.2.161 or 5500i 3.1.0.2) speaking the opcodes captured from the
    /// original PC Remote and firmware: F9h/FAh boundaries, the on-air file
    /// (EEh), the setup file (EFh), user preset files (F2h), Recall by an
    /// unnamed on-air file followed by FAh, files saved a few reads after a
    /// change, and a pushed on-air file followed by its own FAh.
    /// What differs between the synthetic file-sync units.
    #[derive(Clone, Copy)]
    struct FileModel {
        wire: orban_protocol::adapter::Wire,
        model: DeviceModel,
        login: &'static [u8],
        terminal: &'static [u8],
        version: &'static str,
        user: &'static str,
        on_air: &'static str,
        setup: &'static str,
    }

    const M8700HD: FileModel = FileModel {
        wire: orban_protocol::adapter::WIRE_8700HD,
        model: DeviceModel::Optimod8700Hd,
        login: b"connect ok\n0\n8700HD V 1.0.2.161\n123\n",
        terminal: b"Orban Optimod 8700HD V 1.0.2.161\n\r",
        version: "8700.51",
        user: ".ORB86USER",
        on_air: "OnAir.orb86onair",
        setup: "Setup.orb86setup",
    };

    const M5500I: FileModel = FileModel {
        wire: orban_protocol::adapter::WIRE_5500I,
        model: DeviceModel::Optimod5500i,
        login: b"connect ok\n0\n5500i V 3.1.0.2\n123\n",
        terminal: b"Orban Optimod 5500i V 3.1.0.2\n\r",
        version: "5500.50",
        user: ".ORB55USER",
        on_air: "OnAir.orb55onair",
        setup: "Setup.orbsetup",
    };

    /// The 5500 may end its login answer without a session line (issue #1).
    const M5500: FileModel = FileModel {
        wire: orban_protocol::adapter::WIRE_5500,
        model: DeviceModel::Optimod5500,
        login: b"connect ok\n0\n5500 V 1.2.8.24\n",
        terminal: b"Orban Optimod 5500 V 1.2.8.24\n\r",
        // What the firmware 1.2.8.24 writes; its installer's factory files are 8300.10.
        version: "5500.50",
        user: ".ORB55USER",
        on_air: "OnAir.orb55onair",
        setup: "Setup.orb55setup",
    };

    const M6300: FileModel = FileModel {
        wire: orban_protocol::adapter::WIRE_6300,
        model: DeviceModel::Optimod6300,
        login: b"connect ok\n0\n6300 V 4.1.1.19\n123\n",
        terminal: b"Orban Optimod 6300 V 4.1.1.19\n\r",
        version: "6300.50",
        user: ".ORB63USER",
        on_air: "OnAir.orb63onair",
        setup: "Setup.orb63setup",
    };

    const M8700I: FileModel = FileModel {
        wire: orban_protocol::adapter::WIRE_8700I,
        model: DeviceModel::Optimod8700i,
        login: b"connect ok\n0\n8700i V 1.5.1.11\n123\n",
        terminal: b"Orban Optimod 8700i V 1.5.1.11\n\r",
        version: "8700.52",
        user: ".ORB87USER",
        on_air: "OnAir.orb87onair",
        setup: "Setup.orb87setup",
    };

    /// A 5700i whose status port does not answer: it runs on files as its
    /// original PC Remote does.
    const M5700I: FileModel = FileModel {
        wire: orban_protocol::adapter::WIRE_5700I,
        model: DeviceModel::Optimod5700i,
        login: b"connect ok\n0\n5700i V 3.0.1.20\n123\n",
        terminal: b"",
        version: "5700.50",
        user: ".ORB57USER",
        on_air: "OnAir.orb57onair",
        setup: "Setup.orb57setup",
    };

    const M2300: FileModel = FileModel {
        wire: orban_protocol::adapter::WIRE_2300,
        model: DeviceModel::Optimod2300,
        login: b"connect ok\n0\n2300 V 2.0.3.85\n",
        terminal: b"",
        version: "2300.20",
        user: ".orb23user",
        on_air: "",
        setup: "",
    };

    struct FakeFileUnit {
        model: FileModel,
        pc: std::net::SocketAddr,
        terminal: std::net::SocketAddr,
        /// The on-air document the unit runs and the one its file still holds.
        running: Arc<std::sync::Mutex<String>>,
        saved: Arc<std::sync::Mutex<String>>,
        /// Requests and messages the unit received, by opcode.
        received: Arc<std::sync::Mutex<Vec<Received>>>,
        terminal_sessions: Arc<AtomicUsize>,
        push: Arc<watch::Sender<u64>>,
        /// The user preset file `LATE NIGHT`; a Recall of it saves the
        /// document it sent, with the upload's LF line ends.
        late_night: Arc<std::sync::Mutex<String>>,
    }

    /// One message the synthetic unit received: its opcode and body.
    type Received = (u16, Vec<u8>);

    const FILE_UNIT_CODE: &str = "ADMIN";

    fn doc_8700(name: &str, drive: i32) -> String {
        doc_with("8700.51", name, drive)
    }

    fn doc_with(version: &str, name: &str, drive: i32) -> String {
        format!(
            "OptimodVersion=<{version}>\r\nPreset Name=<{name}> size=3\r\nFactory Preset Name=<GREGG>\r\nPreset Type= <MULTIBAND PRESET>\r\nPreset File Name= <{name}{}>\r\nC:<AGC DRIVE>Int:{drive};D:{};\r\nC:<HD COUPLING>String:<FM->HD>;D:0;\r\nEnd Preset<end>\r\n",
            if version.starts_with("2300") {
                ".orb23user"
            } else {
                ".orbuser"
            },
            drive + 10
        )
    }

    /// A file message as `m` sends it: the 2300 has no name line.
    fn file_for_model(m: FileModel, kind: u16, name: &str, text: &str) -> Vec<u8> {
        if m.wire.file_names {
            return file_for(kind, name, text);
        }
        let sealed =
            orban_protocol::archive::encrypt(text.as_bytes(), FILE_UNIT_CODE, [7; 16]).unwrap();
        let mut body = format!("0\n0\n{}\n", sealed.len()).into_bytes();
        body.extend(sealed);
        orban_protocol::message::encode(kind, &body).unwrap()
    }

    fn file_for(kind: u16, name: &str, text: &str) -> Vec<u8> {
        orban_protocol::message::encode(kind, &file_body_with(FILE_UNIT_CODE, name, text)).unwrap()
    }

    /// A named file message body sealed with `code`.
    fn file_body_with(code: &str, name: &str, text: &str) -> Vec<u8> {
        let sealed = orban_protocol::archive::encrypt(text.as_bytes(), code, [7; 16]).unwrap();
        let mut body = format!("0\n0\n{name}\n{}\n", sealed.len()).into_bytes();
        body.extend(sealed);
        body
    }

    /// A file body for the preset-file tests, whose code is 1234.
    fn file_body(name: &str, text: &str) -> Vec<u8> {
        file_body_with("1234", name, text)
    }

    impl FakeFileUnit {
        async fn start(m: FileModel) -> Self {
            let pc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let terminal_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let unit = Self {
                model: m,
                pc: pc_listener.local_addr().unwrap(),
                terminal: terminal_listener.local_addr().unwrap(),
                running: Arc::new(std::sync::Mutex::new(doc_with(m.version, "Test01", 10))),
                saved: Arc::new(std::sync::Mutex::new(doc_with(m.version, "Test01", 10))),
                received: Arc::default(),
                terminal_sessions: Arc::default(),
                push: Arc::new(watch::channel(0).0),
                late_night: Arc::new(std::sync::Mutex::new(doc_with(m.version, "Late Night", 14))),
            };
            let (running, saved, received, pushes, late_night) = (
                unit.running.clone(),
                unit.saved.clone(),
                unit.received.clone(),
                unit.push.clone(),
                unit.late_night.clone(),
            );
            tokio::spawn(async move {
                while let Ok((stream, _)) = pc_listener.accept().await {
                    let (running, saved, received) =
                        (running.clone(), saved.clone(), received.clone());
                    let late_night = late_night.clone();
                    let mut pushed = pushes.subscribe();
                    tokio::spawn(async move {
                        let (mut reader, mut writer) = stream.into_split();
                        while reader.read_u8().await.is_ok_and(|byte| byte != 0) {}
                        writer.write_all(m.login).await.unwrap();
                        let mut decoder = orban_protocol::framing::Decoder::default();
                        let mut buffer = [0_u8; 16384];
                        // On-air reads still answered from the file saved before a change.
                        let mut lag = 0_usize;
                        // Until it is deleted, the unit holds an automation file;
                        // uploaded tool files and setup writes replace what it holds.
                        let mut automation = Some(format!(
                            "OptimodVersion=<{}>\nPreset Name=<auto_event_00> size=0\nEnd Preset<end>\n",
                            m.version
                        ));
                        let mut passcode_file: Option<String> = None;
                        let mut setup = format!(
                            "OptimodVersion=<{}>\r\nPreset Name=<SYSTEM PRESET> size=2\r\nC:<CONTRAST>Int:2;D:2;\r\nC:<PILOT SYNC>String:<REF IN>;D:0;\r\nC:<PILOT SYNC>String:<REF IN>;D:0;\r\nEnd Preset<end>\r\n",
                            m.version
                        );
                        if matches!(
                            m.model,
                            DeviceModel::Optimod5700i | DeviceModel::Optimod2300
                        ) {
                            setup = setup
                                .replace("End Preset", "C:<PRE-E>String:<75us>;D:1;\r\nEnd Preset");
                        }
                        loop {
                            let count = tokio::select! {
                                read = reader.read(&mut buffer) => match read {
                                    Ok(0) | Err(_) => return,
                                    Ok(count) => count,
                                },
                                _ = pushed.changed() => {
                                    // Units push their document with LF line ends.
                                    let text = running.lock().unwrap().replace("\r\n", "\n");
                                    let mut bytes = file_for_model(m, 229, "", &text);
                                    if let Some(end) = m.wire.file_end {
                                        bytes.extend(orban_protocol::message::encode(end, b"").unwrap());
                                    }
                                    if writer.write_all(&bytes).await.is_err() { return; }
                                    continue;
                                }
                            };
                            for frame in decoder.feed(&buffer[..count]).unwrap() {
                                let message =
                                    orban_protocol::message::decode_for(&frame, None).unwrap();
                                let (kind, data) = match message {
                                    Message::Other { kind, data } => (kind, data),
                                    // A file is recorded as its name line and its bytes.
                                    Message::File { kind, name, data } => {
                                        let mut named = format!("{name}\n").into_bytes();
                                        named.extend(data);
                                        (kind, named)
                                    }
                                    _ => continue,
                                };
                                received.lock().unwrap().push((kind, data.clone()));
                                let reply: Vec<u8> = match kind {
                                    k if k == m.wire.boundary.0 || Some(k) == m.wire.file_end => {
                                        orban_protocol::message::encode(m.wire.boundary.1, b"")
                                            .unwrap()
                                    }
                                    219 => {
                                        let mut meter = vec![1];
                                        if m.model == DeviceModel::Optimod5700i {
                                            meter.extend(b"112\n");
                                            meter.extend([0x55; 112]);
                                        } else if m.wire.file_names {
                                            meter.extend(b"104\n");
                                            meter.extend([0x55; 104]);
                                        } else {
                                            // The 2300 gives the count as one byte.
                                            meter.push(59);
                                            meter.extend([0x55; 59]);
                                        }
                                        orban_protocol::message::encode(228, &meter).unwrap()
                                    }
                                    k if k == m.wire.on_air_request => {
                                        if lag > 0 {
                                            lag -= 1;
                                            if lag == 0 {
                                                let now = running.lock().unwrap().clone();
                                                *saved.lock().unwrap() = now;
                                            }
                                        }
                                        let text = saved.lock().unwrap().clone();
                                        file_for_model(m, 229, m.on_air, &text)
                                    }
                                    k if Some(k) == m.wire.setup_request.map(|r| r.0) => {
                                        file_for_model(
                                            m,
                                            m.wire.setup_request.unwrap().1,
                                            m.setup,
                                            &setup,
                                        )
                                    }
                                    k if Some(k) == m.wire.user_presets_request.map(|r| r.0) => {
                                        let user = m.wire.user_presets_request.unwrap().1;
                                        let modif = doc_with(m.version, "modif Test01", 11);
                                        let mut bytes = file_for_model(
                                            m,
                                            user,
                                            &format!("TEST01{}", m.user),
                                            &doc_with(m.version, "Test01", 10),
                                        );
                                        bytes.extend(file_for_model(
                                            m,
                                            user,
                                            &format!("LATE NIGHT{}", m.user),
                                            &late_night.lock().unwrap().clone(),
                                        ));
                                        bytes.extend(file_for_model(
                                            m,
                                            user,
                                            &format!("MODIF TEST01{}", m.user),
                                            &modif,
                                        ));
                                        bytes.extend(file_for_model(
                                            m,
                                            user,
                                            &format!("MODIF TEST01{}", m.user),
                                            &modif,
                                        ));
                                        bytes.extend(file_for_model(m,
                                            user,
                                            &format!("FROM 6300{}", m.user),
                                            "OptimodVersion=<6300.50>\r\nPreset Name=<FROM 6300> size=1\r\nC:<AGC>String:<On>;D:1;\r\nEnd Preset<end>\r\n",
                                        ));
                                        bytes.extend(file_for_model(
                                            m,
                                            user,
                                            "SETUP.ORB86SETUP",
                                            &format!(
                                                "OptimodVersion=<{}>\r\nEnd Preset<end>\r\n",
                                                m.version
                                            ),
                                        ));
                                        bytes
                                    }
                                    227 => {
                                        let text = String::from_utf8(data).unwrap();
                                        let mut parts = text.split(';');
                                        let (name, _, value) = (
                                            parts.next().unwrap(),
                                            parts.next(),
                                            parts.next().unwrap(),
                                        );
                                        if name == "AGC DRIVE" {
                                            let drive: i32 = value
                                                .strip_prefix("Int:")
                                                .unwrap()
                                                .parse()
                                                .unwrap();
                                            let current = running.lock().unwrap().clone();
                                            let base = current.lines().nth(1).unwrap().to_owned();
                                            let name = base
                                                .strip_prefix("Preset Name=<")
                                                .and_then(|r| r.split_once('>'))
                                                .unwrap()
                                                .0
                                                .trim_start_matches("modif ")
                                                .to_owned();
                                            *running.lock().unwrap() = doc_with(
                                                m.version,
                                                &format!("modif {name}"),
                                                drive,
                                            );
                                            lag = 2;
                                        } else if let Some(line) = setup
                                            .lines()
                                            .find(|l| l.starts_with(&format!("C:<{name}>")))
                                            .map(str::to_owned)
                                        {
                                            let index = text.split(';').nth(1).unwrap();
                                            setup = setup.replace(
                                                &line,
                                                &format!("C:<{name}>{value};D:{index};"),
                                            );
                                        }
                                        continue;
                                    }
                                    229 => {
                                        let start =
                                            data.iter().position(|b| *b == b'\n').unwrap() + 1;
                                        let sealed = &data[start..];
                                        let plain = orban_protocol::archive::decrypt(
                                            sealed,
                                            FILE_UNIT_CODE,
                                        )
                                        .unwrap();
                                        let text = String::from_utf8(plain).unwrap();
                                        if text.contains("Preset Name=<Late Night>") {
                                            *late_night.lock().unwrap() = text.clone();
                                        }
                                        *running.lock().unwrap() = text.replace('\n', "\r\n");
                                        lag = 2;
                                        // The 5700i answers an upload with a boundary answer.
                                        if m.model != DeviceModel::Optimod5700i {
                                            continue;
                                        }
                                        orban_protocol::message::encode(251, b"").unwrap()
                                    }
                                    218 => return,
                                    // A preset file upload gets a boundary answer of its own,
                                    // as the 5500i firmware gives it.
                                    231 if m.wire.file_end.is_some() => {
                                        orban_protocol::message::encode(m.wire.boundary.1, b"")
                                            .unwrap()
                                    }
                                    // The automation and passcode files, by the model's opcodes.
                                    k if Some(k) == m.wire.preset_file_delete
                                        && data.starts_with(b"Automation.") =>
                                    {
                                        automation = None;
                                        continue;
                                    }
                                    k if m
                                        .wire
                                        .tool_files
                                        .is_some_and(|t| k == t.automation_request) =>
                                    {
                                        let Some(text) = &automation else {
                                            continue;
                                        };
                                        let tools = m.wire.tool_files.unwrap();
                                        file_for_model(
                                            m,
                                            tools.automation_file,
                                            tools.automation_name,
                                            text,
                                        )
                                    }
                                    k if m
                                        .wire
                                        .tool_files
                                        .is_some_and(|t| k == t.passcodes_request) =>
                                    {
                                        let passcodes = [crate::security::Passcode {
                                            code: FILE_UNIT_CODE.into(),
                                            level: crate::security::flavour(m.model).0[0].into(),
                                        }];
                                        let text = passcode_file.clone().unwrap_or_else(|| {
                                            crate::security::render(&passcodes, m.model).unwrap()
                                        });
                                        let name = format!(
                                            "Password.{}",
                                            crate::security::flavour(m.model).2
                                        );
                                        file_for_model(m, 230, &name, &text)
                                    }
                                    // An uploaded passcode or automation file replaces the unit's.
                                    k if m
                                        .wire
                                        .tool_files
                                        .is_some_and(|t| k == 230 || k == t.automation_file) =>
                                    {
                                        let start =
                                            data.iter().position(|b| *b == b'\n').unwrap() + 1;
                                        let plain = orban_protocol::archive::decrypt(
                                            &data[start..],
                                            FILE_UNIT_CODE,
                                        )
                                        .unwrap();
                                        let text = String::from_utf8(plain).unwrap();
                                        if k == 230 {
                                            passcode_file = Some(text);
                                        } else {
                                            automation = Some(text);
                                        }
                                        continue;
                                    }
                                    _ => continue,
                                };
                                if writer.write_all(&reply).await.is_err() {
                                    return;
                                }
                            }
                        }
                    });
                }
            });
            let (running, saved, sessions, push) = (
                unit.running.clone(),
                unit.saved.clone(),
                unit.terminal_sessions.clone(),
                unit.push.clone(),
            );
            tokio::spawn(async move {
                while let Ok((mut stream, _)) = terminal_listener.accept().await {
                    sessions.fetch_add(1, Ordering::SeqCst);
                    let (running, saved, push) = (running.clone(), saved.clone(), push.clone());
                    tokio::spawn(async move {
                        let _ = stream.write_all(m.terminal).await;
                        let mut command = Vec::new();
                        while !command.ends_with(b"\r\n") {
                            match stream.read_u8().await {
                                Ok(byte) => command.push(byte),
                                Err(_) => return,
                            }
                        }
                        let command = String::from_utf8(command).unwrap();
                        if let Some(name) = command
                            .strip_prefix("RP ")
                            .and_then(|r| r.split_once('['))
                            .map(|(n, _)| n.to_owned())
                        {
                            let text = doc_with(m.version, &name, 12);
                            *running.lock().unwrap() = text.clone();
                            *saved.lock().unwrap() = text;
                            // Every open PC Remote session gets the new document.
                            push.send_modify(|n| *n += 1);
                            let _ = stream
                                .write_all(format!("ON AIR: {name}\n\r").as_bytes())
                                .await;
                        }
                        let mut tail = [0_u8; 64];
                        let _ = stream.read(&mut tail).await;
                    });
                }
            });
            unit
        }

        fn connect(&self) -> Connect {
            Connect {
                host: "127.0.0.1".into(),
                code: FILE_UNIT_CODE.into(),
                port: self.pc.port(),
                terminal_port: self.terminal.port(),
                model: self.model.model,
                device_id: None,
            }
        }

        fn received(&self, kind: u16) -> Vec<Vec<u8>> {
            self.received
                .lock()
                .unwrap()
                .iter()
                .filter(|(k, _)| *k == kind)
                .map(|(_, d)| d.clone())
                .collect()
        }
    }

    #[tokio::test]
    async fn the_8700hd_synchronizes_over_pc_remote_without_its_terminal() {
        let unit = FakeFileUnit::start(M8700HD).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let state = app.state.read().await;
        assert_eq!(state.sync_error, None);
        assert_eq!(state.processing.as_ref().unwrap().name, "Test01");
        assert_eq!(
            state.system.as_ref().unwrap().fields["PILOT SYNC"].value,
            Value::Choice("REF IN".into())
        );
        let factory = state
            .presets
            .iter()
            .filter(|p| p.kind == PresetKind::Factory)
            .count();
        let named: Vec<_> = state
            .presets
            .iter()
            .filter(|p| p.kind != PresetKind::Factory)
            .map(|p| (p.name.as_str(), p.kind.clone()))
            .collect();
        assert_eq!(factory, 64);
        assert_eq!(
            named,
            [
                ("Test01", PresetKind::User),
                ("Late Night", PresetKind::User),
                ("modif Test01", PresetKind::Unsaved),
            ]
        );
        drop(state);
        assert_eq!(unit.terminal_sessions.load(Ordering::SeqCst), 0);
        // The 8700HD boundary is F9h; the 5700i's FAh request is never sent.
        sleep(Duration::from_millis(300)).await;
        assert!(!unit.received(249).is_empty());
        assert!(unit.received(250).is_empty());
        assert!(!unit.received(219).is_empty());
    }

    #[tokio::test]
    async fn the_5500i_syncs_writes_and_recalls_with_its_own_opcodes() {
        file_sync_round_trip(M5500I, 27).await;
    }

    #[tokio::test]
    async fn the_5500_syncs_writes_and_recalls_with_the_8700hd_opcodes() {
        file_sync_round_trip(M5500, 27).await;
    }

    /// The original 5500 has full control with its own opcodes: live meters,
    /// a System Setup write read back from its setup file (EFh), and its
    /// Security (E6h) and Automation (EAh) files replaced and read back.
    #[tokio::test]
    async fn the_5500_controls_meters_setup_security_and_automation() {
        let unit = FakeFileUnit::start(M5500).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let session = {
            let state = app.state.read().await;
            assert!(state.write_enabled);
            assert_eq!(
                state.system.as_ref().unwrap().fields["CONTRAST"].value,
                Value::Int(2)
            );
            state.session_id.clone().unwrap()
        };
        sleep(Duration::from_millis(300)).await;
        assert!(!unit.received(219).is_empty());
        assert_eq!(app.meters.borrow()["live"], true);
        let change = Change {
            scope: Scope::System,
            name: "CONTRAST".into(),
            index: 3,
            expected: Field {
                index: 2,
                value: Value::Int(2),
            },
            requested: None,
            expected_host: "127.0.0.1".into(),
            expected_session: session.clone(),
            expected_preset: None,
        };
        command(&app, |r| Command::Change(change, r)).await.unwrap();
        assert!(
            unit.received(227)
                .iter()
                .any(|d| d.starts_with(b"CONTRAST;3;Int:3;"))
        );
        assert_eq!(
            app.state.read().await.system.as_ref().unwrap().fields["CONTRAST"].value,
            Value::Int(3)
        );
        let write = |kind, document: String| {
            let session = session.clone();
            let app = app.clone();
            async move {
                command(&app, |r| {
                    Command::WriteUnitFile(
                        UnitFileWrite {
                            kind,
                            document: Zeroizing::new(document),
                            expected_host: "127.0.0.1".into(),
                            expected_session: session,
                        },
                        r,
                    )
                })
                .await
            }
        };
        let levels = crate::security::flavour(DeviceModel::Optimod5500).0;
        let passcodes = crate::security::render(
            &[
                crate::security::Passcode {
                    code: FILE_UNIT_CODE.into(),
                    level: levels[0].into(),
                },
                crate::security::Passcode {
                    code: "4321".into(),
                    level: levels[5].into(),
                },
            ],
            DeviceModel::Optimod5500,
        )
        .unwrap();
        write(230, passcodes).await.unwrap();
        assert_eq!(unit.received(230).len(), 1);
        let events = crate::automation::parse(
            "OptimodVersion=<5500.50>\nPreset Name=<auto_event_00> size=17\nFactory Preset Name=<Automation Preset>\nPreset Type= <AUTOMATION PRESET>\nPreset File Name= <auto_event_00.orb55autom>\nC:<AUTO SET MINUTE>String:<00:>;D:0;\nC:<AUTO SET HOUR>String:<00:>;D:0;\nC:<AUTO SET SECOND>String:<00>;D:0;\nC:<AUTO SET DAY>String:<01>;D:0;\nC:<AUTO SET MONTH>String:<JAN>;D:0;\nC:<AUTO SET YEAR>Int:2005;D:2005;\nC:<SUNDAY>String:<S>;D:1;\nC:<MONDAY>String:<M>;D:1;\nC:<TUESDAY>String:<T>;D:1;\nC:<WEDNESDAY>String:<W>;D:1;\nC:<THURSDAY>String:<T>;D:1;\nC:<FRIDAY>String:<F>;D:1;\nC:<SATURDAY>String:<S>;D:1;\nC:<SET BY DAY/WEEK>UserString:<DAY>;D:0;\nC:<AUTOMATION EVENT>UserString:<no function>;D:0;\nEnd Preset<end>\n",
        )
        .unwrap();
        let automation = crate::automation::render(&events, DeviceModel::Optimod5500).unwrap();
        write(235, automation).await.unwrap();
        assert!(
            unit.received(234)
                .iter()
                .any(|d| d.starts_with(b"Automation.orb55autom\n"))
        );
        // None of the 5500i's requests or uploads.
        for kind in [235, 237, 240, 244, 251] {
            assert!(unit.received(kind).is_empty(), "{kind}");
        }
    }

    /// The 8700HD's client has the 5500's file code with `.orb86` names:
    /// Security F3h/E6h, Automation F1h/EAh, Delete ECh, Backup EFh F1h F2h,
    /// Restore EAh + 231, and the 5700i client's level names and functions.
    #[tokio::test]
    async fn the_8700hd_files_use_its_clients_opcodes_and_names() {
        let unit = FakeFileUnit::start(M8700HD).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let session = app.state.read().await.session_id.clone().unwrap();
        let read = |request| {
            let app = app.clone();
            async move {
                let (reply, answer) = oneshot::channel();
                app.commands
                    .send(Command::ReadUnitFile(request, reply))
                    .await
                    .unwrap();
                answer.await.unwrap()
            }
        };
        let passcodes = read(230).await.unwrap().unwrap();
        assert!(passcodes.contains("C:<ADMIN>String:<All screens>;D:0;"));
        assert!(passcodes.starts_with("OptimodVersion=<8700.51>"));
        assert!(read(235).await.unwrap().is_some());
        assert!(!unit.received(204).is_empty());
        let write = |kind, document: String| {
            let session = session.clone();
            let app = app.clone();
            async move {
                command(&app, |r| {
                    Command::WriteUnitFile(
                        UnitFileWrite {
                            kind,
                            document: Zeroizing::new(document),
                            expected_host: "127.0.0.1".into(),
                            expected_session: session,
                        },
                        r,
                    )
                })
                .await
            }
        };
        let levels = crate::security::LEVELS;
        let codes = [
            crate::security::Passcode {
                code: FILE_UNIT_CODE.into(),
                level: levels[0].into(),
            },
            crate::security::Passcode {
                code: "4321".into(),
                level: levels[5].into(),
            },
        ];
        let passcodes = crate::security::render(&codes, DeviceModel::Optimod8700Hd).unwrap();
        assert!(passcodes.contains("Preset File Name= <Password.orb86pas>"));
        write(230, passcodes).await.unwrap();
        assert_eq!(unit.received(230).len(), 1);
        // SSB is one of the client's functions (0x4ad720).
        let event = crate::automation::Event {
            hour: 6,
            minute: 0,
            second: 0,
            day: 1,
            month: 1,
            year: 2026,
            weekdays: [true; 7],
            daily: true,
            action: "SSB".into(),
        };
        let automation = crate::automation::render(&[event], DeviceModel::Optimod8700Hd).unwrap();
        assert!(automation.contains("auto_event_00.orb86autom"));
        write(235, automation).await.unwrap();
        assert!(
            unit.received(234)
                .iter()
                .any(|d| d.starts_with(b"Automation.orb86autom\n"))
        );
        // No events left: the automation file is deleted with ECh.
        write(235, String::new()).await.unwrap();
        assert_eq!(unit.received(236), [b"Automation.orb86autom\n".to_vec()]);
        let _ = command(&app, |r| {
            Command::PresetFile(
                PresetFile {
                    action: PresetFileAction::Delete {
                        name: "Late Night".into(),
                    },
                    expected_name: "Test01".into(),
                    confirmed: true,
                    expected_host: "127.0.0.1".into(),
                    expected_session: session.clone(),
                },
                r,
            )
        })
        .await;
        assert!(
            unit.received(236)
                .contains(&b"Late Night.orb86user\n".to_vec())
        );
        let before = (
            unit.received(239).len(),
            unit.received(241).len(),
            unit.received(242).len(),
        );
        let (reply, answer) = oneshot::channel();
        app.commands
            .send(Command::BackupFiles(reply))
            .await
            .unwrap();
        let _ = answer.await.unwrap();
        assert_eq!(
            (
                unit.received(239).len(),
                unit.received(241).len(),
                unit.received(242).len()
            ),
            (before.0 + 1, before.1 + 1, before.2 + 1)
        );
        command(&app, |r| {
            Command::RestorePresetFiles(
                RestorePresetFiles {
                    files: vec![
                        BackupFile {
                            name: "Automation.orb86autom".into(),
                            document: "OptimodVersion=<8700.51>\nPreset Name=<auto_event_00> size=0\nEnd Preset<end>\n".into(),
                            modified: Some(0),
                        },
                        BackupFile {
                            name: "LATE NIGHT.orb86user".into(),
                            document: doc_8700("Late Night", 14),
                            modified: Some(0),
                        },
                    ],
                    confirmed: true,
                    expected_host: "127.0.0.1".into(),
                    expected_session: session.clone(),
                },
                r,
            )
        })
        .await
        .unwrap();
        assert_eq!(unit.received(231).len(), 1);
        // None of the 5700i/5500i requests: F0h setup, F4h passcodes, EDh delete, FBh.
        for kind in [237, 244, 251] {
            assert!(unit.received(kind).is_empty(), "{kind}");
        }
    }

    /// Delete, Backup, Restore, Security and Diagnostics as the 6300 client
    /// (the 8700HD's code, `.orb63`) and the 8700i client (the 5500i's code,
    /// `.orb87`) send them; Automation is refused for both.
    async fn preset_files_round_trip(
        m: FileModel,
        delete: u16,
        backup: [u16; 3],
        automation: (&str, u16),
        user: &str,
    ) {
        let unit = FakeFileUnit::start(m).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let session = app.state.read().await.session_id.clone().unwrap();
        let _ = command(&app, |r| {
            Command::PresetFile(
                PresetFile {
                    action: PresetFileAction::Delete {
                        name: "Late Night".into(),
                    },
                    expected_name: "Test01".into(),
                    confirmed: true,
                    expected_host: "127.0.0.1".into(),
                    expected_session: session.clone(),
                },
                r,
            )
        })
        .await;
        assert_eq!(
            unit.received(delete),
            [format!("Late Night{user}\n").into_bytes()]
        );
        let before = backup.map(|k| unit.received(k).len());
        let (reply, answer) = oneshot::channel();
        app.commands
            .send(Command::BackupFiles(reply))
            .await
            .unwrap();
        let _ = answer.await.unwrap();
        assert_eq!(
            backup.map(|k| unit.received(k).len()),
            before.map(|n| n + 1)
        );
        command(&app, |r| {
            Command::RestorePresetFiles(
                RestorePresetFiles {
                    files: vec![
                        BackupFile {
                            name: automation.0.into(),
                            document: format!(
                                "OptimodVersion=<{}>\nPreset Name=<auto_event_00> size=0\nEnd Preset<end>\n",
                                m.version
                            ),
                            modified: Some(0),
                        },
                        BackupFile {
                            name: format!("LATE NIGHT{user}"),
                            document: doc_with(m.version, "Late Night", 14),
                            modified: Some(0),
                        },
                    ],
                    confirmed: true,
                    expected_host: "127.0.0.1".into(),
                    expected_session: session.clone(),
                },
                r,
            )
        })
        .await
        .unwrap();
        assert_eq!(unit.received(231).len(), 1);
        assert!(
            unit.received(automation.1)
                .iter()
                .any(|d| d.starts_with(format!("{}\n", automation.0).as_bytes()))
        );
        // Diagnostics at sync; Security reads and replaces the passcode file.
        assert!(!unit.received(204).is_empty());
        let (levels, version, extension) = crate::security::flavour(m.model);
        let codes = [
            crate::security::Passcode {
                code: FILE_UNIT_CODE.into(),
                level: levels[0].into(),
            },
            crate::security::Passcode {
                code: "4321".into(),
                level: levels[5].into(),
            },
        ];
        let passcodes = crate::security::render(&codes, m.model).unwrap();
        assert!(passcodes.starts_with(&format!("OptimodVersion=<{version}>")));
        assert!(passcodes.contains(&format!("<Password.{extension}>")));
        let write = |kind, document: String| {
            let (app, session) = (app.clone(), session.clone());
            async move {
                command(&app, |r| {
                    Command::WriteUnitFile(
                        UnitFileWrite {
                            kind,
                            document: Zeroizing::new(document),
                            expected_host: "127.0.0.1".into(),
                            expected_session: session,
                        },
                        r,
                    )
                })
                .await
            }
        };
        write(230, passcodes).await.unwrap();
        assert_eq!(unit.received(230).len(), 1);
        // Automation as the client writes it (captured 29 October 2026):
        // its own version, extension and functions, uploaded with its name.
        let (functions, version, extension) = crate::automation::flavour(m.model);
        let event = crate::automation::Event {
            hour: 6,
            minute: 30,
            second: 0,
            day: 29,
            month: 10,
            year: 2026,
            weekdays: [true; 7],
            daily: true,
            action: functions[functions.len() - 1].into(),
        };
        let document = crate::automation::render(std::slice::from_ref(&event), m.model).unwrap();
        assert!(document.starts_with(&format!("OptimodVersion=<{version}>")));
        assert!(document.contains(&format!("auto_event_00.{extension}>")));
        let uploads = unit.received(automation.1).len();
        write(235, document.clone()).await.unwrap();
        let sent = unit.received(automation.1);
        assert_eq!(sent.len(), uploads + 1);
        let upload = sent.last().unwrap();
        assert!(upload.starts_with(format!("{}\n", automation.0).as_bytes()));
        let (reply, answer) = oneshot::channel();
        app.commands
            .send(Command::ReadUnitFile(235, reply))
            .await
            .unwrap();
        let read = answer.await.unwrap().unwrap().unwrap();
        assert_eq!(crate::automation::parse(&read).unwrap(), vec![event]);
    }

    #[tokio::test]
    async fn the_6300_deletes_backs_up_and_restores_as_its_client_does() {
        preset_files_round_trip(
            M6300,
            236,
            [239, 241, 242],
            ("Automation.orb63autom", 234),
            ".orb63user",
        )
        .await;
    }

    #[tokio::test]
    async fn the_8700i_deletes_backs_up_and_restores_as_its_client_does() {
        preset_files_round_trip(
            M8700I,
            237,
            [240, 242, 243],
            ("Automation.orb87autom", 235),
            ".orb87user",
        )
        .await;
    }

    /// The 5500's tool files, Delete, Backup and Restore use its firmware's
    /// opcodes (one below the 5500i's): never a 5500i request.
    #[tokio::test]
    async fn the_5500_tools_backup_and_restore_use_its_own_opcodes() {
        let unit = FakeFileUnit::start(M5500).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let read = |request| {
            let app = app.clone();
            async move {
                let (reply, answer) = oneshot::channel();
                app.commands
                    .send(Command::ReadUnitFile(request, reply))
                    .await
                    .unwrap();
                answer.await.unwrap()
            }
        };
        let passcodes = read(230).await.unwrap().unwrap();
        assert!(passcodes.contains("C:<ADMIN>String:<all access>"));
        assert!(read(235).await.is_ok());
        // Diagnostics at sync, passcodes F3h, automation F1h.
        assert!(!unit.received(204).is_empty());
        assert!(!unit.received(243).is_empty() && !unit.received(241).is_empty());
        let session = app.state.read().await.session_id.clone().unwrap();
        // No events left: the automation file is deleted with ECh.
        command(&app, |r| {
            Command::WriteUnitFile(
                UnitFileWrite {
                    kind: 235,
                    document: Zeroizing::new(String::new()),
                    expected_host: "127.0.0.1".into(),
                    expected_session: session.clone(),
                },
                r,
            )
        })
        .await
        .ok();
        assert_eq!(unit.received(236), [b"Automation.orb55autom\n".to_vec()]);
        // Delete Preset is ECh with the preset's file name, as the client sends it.
        let _ = command(&app, |r| {
            Command::PresetFile(
                PresetFile {
                    action: PresetFileAction::Delete {
                        name: "Late Night".into(),
                    },
                    expected_name: "Test01".into(),
                    confirmed: true,
                    expected_host: "127.0.0.1".into(),
                    expected_session: session.clone(),
                },
                r,
            )
        })
        .await;
        assert!(
            unit.received(236)
                .contains(&b"Late Night.orb55user\n".to_vec())
        );
        // Backup asks for setup (EFh), automation (F1h) and preset files
        // (F2h); the fake's preset answer holds a 6300 file, which stops it.
        let before = (
            unit.received(239).len(),
            unit.received(241).len(),
            unit.received(242).len(),
        );
        let (reply, answer) = oneshot::channel();
        app.commands
            .send(Command::BackupFiles(reply))
            .await
            .unwrap();
        let _ = answer.await.unwrap();
        assert_eq!(
            (
                unit.received(239).len(),
                unit.received(241).len(),
                unit.received(242).len()
            ),
            (before.0 + 1, before.1 + 1, before.2 + 1)
        );
        // Its Restore sends the automation file as EAh, presets as 231.
        let automation =
            "OptimodVersion=<5500.50>\nPreset Name=<auto_event_00> size=0\nEnd Preset<end>\n";
        command(&app, |r| {
            Command::RestorePresetFiles(
                RestorePresetFiles {
                    files: vec![
                        BackupFile {
                            name: "Automation.orb55autom".into(),
                            document: automation.into(),
                            modified: Some(0),
                        },
                        BackupFile {
                            name: "LATE NIGHT.orb55user".into(),
                            document: doc_with("5500.50", "Late Night", 14),
                            modified: Some(0),
                        },
                    ],
                    confirmed: true,
                    expected_host: "127.0.0.1".into(),
                    expected_session: session.clone(),
                },
                r,
            )
        })
        .await
        .unwrap();
        assert_eq!(unit.received(231).len(), 1);
        assert!(
            unit.received(234)
                .iter()
                .any(|d| d.starts_with(b"Automation.orb55autom\n"))
        );
        // None of the 5500i's requests: F0h setup, F4h passcodes, F2h automation, EDh delete, FAh.
        for kind in [244, 237, 251] {
            assert!(unit.received(kind).is_empty(), "{kind}");
        }
        assert!(unit.received(240).is_empty());
    }

    #[tokio::test]
    async fn the_6300_syncs_writes_and_recalls_with_its_own_opcodes() {
        file_sync_round_trip(M6300, 68).await;
    }

    #[tokio::test]
    async fn the_2300_syncs_without_session_numbers_or_file_names_and_uploads_factory_presets() {
        file_sync_round_trip(M2300, 10).await;
        let unit = FakeFileUnit::start(M2300).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let session = app.state.read().await.session_id.clone().unwrap();
        let recall = Recall {
            name: "MUSIC MEDIUM".into(),
            expected_name: "Test01".into(),
            confirmed: true,
            expected_host: "127.0.0.1".into(),
            expected_session: session,
        };
        // As the original client: the installer's factory file for the
        // unit's pre-emphasis, uploaded without a name line.
        command(&app, |r| Command::Recall(recall, r)).await.unwrap();
        assert_eq!(
            app.state.read().await.processing.as_ref().unwrap().name,
            "MUSIC MEDIUM"
        );
        let uploads = unit.received(229);
        assert_eq!(uploads.len(), 1);
        let sealed = &uploads[0][uploads[0].iter().position(|b| *b == b'\n').unwrap() + 1..];
        let text =
            String::from_utf8(orban_protocol::archive::decrypt(sealed, FILE_UNIT_CODE).unwrap())
                .unwrap();
        assert!(text.starts_with("OptimodVersion=<2300.10>\nPreset Name=<MUSIC MEDIUM>"));
        assert!(text.contains("Preset File Name= <MUSIC MEDIUM.orb23f75>"));
        assert_eq!(unit.terminal_sessions.load(Ordering::SeqCst), 0);
        assert!(app.state.read().await.connected);
    }

    /// Sync, a write, a user preset Recall and a pushed change on a
    /// file-sync unit, with that model's own opcodes only.
    async fn file_sync_round_trip(m: FileModel, factory: usize) {
        let unit = FakeFileUnit::start(m).await;
        let mut connect = unit.connect();
        if m.model == DeviceModel::Optimod5700i {
            // A port that takes no connection, as on the physical 5700i.
            connect.terminal_port = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .unwrap()
                .local_addr()
                .unwrap()
                .port();
        }
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(connect, r))
            .await
            .unwrap();
        let session = {
            let state = app.state.read().await;
            assert_eq!(state.sync_error, None);
            assert_eq!(state.processing.as_ref().unwrap().name, "Test01");
            assert_eq!(
                state
                    .presets
                    .iter()
                    .filter(|p| p.kind == PresetKind::Factory)
                    .count(),
                factory
            );
            assert!(
                state
                    .presets
                    .iter()
                    .any(|p| p.name == "Late Night" && p.kind == PresetKind::User)
            );
            state.session_id.clone().unwrap()
        };
        let change = Change {
            scope: Scope::Processing,
            name: "AGC DRIVE".into(),
            index: 21,
            expected: Field {
                index: 20,
                value: Value::Int(10),
            },
            requested: None,
            expected_host: "127.0.0.1".into(),
            expected_session: session.clone(),
            expected_preset: Some("Test01".into()),
        };
        command(&app, |r| Command::Change(change, r)).await.unwrap();
        let recall = Recall {
            name: "Late Night".into(),
            expected_name: "modif Test01".into(),
            confirmed: true,
            expected_host: "127.0.0.1".into(),
            expected_session: session.clone(),
        };
        command(&app, |r| Command::Recall(recall, r)).await.unwrap();
        assert_eq!(
            app.state.read().await.processing.as_ref().unwrap().name,
            "Late Night"
        );
        // Only this model's boundary request; the upload ends with its own
        // marker, which is never a boundary request.
        let (boundary, _) = m.wire.boundary;
        assert!(!unit.received(boundary).is_empty());
        if let Some(end) = m.wire.file_end {
            assert_ne!(end, boundary);
            assert_eq!(unit.received(end).len(), 1);
        }
        assert_eq!(unit.terminal_sessions.load(Ordering::SeqCst), 0);
        unit.push.send_modify(|n| *n += 1);
        sleep(Duration::from_millis(200)).await;
        command(&app, Command::Refresh).await.unwrap();
        assert_eq!(app.state.read().await.sync_error, None);
    }

    /// Issue #18: without its status port the 5700i syncs, writes, confirms
    /// and recalls over PC Remote alone, as its original client does.
    #[tokio::test]
    async fn a_5700i_without_its_status_port_runs_over_pc_remote() {
        file_sync_round_trip(M5700I, 46).await;
    }

    /// Without the status port a factory Recall uploads the factory file of
    /// the original client for the unit's pre-emphasis, as that client does.
    #[tokio::test]
    async fn a_5700i_without_its_status_port_recalls_a_factory_preset_by_upload() {
        let unit = FakeFileUnit::start(M5700I).await;
        let mut connect = unit.connect();
        connect.terminal_port = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(connect, r))
            .await
            .unwrap();
        let session = app.state.read().await.session_id.clone().unwrap();
        let recall = Recall {
            name: "GREGG".into(),
            expected_name: "Test01".into(),
            confirmed: true,
            expected_host: "127.0.0.1".into(),
            expected_session: session,
        };
        command(&app, |r| Command::Recall(recall, r)).await.unwrap();
        assert_eq!(
            app.state.read().await.processing.as_ref().unwrap().name,
            "GREGG"
        );
        let uploads = unit.received(229);
        assert_eq!(uploads.len(), 1);
        let sealed = &uploads[0][uploads[0].iter().position(|b| *b == b'\n').unwrap() + 1..];
        let plain = orban_protocol::archive::decrypt(sealed, FILE_UNIT_CODE).unwrap();
        let text = String::from_utf8(plain).unwrap();
        assert!(
            text.starts_with("OptimodVersion=<5700.50>\nPreset Name=<GREGG>"),
            "{text}"
        );
        assert!(
            text.contains("Preset File Name= <GREGG.orb57f75>"),
            "{text}"
        );
        assert!(!text.contains('\r'));
        assert_eq!(unit.terminal_sessions.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_8700hd_write_is_confirmed_once_the_unit_saves_its_on_air_file() {
        let unit = FakeFileUnit::start(M8700HD).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let session = app.state.read().await.session_id.clone().unwrap();
        let change = Change {
            scope: Scope::Processing,
            name: "AGC DRIVE".into(),
            index: 21,
            expected: Field {
                index: 20,
                value: Value::Int(10),
            },
            requested: None,
            expected_host: "127.0.0.1".into(),
            expected_session: session,
            expected_preset: Some("Test01".into()),
        };
        command(&app, |r| Command::Change(change, r)).await.unwrap();
        assert_eq!(unit.received(227), [b"AGC DRIVE;21;Int:11;1;".to_vec()]);
        let state = app.state.read().await;
        let processing = state.processing.as_ref().unwrap();
        assert_eq!(processing.name, "modif Test01");
        assert_eq!(processing.fields["AGC DRIVE"].value, Value::Int(11));
        assert_eq!(unit.terminal_sessions.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_8700hd_recall_sends_a_user_preset_back_and_lets_the_firmware_load_a_factory_one() {
        let unit = FakeFileUnit::start(M8700HD).await;
        let (app, _storage) = running_owner().await;
        command(&app, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let recall = |name: &str, on_air: &str, session: &str| Recall {
            name: name.into(),
            expected_name: on_air.into(),
            confirmed: true,
            expected_host: "127.0.0.1".into(),
            expected_session: session.into(),
        };
        let session = app.state.read().await.session_id.clone().unwrap();
        command(&app, |r| {
            Command::Recall(recall("Late Night", "Test01", &session), r)
        })
        .await
        .unwrap();
        // The original client sends the document with LF line ends, then FAh.
        let uploads = unit.received(229);
        assert_eq!(uploads.len(), 1);
        // An unnamed file: its name line is empty.
        assert_eq!(uploads[0][0], b'\n');
        let sealed = &uploads[0][1..];
        let plain = orban_protocol::archive::decrypt(sealed, FILE_UNIT_CODE).unwrap();
        assert_eq!(
            plain,
            doc_8700("Late Night", 14)
                .replace("\r\n", "\n")
                .into_bytes()
        );
        assert_eq!(unit.received(250).len(), 1);
        assert_eq!(
            app.state.read().await.processing.as_ref().unwrap().name,
            "Late Night"
        );
        assert_eq!(unit.terminal_sessions.load(Ordering::SeqCst), 0);

        command(&app, |r| {
            Command::Recall(recall("GREGG", "Late Night", &session), r)
        })
        .await
        .unwrap();
        assert_eq!(unit.terminal_sessions.load(Ordering::SeqCst), 1);
        assert_eq!(
            app.state.read().await.processing.as_ref().unwrap().name,
            "GREGG"
        );
        // The document the unit pushed after RP does not replace it later.
        sleep(Duration::from_millis(300)).await;
        assert_eq!(
            app.state.read().await.processing.as_ref().unwrap().name,
            "GREGG"
        );

        // A change made elsewhere: the pushed file and its FAh do not answer
        // the next request in place of the unit's real answer.
        unit.push.send_modify(|n| *n += 1);
        sleep(Duration::from_millis(200)).await;
        command(&app, Command::Refresh).await.unwrap();
        let state = app.state.read().await;
        assert!(state.connected);
        assert_eq!(state.sync_error, None);
        assert_eq!(state.processing.as_ref().unwrap().name, "GREGG");
        // The recalled user preset is still listed, now saved with LF.
        assert!(
            state
                .presets
                .iter()
                .any(|p| p.name == "Late Night" && p.kind == PresetKind::User)
        );
        assert!(!unit.late_night.lock().unwrap().contains('\r'));
    }

    fn test_app() -> (App, tempfile::TempDir) {
        let (commands, _) = mpsc::channel(1);
        let (meters, _) = watch::channel(json!({"live":false}));
        let storage = tempfile::tempdir().unwrap();
        let app = App {
            activity: Arc::default(),
            peer: sessions::Peer::default(),
            state: Arc::new(RwLock::new(Snapshot::default())),
            meters,
            commands,
            profile: shared_profile(reference_profile()),
            book: Arc::new(std::sync::Mutex::new(
                devices::Book::open(
                    storage.path().join("connections.json"),
                    Box::new(devices::FileVault::new(
                        storage.path().join("credentials.json"),
                    )),
                )
                .unwrap(),
            )),
        };
        (app, storage)
    }

    /// A PC Remote peer that answers the first 250 only after `late`, then
    /// answers each later 250 with a meter record carrying its number.
    async fn late_boundary_peer(
        late: Duration,
    ) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            while stream.read_u8().await.unwrap() != 0 {}
            stream
                .write_all(b"connect ok\n0\n5700i V 3.0.1.20\n123\n")
                .await
                .unwrap();
            let mut decoder = orban_protocol::framing::Decoder::default();
            let mut buffer = [0_u8; 4096];
            let mut boundary = 0_u8;
            loop {
                let count = match stream.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                for frame in decoder.feed(&buffer[..count]).unwrap() {
                    if !matches!(
                        orban_protocol::message::decode(&frame).unwrap(),
                        Message::Other { kind: 250, .. }
                    ) {
                        continue;
                    }
                    boundary += 1;
                    if boundary == 1 {
                        sleep(late).await;
                    } else {
                        let mut meter = vec![1];
                        meter.extend(b"112\n");
                        meter.extend([boundary; 112]);
                        stream
                            .write_all(&orban_protocol::message::encode(228, &meter).unwrap())
                            .await
                            .unwrap();
                    }
                    stream
                        .write_all(&orban_protocol::message::encode(251, b"").unwrap())
                        .await
                        .unwrap();
                }
            }
        });
        (address, server)
    }

    #[tokio::test]
    async fn a_late_boundary_never_confirms_the_next_one() {
        let (address, server) = late_boundary_peer(Duration::from_millis(400)).await;
        let session = Session::connect(address, "1234", Duration::from_millis(250))
            .await
            .unwrap();
        let (app, _storage) = test_app();
        let mut device = Device {
            session,
            boundaries: Boundaries::default(),
            lost: None,
            pushed: None,
            expected: None,
            archive_requested: false,
            archive: None,
            archive_retry: None,
            wanted_files: None,
            files: Vec::new(),
            user_presets: BTreeMap::new(),
            terminal: address,
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: None,
            profile: Some(reference_profile()),
            input_status: None,
            sync_gap: None,
            status_port_down: false,
        };
        assert_eq!(
            drain(&mut device, &app).await.unwrap_err(),
            "Receive timed out"
        );
        assert!(session_lost(&device, "Receive timed out").is_none());
        // The first boundary's 251 arrives during this drain. It must not end
        // it: only the second boundary's own 251, after its record, does.
        drain(&mut device, &app).await.unwrap();
        assert_eq!(app.meters.borrow()["values"][0], 2);
        assert_eq!(device.boundaries.unanswered_for(), Duration::ZERO);
        server.abort();
    }

    #[tokio::test]
    async fn only_a_lost_connection_or_a_long_silence_ends_the_session() {
        let (address, server) = late_boundary_peer(Duration::from_secs(60)).await;
        let session = Session::connect(address, "1234", Duration::from_millis(100))
            .await
            .unwrap();
        let mut device = Device {
            session,
            boundaries: Boundaries::default(),
            lost: None,
            pushed: None,
            expected: None,
            archive_requested: false,
            archive: None,
            archive_retry: None,
            wanted_files: None,
            files: Vec::new(),
            user_presets: BTreeMap::new(),
            terminal: address,
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: None,
            profile: None,
            input_status: None,
            sync_gap: None,
            status_port_down: false,
        };
        assert!(session_lost(&device, "Receive timed out").is_none());
        assert!(session_lost(&device, "Invalid meter count").is_none());
        assert!(session_lost(&device, "Too many device events before response boundary").is_none());
        assert!(session_lost(&device, "Device disconnected").is_some());
        assert!(session_lost(&device, "Connection reset by peer").is_some());
        device.boundaries.sent();
        device.boundaries.since =
            Some(tokio::time::Instant::now() - UNANSWERED_LIMIT + Duration::from_secs(1));
        assert!(session_lost(&device, "Receive timed out").is_none());
        device.boundaries.since = Some(tokio::time::Instant::now() - UNANSWERED_LIMIT);
        let lost = session_lost(&device, "Receive timed out").unwrap();
        assert!(lost.contains("has not answered for 20 seconds"), "{lost}");
        server.abort();
    }

    /// A 251 that never comes leaves one boundary outstanding for good. The
    /// later 251s each answer an older 250, so the count never reaches zero:
    /// the silence is measured from the last time it did.
    #[test]
    fn a_lost_boundary_answer_still_ends_the_session() {
        let mut boundaries = Boundaries::default();
        boundaries.sent();
        let first = boundaries.since;
        boundaries.sent();
        assert!(!boundaries.answered());
        boundaries.sent();
        assert!(!boundaries.answered());
        assert_eq!(boundaries.since, first);
        assert!(boundaries.answered());
        assert_eq!(boundaries.unanswered_for(), Duration::ZERO);
    }

    /// Virtual 5700i, 20 October 2026: with its send buffer full after a
    /// push, the unit answered a meter poll but dropped the 251 entirely.
    /// That boundary is forgotten once the unit has answered something else;
    /// a unit that answers nothing keeps it outstanding.
    #[tokio::test]
    async fn a_dropped_boundary_is_forgotten_once_the_unit_answers_again() {
        for drops_while_answering in [true, false] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                while stream.read_u8().await.unwrap() != 0 {}
                stream
                    .write_all(b"connect ok\n0\n5700i V 3.0.1.20\n123\n")
                    .await
                    .unwrap();
                let mut decoder = orban_protocol::framing::Decoder::default();
                let mut buffer = [0_u8; 4096];
                let mut boundary = 0;
                loop {
                    let count = match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => break,
                        Ok(count) => count,
                    };
                    for frame in decoder.feed(&buffer[..count]).unwrap() {
                        if !matches!(
                            orban_protocol::message::decode(&frame).unwrap(),
                            Message::Other { kind: 250, .. }
                        ) {
                            continue;
                        }
                        boundary += 1;
                        if boundary == 1 {
                            // The first boundary's 251 is dropped.
                            if drops_while_answering {
                                let mut meter = vec![1];
                                meter.extend(b"112\n");
                                meter.extend([1; 112]);
                                let frame = orban_protocol::message::encode(228, &meter).unwrap();
                                stream.write_all(&frame).await.unwrap();
                            }
                            continue;
                        }
                        let answer = orban_protocol::message::encode(251, b"").unwrap();
                        stream.write_all(&answer).await.unwrap();
                    }
                }
            });
            let session = Session::connect(address, "1234", Duration::from_millis(300))
                .await
                .unwrap();
            let (app, _dir) = test_app();
            let mut device = Device {
                session,
                boundaries: Boundaries::default(),
                lost: None,
                pushed: None,
                expected: None,
                archive_requested: false,
                archive: None,
                archive_retry: None,
                wanted_files: None,
                files: Vec::new(),
                user_presets: BTreeMap::new(),
                terminal: address,
                code: Zeroizing::new("1234".to_owned()),
                processing_baseline: None,
                profile: None,
                input_status: None,
                sync_gap: None,
                status_port_down: false,
            };
            assert_eq!(
                drain(&mut device, &app).await.unwrap_err(),
                "Receive timed out"
            );
            assert_eq!(
                device.boundaries.outstanding,
                usize::from(!drops_while_answering)
            );
            if drops_while_answering {
                drain(&mut device, &app).await.unwrap();
                assert_eq!(device.boundaries.outstanding, 0);
            }
            server.abort();
        }
    }

    /// The 5700i pushes opcode 247 about every 0.85 s. A unit that keeps
    /// pushing but stops answering 250 must not hold the owner past one
    /// deadline per drain.
    #[tokio::test]
    async fn steady_pushes_do_not_extend_a_drain() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            while stream.read_u8().await.unwrap() != 0 {}
            stream
                .write_all(b"connect ok\n0\n5700i V 3.0.1.20\n123\n")
                .await
                .unwrap();
            loop {
                if stream
                    .write_all(&orban_protocol::message::encode(247, b"").unwrap())
                    .await
                    .is_err()
                {
                    break;
                }
                sleep(Duration::from_millis(20)).await;
            }
        });
        let session = Session::connect(address, "1234", Duration::from_millis(300))
            .await
            .unwrap();
        let (app, _dir) = test_app();
        let mut device = Device {
            session,
            boundaries: Boundaries::default(),
            lost: None,
            pushed: None,
            expected: None,
            archive_requested: false,
            archive: None,
            archive_retry: None,
            wanted_files: None,
            files: Vec::new(),
            user_presets: BTreeMap::new(),
            terminal: address,
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: None,
            profile: None,
            input_status: None,
            sync_gap: None,
            status_port_down: false,
        };
        let started = std::time::Instant::now();
        assert_eq!(
            drain(&mut device, &app).await.unwrap_err(),
            "Receive timed out"
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "{:?}",
            started.elapsed()
        );
        server.abort();
    }

    /// Reference-unit behaviour (docs/verification/2026-10-20-recall-session.md):
    /// a terminal RP makes the 5700i push the new on-air document on the PC
    /// Remote session, announcing the full file but sending only the first 4096
    /// bytes. The session must keep answering the 50 ms ticks afterwards.
    #[tokio::test]
    async fn recall_survives_the_cut_document_the_5700i_pushes() {
        let pc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let pc_address = pc_listener.local_addr().unwrap();
        let pushed = Arc::new(tokio::sync::Notify::new());
        let push = pushed.clone();
        let pc_server = tokio::spawn(async move {
            let (stream, _) = pc_listener.accept().await.unwrap();
            let (mut reader, writer) = stream.into_split();
            let writer = Arc::new(tokio::sync::Mutex::new(writer));
            loop {
                if reader.read_u8().await.unwrap() == 0 {
                    break;
                }
            }
            writer
                .lock()
                .await
                .write_all(b"connect ok\n0\n5700i V 3.0.1.20\n123\n")
                .await
                .unwrap();
            let pusher = writer.clone();
            tokio::spawn(async move {
                push.notified().await;
                let mut payload = format!("{:8}{:8}0\n0\n\n7391\n", 229 - 204, 6).into_bytes();
                payload.extend((0..7417 - payload.len()).map(|i| (i * 131 % 251) as u8));
                let frame = orban_protocol::framing::encode(&payload).unwrap();
                pusher.lock().await.write_all(&frame[..4096]).await.unwrap();
            });
            let mut decoder = orban_protocol::framing::Decoder::default();
            let mut buffer = [0_u8; 4096];
            loop {
                let count = match reader.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                for frame in decoder.feed(&buffer[..count]).unwrap() {
                    let reply = match orban_protocol::message::decode(&frame).unwrap() {
                        Message::Other { kind: 219, .. } => {
                            let mut meter = vec![1];
                            meter.extend(b"112\n");
                            meter.extend([9; 112]);
                            orban_protocol::message::encode(228, &meter).unwrap()
                        }
                        Message::Other { kind: 250, .. } => {
                            orban_protocol::message::encode(251, b"").unwrap()
                        }
                        _ => continue,
                    };
                    writer.lock().await.write_all(&reply).await.unwrap();
                }
            }
        });

        let terminal_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let terminal_address = terminal_listener.local_addr().unwrap();
        let terminal_server = tokio::spawn(async move {
            let (mut stream, _) = terminal_listener.accept().await.unwrap();
            stream
                .write_all(b"Orban Optimod 5700i test\r\n")
                .await
                .unwrap();
            let mut command = Vec::new();
            let mut byte = [0_u8; 1];
            while !command.ends_with(b"??\r\n") {
                stream.read_exact(&mut byte).await.unwrap();
                command.push(byte[0]);
            }
            pushed.notify_one();
            sleep(Duration::from_millis(100)).await;
            stream
                .write_all(
                    b"ON AIR: TARGET\r\nOptimodVersion=<5700.51>\r\nPreset Name=<TARGET> size=1\r\nC:<AGC DRIVE>Int:10;D:20;\r\nEnd Preset<end>\r\n",
                )
                .await
                .unwrap();
        });

        let session = Session::connect(pc_address, "1234", Duration::from_secs(1))
            .await
            .unwrap();
        let (commands, _) = mpsc::channel(1);
        let (meters, _) = watch::channel(json!({"live":false}));
        let storage = tempfile::tempdir().unwrap();
        let current = processing("CURRENT", 20);
        let app = App {
            activity: Arc::default(),
            peer: sessions::Peer::default(),
            state: Arc::new(RwLock::new(Snapshot {
                connected: true,
                host: "127.0.0.1".into(),
                processing: Some(current.clone()),
                write_enabled: true,
                session_id: Some("test-session".into()),
                presets: vec![orban_protocol::presets::Preset {
                    name: "TARGET".into(),
                    kind: orban_protocol::presets::PresetKind::Factory,
                }],
                ..Snapshot::default()
            })),
            meters,
            commands,
            profile: shared_profile(reference_profile()),
            book: Arc::new(std::sync::Mutex::new(
                devices::Book::open(
                    storage.path().join("connections.json"),
                    Box::new(devices::FileVault::new(
                        storage.path().join("credentials.json"),
                    )),
                )
                .unwrap(),
            )),
        };
        let mut device = Device {
            session,
            boundaries: Boundaries::default(),
            lost: None,
            pushed: None,
            expected: None,
            archive_requested: false,
            archive: None,
            archive_retry: None,
            wanted_files: None,
            files: Vec::new(),
            user_presets: BTreeMap::new(),
            terminal: terminal_address,
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: Some(current),
            profile: Some(reference_profile()),
            input_status: None,
            sync_gap: None,
            status_port_down: false,
        };

        recall(
            &mut device,
            &app,
            Recall {
                name: "TARGET".into(),
                expected_name: "CURRENT".into(),
                confirmed: true,
                expected_host: "127.0.0.1".into(),
                expected_session: "test-session".into(),
            },
        )
        .await
        .unwrap();
        for tick in 0..5 {
            tokio::time::timeout(Duration::from_millis(500), async {
                device.session.send(219, b"").await?;
                drain(&mut device, &app).await
            })
            .await
            .unwrap_or_else(|_| panic!("tick {tick} got no response boundary"))
            .unwrap();
        }
        assert_eq!(app.meters.borrow()["live"], true);
        assert_eq!(
            app.state.read().await.processing.as_ref().unwrap().name,
            "TARGET"
        );
        terminal_server.abort();
        pc_server.abort();
    }

    #[test]
    fn less_more_stays_available_across_its_modified_factory_document() {
        let presets = vec![orban_protocol::presets::Preset {
            name: "GREGG OPEN".into(),
            kind: orban_protocol::presets::PresetKind::Factory,
        }];
        let factory = processing_with_less_more("GREGG OPEN", 10);
        assert!(refreshed_less_more_availability(
            &factory, None, false, &presets
        ));
        let modified = processing_with_less_more("modif GREGG OPEN", 12);
        assert!(refreshed_less_more_availability(
            &modified,
            Some("GREGG OPEN"),
            true,
            &presets
        ));
        assert!(!refreshed_less_more_availability(
            &modified,
            Some("GREGG OPEN"),
            false,
            &presets
        ));
    }
    #[test]
    fn coupled_processing_keeps_unique_hd_limiting_controls_editable() {
        for name in [
            "IBOC EQ GAIN",
            "IBOC EQ FREQ",
            "IBOC LIM DR",
            "HD DE ESS",
            "HD COUPLING",
        ] {
            assert!(!hd_control_tracks_fm(name), "{name}");
        }
        for name in [
            "HD PEQ LOW GAIN",
            "HD MB DRIVE",
            "HD B1 COMP THRSH",
            "HD B1 OUTPUT MIX",
        ] {
            assert!(hd_control_tracks_fm(name), "{name}");
        }
        assert!(!hd_control_tracks_fm("FINAL CLIP DRV"));
    }
    #[test]
    fn transient_receive_timeouts_do_not_immediately_end_a_session() {
        assert!(!is_definite_disconnect("Receive timed out"));
        assert!(is_definite_disconnect("Device disconnected"));
        assert!(is_definite_disconnect("Connection reset by peer"));
        assert_eq!(
            OperationError::uncertain("readback failed").message,
            "readback failed"
        );
    }
    #[test]
    fn embedded_window_can_disable_default_browser_launch() {
        use std::ffi::OsStr;
        assert!(should_open_default_browser(None));
        assert!(should_open_default_browser(Some(OsStr::new("0"))));
        assert!(!should_open_default_browser(Some(OsStr::new("1"))));
    }
    #[test]
    fn embedded_backend_finds_ui_next_to_its_resource_directory() {
        let executable = std::path::Path::new(
            "/Applications/Open Optimod Remote.app/Contents/Resources/backend/orban-web",
        );
        assert_eq!(
            bundled_asset_candidates(executable),
            [
                std::path::PathBuf::from(
                    "/Applications/Open Optimod Remote.app/Contents/Resources/backend/../ui",
                ),
                std::path::PathBuf::from(
                    "/Applications/Open Optimod Remote.app/Contents/Resources/backend/../Resources/ui",
                ),
            ]
        );
    }
    #[tokio::test]
    async fn foreign_websites_and_rebound_hosts_cannot_send_device_commands() {
        for (host, origin, expected) in [
            ("127.0.0.1:5701", Some(ORIGIN.as_str()), StatusCode::OK),
            (
                "127.0.0.1:5701",
                Some("https://foreign.invalid"),
                StatusCode::FORBIDDEN,
            ),
            (
                "rebound.invalid:5701",
                Some(ORIGIN.as_str()),
                StatusCode::FORBIDDEN,
            ),
            ("127.0.0.1:5701", None, StatusCode::FORBIDDEN),
        ] {
            let router = Router::new()
                .route("/test", post(|| async { "ok" }))
                .layer(middleware::from_fn(local_origin));
            let mut req = Request::builder()
                .uri("/test")
                .method("POST")
                .header("host", host);
            if let Some(origin) = origin {
                req = req.header("origin", origin);
            }
            let response = router
                .oneshot(req.body(axum::body::Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
    }
    #[test]
    fn only_an_unprivileged_port_moves_the_service() {
        assert_eq!(service_port(None), 5701);
        assert_eq!(service_port(Some("15701")), 15701);
        assert_eq!(service_port(Some("80")), 5701);
        assert_eq!(service_port(Some("x")), 5701);
    }
    /// `GET /api/presets/current-file` and `/api/backup` read from the
    /// processor; an image tag on another site must not trigger them.
    #[tokio::test]
    async fn foreign_websites_cannot_trigger_reads_without_an_origin() {
        for (site, expected) in [
            (None, StatusCode::OK),
            (Some("same-origin"), StatusCode::OK),
            (Some("none"), StatusCode::OK),
            (Some("cross-site"), StatusCode::FORBIDDEN),
            (Some("same-site"), StatusCode::FORBIDDEN),
        ] {
            let router = Router::new()
                .route("/test", get(|| async { "ok" }))
                .layer(middleware::from_fn(local_origin));
            let mut req = Request::builder()
                .uri("/test")
                .header("host", "127.0.0.1:5701");
            if let Some(site) = site {
                req = req.header("sec-fetch-site", site);
            }
            let response = router
                .oneshot(req.body(axum::body::Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected, "{site:?}");
        }
    }
    /// Each window reads the one meter stream; opening one adds a reader and
    /// sends nothing to the processor.
    #[tokio::test]
    async fn windows_read_the_one_session_without_traffic() {
        let (tx, mut rx) = mpsc::channel(8);
        let (meters, _) = watch::channel(json!({"live":false}));
        let storage = tempfile::tempdir().unwrap();
        let app = App {
            activity: Arc::default(),
            peer: sessions::Peer::default(),
            state: Arc::new(RwLock::new(Snapshot::default())),
            meters,
            commands: tx,
            profile: shared_profile(reference_profile()),
            book: Arc::new(std::sync::Mutex::new(
                devices::Book::open(
                    storage.path().join("connections.json"),
                    Box::new(devices::FileVault::new(
                        storage.path().join("credentials.json"),
                    )),
                )
                .unwrap(),
            )),
        };
        let meters_window = app.meters.subscribe();
        let processing_window = app.meters.subscribe();
        let Json(read) = diagnostics_handler(State(app.clone())).await;
        assert_eq!(read["meter_readers"], 2);
        for counter in [
            "session_opens",
            "meter_polls",
            "meter_frames",
            "device_writes",
        ] {
            assert_eq!(read[counter], 0, "{counter}");
        }
        drop((meters_window, processing_window));
        let Json(read) = diagnostics_handler(State(app)).await;
        assert_eq!(read["meter_readers"], 0);
        assert!(
            rx.try_recv().is_err(),
            "reading diagnostics queued a command"
        );
    }
    #[tokio::test]
    async fn no_device_means_no_write_and_no_optimistic_state() {
        let (tx, rx) = mpsc::channel(8);
        let (meters, _) = watch::channel(json!({"live":false}));
        let app = App {
            activity: Arc::default(),
            peer: sessions::Peer::default(),
            state: Arc::new(RwLock::new(Snapshot::default())),
            meters,
            commands: tx,
            profile: shared_profile(reference_profile()),
            book: Arc::new(std::sync::Mutex::new(
                devices::Book::open(
                    tempfile::tempdir().unwrap().path().join("test.json"),
                    Box::new(devices::FileVault::new(
                        tempfile::tempdir().unwrap().path().join("credentials.json"),
                    )),
                )
                .unwrap(),
            )),
        };
        let worker = tokio::spawn(owner(app.clone(), rx));
        let result = dispatch(app.clone(), |reply| {
            Command::Change(
                Change {
                    scope: Scope::System,
                    name: "CONTRAST".into(),
                    index: 2,
                    requested: None,
                    expected_host: "192.0.2.1".into(),
                    expected_session: "missing-session".into(),
                    expected_preset: None,
                    expected: Field {
                        index: 3,
                        value: Value::Int(3),
                    },
                },
                reply,
            )
        })
        .await;
        assert!(result.is_err());
        assert!(!app.state.read().await.connected);
        assert!(app.state.read().await.system.is_none());
        worker.abort();
    }

    /// One write against simulated PC Remote and terminal servers.
    struct WriteCase {
        banner: &'static str,
        terminal_banner: &'static str,
        family: &'static str,
        name: &'static str,
        before: Field,
        target: u32,
        wire: &'static str,
        /// What the terminal returns if the app reads the document back.
        readback: Field,
    }

    struct WriteOutcome {
        result: Result<(), OperationError>,
        state: Snapshot,
        changes: usize,
        readbacks: usize,
        evidence: Evidence,
    }

    fn wire_value(value: &Value) -> String {
        match value {
            Value::Int(v) => format!("Int:{v}"),
            Value::Cent(v) => format!("Cent:{v}"),
            Value::Choice(v) => format!("String:<{v}>"),
            Value::Text(v) => format!("UserString:<{v}>"),
        }
    }

    async fn simulated_write(case: WriteCase) -> WriteOutcome {
        let pc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let pc_address = pc_listener.local_addr().unwrap();
        let changes = Arc::new(AtomicUsize::new(0));
        let server_changes = changes.clone();
        let banner = case.banner;
        let wire = case.wire;
        let pc_server = tokio::spawn(async move {
            let (mut stream, _) = pc_listener.accept().await.unwrap();
            loop {
                if stream.read_u8().await.unwrap() == 0 {
                    break;
                }
            }
            stream
                .write_all(format!("connect ok\n0\n{banner}\n123\n").as_bytes())
                .await
                .unwrap();
            let mut decoder = orban_protocol::framing::Decoder::default();
            let mut buffer = [0_u8; 4096];
            loop {
                let count = match stream.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                for frame in decoder.feed(&buffer[..count]).unwrap() {
                    match orban_protocol::message::decode(&frame).unwrap() {
                        Message::Other { kind: 227, data } => {
                            assert_eq!(data, wire.as_bytes());
                            server_changes.fetch_add(1, Ordering::SeqCst);
                        }
                        Message::Other { kind: 219, .. } => {
                            stream
                                .write_all(&orban_protocol::message::encode(220, b"").unwrap())
                                .await
                                .unwrap();
                        }
                        Message::Other { kind: 250, .. } => {
                            stream
                                .write_all(&orban_protocol::message::encode(251, b"").unwrap())
                                .await
                                .unwrap();
                        }
                        _ => {}
                    }
                }
            }
        });
        let terminal_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let terminal_address = terminal_listener.local_addr().unwrap();
        let readbacks = Arc::new(AtomicUsize::new(0));
        let server_readbacks = readbacks.clone();
        let document = format!(
            "OptimodVersion=<{}>\r\nPreset Name=<modif TEST> size=1\r\nC:<{}>{};D:{};\r\nEnd Preset<end>\r\n",
            case.family,
            case.name,
            wire_value(&case.readback.value),
            case.readback.index
        );
        let terminal_banner = case.terminal_banner;
        let terminal_server = tokio::spawn(async move {
            let (mut stream, _) = terminal_listener.accept().await.unwrap();
            server_readbacks.fetch_add(1, Ordering::SeqCst);
            stream
                .write_all(format!("{terminal_banner}\r\n").as_bytes())
                .await
                .unwrap();
            let mut command = Vec::new();
            let mut byte = [0_u8; 1];
            while !command.ends_with(b"??\r\n") {
                stream.read_exact(&mut byte).await.unwrap();
                command.push(byte[0]);
            }
            stream.write_all(document.as_bytes()).await.unwrap();
        });
        let session = Session::connect(pc_address, "1234", Duration::from_secs(1))
            .await
            .unwrap();
        let evidence = session.info.evidence;
        let (commands, _) = mpsc::channel(1);
        let (meters, _) = watch::channel(json!({"live":false}));
        let storage = tempfile::tempdir().unwrap();
        let before = Document {
            name: "TEST".into(),
            fields: [(case.name.into(), case.before.clone())]
                .into_iter()
                .collect(),
            raw: String::new(),
        };
        let profile = Arc::new(
            Profile::for_adapter(session.info.adapter_id)
                .unwrap()
                .unwrap(),
        );
        let app = App {
            activity: Arc::default(),
            peer: sessions::Peer::default(),
            state: Arc::new(RwLock::new(Snapshot {
                connected: true,
                host: "127.0.0.1".into(),
                processing: Some(before.clone()),
                write_enabled: true,
                session_id: Some("test-session".into()),
                ..Snapshot::default()
            })),
            meters,
            commands,
            profile: shared_profile(profile.clone()),
            book: Arc::new(std::sync::Mutex::new(
                devices::Book::open(
                    storage.path().join("connections.json"),
                    Box::new(devices::FileVault::new(
                        storage.path().join("credentials.json"),
                    )),
                )
                .unwrap(),
            )),
        };
        let mut device = Device {
            session,
            boundaries: Boundaries::default(),
            lost: None,
            pushed: None,
            expected: None,
            archive_requested: false,
            archive: None,
            archive_retry: None,
            wanted_files: None,
            files: Vec::new(),
            user_presets: BTreeMap::new(),
            terminal: terminal_address,
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: Some(before),
            profile: Some(profile),
            input_status: None,
            sync_gap: None,
            status_port_down: false,
        };
        let result = change(
            &mut device,
            &app,
            Change {
                scope: Scope::Processing,
                name: case.name.into(),
                index: case.target,
                expected: case.before,
                requested: None,
                expected_host: "127.0.0.1".into(),
                expected_session: "test-session".into(),
                expected_preset: Some("TEST".into()),
            },
        )
        .await;
        let state = app.state.read().await.clone();
        pc_server.abort();
        terminal_server.abort();
        WriteOutcome {
            result,
            state,
            changes: changes.load(Ordering::SeqCst),
            readbacks: readbacks.load(Ordering::SeqCst),
            evidence,
        }
    }

    fn int(index: u32, value: i32) -> Field {
        Field {
            index,
            value: Value::Int(value),
        }
    }

    /// A two-band 5700i field exists only in the statically derived supplement.
    fn static_5700i(readback: Field) -> WriteCase {
        WriteCase {
            banner: "5700i V 3.0.1.20",
            terminal_banner: "Orban Optimod 5700i test",
            family: "5700.51",
            name: "2B DRIVE",
            before: int(10, 0),
            target: 11,
            wire: "2B DRIVE;11;Int:1;1;",
            readback,
        }
    }

    #[tokio::test]
    async fn static_profile_writes_are_confirmed_by_a_full_readback() {
        let outcome = simulated_write(static_5700i(int(11, 1))).await;
        assert!(outcome.result.is_ok(), "{:?}", outcome.result.err());
        assert_eq!((outcome.changes, outcome.readbacks), (1, 1));
        let field = &outcome.state.processing.unwrap().fields["2B DRIVE"];
        assert_eq!(field, &int(11, 1));
    }

    #[tokio::test]
    async fn static_profile_mismatch_shows_the_processor_value_and_never_retries() {
        let outcome = simulated_write(static_5700i(int(10, 0))).await;
        assert_eq!(
            outcome.result.unwrap_err().message,
            "The processor did not confirm the change"
        );
        assert_eq!((outcome.changes, outcome.readbacks), (1, 1));
        let field = &outcome.state.processing.unwrap().fields["2B DRIVE"];
        assert_eq!(field, &int(10, 0));
    }

    #[tokio::test]
    async fn verified_5700i_fields_keep_the_fast_path_and_static_5700i_fields_are_read_back() {
        let verified = simulated_write(WriteCase {
            banner: "5700i V 3.0.1.20",
            terminal_banner: "Orban Optimod 5700i test",
            family: "5700.51",
            name: "AGC DRIVE",
            before: int(20, 10),
            target: 21,
            wire: "AGC DRIVE;21;Int:11;1;",
            readback: int(21, 11),
        })
        .await;
        assert_eq!(verified.evidence, Evidence::Hardware);
        assert!(verified.result.is_ok(), "{:?}", verified.result.err());
        assert_eq!((verified.changes, verified.readbacks), (1, 0));

        // A two-band field exists only in the statically derived supplement.
        let supplement = simulated_write(WriteCase {
            banner: "5700i V 3.0.1.20",
            terminal_banner: "Orban Optimod 5700i test",
            family: "5700.51",
            name: "2B DRIVE",
            before: int(10, 0),
            target: 11,
            wire: "2B DRIVE;11;Int:1;1;",
            readback: int(11, 1),
        })
        .await;
        assert!(supplement.result.is_ok(), "{:?}", supplement.result.err());
        assert_eq!((supplement.changes, supplement.readbacks), (1, 1));
    }

    struct PresetFileCase {
        banner: &'static str,
        terminal_banner: &'static str,
        on_air: &'static str,
        users: &'static [&'static str],
        /// The processor ignores SP and DP, as if the command had no effect.
        ignore: bool,
        action: PresetFileAction,
    }
    struct PresetFileOutcome {
        result: Result<(), String>,
        commands: Vec<String>,
    }

    /// A 5500-style terminal that keeps a user preset list, answers SP and DP
    /// like the firmware and always ends a reply with an AP document.
    async fn simulated_preset_file(case: PresetFileCase) -> PresetFileOutcome {
        let pc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let pc_address = pc_listener.local_addr().unwrap();
        let banner = case.banner;
        let users = Arc::new(std::sync::Mutex::new(
            case.users.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
        ));
        let on_air = Arc::new(std::sync::Mutex::new(case.on_air.to_string()));
        let (pc_users, pc_on_air) = (users.clone(), on_air.clone());
        tokio::spawn(async move {
            let (mut stream, _) = pc_listener.accept().await.unwrap();
            loop {
                if stream.read_u8().await.unwrap() == 0 {
                    break;
                }
            }
            stream
                .write_all(format!("connect ok\n0\n{banner}\n123\n").as_bytes())
                .await
                .unwrap();
            let mut decoder = orban_protocol::framing::Decoder::default();
            let mut buffer = [0_u8; 4096];
            loop {
                let count = match stream.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                for frame in decoder.feed(&buffer[..count]).unwrap() {
                    // The 5500 answers in the 8700HD generation: F9h by FAh,
                    // F2h by one E7h file per user preset.
                    let mut reply = Vec::new();
                    match orban_protocol::message::decode(&frame).unwrap() {
                        Message::Other { kind: 219, .. } => reply.push((220, Vec::new())),
                        Message::Other { kind: 249, .. } => reply.push((250, Vec::new())),
                        Message::Other { kind: 238, .. } => {
                            let name = pc_on_air.lock().unwrap().clone();
                            let text = format!(
                                "OptimodVersion=<8300.10>\nPreset Name=<{name}> size=1\nC:<AGC DRIVE>Int:10;D:20;\nEnd Preset<end>\n"
                            );
                            reply.push((229, file_body("OnAir.orb55onair", &text)));
                        }
                        Message::Other { kind: 239, .. } => reply.push((
                            232,
                            file_body(
                                "Setup.orb55setup",
                                "OptimodVersion=<8300.10>\nPreset Name=<SYSTEM PRESET> size=1\nC:<CONTRAST>Int:2;D:2;\nEnd Preset<end>\n",
                            ),
                        )),
                        Message::Other { kind: 242, .. } => {
                            for user in pc_users.lock().unwrap().iter() {
                                let text = format!(
                                    "OptimodVersion=<8300.10>\r\nPreset Name=<{user}> size=1\r\nC:<AGC DRIVE>Int:10;D:20;\r\nEnd Preset<end>\r\n"
                                );
                                reply.push((231, file_body(&format!("{user}.orb55user"), &text)));
                            }
                        }
                        _ => {}
                    };
                    for (kind, body) in reply {
                        let _ = stream
                            .write_all(&orban_protocol::message::encode(kind, &body).unwrap())
                            .await;
                    }
                }
            }
        });

        let terminal_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let terminal_address = terminal_listener.local_addr().unwrap();
        let commands = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let (server_commands, server_users, server_on_air) =
            (commands.clone(), users.clone(), on_air.clone());
        let terminal_banner = case.terminal_banner;
        let ignore = case.ignore;
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = terminal_listener.accept().await else {
                    break;
                };
                stream
                    .write_all(format!("{terminal_banner}\r\n").as_bytes())
                    .await
                    .unwrap();
                let mut request = Vec::new();
                let mut byte = [0_u8; 1];
                while !request.ends_with(b"??\r\n") {
                    if stream.read_exact(&mut byte).await.is_err() {
                        break;
                    }
                    request.push(byte[0]);
                }
                let request = String::from_utf8(request).unwrap();
                let first = request.lines().next().unwrap_or_default().to_owned();
                let mut reply = String::new();
                let name = first
                    .get(3..)
                    .and_then(|rest| rest.split_once('['))
                    .map(|(name, _)| name.to_owned());
                if first.starts_with("SP ") || first.starts_with("DP ") {
                    server_commands.lock().unwrap().push(first.clone());
                    let name = name.unwrap();
                    let mut users = server_users.lock().unwrap();
                    if first.starts_with("SP ") {
                        if users.contains(&name) {
                            reply.push_str(&format!(
                                " SP: {name} already exists. Please choose another name.\r\n"
                            ));
                        } else if !ignore {
                            users.push(name.clone());
                            *server_on_air.lock().unwrap() = name.clone();
                            reply.push_str(&format!(" SP: {name}\r\n"));
                        }
                    } else if !users.contains(&name) {
                        reply.push_str(" DP: preset does not exist\r\n");
                    } else if !ignore {
                        users.retain(|u| *u != name);
                        reply.push_str(&format!(" DP: {name}\r\n"));
                    }
                } else if first.starts_with("LP ") {
                    reply.push_str("FACTORY ONE factory\r\n");
                    for user in server_users.lock().unwrap().iter() {
                        reply.push_str(&format!("{user} user\r\n"));
                    }
                }
                let on_air = server_on_air.lock().unwrap().clone();
                reply.push_str(&format!(
                    "OptimodVersion=<8300.10>\r\nPreset Name=<{on_air}> size=1\r\nC:<AGC DRIVE>Int:10;D:20;\r\nEnd Preset<end>\r\n"
                ));
                let _ = stream.write_all(reply.as_bytes()).await;
                let mut tail = [0_u8; 64];
                let _ = stream.read(&mut tail).await;
            }
        });

        let session = Session::connect(pc_address, "1234", Duration::from_secs(1))
            .await
            .unwrap();
        let (command_tx, _) = mpsc::channel(1);
        let (meters, _) = watch::channel(json!({"live":false}));
        let storage = tempfile::tempdir().unwrap();
        let current = processing(case.on_air, 20);
        let mut presets = vec![orban_protocol::presets::Preset {
            name: "FACTORY ONE".into(),
            kind: orban_protocol::presets::PresetKind::Factory,
        }];
        presets.extend(case.users.iter().map(|u| orban_protocol::presets::Preset {
            name: (*u).into(),
            kind: orban_protocol::presets::PresetKind::User,
        }));
        let app = App {
            activity: Arc::default(),
            peer: sessions::Peer::default(),
            state: Arc::new(RwLock::new(Snapshot {
                connected: true,
                host: "127.0.0.1".into(),
                processing: Some(current.clone()),
                write_enabled: true,
                session_id: Some("test-session".into()),
                presets,
                ..Snapshot::default()
            })),
            meters,
            commands: command_tx,
            profile: shared_profile(reference_profile()),
            book: Arc::new(std::sync::Mutex::new(
                devices::Book::open(
                    storage.path().join("connections.json"),
                    Box::new(devices::FileVault::new(
                        storage.path().join("credentials.json"),
                    )),
                )
                .unwrap(),
            )),
        };
        let mut device = Device {
            session,
            boundaries: Boundaries::default(),
            lost: None,
            pushed: None,
            expected: None,
            archive_requested: false,
            archive: None,
            archive_retry: None,
            wanted_files: None,
            files: Vec::new(),
            user_presets: BTreeMap::new(),
            terminal: terminal_address,
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: Some(current),
            profile: Some(reference_profile()),
            input_status: None,
            sync_gap: None,
            status_port_down: false,
        };
        let result = manage_preset(
            &mut device,
            &app,
            PresetFile {
                action: case.action,
                expected_name: case.on_air.into(),
                confirmed: true,
                expected_host: "127.0.0.1".into(),
                expected_session: "test-session".into(),
            },
        )
        .await
        .map_err(|e| e.message);
        let commands = commands.lock().unwrap().clone();
        PresetFileOutcome { result, commands }
    }

    const P5500: (&str, &str) = ("5500 V 1.2.8.24", "Orban Optimod 5500");

    #[tokio::test]
    async fn preset_changes_are_refused_before_any_command_when_unsafe() {
        for (users, on_air, action, expected) in [
            (
                &["MORNING"][..],
                "MORNING",
                PresetFileAction::Delete {
                    name: "MORNING".into(),
                },
                "on-air",
            ),
            (
                &["MORNING"][..],
                "FACTORY ONE",
                PresetFileAction::Delete {
                    name: "FACTORY ONE".into(),
                },
                "Only user presets",
            ),
            (
                &["MORNING"][..],
                "FACTORY ONE",
                PresetFileAction::Save {
                    name: "MORNING".into(),
                },
                "already exists",
            ),
            (
                &["MORNING"][..],
                "FACTORY ONE",
                PresetFileAction::Save {
                    name: "FACTORY ONE".into(),
                },
                "factory preset",
            ),
            (
                &["MORNING"][..],
                "FACTORY ONE",
                PresetFileAction::Save {
                    name: "NINETEEN CHARACTERS".into(),
                },
                "at most 18",
            ),
            (
                &["MORNING"][..],
                "FACTORY ONE",
                PresetFileAction::Save {
                    name: "modif X".into(),
                },
                "modif",
            ),
            (
                &["MORNING"][..],
                "modif MORNING",
                PresetFileAction::Rename {
                    name: "MORNING".into(),
                    new_name: "NEW".into(),
                },
                "without changes",
            ),
        ] {
            let outcome = simulated_preset_file(PresetFileCase {
                banner: P5500.0,
                terminal_banner: P5500.1,
                on_air,
                users,
                ignore: false,
                action,
            })
            .await;
            let error = outcome.result.expect_err(expected);
            assert!(error.contains(expected), "{error}");
            assert!(
                outcome.commands.is_empty(),
                "{expected}: {:?}",
                outcome.commands
            );
        }
    }

    #[tokio::test]
    async fn models_without_a_documented_save_command_refuse_preset_changes() {
        // Firmware without an exact profile has no file messages.
        for (banner, terminal_banner) in [("6300 V 4.1.1.18", "Orban Optimod 6300 V 4.1.1.18")] {
            let outcome = simulated_preset_file(PresetFileCase {
                banner,
                terminal_banner,
                on_air: "FACTORY ONE",
                users: &["MORNING"],
                ignore: false,
                action: PresetFileAction::Delete {
                    name: "MORNING".into(),
                },
            })
            .await;
            let error = outcome.result.unwrap_err();
            assert!(error.contains("not available"), "{error}");
            assert!(outcome.commands.is_empty());
        }
    }

    fn system_document(fields: &[(&str, u32, Value)]) -> Document {
        Document {
            name: "SYSTEM".into(),
            fields: fields
                .iter()
                .map(|(name, index, value)| {
                    (
                        (*name).to_owned(),
                        Field {
                            index: *index,
                            value: value.clone(),
                        },
                    )
                })
                .collect(),
            raw: String::new(),
        }
    }

    #[test]
    fn setup_restore_plan_writes_only_valid_changes_and_explains_the_rest() {
        let profile = reference_profile();
        let current = system_document(&[
            ("CONTRAST", 3, Value::Int(3)),
            ("LANGUAGE", 0, Value::Choice("English".into())),
            ("NETWORK PORT", 6201, Value::Int(6201)),
            ("SET HOUR", 10, Value::Int(10)),
        ]);
        let imported = system_document(&[
            ("CONTRAST", 2, Value::Int(2)),
            ("LANGUAGE", 0, Value::Choice("English".into())),
            ("NETWORK PORT", 6202, Value::Int(6202)),
            ("SET HOUR", 4, Value::Int(4)),
            ("NOT ON THIS UNIT", 0, Value::Int(0)),
        ]);
        let (changes, skipped) = setup_restore_plan(&profile, &current, &imported);
        assert_eq!(
            changes,
            [(
                "CONTRAST".to_owned(),
                Field {
                    index: 2,
                    value: Value::Int(2)
                }
            )]
        );
        let skipped: Vec<_> = skipped.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(skipped, ["NETWORK PORT", "NOT ON THIS UNIT", "SET HOUR"]);

        // An index outside the profile is never written.
        let invalid = system_document(&[("CONTRAST", 9, Value::Int(9))]);
        let (changes, skipped) = setup_restore_plan(&profile, &current, &invalid);
        assert!(changes.is_empty());
        assert_eq!(skipped[0].1, "not writable with this processor's profile");
    }

    /// A 5700i that records opcode-227 writes and answers terminal reads with
    /// the given documents in order, repeating the last one.
    async fn scripted_5700i(
        documents: Vec<String>,
    ) -> (App, Device, Arc<std::sync::Mutex<Vec<String>>>) {
        let pc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let pc_address = pc_listener.local_addr().unwrap();
        let writes = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let server_writes = writes.clone();
        tokio::spawn(async move {
            let (mut stream, _) = pc_listener.accept().await.unwrap();
            loop {
                if stream.read_u8().await.unwrap() == 0 {
                    break;
                }
            }
            stream
                .write_all(b"connect ok\n0\n5700i V 3.0.1.20\n123\n")
                .await
                .unwrap();
            let mut decoder = orban_protocol::framing::Decoder::default();
            let mut buffer = [0_u8; 4096];
            loop {
                let count = match stream.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                for frame in decoder.feed(&buffer[..count]).unwrap() {
                    match orban_protocol::message::decode(&frame).unwrap() {
                        Message::Other { kind: 227, data } => {
                            server_writes
                                .lock()
                                .unwrap()
                                .push(String::from_utf8(data).unwrap());
                        }
                        Message::Other { kind: 219, .. } => {
                            let _ = stream
                                .write_all(&orban_protocol::message::encode(220, b"").unwrap())
                                .await;
                        }
                        Message::Other { kind: 250, .. } => {
                            let _ = stream
                                .write_all(&orban_protocol::message::encode(251, b"").unwrap())
                                .await;
                        }
                        _ => {}
                    }
                }
            }
        });
        let terminal_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let terminal_address = terminal_listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut next = 0;
            loop {
                let Ok((mut stream, _)) = terminal_listener.accept().await else {
                    break;
                };
                stream
                    .write_all(b"Orban Optimod 5700i test\r\n")
                    .await
                    .unwrap();
                let mut request = Vec::new();
                let mut byte = [0_u8; 1];
                while !request.ends_with(b"??\r\n") {
                    if stream.read_exact(&mut byte).await.is_err() {
                        break;
                    }
                    request.push(byte[0]);
                }
                let reply = &documents[next.min(documents.len() - 1)];
                next += 1;
                let _ = stream.write_all(reply.as_bytes()).await;
                let mut tail = [0_u8; 64];
                let _ = stream.read(&mut tail).await;
            }
        });
        let session = Session::connect(pc_address, "1234", Duration::from_secs(1))
            .await
            .unwrap();
        let (command_tx, _) = mpsc::channel(1);
        let (meters, _) = watch::channel(json!({"live":false}));
        let storage = tempfile::tempdir().unwrap();
        let app = App {
            activity: Arc::default(),
            peer: sessions::Peer::default(),
            state: Arc::new(RwLock::new(Snapshot {
                connected: true,
                host: "127.0.0.1".into(),
                write_enabled: true,
                session_id: Some("test-session".into()),
                ..Snapshot::default()
            })),
            meters,
            commands: command_tx,
            profile: shared_profile(reference_profile()),
            book: Arc::new(std::sync::Mutex::new(
                devices::Book::open(
                    storage.path().join("connections.json"),
                    Box::new(devices::FileVault::new(
                        storage.path().join("credentials.json"),
                    )),
                )
                .unwrap(),
            )),
        };
        let device = Device {
            session,
            boundaries: Boundaries::default(),
            lost: None,
            pushed: None,
            expected: None,
            archive_requested: false,
            archive: None,
            archive_retry: None,
            wanted_files: None,
            files: Vec::new(),
            user_presets: BTreeMap::new(),
            terminal: terminal_address,
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: None,
            profile: Some(reference_profile()),
            input_status: None,
            sync_gap: None,
            status_port_down: false,
        };
        (app, device, writes)
    }

    /// The fake unit's on-air document: long enough that its push is cut.
    /// A preset named `… 5B` or `… 2B` carries that processing structure's
    /// field, as the 5700i's documents do (`B1 ATTACK` or `2B DRIVE`).
    fn on_air_text(name: &str) -> String {
        let mut fields = vec![("AGC DRIVE".to_owned(), "Int:10;D:20;".to_owned())];
        if name.ends_with(" 5B") {
            fields.push(("B1 ATTACK".to_owned(), "Int:30;D:30;".to_owned()));
        } else if name.ends_with(" 2B") {
            fields.push(("2B DRIVE".to_owned(), "Int:12;D:12;".to_owned()));
        }
        fields.extend((0..200).map(|i| (format!("FIELD {i:03}"), format!("Int:{i};D:{i};"))));
        let fields: Vec<(&str, &str)> = fields
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        document_text(name, &fields)
    }

    fn document_text(name: &str, fields: &[(&str, &str)]) -> String {
        let mut text = format!(
            "OptimodVersion=<5700.51>\r\nPreset Name=<{name}> size={}\r\n",
            fields.len()
        );
        for (field, value) in fields {
            text.push_str(&format!("C:<{field}>{value}\r\n"));
        }
        text + "End Preset<end>\r\n"
    }

    fn input_system(input: &str, actual: &str, fallback: &str) -> String {
        let choice = |value: &str| {
            let index = match value {
                "Digital" | "Yes" => 1,
                _ => 0,
            };
            format!("String:<{value}>;D:{index};")
        };
        document_text(
            "SYSTEM PRESET",
            &[
                ("INPUT A OR D", &choice(input)),
                ("ACTUAL A OR D", &choice(actual)),
                ("ANALOG FALLBACK", &choice(fallback)),
                ("DIGITAL FALLBACK", &choice("No")),
                ("SILENCE DELAY", "Int:3;D:1;"),
            ],
        )
    }

    /// The 5700i pushes nothing when its input changes and its setup file
    /// keeps a stale `ACTUAL A OR D`: after an input change the app reads the
    /// live AS document once more, and while a fallback is on it keeps
    /// reading it, first after the silence delay.
    #[tokio::test]
    async fn the_active_input_is_read_again_after_an_input_change() {
        for (fallback, actual, follow_up) in [("No", "Digital", None), ("Yes", "Analog", Some(4))] {
            let before = input_system("Analog", "Analog", fallback);
            let after = input_system("Digital", actual, fallback);
            let (app, mut device, writes) = scripted_5700i(vec![after]).await;
            app.state.write().await.system = Some(
                Document::parse_for_model(before.as_bytes(), DeviceModel::Optimod5700i).unwrap(),
            );
            let expected =
                app.state.read().await.system.as_ref().unwrap().fields["INPUT A OR D"].clone();
            change(
                &mut device,
                &app,
                Change {
                    scope: Scope::System,
                    name: "INPUT A OR D".into(),
                    index: 1,
                    expected,
                    requested: None,
                    expected_host: "127.0.0.1".into(),
                    expected_session: "test-session".into(),
                    expected_preset: None,
                },
            )
            .await
            .unwrap();
            assert_eq!(
                *writes.lock().unwrap(),
                ["INPUT A OR D;1;String:<Digital>;2;"]
            );
            let due = device
                .input_status
                .expect("a read of the active input is due");
            assert!(due.after_change && due.at <= tokio::time::Instant::now());
            let revision = app.state.read().await.revision;
            read_input_status(&app, &mut device).await;
            let state = app.state.read().await;
            let system = state.system.as_ref().unwrap();
            assert_eq!(
                system.fields["ACTUAL A OR D"].value,
                Value::Choice(actual.into())
            );
            // Only a value that differs from the shown one is published.
            let published = if actual == "Analog" {
                revision
            } else {
                revision + 1
            };
            assert_eq!(state.revision, published);
            let next = device
                .input_status
                .map(|due| (due.at - tokio::time::Instant::now()).as_secs_f32().round() as u64);
            assert_eq!(next, follow_up, "fallback {fallback}");
        }
    }

    #[tokio::test]
    async fn setup_restore_writes_each_change_once_and_requires_the_readback() {
        for (after_contrast, expect_ok) in [(2, true), (3, false)] {
            let system = |contrast: u32| {
                document_text(
                    "SYSTEM",
                    &[
                        ("CONTRAST", &format!("Int:{contrast};D:{contrast};")),
                        ("NETWORK PORT", "Int:6201;D:6201;"),
                    ],
                )
            };
            let (app, mut device, writes) =
                scripted_5700i(vec![system(3), system(after_contrast)]).await;
            let backup = document_text(
                "SYSTEM",
                &[
                    ("CONTRAST", "Int:2;D:2;"),
                    ("NETWORK PORT", "Int:6202;D:6202;"),
                ],
            );
            let result = restore_setup(
                &mut device,
                &app,
                RestoreSetup {
                    document: backup,
                    confirmed: true,
                    expected_host: "127.0.0.1".into(),
                    expected_session: "test-session".into(),
                },
            )
            .await;
            assert_eq!(
                result.is_ok(),
                expect_ok,
                "{:?}",
                result.err().map(|e| e.message)
            );
            // CONTRAST once; the network port is never written.
            assert_eq!(*writes.lock().unwrap(), ["CONTRAST;2;Int:2;2;"]);
            let shown = app.state.read().await.system.as_ref().unwrap().fields["CONTRAST"].index;
            assert_eq!(shown, after_contrast);
        }
    }

    #[tokio::test]
    async fn applying_a_preset_file_writes_differences_once_and_verifies_them() {
        let before = document_text(
            "CURRENT",
            &[("AGC DRIVE", "Int:10;D:20;"), ("MB DRIVE", "Int:0;D:0;")],
        );
        let file = document_text(
            "FILE",
            &[("AGC DRIVE", "Int:11;D:21;"), ("MB DRIVE", "Int:0;D:0;")],
        );
        let after = document_text(
            "modif CURRENT",
            &[("AGC DRIVE", "Int:11;D:21;"), ("MB DRIVE", "Int:0;D:0;")],
        );
        let (app, mut device, writes) = scripted_5700i(vec![before, after]).await;
        let result = apply_preset(
            &mut device,
            &app,
            ApplyPreset {
                document: file,
                expected_name: "CURRENT".into(),
                confirmed: true,
                expected_host: "127.0.0.1".into(),
                expected_session: "test-session".into(),
            },
        )
        .await;
        assert!(result.is_ok(), "{:?}", result.err().map(|e| e.message));
        assert_eq!(*writes.lock().unwrap(), ["AGC DRIVE;21;Int:11;1;"]);
        let state = app.state.read().await;
        assert_eq!(
            state.processing.as_ref().unwrap().fields["AGC DRIVE"].index,
            21
        );
        assert_eq!(state.preset_base_name.as_deref(), Some("FILE"));

        // A file for another preset than the one on air is refused before writing.
        drop(state);
        let refused = apply_preset(
            &mut device,
            &app,
            ApplyPreset {
                document: document_text("FILE", &[("AGC DRIVE", "Int:11;D:21;")]),
                expected_name: "SOMETHING ELSE".into(),
                confirmed: true,
                expected_host: "127.0.0.1".into(),
                expected_session: "test-session".into(),
            },
        )
        .await;
        assert!(refused.is_err());
        assert_eq!(writes.lock().unwrap().len(), 1);
    }

    /// Two processor sessions of one service, as the hub starts them.
    fn two_sessions() -> (App, App, tempfile::TempDir) {
        let (base, storage) = test_app();
        let first = start_session(&base, "first");
        let second = start_session(&base, "second");
        (first, second, storage)
    }

    #[tokio::test]
    async fn two_processors_run_independent_sessions() {
        let one = FakeUnit::start(Terminal::Answering).await;
        let two = FakeUnit::start(Terminal::Answering).await;
        let (first, second, _storage) = two_sessions();
        command(&first, |r| Command::Connect(one.connect(), r))
            .await
            .unwrap();
        command(&second, |r| Command::Connect(two.connect(), r))
            .await
            .unwrap();
        let (a, b) = (
            first.state.read().await.clone(),
            second.state.read().await.clone(),
        );
        assert!(a.connected && b.connected);
        assert_ne!(a.session_id, b.session_id);
        assert_eq!((one.count(|c| &c.logins), two.count(|c| &c.logins)), (1, 1));
        // Each session polls only its own processor.
        let polls = (one.count(|c| &c.polls), two.count(|c| &c.polls));
        sleep(Duration::from_millis(300)).await;
        assert!(one.count(|c| &c.polls) > polls.0 && two.count(|c| &c.polls) > polls.1);
        assert_eq!(first.activity.session_opens.load(Ordering::Relaxed), 1);
        assert_eq!(second.activity.session_opens.load(Ordering::Relaxed), 1);

        // One processor failing leaves the other session untouched.
        one.drop_sessions();
        assert!(wait_until(&first, Duration::from_secs(1), |s| s.reconnect.is_some()).await);
        let polls = two.count(|c| &c.polls);
        sleep(Duration::from_millis(300)).await;
        assert!(two.count(|c| &c.polls) > polls);
        {
            let state = second.state.read().await;
            assert!(state.connected && state.reconnect.is_none() && state.error.is_none());
            assert_eq!(state.session_id, b.session_id);
        }
        // Nor does disconnecting it.
        command(&first, Command::Disconnect).await.unwrap();
        assert!(second.state.read().await.connected);
        assert_eq!(two.count(|c| &c.logins), 1);
        assert_eq!(two.count(|c| &c.disconnects), 0);
        assert_eq!(one.count(|c| &c.writes) + two.count(|c| &c.writes), 0);
    }

    #[tokio::test]
    async fn a_processor_is_held_by_one_session_only() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (first, second, _storage) = two_sessions();
        command(&first, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        let refused = command(&second, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap_err();
        assert!(refused.contains("already open in another tab"), "{refused}");
        assert!(!second.state.read().await.connected);
        // Still held while the first session waits to reconnect.
        unit.drop_sessions();
        assert!(wait_until(&first, Duration::from_secs(1), |s| s.reconnect.is_some()).await);
        assert!(
            command(&second, |r| Command::Connect(unit.connect(), r))
                .await
                .is_err()
        );
        assert_eq!(unit.count(|c| &c.logins), 1);
        // Free again once the first session lets it go.
        command(&first, Command::Disconnect).await.unwrap();
        command(&second, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
        assert_eq!(unit.count(|c| &c.logins), 2);
        // A failed connect holds nothing.
        command(&second, Command::Disconnect).await.unwrap();
        let mut wrong = unit.connect();
        wrong.port = 1;
        assert!(
            command(&first, |r| Command::Connect(wrong, r))
                .await
                .is_err()
        );
        command(&second, |r| Command::Connect(unit.connect(), r))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn device_routes_reach_their_own_session() {
        let unit = FakeUnit::start(Terminal::Answering).await;
        let (base, _storage) = test_app();
        let hub = sessions::Hub::new(base, device_routes());
        let routes = Router::new()
            .route(
                "/api/d/{device}/{*rest}",
                axum::routing::any(sessions::device_request),
            )
            .route("/api/sessions", get(sessions::sessions))
            .with_state(hub);
        let call = |method: &str, path: &str, body: Option<JsonValue>| {
            let request = Request::builder()
                .method(method)
                .uri(path)
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    body.map(|b| b.to_string()).unwrap_or_default(),
                ))
                .unwrap();
            let routes = routes.clone();
            async move {
                let response = routes.oneshot(request).await.unwrap();
                let status = response.status();
                let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
                    .await
                    .unwrap();
                (
                    status,
                    serde_json::from_slice::<JsonValue>(&bytes).unwrap_or(JsonValue::Null),
                )
            }
        };
        let connect = unit.connect();
        let body = json!({"host": connect.host, "port": connect.port, "terminal_port": connect.terminal_port,
                          "code": connect.code, "model": "auto"});
        let (status, _) = call("POST", "/api/d/one/connect", Some(body)).await;
        assert_eq!(status, StatusCode::OK);
        let (_, one) = call("GET", "/api/d/one/state", None).await;
        let (_, two) = call("GET", "/api/d/two/state", None).await;
        assert_eq!(
            (one["connected"].as_bool(), two["connected"].as_bool()),
            (Some(true), Some(false))
        );
        let (_, list) = call("GET", "/api/sessions", None).await;
        let keys: Vec<_> = list["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["key"].clone())
            .collect();
        assert_eq!(keys, [json!("one"), json!("two")]);
        assert_eq!(list["sessions"][0]["connected"], true);
        // A session never connects another processor's saved connection.
        let (status, refused) = call(
            "POST",
            "/api/d/two/devices/connect",
            Some(json!({"id": "one"})),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(
            refused["error"]
                .as_str()
                .unwrap()
                .contains("another processor")
        );
        let (status, _) = call("GET", "/api/d/no%20such/state", None).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(unit.count(|c| &c.logins), 1);
    }

    #[tokio::test]
    async fn one_meter_port_serves_every_8400_by_its_address() {
        let (first, second, _storage) = two_sessions();
        let listener = o8400::MeterListener::default();
        let here: std::net::IpAddr = "127.0.0.1".parse().unwrap();
        let elsewhere: std::net::IpAddr = "192.0.2.7".parse().unwrap();
        let route_first = listener.route(here, first.clone()).unwrap();
        let route_second = listener.route(elsewhere, second.clone()).unwrap();
        // One session per unit address; the port itself is bound once.
        assert!(listener.route(here, second.clone()).is_err());
        assert!(std::net::UdpSocket::bind(("0.0.0.0", orban_protocol::o8400::METER_PORT)).is_err());
        let sender = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut packet = [0u8; 34];
        packet[0] = 1;
        packet[5] = 42;
        sender
            .send_to(&packet, ("127.0.0.1", orban_protocol::o8400::METER_PORT))
            .unwrap();
        let mut live = false;
        for _ in 0..100 {
            if first.meters.borrow()["live"] == true {
                live = true;
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        assert!(live);
        assert_eq!(first.meters.borrow()["values"][5], 42);
        assert_eq!(second.meters.borrow()["live"], false);
        // The port is given back once no 8400 is connected.
        drop(route_first);
        assert!(std::net::UdpSocket::bind(("0.0.0.0", orban_protocol::o8400::METER_PORT)).is_err());
        drop(route_second);
        let mut free = false;
        for _ in 0..100 {
            if std::net::UdpSocket::bind(("0.0.0.0", orban_protocol::o8400::METER_PORT)).is_ok() {
                free = true;
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        assert!(free);
    }
}
