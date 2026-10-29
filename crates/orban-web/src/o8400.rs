//! The OPTIMOD-FM 8400 session of the service: the 8400's own PC Remote
//! protocol (`orban_protocol::o8400`) instead of the PC Remote family's
//! opcodes and terminal. Settings and presets are files on the unit, read
//! over the session's file calls as the original client reads them; a change
//! is confirmed by reading the file the unit saves it to; meters arrive over
//! UDP. See docs/protocol/8400.md.

use crate::{
    App, Change, Connect, OperationError, PresetFile, PresetFileAction, Recall, Snapshot,
    UnitFileWrite, ensure_session, refresh_processing_identity, refreshed_less_more_availability,
};
use orban_protocol::{
    adapter::{AdapterRegistry, Capabilities, DeviceModel},
    control::Scope,
    document::{Document, Field, Value},
    o8400::{self, Session},
    presets::{Preset, PresetKind},
    profile::Profile,
};
use serde_json::json;
use std::{collections::HashMap, net::IpAddr, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::UdpSocket, task::JoinHandle, time::Instant};
use zeroize::Zeroizing;

/// The original client sends a keepalive every 2 s; the unit ends a session
/// after 30 s without a byte.
pub const KEEPALIVE_PERIOD: Duration = Duration::from_secs(2);
/// A keepalive tick comes a few milliseconds less than a period after the
/// last message, which is set once its answer is in: without this slack
/// every second tick was skipped and the keepalive went every 4 s.
const KEEPALIVE_SLACK: Duration = Duration::from_millis(250);
/// The unit keeps a session until 30 s without a received byte (watchdog
/// 0x1370b0). It drops some received segments; under host load TCP then
/// backs off for many seconds (virtual 8400, 29 October 2026). An answer
/// that arrives within those 30 s still belongs to a live session, so the
/// app gives up only after the unit itself would have, with a margin.
const UNANSWERED_LIMIT: Duration = Duration::from_secs(35);
/// The unit saves a change to its on-air or system file 8-10 s after it
/// (virtual 8400, 21 October 2026).
const READBACK_LIMIT: Duration = Duration::from_secs(25);
const READBACK_INTERVAL: Duration = Duration::from_secs(1);
/// A recalled preset is in the on-air file within a few seconds.
const RECALL_LIMIT: Duration = Duration::from_secs(15);
/// The unit's own files in `c:\8400\user\`; every other `.orb` is a user preset.
const STATE_FILES: [&str; 5] = [
    "NEXT.ORB",
    "ONAIR.ORB",
    "SYSTEM.ORB",
    "TEST.ORB",
    "TIME.ORB",
];
/// PEG messages the original client forwards (type 47).
const RECALL_EVENT: u16 = 0x400B;
const METERS_ON_EVENT: u16 = 0x40DC;

pub struct Unit {
    pub session: Session,
    capabilities: Capabilities,
    profile: Option<Arc<Profile>>,
    meters: Option<MeterRoute>,
    processing_baseline: Option<Document>,
    /// The file of each user preset, as the unit lists it (`EURO AC .ORB`
    /// holds `EURO AC`): a Delete removes that file.
    user_files: HashMap<String, String>,
    /// The passcode of this session, which a Security change must keep.
    code: Zeroizing<String>,
    /// The time of the last message to the unit.
    pub last_sent: Instant,
    /// The time of the last keepalive answer.
    answered: Instant,
}

impl Unit {
    pub async fn disconnect(mut self) -> Result<(), String> {
        self.meters.take();
        self.session.disconnect().await
    }
}

/// Whether a connection request is for an 8400: chosen as the model, or
/// "Auto Detect" on the 8400's own port.
pub fn wanted(body: &Connect) -> bool {
    body.model == DeviceModel::Optimod8400
        || (body.model == DeviceModel::Auto && body.port == o8400::DEFAULT_PORT)
}

