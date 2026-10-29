//! The OPTIMOD-FM 8400 session of the service: the 8400's own PC Remote
//! protocol (`orban_protocol::o8400`) instead of the PC Remote family's
//! opcodes and terminal. Settings and presets are files on the unit, read
//! over the session's file calls as the original client reads them; a change
//! is confirmed by reading the file the unit saves it to; meters arrive over
//! UDP. See docs/protocol/8400.md.

use crate::{
    App, BackupFile, Change, Connect, OperationError, PresetFile, PresetFileAction, Recall,
    RestorePresetFiles, Snapshot, UnitFileWrite, ensure_session, refresh_processing_identity,
    refreshed_less_more_availability,
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

/// A restored preset keeps the name its file gives itself, spaces at the
/// end included (the unit holds `EURO AC .orb`); it must still be a plain
/// file name within the user folder.
fn restorable_name(name: &str) -> Result<(), String> {
    use orban_protocol::presets::{max_store_name_length, validate_name};
    validate_name(name)?;
    if name.len() > max_store_name_length(DeviceModel::Optimod8400) || name.trim().is_empty() {
        return Err("Unsupported preset name".into());
    }
    if name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']) || name.trim() == ".." {
        return Err("A preset name cannot contain \\ / : * ? \" < > or |".into());
    }
    if name.to_ascii_lowercase().starts_with("modif ") {
        return Err("A preset name cannot start with \"modif \"".into());
    }
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

/// A user preset was added (GUI event, after writing its file).
const ADDED_EVENT: u16 = 16391;

/// The original client's Copy All on its Memory screen (capture, 29 October
/// 2026): every user preset as `<Preset Name>.orb` in its `user` folder,
/// with Windows line ends. The client writes them from its own copy of the
/// presets, which adds controls an older file lacks; the app keeps each
/// file as the unit stores it.
pub async fn backup_files(unit: &mut Unit) -> Result<Vec<BackupFile>, String> {
    if unit.session.login.access_level != 0 {
        return Err("Backup is not available for this access level".into());
    }
    let mut presets: Vec<(String, String)> = unit
        .user_files
        .iter()
        .map(|(name, file)| (name.clone(), file.clone()))
        .collect();
    presets.sort();
    let mut files = Vec::new();
    for (name, file) in presets {
        let read = unit.session.read(o8400::USER_DIR, &file).await?;
        unit.last_sent = Instant::now();
        let text = std::str::from_utf8(o8400::document_text(&read)?)
            .map_err(|_| "Unsupported document encoding")?;
        Document::parse_for_model(text.as_bytes(), DeviceModel::Optimod8400)?;
        files.push(BackupFile {
            name: format!("{name}.orb"),
            document: text.replace("\r\n", "\n").replace('\n', "\r\n"),
            modified: None,
        });
    }
    Ok(files)
}

/// The original client's Copy from its PC list to the unit (capture, 29
/// October 2026): the document with Unix line ends as `<Preset Name>.orb`
/// through the write calls, then the added event with the name. A file
/// that would take a factory preset's or one of the unit's own names is
/// refused; nothing is written before every file is checked.
pub async fn restore_preset_files(
    unit: &mut Unit,
    app: &App,
    body: RestorePresetFiles,
) -> Result<(), OperationError> {
    if !unit.capabilities.preset_store || unit.session.login.access_level != 0 {
        return Err(OperationError::safe(
            "Preset changes are not enabled for this access level",
        ));
    }
    if !body.confirmed {
        return Err(OperationError::safe("Confirm the restore first"));
    }
    let presets = {
        let state = app.state.read().await;
        ensure_session(&state, &body.expected_session).map_err(OperationError::safe)?;
        if state.host != body.expected_host {
            return Err(OperationError::safe(
                "The active device changed. Review the connection first",
            ));
        }
        state.presets.clone()
    };
    let mut wanted = Vec::new();
    for file in &body.files {
        if !file.name.to_ascii_lowercase().ends_with(".orb") || file.name.contains(['\\', '/']) {
            return Err(OperationError::safe(format!(
                "{} is not a user preset file of the OPTIMOD-FM 8400",
                file.name
            )));
        }
        let text = file.document.replace("\r\n", "\n");
        let document = Document::parse_for_model(text.as_bytes(), DeviceModel::Optimod8400)
            .map_err(|e| OperationError::safe(format!("{}: {e}", file.name)))?;
        let name = document.name;
        restorable_name(&name).map_err(|e| OperationError::safe(format!("{}: {e}", file.name)))?;
        reject_state_file(&name)?;
        if presets
            .iter()
            .any(|p| p.name == name && p.kind == PresetKind::Factory)
        {
            return Err(OperationError::safe(format!(
                "{name} is a factory preset name"
            )));
        }
        if text.len() + 64 > 4096 {
            return Err(OperationError::safe(format!(
                "{} is too long for the processor",
                file.name
            )));
        }
        wanted.push((name, text));
    }
    for (name, text) in &wanted {
        unit.session
            .write(o8400::USER_DIR, &format!("{name}.orb"), text.as_bytes())
            .await
            .map_err(OperationError::uncertain)?;
        unit.session
            .event(ADDED_EVENT, 18, name)
            .await
            .map_err(OperationError::uncertain)?;
        unit.last_sent = Instant::now();
    }
    let system = app.state.read().await.system.clone();
    let listed = match system {
        Some(system) => read_presets(unit, &system).await,
        None => Err("The system settings are not available".into()),
    }
    .map_err(|e| {
        OperationError::uncertain(format!(
            "The restore could not be confirmed. Refresh the preset list. {e}"
        ))
    })?;
    let missing: Vec<&str> = wanted
        .iter()
        .filter(|(name, _)| !listed.iter().any(|p| &p.name == name))
        .map(|(name, _)| name.as_str())
        .collect();
    app.state.write().await.presets = listed;
    if !missing.is_empty() {
        return Err(OperationError::uncertain(format!(
            "The processor does not list {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

/// Passcodes changed: the unit re-reads `password.pas` (GUI event).
const PASSCODES_EVENT: u16 = 16390;

/// The unit's passcode file (`kind` 230), or its automation events (235):
/// every `AUTO_<n>.aut` in file order, one document after the other.
pub async fn read_unit_file(unit: &mut Unit, kind: u16) -> Result<Option<String>, String> {
    // The app gives the 8400's tools to the All Screens level only, as its
    // writes (`orban_protocol::access`).
    if kind == 235 && unit.session.login.access_level != 0 {
        return Err("Automation is not available for this access level".into());
    }
    if kind == 235 {
        let files = automation_files(unit).await?;
        if files.is_empty() {
            return Ok(None);
        }
        return Ok(Some(files.into_iter().map(|(_, text)| text).collect()));
    }
    if kind != 230 {
        return Err("This file is not available for the OPTIMOD-FM 8400".into());
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
    if body.kind != 230 && body.kind != 235 {
        return Err(OperationError::safe(
            "This file is not available for the OPTIMOD-FM 8400",
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
    if body.kind == 235 {
        return write_automation(unit, app, &body.document).await;
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

/// The automation files event (GUI event): after an `AUTO_<n>.aut` was
/// written or removed, with its file name (original client, 29 October
/// 2026; the client sends 784 as data).
const AUTOMATION_EVENT: u16 = 16389;
/// The client's Add allows 64 events (`8400_pc.exe` 0x401dcf).
const MAX_AUTOMATION_EVENTS: usize = 64;

/// `AUTO_<n>.aut` in `c:\\8400\\user\\` by number, with each file's text.
async fn automation_files(unit: &mut Unit) -> Result<Vec<(String, String)>, String> {
    let mut names: Vec<(u32, String)> = unit
        .session
        .list(o8400::USER_DIR, "*.aut")
        .await?
        .into_iter()
        .filter_map(|name| {
            let number = name
                .to_ascii_uppercase()
                .strip_prefix("AUTO_")?
                .strip_suffix(".AUT")?
                .parse()
                .ok()?;
            Some((number, name))
        })
        .collect();
    names.sort();
    let mut files = Vec::new();
    for (_, name) in names {
        let file = unit.session.read(o8400::USER_DIR, &name).await?;
        unit.last_sent = Instant::now();
        let text = String::from_utf8(o8400::document_text(&file)?.to_vec())
            .map_err(|_| "Unsupported document encoding")?;
        files.push((name, text.replace("\r\n", "\n")));
    }
    Ok(files)
}

/// The original client's Event Type list: No Function, the presets as the
/// unit lists them, then its functions.
fn action_index(action: &str, presets: &[Preset]) -> Option<usize> {
    if action.is_empty() {
        return Some(0);
    }
    presets
        .iter()
        .map(|p| p.name.as_str())
        .chain(crate::automation::FUNCTIONS_8400)
        .position(|name| name == action)
        .map(|at| at + 1)
}

/// Makes the unit's events the given ones, as the client's Add, Edit and
/// Delete do: each changed `AUTO_<n>.aut` is written (Unix line ends, the
/// write calls), each one no longer wanted is removed, and the automation
/// event follows each. Read back afterwards.
async fn write_automation(
    unit: &mut Unit,
    app: &App,
    document: &str,
) -> Result<(), OperationError> {
    let events = crate::automation::parse(document).map_err(OperationError::safe)?;
    if events.len() > MAX_AUTOMATION_EVENTS {
        return Err(OperationError::safe(format!(
            "At most {MAX_AUTOMATION_EVENTS} automation events"
        )));
    }
    let presets: Vec<Preset> = app
        .state
        .read()
        .await
        .presets
        .iter()
        .filter(|p| p.kind != PresetKind::Unsaved)
        .cloned()
        .collect();
    let mut wanted = Vec::new();
    for (index, event) in events.iter().enumerate() {
        let action = action_index(&event.action, &presets).ok_or_else(|| {
            OperationError::safe(format!(
                "{} is not one of the processor's presets or actions",
                event.action
            ))
        })?;
        let text =
            crate::automation::render_8400(event, index, action).map_err(OperationError::safe)?;
        wanted.push((format!("AUTO_{index}.aut"), text));
    }
    let current = automation_files(unit)
        .await
        .map_err(OperationError::uncertain)?;
    for (name, text) in &wanted {
        let same = current
            .iter()
            .any(|(file, held)| file.eq_ignore_ascii_case(name) && held == text);
        if same {
            continue;
        }
        unit.session
            .write(o8400::USER_DIR, name, text.as_bytes())
            .await
            .map_err(OperationError::uncertain)?;
        unit.session
            .event(AUTOMATION_EVENT, 784, name)
            .await
            .map_err(OperationError::uncertain)?;
        unit.last_sent = Instant::now();
    }
    for (file, _) in &current {
        if wanted
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(file))
        {
            continue;
        }
        unit.session
            .remove(o8400::USER_DIR, file)
            .await
            .map_err(OperationError::uncertain)?;
        unit.session
            .event(AUTOMATION_EVENT, 784, file)
            .await
            .map_err(OperationError::uncertain)?;
        unit.last_sent = Instant::now();
    }
    let held: String = automation_files(unit)
        .await
        .map_err(|e| OperationError::uncertain(format!("The change could not be confirmed. {e}")))?
        .into_iter()
        .map(|(_, text)| text)
        .collect();
    match crate::automation::parse(&held) {
        Ok(read) if read == events => Ok(()),
        _ => Err(OperationError::uncertain(
            "The processor's files do not show the change. Open the window again to see what it holds",
        )),
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

    /// The client's Event Type list: No Function, the presets as listed,
    /// then Bypass and Exit Bypass (its CLASSICAL-2 BAND, the first factory
    /// preset, was index 1 in the captured `AUTO_0.aut`).
    #[test]
    fn automation_actions_count_as_the_clients_list() {
        let preset = |name: &str, kind| Preset {
            name: name.into(),
            kind,
        };
        let presets = [
            preset("CLASSICAL-2 BAND", PresetKind::Factory),
            preset("ROCK-DENSE", PresetKind::Factory),
            preset("TEST 02", PresetKind::User),
        ];
        assert_eq!(action_index("", &presets), Some(0));
        assert_eq!(action_index("CLASSICAL-2 BAND", &presets), Some(1));
        assert_eq!(action_index("TEST 02", &presets), Some(3));
        assert_eq!(action_index("Exit Bypass", &presets), Some(5));
        assert_eq!(action_index("Exit Test", &presets), None);
    }

    #[test]
    fn a_restored_preset_keeps_its_own_name_inside_the_user_folder() {
        assert!(restorable_name("EURO AC ").is_ok());
        assert!(restorable_name("TEST 02").is_ok());
        for bad in [
            "..\\system",
            "a/b",
            "c:x",
            "  ",
            "",
            "modif TEST",
            "x".repeat(21).as_str(),
        ] {
            assert!(restorable_name(bad).is_err(), "{bad:?}");
        }
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
