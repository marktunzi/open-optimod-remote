mod devices;
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
    profile::Profile,
    session::Session,
    terminal::read_snapshot_for_model,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use std::{
    collections::BTreeSet, convert::Infallible, ffi::OsStr, net::SocketAddr, sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{RwLock, mpsc, oneshot, watch},
    time::{MissedTickBehavior, interval, sleep},
};
use tokio_stream::{StreamExt, wrappers::WatchStream};
use tower_http::services::ServeDir;
use zeroize::Zeroizing;
const ORIGIN: &str = "http://127.0.0.1:5701";
const METER_POLL_PERIOD: Duration = Duration::from_millis(50);
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
struct Device {
    session: Session,
    terminal: SocketAddr,
    code: Zeroizing<String>,
    processing_baseline: Option<Document>,
    /// The exact-banner profile writes are validated against. `None` = read-only.
    profile: Option<Arc<Profile>>,
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
    let routes = Router::new()
        .route("/api/health", get(|| async { "open-optimod-remote/0.2.0" }))
        .route("/api/state", get(state))
        .route("/api/profile", get(profile_handler))
        .route("/api/meters", get(meters_handler))
        .route("/api/connect", post(connect_handler))
        .route("/api/disconnect", post(disconnect_handler))
        .route("/api/change", post(change_handler))
        .route("/api/refresh", post(refresh_handler))
        .route("/api/presets/recall", post(recall_handler))
        .route("/api/presets/current-file", get(preset_file_handler))
        .route("/api/presets/apply-file", post(apply_preset_handler))
        .route("/api/presets/manage", post(preset_file_action_handler))
        .route("/api/backup", get(backup_handler))
        .route("/api/backup/setup-plan", post(setup_plan_handler))
        .route("/api/backup/restore-setup", post(restore_setup_handler))
        .route("/api/devices", get(devices_list).post(devices_save))
        .route("/api/devices/delete", post(devices_delete))
        .route("/api/devices/code", post(devices_code))
        .route("/api/devices/connect", post(devices_connect))
        .fallback_service(ServeDir::new(assets))
        .layer(middleware::from_fn(local_origin))
        .with_state(app);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:5701")
        .await
        .expect("Local port 5701 available");
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
                    .arg(ORIGIN)
                    .spawn();
            });
        }
    }
    println!("Open Optimod Remote development build: {ORIGIN}");
    axum::serve(listener, routes).await.unwrap();
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
    let valid_host = host == Some("127.0.0.1:5701");
    let mutation =
        req.method() != axum::http::Method::GET && req.method() != axum::http::Method::HEAD;
    if !valid_host
        || (origin.is_some() && origin != Some(ORIGIN))
        || (mutation && origin != Some(ORIGIN))
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
    let devices = book_task(&app, |book| Ok(book.list())).await?;
    Ok(Json(
        json!({"devices":devices,"credential_storage":"local"}),
    ))
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
                device.session.send(219, b"").await?;
                drain(device, app).await?;
            }
        }
    }
}
async fn live_snapshot(device: &mut Device, app: &App, scope: Scope) -> Result<Document, String> {
    let address = device.terminal;
    let code = device.code.clone();
    with_meter_polling(
        device,
        app,
        read_snapshot_for_model(
            address,
            &code,
            scope,
            Duration::from_secs(4),
            device.session.info.model,
        ),
    )
    .await
}
async fn documents(device: &mut Device, app: &App) -> Result<(Document, Document), String> {
    let system = live_snapshot(device, app, Scope::System).await?;
    let processing = live_snapshot(device, app, Scope::Processing).await?;
    Ok((system, processing))
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

fn is_definite_disconnect(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("device disconnected")
        || lower.contains("connection reset")
        || lower.contains("broken pipe")
        || lower.contains("not connected")
}

async fn publish_documents(app: &App, device: &mut Device) -> Result<(), String> {
    let (system, processing) = documents(device, app).await?;
    let mut state = app.state.write().await;
    let (base_name, modified_fields) =
        refresh_processing_identity(&processing, &state.presets, &mut device.processing_baseline);
    state.system = Some(system);
    state.processing = Some(processing);
    state.preset_base_name = base_name;
    state.modified_fields = modified_fields;
    state.revision += 1;
    state.error = None;
    Ok(())
}
async fn drain(device: &mut Device, app: &App) -> Result<(), String> {
    device.session.send(250, b"").await?;
    for _ in 0..256 {
        match device.session.next_event().await? {
            Message::Meters { bank, values } => {
                if bank == 1 {
                    app.meters
                        .send_replace(json!({"live":true,"bank":bank,"values":values}));
                }
            }
            Message::Other { kind: 251, .. } => return Ok(()),
            _ => {}
        }
    }
    Err("Too many device events before response boundary".into())
}
async fn make_device(body: Connect, app: &App) -> Result<Device, String> {
    let code = Zeroizing::new(body.code);
    let ip = body.host.parse().map_err(|_| "Enter an IP address")?;
    let session = Session::connect_for_model(
        SocketAddr::new(ip, body.port),
        &code,
        Duration::from_secs(4),
        body.model,
    )
    .await?;
    let profile = Profile::for_adapter(session.info.adapter_id)?.map(Arc::new);
    let mut device = Device {
        session,
        terminal: SocketAddr::new(ip, body.terminal_port),
        code,
        processing_baseline: None,
        profile,
    };
    let init = async {
        if device.session.info.capabilities.parameter_reads {
            publish_documents(app, &mut device).await?;
        }
        if device.session.info.capabilities.preset_catalog {
            refresh_presets(app, &mut device).await?;
        }
        if device.session.info.capabilities.live_meters {
            device.session.send(222, b"").await?;
        }
        Ok::<(), String>(())
    }
    .await;
    if let Err(e) = init {
        let _ = device.session.disconnect().await;
        return Err(e);
    }
    if let Ok(mut active) = app.profile.write() {
        *active = device.profile.clone().unwrap_or_else(reference_profile);
    }
    let mut state = app.state.write().await;
    state.connected = true;
    state.write_enabled =
        device.session.info.access_level == 0 && device.session.info.capabilities.parameter_writes;
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
    Ok(device)
}
async fn refresh_presets(app: &App, device: &mut Device) -> Result<(), String> {
    let address = device.terminal;
    let code = device.code.clone();
    let (presets, processing) = with_meter_polling(
        device,
        app,
        orban_protocol::terminal::read_presets_for_model(
            address,
            &code,
            Duration::from_secs(6),
            device.session.info.model,
        ),
    )
    .await?;
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
    if device.session.info.access_level != 0 {
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
        Duration::from_secs(10),
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
                Duration::from_secs(6),
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
    let _ = device.session.send(222, b"").await;
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
    state.error = None;
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
    let sent = if store {
        orban_protocol::terminal::store_preset_for_model(
            address,
            &code,
            name,
            Duration::from_secs(10),
            model,
        )
        .await
    } else {
        orban_protocol::terminal::delete_preset_for_model(
            address,
            &code,
            name,
            Duration::from_secs(10),
            model,
        )
        .await
    };
    let _ = device.session.send(222, b"").await;
    // Read the list even after an error: the command may have taken effect.
    let listed = refresh_presets(app, device).await;
    let present = app
        .state
        .read()
        .await
        .presets
        .iter()
        .any(|p| p.name == name && p.kind == orban_protocol::presets::PresetKind::User);
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

async fn manage_preset(
    device: &mut Device,
    app: &App,
    body: PresetFile,
) -> Result<(), OperationError> {
    use orban_protocol::presets::{PresetKind, max_store_name_length, validate_store_name};
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
    if device.session.info.access_level != 0 {
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
        if let Some(name) = new_name {
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
    if device.session.info.access_level != 0 {
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
        let snapshot = live_snapshot(device, app, Scope::Processing)
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
    let after = after.expect("verification loop always produces a snapshot");
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
    if !device.session.info.capabilities.parameter_writes {
        return Err(OperationError::safe(format!(
            "Settings cannot be written on {}",
            device.session.info.model.label()
        )));
    }
    if !body.confirmed {
        return Err(OperationError::safe("Confirm the restore first"));
    }
    if device.session.info.access_level != 0 {
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
        let snapshot = live_snapshot(device, app, Scope::System)
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
            let snapshot = live_snapshot(device, app, body.scope)
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

fn hd_control_tracks_fm(name: &str) -> bool {
    name.starts_with("HD ") && !matches!(name, "HD COUPLING" | "HD DE ESS")
}

async fn owner(app: App, mut rx: mpsc::Receiver<Command>) {
    let mut device: Option<Device> = None;
    let mut tick = interval(METER_POLL_PERIOD);
    let mut meter_failures = 0_u8;
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
         command=rx.recv()=>{
          let Some(command)=command else{break};
          match command{
           Command::Connect(body,reply)=>{
            if device.is_some(){let _=reply.send(Err("Disconnect the current device first".into()));continue;}
            *app.state.write().await = Snapshot::default();
            match make_device(body,&app).await{Ok(d)=>{device=Some(d);meter_failures=0;let _=reply.send(Ok(()));},Err(e)=>{let _=reply.send(Err(e));}}
           }
           Command::Disconnect(reply)=>{
            let disconnecting=device.take();
            meter_failures=0;
            let mut state=app.state.write().await;state.connected=false;state.session_id=None;state.error=None;drop(state);app.meters.send_replace(json!({"live":false}));
            let result=if let Some(d)=disconnecting{d.session.disconnect().await}else{Ok(())};
            let _=reply.send(result);
           }
           Command::Refresh(reply)=>{let result=if let Some(d)=&mut device{async { publish_documents(&app,d).await?; refresh_presets(&app,d).await }.await}else{Err("Not connected".into())};let _=reply.send(result);}
           Command::Recall(body,reply)=>{
            let result=if let Some(d)=&mut device{recall(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
            meter_failures=0;
            tick.reset();
           match result {
            Ok(())=>{let _=reply.send(Ok(()));}
            Err(e)=>{
              let _=reply.send(Err(e.message));
             }
            }
           }
           Command::Change(body,reply)=>{
            let result=if let Some(d)=&mut device{change(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
            match result {
             Ok(())=>{let _=reply.send(Ok(()));}
            Err(e)=>{
              // Operation failures are logged; the heartbeat owns transport-loss detection.
              let _=reply.send(Err(e.message));
             }
            }
           }
           Command::PresetFile(body,reply)=>{
            let result=if let Some(d)=&mut device{manage_preset(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
            meter_failures=0;
            tick.reset();
            let _=reply.send(result.map_err(|e| e.message));
           }
           Command::RestoreSetup(body,reply)=>{
            let result=if let Some(d)=&mut device{restore_setup(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
            let _=reply.send(result.map_err(|e| e.message));
           }
           Command::ApplyPreset(body,reply)=>{
            let result=if let Some(d)=&mut device{apply_preset(d,&app,body).await}else{Err(OperationError::safe("Not connected"))};
            match result {
            Ok(())=>{let _=reply.send(Ok(()));}
            Err(e)=>{
              let _=reply.send(Err(e.message));
             }
            }
           }
          }
         }
         _=tick.tick(),if device.as_ref().is_some_and(|d| d.session.info.capabilities.live_meters)=>{
          let d=device.as_mut().unwrap();let result=async{d.session.send(219,b"").await?;drain(d,&app).await}.await;
          match result {
           Ok(())=>{meter_failures=0;}
           Err(e)=>{
            meter_failures=meter_failures.saturating_add(1);
            if is_definite_disconnect(&e) || meter_failures>=3 {
             if let Some(d)=device.take(){let _=d.session.disconnect().await;}
             let mut state=app.state.write().await;state.connected=false;state.session_id=None;state.error=Some(e);app.meters.send_replace(json!({"live":false}));
             meter_failures=0;
            }
           }
          }
         }
        }
    }
    if let Some(d) = device {
        let _ = d.session.disconnect().await;
    }
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
    use std::sync::atomic::{AtomicUsize, Ordering};
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
            terminal: "127.0.0.1:1".parse().unwrap(),
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: None,
            profile: Some(reference_profile()),
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
            terminal: "127.0.0.1:1".parse().unwrap(),
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: Some(processing),
            profile: Some(reference_profile()),
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
            terminal: terminal_address,
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: Some(current),
            profile: Some(reference_profile()),
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
            ("127.0.0.1:5701", Some(ORIGIN), StatusCode::OK),
            (
                "127.0.0.1:5701",
                Some("https://foreign.invalid"),
                StatusCode::FORBIDDEN,
            ),
            ("rebound.invalid:5701", Some(ORIGIN), StatusCode::FORBIDDEN),
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
    #[tokio::test]
    async fn no_device_means_no_write_and_no_optimistic_state() {
        let (tx, rx) = mpsc::channel(8);
        let (meters, _) = watch::channel(json!({"live":false}));
        let app = App {
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
            terminal: terminal_address,
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: Some(before),
            profile: Some(profile),
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

    fn cent(index: u32, value: i32) -> Field {
        Field {
            index,
            value: Value::Cent(value),
        }
    }

    fn int(index: u32, value: i32) -> Field {
        Field {
            index,
            value: Value::Int(value),
        }
    }

    fn optimod_5500(readback: Field) -> WriteCase {
        WriteCase {
            banner: "5500 V 1.2.8.24",
            terminal_banner: "Orban Optimod 5500 test",
            family: "8300.10",
            name: "2B BASS ATTACK",
            before: cent(7, 1100),
            target: 8,
            wire: "2B BASS ATTACK;8;Cent:1200;1;",
            readback,
        }
    }

    #[tokio::test]
    async fn static_profile_writes_are_confirmed_by_a_full_readback() {
        let outcome = simulated_write(optimod_5500(cent(8, 1200))).await;
        assert_eq!(outcome.evidence, Evidence::Static);
        assert!(outcome.result.is_ok(), "{:?}", outcome.result.err());
        assert_eq!((outcome.changes, outcome.readbacks), (1, 1));
        let field = &outcome.state.processing.unwrap().fields["2B BASS ATTACK"];
        assert_eq!(field, &cent(8, 1200));
    }

    #[tokio::test]
    async fn static_profile_mismatch_shows_the_processor_value_and_never_retries() {
        let outcome = simulated_write(optimod_5500(cent(7, 1100))).await;
        assert_eq!(
            outcome.result.unwrap_err().message,
            "The processor did not confirm the change"
        );
        assert_eq!((outcome.changes, outcome.readbacks), (1, 1));
        let field = &outcome.state.processing.unwrap().fields["2B BASS ATTACK"];
        assert_eq!(field, &cent(7, 1100));
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
        users: Vec<String>,
    }

    /// A 5500-style terminal that keeps a user preset list, answers SP and DP
    /// like the firmware and always ends a reply with an AP document.
    async fn simulated_preset_file(case: PresetFileCase) -> PresetFileOutcome {
        let pc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let pc_address = pc_listener.local_addr().unwrap();
        let banner = case.banner;
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
                    let reply = match orban_protocol::message::decode(&frame).unwrap() {
                        Message::Other { kind: 219, .. } => Some(220),
                        Message::Other { kind: 250, .. } => Some(251),
                        _ => None,
                    };
                    if let Some(kind) = reply {
                        let _ = stream
                            .write_all(&orban_protocol::message::encode(kind, b"").unwrap())
                            .await;
                    }
                }
            }
        });

        let terminal_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let terminal_address = terminal_listener.local_addr().unwrap();
        let commands = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let users = Arc::new(std::sync::Mutex::new(
            case.users.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
        ));
        let on_air = Arc::new(std::sync::Mutex::new(case.on_air.to_string()));
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
            terminal: terminal_address,
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: Some(current),
            profile: Some(reference_profile()),
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
        let listed = app
            .state
            .read()
            .await
            .presets
            .iter()
            .filter(|p| p.kind == orban_protocol::presets::PresetKind::User)
            .map(|p| p.name.clone())
            .collect();
        let commands = commands.lock().unwrap().clone();
        PresetFileOutcome {
            result,
            commands,
            users: listed,
        }
    }

    const P5500: (&str, &str) = ("5500 V 1.2.8.24", "Orban Optimod 5500");

    #[tokio::test]
    async fn a_5500_saves_and_deletes_user_presets_confirmed_by_the_list() {
        let saved = simulated_preset_file(PresetFileCase {
            banner: P5500.0,
            terminal_banner: P5500.1,
            on_air: "modif FACTORY ONE",
            users: &["MORNING"],
            ignore: false,
            action: PresetFileAction::Save {
                name: "EVENING".into(),
            },
        })
        .await;
        assert!(saved.result.is_ok(), "{:?}", saved.result);
        assert_eq!(saved.commands, ["SP EVENING[1234]"]);
        assert_eq!(saved.users, ["MORNING", "EVENING"]);

        let deleted = simulated_preset_file(PresetFileCase {
            banner: P5500.0,
            terminal_banner: P5500.1,
            on_air: "FACTORY ONE",
            users: &["MORNING", "EVENING"],
            ignore: false,
            action: PresetFileAction::Delete {
                name: "MORNING".into(),
            },
        })
        .await;
        assert!(deleted.result.is_ok(), "{:?}", deleted.result);
        assert_eq!(deleted.commands, ["DP MORNING[1234]"]);
        assert_eq!(deleted.users, ["EVENING"]);
    }

    #[tokio::test]
    async fn a_5500_renames_the_on_air_user_preset_by_saving_then_deleting() {
        let renamed = simulated_preset_file(PresetFileCase {
            banner: P5500.0,
            terminal_banner: P5500.1,
            on_air: "MORNING",
            users: &["MORNING"],
            ignore: false,
            action: PresetFileAction::Rename {
                name: "MORNING".into(),
                new_name: "BREAKFAST".into(),
            },
        })
        .await;
        assert!(renamed.result.is_ok(), "{:?}", renamed.result);
        assert_eq!(renamed.commands, ["SP BREAKFAST[1234]", "DP MORNING[1234]"]);
        assert_eq!(renamed.users, ["BREAKFAST"]);
    }

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
    async fn an_ignored_delete_is_reported_from_the_list_and_never_retried() {
        let outcome = simulated_preset_file(PresetFileCase {
            banner: P5500.0,
            terminal_banner: P5500.1,
            on_air: "FACTORY ONE",
            users: &["MORNING"],
            ignore: true,
            action: PresetFileAction::Delete {
                name: "MORNING".into(),
            },
        })
        .await;
        let error = outcome.result.unwrap_err();
        assert!(error.contains("did not delete"), "{error}");
        assert_eq!(outcome.commands, ["DP MORNING[1234]"]);
        assert_eq!(outcome.users, ["MORNING"]);
    }

    #[tokio::test]
    async fn models_without_a_documented_save_command_refuse_preset_changes() {
        for (banner, terminal_banner) in [
            ("5700i V 3.0.1.20", "Orban Optimod 5700i test"),
            (
                "8700HD V 1.0.2.161",
                "Welcome to the Orban Optimod-FM 8700HD.",
            ),
        ] {
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
            terminal: terminal_address,
            code: Zeroizing::new("1234".to_owned()),
            processing_baseline: None,
            profile: Some(reference_profile()),
        };
        (app, device, writes)
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
}