pub async fn connect(body: Connect, app: &App) -> Result<Unit, String> {
    let code = Zeroizing::new(body.code);
    let ip: IpAddr = body.host.parse().map_err(|_| "Enter an IP address")?;
    let session = Session::connect(
        SocketAddr::new(ip, body.port),
        &code,
        crate::SESSION_DEADLINE,
    )
    .await?;
    let banner = format!("8400 V {}", session.login.software);
    let adapter = match AdapterRegistry::identify_banner(&banner) {
        Ok(adapter) if adapter.model == DeviceModel::Optimod8400 => adapter,
        Ok(_) | Err(_) => {
            let _ = session.disconnect().await;
            return Err("Unsupported device model".into());
        }
    };
    let profile = Profile::for_adapter(adapter.id)?.map(Arc::new);
    let mut unit = Unit {
        session,
        capabilities: adapter.capabilities,
        profile,
        meters: None,
        processing_baseline: None,
        user_files: HashMap::new(),
        code: code.clone(),
        last_sent: Instant::now(),
        answered: Instant::now(),
    };
    let synced = synchronize(app, &mut unit).await;
    if let Err(error) = &synced
        && crate::is_definite_disconnect(error)
    {
        let error = error.clone();
        let _ = unit.disconnect().await;
        return Err(error);
    }
    let meters = match app.peer.peers.meters_8400.route(ip, app.clone()) {
        Ok(route) => {
            unit.meters = Some(route);
            unit.session
                .event(METERS_ON_EVENT, 0, "")
                .await
                .map(|_| None)
        }
        Err(error) => Ok(Some(format!(
            "Live meters are unavailable: UDP port {} is in use ({error})",
            o8400::METER_PORT
        ))),
    };
    let meter_error = match meters {
        Ok(message) => message,
        Err(error) => {
            let _ = unit.disconnect().await;
            return Err(error);
        }
    };
    unit.last_sent = Instant::now();
    if let Ok(mut active) = app.profile.write() {
        *active = unit
            .profile
            .clone()
            .unwrap_or_else(crate::reference_profile);
    }
    let mut state = app.state.write().await;
    state.connected = true;
    state.write_enabled =
        unit.session.login.access_level == 0 && adapter.capabilities.parameter_writes;
    state.access = Some(orban_protocol::access::access(
        adapter.model,
        unit.session.login.access_level,
    ));
    state.firmware = banner;
    state.model = adapter.model;
    state.skin_id = Some(adapter.skin);
    state.adapter_id = Some(adapter.id.into());
    state.capabilities = Some(adapter.capabilities);
    state.evidence = Some(adapter.evidence);
    state.host = body.host;
    state.device_id = body.device_id;
    state.session_id = Some(uuid::Uuid::new_v4().to_string());
    state.error = None;
    state.sync_error = match (synced, meter_error) {
        (Err(error), _) => Some(format!("Settings could not be read: {error}")),
        (Ok(()), meters) => meters,
    };
    drop(state);
    Ok(unit)
}

/// Every 8400 sends its meters to the same UDP port of the PC (16540), so
/// the app binds it once for all 8400 sessions and hands each packet to the
/// session of the unit that sent it. The port is bound while at least one
/// 8400 is connected.
#[derive(Default)]
pub struct MeterListener {
    state: Arc<std::sync::Mutex<Listening>>,
}

#[derive(Default)]
struct Listening {
    routes: HashMap<IpAddr, App>,
    task: Option<JoinHandle<()>>,
}

/// One unit's share of the meter port; dropping it stops its packets.
pub struct MeterRoute {
    state: Arc<std::sync::Mutex<Listening>>,
    unit: IpAddr,
}

impl Drop for MeterRoute {
    fn drop(&mut self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.routes.remove(&self.unit);
        if state.routes.is_empty()
            && let Some(task) = state.task.take()
        {
            task.abort();
        }
    }
}

impl MeterListener {
    /// Sends the meter packets of `unit` to `app`.
    pub fn route(&self, unit: IpAddr, app: App) -> Result<MeterRoute, String> {
        let mut state = self.state.lock().map_err(|_| "Meter port unavailable")?;
        if state.routes.contains_key(&unit) {
            return Err(format!(
                "{unit} already sends its meters to another session"
            ));
        }
        if state.task.is_none() {
            let socket = std::net::UdpSocket::bind(("0.0.0.0", o8400::METER_PORT))
                .and_then(|socket| socket.set_nonblocking(true).map(|()| socket))
                .and_then(UdpSocket::from_std)
                .map_err(|error| error.to_string())?;
            state.task = Some(tokio::spawn(receive_meters(socket, self.state.clone())));
        }
        state.routes.insert(unit, app);
        Ok(MeterRoute {
            state: self.state.clone(),
            unit,
        })
    }
}

/// The meter packets of each routed unit (type 1, 34 bytes) as meter values;
/// the clock packets (type 2) and other senders are ignored.
async fn receive_meters(socket: UdpSocket, state: Arc<std::sync::Mutex<Listening>>) {
    let mut buffer = [0; 256];
    loop {
        let Ok((count, from)) = socket.recv_from(&mut buffer).await else {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };
        let Some(app) = state
            .lock()
            .ok()
            .and_then(|routes| routes.routes.get(&from.ip()).cloned())
        else {
            continue;
        };
        if let Some(values) = o8400::meter_values(&buffer[..count]) {
            app.activity
                .meter_frames
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            app.meters
                .send_replace(json!({"live":true,"bank":1,"values":values}));
        }
    }
}

fn document(file: &[u8]) -> Result<Document, String> {
    Document::parse_for_model(o8400::document_text(file)?, DeviceModel::Optimod8400)
}

async fn read_document(unit: &mut Unit, name: &str) -> Result<Document, String> {
    let file = unit.session.read(o8400::USER_DIR, name).await?;
    unit.last_sent = Instant::now();
    document(&file)
}

/// The factory presets folder for the unit's pre-emphasis, as the original
/// client chooses it (system control PRE-E).
fn factory_folder(system: &Document) -> &'static str {
    match system.fields.get("PRE-E").map(|f| &f.value) {
        Some(Value::Choice(value)) if value == "50us" => "c:\\8400\\presets\\50us presets\\",
        _ => "c:\\8400\\presets\\75us presets\\",
    }
}

async fn read_presets(unit: &mut Unit, system: &Document) -> Result<Vec<Preset>, String> {
    let mut presets = Vec::new();
    for file in unit.session.list(factory_folder(system), "*.orb").await? {
        if let Some(name) = file
            .strip_suffix(".ORB")
            .or_else(|| file.strip_suffix(".orb"))
        {
            presets.push(Preset {
                name: name.to_owned(),
                kind: PresetKind::Factory,
            });
        }
    }
    // A user preset's name is the one in its file, as the unit lists it.
    unit.user_files.clear();
    for file in unit.session.list(o8400::USER_DIR, "*.orb").await? {
        if STATE_FILES.iter().any(|f| f.eq_ignore_ascii_case(&file)) {
            continue;
        }
        let name = read_document(unit, &file).await?.name;
        unit.user_files.entry(name.clone()).or_insert(file);
        if !presets.iter().any(|p| p.name == name) {
            presets.push(Preset {
                name,
                kind: PresetKind::User,
            });
        }
    }
    unit.last_sent = Instant::now();
    Ok(presets)
}

pub async fn synchronize(app: &App, unit: &mut Unit) -> Result<(), String> {
    let system = read_document(unit, "system.orb").await?;
    let processing = read_document(unit, "onair.orb").await?;
    let presets = read_presets(unit, &system).await?;
    let mut state = app.state.write().await;
    state.system = Some(system);
    show_processing(&mut state, unit, processing, presets);
    state.error = None;
    state.sync_error = None;
    Ok(())
}

fn show_processing(
    state: &mut Snapshot,
    unit: &mut Unit,
    processing: Document,
    presets: Vec<Preset>,
) {
    let previous_name = state.processing.as_ref().map(|d| d.name.clone());
    let previous_less_more = state.less_more_available;
    let (base_name, modified_fields) =
        refresh_processing_identity(&processing, &presets, &mut unit.processing_baseline);
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
}

/// Read the file that holds `scope` until `done` accepts it or the limit passes.
async fn read_until(
    unit: &mut Unit,
    scope: Scope,
    limit: Duration,
    done: impl Fn(&Document) -> bool,
) -> Result<Document, String> {
    let file = match scope {
        Scope::Processing => "onair.orb",
        Scope::System => "system.orb",
    };
    let end = Instant::now() + limit;
    loop {
        let document = read_document(unit, file).await?;
        if done(&document) || Instant::now() >= end {
            return Ok(document);
        }
        tokio::time::sleep(READBACK_INTERVAL).await;
    }
}

pub async fn change(unit: &mut Unit, app: &App, body: Change) -> Result<(), OperationError> {
    if !unit.capabilities.parameter_writes {
        return Err(OperationError::safe(
            "Parameter writes are not verified for this OPTIMOD-FM 8400 software",
        ));
    }
    if unit.session.login.access_level != 0 {
        return Err(OperationError::safe(
            "Write permissions for this access level have not been verified",
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
    }
    if body.requested.is_some() {
        return Err(OperationError::safe("Unsupported typed setting value"));
    }
    let profile = unit.profile.as_ref().ok_or_else(|| {
        OperationError::safe("This processor has no parameter profile; it is read-only")
    })?;
    // The unit looks a name up in the on-air settings first, so a system
    // control that shares a processing control's name cannot be set.
    if body.scope == Scope::System && profile.value(Scope::Processing, &body.name, 0).is_ok() {
        return Err(OperationError::safe(
            "This system setting shares its name with a processing setting and cannot be changed remotely",
        ));
    }
    if profile
        .value(body.scope, &body.name, body.expected.index)
        .map_err(OperationError::safe)?
        != body.expected.value
    {
        return Err(OperationError::safe(
            "The current value does not match the reference profile",
        ));
    }
    let expected = Field {
        index: body.index,
        value: profile
            .value(body.scope, &body.name, body.index)
            .map_err(OperationError::safe)?,
    };
    o8400::change_payload(&body.name, &expected).map_err(OperationError::safe)?;
    unit.session
        .change(&body.name, &expected)
        .await
        .map_err(OperationError::uncertain)?;
    unit.last_sent = Instant::now();
    let after = read_until(unit, body.scope, READBACK_LIMIT, |document| {
        document.fields.get(&body.name) == Some(&expected)
    })
    .await
    .map_err(|error| {
        OperationError::safe(format!(
            "The change could not be read back; the connection remains active. {error}"
        ))
    })?;
    let confirmed = after.fields.get(&body.name) == Some(&expected);
    let mut state = app.state.write().await;
    match body.scope {
        Scope::System => state.system = Some(after),
        Scope::Processing => {
            let presets = std::mem::take(&mut state.presets);
            show_processing(&mut state, unit, after, presets);
            if body.name != "LESS MORE" {
                state.less_more_available = false;
            }
        }
    }
    state.revision += 1;
    if !confirmed {
        return Err(OperationError::safe(
            "The processor did not confirm the change",
        ));
    }
    state.error = None;
    Ok(())
}

/// Recall as the original client does: the unit puts the named preset from
/// its own list on air. The on-air file confirms it.
pub async fn recall(unit: &mut Unit, app: &App, body: Recall) -> Result<(), OperationError> {
    if !unit.capabilities.preset_recall {
        return Err(OperationError::safe(
            "Preset recall is not enabled for this OPTIMOD-FM 8400 software",
        ));
    }
    if !body.confirmed {
        return Err(OperationError::safe(
            "Confirm the on-air preset change first",
        ));
    }
    if unit.session.login.access_level != 0 {
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
        if !state.presets.iter().any(|p| p.name == body.name) {
            return Err(OperationError::safe(
                "Select an available factory or user preset",
            ));
        }
    }
    unit.session
        .event(RECALL_EVENT, 0, &body.name)
        .await
        .map_err(OperationError::uncertain)?;
    unit.last_sent = Instant::now();
    let after = read_until(unit, Scope::Processing, RECALL_LIMIT, |document| {
        document.name == body.name
    })
    .await
    .map_err(|error| {
        OperationError::safe(format!(
            "The recall could not be confirmed; the connection remains active. {error}"
        ))
    })?;
    let confirmed = after.name == body.name;
    unit.processing_baseline = None;
    let mut state = app.state.write().await;
    let presets = std::mem::take(&mut state.presets);
    show_processing(&mut state, unit, after, presets);
    if !confirmed {
        return Err(OperationError::safe(
            "The processor did not confirm the recall",
        ));
    }
    state.error = None;
    Ok(())
}

/// Save tells the unit to re-read the files the client wrote (GUI event).
const SAVE_EVENT: u16 = 16388;
/// A user preset was deleted (GUI event, after removing its file).
const DELETED_EVENT: u16 = 16392;

/// Save, Save As, Rename and Delete as the original client does them
/// (capture of its Save As, 21 October 2026): it writes the on-air
/// document as `next.orb` (keeping that file's name), `onair.orb` and
/// `<NAME>.orb`, each with its own `Preset Type`, then sends the Save event.
/// Delete removes `<NAME>.orb` and sends the deleted event.
pub async fn manage_preset(
    unit: &mut Unit,
    app: &App,
    body: PresetFile,
) -> Result<(), OperationError> {
    use orban_protocol::presets::{max_store_name_length, validate_store_name};
    if !unit.capabilities.preset_store || !unit.capabilities.preset_delete {
        return Err(OperationError::safe(
            "Saving and deleting presets is not enabled for this OPTIMOD-FM 8400 software",
        ));
    }
    if !body.confirmed {
        return Err(OperationError::safe("Confirm the preset change first"));
    }
    if unit.session.login.access_level != 0 {
        return Err(OperationError::safe(
            "Preset changes are not enabled for this access level",
        ));
    }
    let (shown, presets) = {
        let state = app.state.read().await;
        ensure_session(&state, &body.expected_session).map_err(OperationError::safe)?;
        if state.host != body.expected_host {
            return Err(OperationError::safe(
                "The active device changed. Review the connection first",
            ));
        }
        let shown = state
            .processing
            .clone()
            .filter(|d| d.name == body.expected_name)
            .ok_or_else(|| {
                OperationError::safe(
                    "The on-air preset changed elsewhere. Review the current preset first",
                )
            })?;
        (shown, state.presets.clone())
    };
    let kind = |name: &str| {
        presets
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.kind.clone())
    };
    let base = shown
        .name
        .strip_prefix("modif ")
        .unwrap_or(&shown.name)
        .to_owned();
    match &body.action {
        PresetFileAction::Save { name } => {
            validate_store_name(name, max_store_name_length(DeviceModel::Optimod8400))
                .map_err(OperationError::safe)?;
            reject_state_file(name)?;
            if kind(name) == Some(PresetKind::Factory) {
                return Err(OperationError::safe("A factory preset cannot be replaced"));
            }
            save(unit, &shown, name).await?;
        }
        PresetFileAction::Delete { name } => {
            if kind(name) != Some(PresetKind::User) {
                return Err(OperationError::safe("Select a user preset to delete"));
            }
            if *name == base {
                return Err(OperationError::safe(format!(
                    "Cannot delete the on-air preset {name}"
                )));
            }
            delete(unit, name).await?;
        }
        PresetFileAction::Rename { name, new_name } => {
            if kind(name) != Some(PresetKind::User) || *name != shown.name {
                return Err(OperationError::safe(
                    "Rename the user preset on air without changes",
                ));
            }
            validate_store_name(new_name, max_store_name_length(DeviceModel::Optimod8400))
                .map_err(OperationError::safe)?;
            reject_state_file(new_name)?;
            if kind(new_name).is_some() {
                return Err(OperationError::safe(format!("{new_name} already exists")));
            }
            save(unit, &shown, new_name).await?;
            delete(unit, name).await?;
        }
    }
    unit.last_sent = Instant::now();
    // The list and the on-air file now say what happened.
    let system = app.state.read().await.system.clone();
    let listed = match system {
        Some(system) => read_presets(unit, &system).await,
        None => Err("The system settings are not available".into()),
    };
    let after = read_document(unit, "onair.orb").await;
    let (Ok(listed), Ok(after)) = (listed, after) else {
        return Err(OperationError::uncertain(
            "The result could not be confirmed. Refresh the preset list",
        ));
    };
    let wanted = match &body.action {
        PresetFileAction::Save { name } | PresetFileAction::Rename { new_name: name, .. } => {
            listed.iter().any(|p| &p.name == name) && after.name == *name
        }
        PresetFileAction::Delete { name } => !listed.iter().any(|p| &p.name == name),
    };
    unit.processing_baseline = None;
    let mut state = app.state.write().await;
    show_processing(&mut state, unit, after, listed);
    if !wanted {
        return Err(OperationError::uncertain(
            "The processor's preset list does not show the change",
        ));
    }
    state.error = None;
    Ok(())
}

/// A user preset is `<NAME>.orb` next to the unit's own files: a name like
/// `system` would replace the system settings.
fn reject_state_file(name: &str) -> Result<(), OperationError> {
    let file = format!("{name}.orb");
    if STATE_FILES.iter().any(|f| f.eq_ignore_ascii_case(&file)) {
        return Err(OperationError::safe(format!(
            "{name} is the name of one of the processor's own files"
        )));
    }
    Ok(())
}

/// The on-air document once the unit has saved what is shown (it writes
/// `onair.orb` 8-10 s after a change), as text up to `End Preset`.
async fn on_air_text(unit: &mut Unit, shown: &Document) -> Result<String, OperationError> {
    let end = Instant::now() + READBACK_LIMIT;
    loop {
        let file = unit
            .session
            .read(o8400::USER_DIR, "onair.orb")
            .await
            .map_err(OperationError::uncertain)?;
        unit.last_sent = Instant::now();
        let text = o8400::document_text(&file).map_err(OperationError::safe)?;
        if document(&file).is_ok_and(|d| d.fields == shown.fields) {
            return String::from_utf8(text.to_vec())
                .map_err(|_| OperationError::safe("Unsupported document encoding"));
        }
        if Instant::now() >= end {
            return Err(OperationError::safe(
                "The processor has not saved the shown settings yet; try again in a few seconds",
            ));
        }
        tokio::time::sleep(READBACK_INTERVAL).await;
    }
}

/// `text` with its `Preset Name` (keeping the size after it) and its
/// `Preset Type` replaced.
fn with_header(text: &str, name: &str, kind: &str) -> Result<String, OperationError> {
    let mut named = false;
    let mut typed = false;
    let lines: Vec<String> = text
        .split('\n')
        .map(|line| {
            if !named && let Some(rest) = line.strip_prefix("Preset Name=<") {
                named = true;
                let after = rest.split_once('>').map_or("", |(_, after)| after);
                format!("Preset Name=<{name}>{after}")
            } else if !typed && line.starts_with("Preset Type=") {
                typed = true;
                format!("Preset Type= <{kind}>")
            } else {
                line.to_owned()
            }
        })
        .collect();
    if !named || !typed {
        return Err(OperationError::safe(
            "The on-air document has no preset header",
        ));
    }
    Ok(lines.join("\n"))
}

async fn save(unit: &mut Unit, shown: &Document, name: &str) -> Result<(), OperationError> {
    let text = on_air_text(unit, shown).await?;
    let next = read_document(unit, "next.orb")
        .await
        .map_err(OperationError::uncertain)?;
    let files = [
        (
            "next.orb".to_owned(),
            with_header(&text, &next.name, "NEXT PRESET")?,
        ),
        (
            "onair.orb".to_owned(),
            with_header(&text, name, "ON AIR PRESET")?,
        ),
        (
            format!("{name}.orb"),
            with_header(&text, name, "USER PRESET")?,
        ),
    ];
    for (file, contents) in files {
        unit.session
            .write(o8400::USER_DIR, &file, contents.as_bytes())
            .await
            .map_err(OperationError::uncertain)?;
        unit.last_sent = Instant::now();
    }
    unit.session
        .event(SAVE_EVENT, 18, "")
        .await
        .map_err(OperationError::uncertain)
}

async fn delete(unit: &mut Unit, name: &str) -> Result<(), OperationError> {
    let file = unit
        .user_files
        .get(name)
        .cloned()
        .ok_or_else(|| OperationError::safe("Refresh the preset list first"))?;
    unit.session
        .remove(o8400::USER_DIR, &file)
        .await
        .map_err(OperationError::uncertain)?;
    unit.session
        .event(DELETED_EVENT, 0, name)
        .await
        .map_err(OperationError::uncertain)
}

/// Passcodes changed: the unit re-reads `password.pas` (GUI event).
const PASSCODES_EVENT: u16 = 16390;

/// The unit's passcode file (`kind` 230); the 8400 has no automation file
/// the app reads yet.
pub async fn read_unit_file(unit: &mut Unit, kind: u16) -> Result<Option<String>, String> {
    if kind != 230 {
        return Err("Automation is not available for the OPTIMOD-FM 8400 in this version".into());
    }
    if unit.session.login.access_level != 0 {
        return Err("Security is not available for this access level".into());
    }
    let file = unit.session.read(o8400::USER_DIR, "password.pas").await?;
    unit.last_sent = Instant::now();
    String::from_utf8(o8400::document_text(&file)?.to_vec())
        .map(Some)
        .map_err(|_| "Unsupported document encoding".into())
}

/// Replaces `password.pas` and tells the unit, as the client does after
/// its Security screen (the passcodes event), then reads it back.
pub async fn write_unit_file(
    unit: &mut Unit,
    app: &App,
    body: UnitFileWrite,
) -> Result<(), OperationError> {
    if body.kind != 230 {
        return Err(OperationError::safe(
            "Automation is not available for the OPTIMOD-FM 8400 in this version",
        ));
    }
    if !unit.capabilities.parameter_writes || unit.session.login.access_level != 0 {
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
    // The session's own passcode must stay, with all screens.
    let passcodes = crate::security::parse(&body.document).map_err(OperationError::safe)?;
    if !passcodes.iter().any(|p| {
        p.code.eq_ignore_ascii_case(&unit.code) && p.level == crate::security::LEVELS_8400[0]
    }) {
        return Err(OperationError::safe(
            "Keep the passcode this connection uses, with access to all screens",
        ));
    }
    unit.session
        .write(o8400::USER_DIR, "password.pas", body.document.as_bytes())
        .await
        .map_err(OperationError::uncertain)?;
    unit.session
        .event(PASSCODES_EVENT, 0, "")
        .await
        .map_err(OperationError::uncertain)?;
    unit.last_sent = Instant::now();
    let normalize = |text: &str| text.replace("\r\n", "\n");
    match read_unit_file(unit, 230).await {
        Ok(Some(text)) if normalize(&text) == normalize(&body.document) => Ok(()),
        Ok(_) => Err(OperationError::uncertain(
            "The processor's file does not show the change. Open the window again to see what it holds",
        )),
        Err(e) => Err(OperationError::uncertain(format!(
            "The change could not be confirmed. {e}"
        ))),
    }
}

/// The periodic keepalive; an error ends the session.
pub async fn keepalive(unit: &mut Unit) -> Result<(), String> {
    if !keepalive_due(unit.last_sent) {
        return Ok(());
    }
    let result = unit.session.keepalive().await;
    unit.last_sent = Instant::now();
    keepalive_outcome(&mut unit.answered, result)
}

/// Whether the 2-s tick sends a keepalive: not when another message went
/// out less than a period before.
fn keepalive_due(last_sent: Instant) -> bool {
    last_sent.elapsed() + KEEPALIVE_SLACK >= KEEPALIVE_PERIOD
}

/// A late keepalive answer is not a lost session: only a closed connection
/// or `UNANSWERED_LIMIT` without any answer ends it. (A busy 8400 answered
/// one keepalive 7.9 s late and went on.)
fn keepalive_outcome(answered: &mut Instant, result: Result<(), String>) -> Result<(), String> {
    match result {
        Ok(()) => {
            *answered = Instant::now();
            Ok(())
        }
        Err(error) if crate::is_definite_disconnect(&error) => Err(error),
        Err(_) if answered.elapsed() < UNANSWERED_LIMIT => Ok(()),
        Err(error) => Err(format!(
            "The processor has not answered for {} seconds. {error}",
            UNANSWERED_LIMIT.as_secs()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ago(seconds: u64) -> Instant {
        Instant::now() - Duration::from_secs(seconds)
    }

    /// The three files of the original client's Save As (capture, 21 October
    /// 2026): one body, each with its own name and type.
    #[test]
    fn save_as_writes_the_clients_three_headers() {
        let on_air = "OptimodVersion=<00.08>\nPreset Name=<modif TEST 02> size=126\nFactory Preset Name=<ROCK-DENSE>\nPreset Type= <ON AIR PRESET>\nC:<AGC>String:<On>;D:1;\nEnd Preset\n";
        assert_eq!(
            with_header(on_air, "TEST 02", "NEXT PRESET").unwrap(),
            "OptimodVersion=<00.08>\nPreset Name=<TEST 02> size=126\nFactory Preset Name=<ROCK-DENSE>\nPreset Type= <NEXT PRESET>\nC:<AGC>String:<On>;D:1;\nEnd Preset\n"
        );
        let user = with_header(on_air, "TEST 027", "USER PRESET").unwrap();
        assert!(user.contains("Preset Name=<TEST 027> size=126\n"));
        assert!(user.contains("Preset Type= <USER PRESET>\n"));
        assert!(with_header("C:<AGC>String:<On>;D:1;\n", "X", "USER PRESET").is_err());
    }

    #[test]
    fn a_preset_cannot_take_a_state_file_name() {
        for name in ["system", "ONAIR", "Next", "test", "time"] {
            assert!(reject_state_file(name).is_err(), "{name}");
        }
        assert!(reject_state_file("SYSTEM 2").is_ok());
    }

    /// The tick after a keepalive comes a few milliseconds short of 2 s; it
    /// must still send one, as the original client does every 2 s.
    #[test]
    fn the_keepalive_goes_every_two_seconds() {
        let at = |ms| Instant::now() - Duration::from_millis(ms);
        assert!(keepalive_due(at(1_990)));
        assert!(keepalive_due(at(2_000)));
        assert!(!keepalive_due(at(1_000)));
    }

    #[test]
    fn a_late_keepalive_answer_keeps_the_session() {
        let timed_out = || Err("Receive timed out".to_owned());
        // 8 s without an answer (as the busy unit): the session stays.
        let mut answered = ago(8);
        assert!(keepalive_outcome(&mut answered, timed_out()).is_ok());
        // The late answer comes: the clock starts again.
        assert!(keepalive_outcome(&mut answered, Ok(())).is_ok());
        assert!(answered.elapsed() < Duration::from_secs(1));
        // The unit itself holds a silent session for 30 s: 25 s without an
        // answer (TCP backing off under load) is not a loss yet.
        let mut answered = ago(25);
        assert!(keepalive_outcome(&mut answered, timed_out()).is_ok());
        let mut answered = ago(34);
        assert!(keepalive_outcome(&mut answered, timed_out()).is_ok());
        let mut answered = ago(36);
        let lost = keepalive_outcome(&mut answered, timed_out()).unwrap_err();
        assert!(lost.contains("35 seconds"), "{lost}");
        // A closed connection ends it at once.
        let mut answered = Instant::now();
        assert!(keepalive_outcome(&mut answered, Err("Device disconnected".into())).is_err());
    }
}
