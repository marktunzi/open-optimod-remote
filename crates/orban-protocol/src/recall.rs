//! Preset Recall as the original PC Remote does it, usable without the web
//! app (moved from `orban-web`'s `recall_with_files`).
//!
//! * File-sync models (6300, 2300, 5500, 5500i, 8700i, 8700HD): a user
//!   preset's document is sent back as an unnamed on-air file (229, sealed
//!   with the access code). A factory preset is recalled by the firmware
//!   itself over the status port (`RP`); without a status port, the factory
//!   file of the original installation is uploaded instead, where the
//!   adapter carries one.
//! * Every route ends with a readback of the on-air document until its name
//!   (and, for an uploaded document, its fields) match. A send that timed
//!   out is never repeated: its outcome is unknown, only read back.
//!
//! `orban-web` multiplexes meters and pushes on its one session and keeps
//! its own event loop; it shares the decisions here (`file_document`,
//! `upload_body`, `factory_text`, `confirms`). `FileLink` is the same
//! exchange on a session this module owns, for callers without that loop
//! (the Virtual OPTIMOD engine's helper).

use crate::{
    adapter::DeviceModel,
    document::Document,
    message::Message,
    presets::PresetKind,
    session::Session,
};
use std::{collections::BTreeMap, net::SocketAddr, time::Duration};
use zeroize::Zeroizing;

/// How long a file-sync model gets to save the recalled document (it writes
/// its on-air file a few seconds after a change), and how often it is read.
pub const FILE_READBACK_LIMIT: Duration = Duration::from_secs(15);
pub const FILE_READBACK_INTERVAL: Duration = Duration::from_secs(1);
/// The status port's recall exchange.
pub const RECALL_DEADLINE: Duration = Duration::from_secs(10);

/// Why a Recall did not end confirmed.
#[derive(Debug, PartialEq)]
pub enum RecallError {
    /// Nothing was sent to the processor.
    Refused(String),
    /// Something was sent; the processor did not confirm it. Never resend:
    /// read back.
    Uncertain(String),
}

impl std::fmt::Display for RecallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(s) | Self::Uncertain(s) => f.write_str(s),
        }
    }
}

/// A file of a file-sync model as a document. The 8700HD saves a preset
/// that a Recall sent with the LF line ends of that upload (the original
/// PC Remote's too); its own files end lines with CR LF.
pub fn file_document(plain: &[u8], model: DeviceModel) -> Result<Document, String> {
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

/// The body of an unnamed on-air upload (229), sealed with the access code.
/// The 2300's upload has no name line at all.
pub fn upload_body(text: &str, code: &str, file_names: bool, iv: [u8; 16]) -> Result<Vec<u8>, String> {
    let sealed = crate::archive::encrypt(text.as_bytes(), code, iv)?;
    let mut body = if file_names {
        format!("0\n0\n\n{}\n", sealed.len())
    } else {
        format!("0\n0\n{}\n", sealed.len())
    }
    .into_bytes();
    body.extend(sealed);
    Ok(body)
}

/// The factory file of `name` the original PC Remote would upload, for the
/// unit's pre-emphasis (system setting `PRE-E`: `50us` or `75us`).
pub fn factory_text(files: &str, name: &str, emphasis: &str) -> Result<String, String> {
    let files: serde_json::Value = serde_json::from_str(files).map_err(|e| e.to_string())?;
    files[name][emphasis]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("No factory file for {name} at {emphasis}"))
}

/// Whether `document`, read back, is the recalled preset: its name, and for
/// an uploaded document its fields. A document of an older version is
/// converted by the firmware when it loads it (2300: presets saved as
/// 2300.10 run as 2300.20); then only the name can confirm it.
pub fn confirms(on_air_name: &str, expected: Option<&Document>, document: &Document) -> bool {
    let version = |d: &Document| d.raw.lines().next().map(str::to_owned);
    document.name == on_air_name
        && expected.is_none_or(|e| version(e) != version(document) || e.fields == document.fields)
}

/// How a preset is recalled on this session's model.
#[derive(Debug, PartialEq)]
pub enum Route {
    /// Its document goes back as the on-air file.
    UploadUser,
    /// The original installation's factory file goes up as the on-air file.
    UploadFactory,
    /// The firmware recalls it itself: terminal `RP`.
    StatusPort,
}

/// The route of `kind` on a model, or why it cannot be recalled over the
/// network. `status_port`: the unit's status (terminal) port answers.
pub fn route(session: &Session, kind: &PresetKind, status_port: bool) -> Result<Route, String> {
    let wire = session.adapter().wire;
    // A status port counts only on a model that has one (the 2300 has none, whatever answers there)
    let status_port = status_port && wire.factory_recall;
    if !session.info.capabilities.preset_recall {
        return Err(format!("Preset recall is not enabled for {}", session.info.model.label()));
    }
    if !session.info.access().recall {
        return Err("Preset recall is not enabled for this access level".into());
    }
    match kind {
        PresetKind::User if wire.file_sync || !status_port => Ok(Route::UploadUser),
        PresetKind::User => Ok(Route::StatusPort),
        PresetKind::Factory if wire.factory_files.is_some() && !status_port => Ok(Route::UploadFactory),
        PresetKind::Factory if !wire.factory_recall || !status_port => Err(format!(
            "A factory preset cannot be recalled over the network on the {}: the original PC Remote converts its own copy of the factory file, and the unit has no status port. Recall it on the front panel, or recall a user preset.",
            session.info.model.label()
        )),
        PresetKind::Factory => Ok(Route::StatusPort),
        PresetKind::Unsaved => Err("Select a saved preset".into()),
    }
}

/// A PC Remote session used for files only: requests, the boundary that ends
/// their answers, and the on-air uploads. Meters are switched off around each
/// exchange, as the original client does.
pub struct FileLink<'a> {
    pub session: &'a mut Session,
    code: Zeroizing<String>,
    outstanding: usize,
}

impl<'a> FileLink<'a> {
    pub fn new(session: &'a mut Session, code: &str) -> Self {
        Self { session, code: Zeroizing::new(code.to_owned()), outstanding: 0 }
    }

    /// The access code the session was opened with (files are sealed with it).
    pub fn code(&self) -> &str {
        &self.code
    }

    /// A message was sent that the unit answers with a boundary answer of its own (an upload on the 5700i / 5500i).
    pub fn expect_boundary(&mut self) {
        self.outstanding += 1;
    }

    /// Sends the boundary and reads until every boundary is answered:
    /// files of opcode `wanted` are collected, the on-air archive kept when
    /// `archive` is asked for.
    async fn drain(&mut self, wanted: Option<u16>, mut archive: bool) -> Result<(Vec<(String, Vec<u8>)>, Option<Vec<u8>>), String> {
        let (boundary, answer) = self.session.adapter().wire.boundary;
        let file_names = self.session.adapter().wire.file_names;
        let file_end = self.session.adapter().wire.file_end.is_some();
        self.session.send(boundary, b"").await?;
        self.outstanding += 1;
        let (mut files, mut got) = (Vec::new(), None);
        for _ in 0..4096 {
            match self.session.next_event().await? {
                Message::Other { kind, .. } if kind == answer => {
                    self.outstanding = self.outstanding.saturating_sub(1);
                    if self.outstanding == 0 {
                        return Ok((files, got));
                    }
                }
                Message::File { kind, name, data } if Some(kind) == wanted => files.push((name, data)),
                Message::File { kind: 229, name, data } if archive && (!name.is_empty() || !file_names) => {
                    archive = false;
                    got = Some(data);
                }
                Message::Truncated { kind: 229, .. } if archive => archive = false,
                // A pushed on-air file: its end marker arrives as one more boundary answer.
                Message::File { kind: 229, .. } if file_end => self.outstanding += 1,
                _ => {}
            }
        }
        Err("Too many device events before response boundary".into())
    }

    async fn exchange(&mut self, request: u16, wanted: Option<u16>, archive: bool) -> Result<(Vec<(String, Vec<u8>)>, Option<Vec<u8>>), String> {
        let meters = self.session.info.capabilities.live_meters;
        if meters {
            self.session.send(223, b"").await?;
        }
        let read = match self.session.send(request, b"").await {
            Ok(()) => self.drain(wanted, archive).await,
            Err(e) => Err(e),
        };
        let resumed = if meters { self.session.send(222, b"").await } else { Ok(()) };
        let read = read?;
        resumed?;
        Ok(read)
    }

    /// The on-air processing document over the PC Remote session.
    pub async fn on_air(&mut self) -> Result<Document, String> {
        let request = self.session.adapter().wire.on_air_request;
        let (_, data) = self.exchange(request, None, true).await?;
        let data = data.ok_or("The processor did not send its on-air document")?;
        let plain = Zeroizing::new(self.session.decrypt_file(&data)?);
        let model = self.session.info.model;
        if self.session.adapter().wire.file_sync {
            file_document(&plain, model)
        } else {
            Document::parse_for_model(&plain, model)
        }
    }

    /// The user presets the unit sends as files, by the name in their
    /// document; the firmware's `modif` working copies are left out.
    pub async fn user_presets(&mut self) -> Result<BTreeMap<String, Zeroizing<Vec<u8>>>, String> {
        let wire = self.session.adapter().wire;
        let Some((request, reply)) = wire.user_presets_request else { return Ok(BTreeMap::new()) };
        let model = self.session.info.model;
        let mut out = BTreeMap::new();
        for (name, data) in self.exchange(request, Some(reply), false).await?.0 {
            let plain = Zeroizing::new(self.session.decrypt_file(&data)?);
            let name = if name.is_empty() { preset_file_name(&plain).unwrap_or_default() } else { name };
            if !name.to_ascii_lowercase().ends_with(wire.user_extension) {
                continue;
            }
            let Ok(document) = file_document(&plain, model) else { continue };
            if document.name.to_ascii_lowercase().starts_with("modif ") {
                continue;
            }
            out.insert(document.name.clone(), plain);
        }
        Ok(out)
    }

    /// The system setting `name` (setup file), where the model sends one.
    pub async fn system_value(&mut self, name: &str) -> Result<Option<String>, String> {
        let Some((request, reply)) = self.session.adapter().wire.setup_request else { return Ok(None) };
        let Some((_, data)) = self.exchange(request, Some(reply), false).await?.0.into_iter().next() else { return Ok(None) };
        let plain = Zeroizing::new(self.session.decrypt_file(&data)?);
        let doc = file_document(&plain, self.session.info.model)?;
        Ok(doc.fields.get(name).and_then(|f| match &f.value {
            crate::document::Value::Choice(t) | crate::document::Value::Text(t) => Some(t.clone()),
            _ => None,
        }))
    }

    /// Puts `text` on air: an unnamed on-air file. No retry.
    pub async fn upload_on_air(&mut self, text: &str) -> Result<(), String> {
        let wire = self.session.adapter().wire;
        let iv = *uuid::Uuid::new_v4().as_bytes();
        let body = upload_body(text, &self.code, wire.file_names, iv)?;
        self.session.send(229, &body).await?;
        if let Some(end) = wire.file_end {
            self.session.send(end, b"").await?;
            self.outstanding += 1;
        }
        Ok(())
    }

    /// Reads the on-air document until `accept` holds or the limit passes;
    /// the last document read is returned either way.
    pub async fn read_back(&mut self, accept: impl Fn(&Document) -> bool, limit: Duration) -> Result<Document, String> {
        let end = tokio::time::Instant::now() + limit;
        loop {
            let read = self.on_air().await;
            let last = tokio::time::Instant::now() + FILE_READBACK_INTERVAL > end;
            match read {
                Ok(d) if accept(&d) || last => return Ok(d),
                Err(e) if last => return Err(e),
                _ => tokio::time::sleep(FILE_READBACK_INTERVAL).await,
            }
        }
    }
}

/// The `Preset File Name` a document gives itself (the 2300 sends no file names).
pub fn preset_file_name(plain: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(plain).ok()?;
    let line = text.lines().find_map(|l| l.strip_prefix("Preset File Name= <"))?;
    Some(line.strip_suffix('>')?.to_owned())
}

/// One Recall, confirmed by the on-air document read back. `terminal`: the
/// unit's status port, when it answers. Nothing is sent twice.
pub async fn recall(
    link: &mut FileLink<'_>,
    terminal: Option<SocketAddr>,
    name: &str,
    kind: &PresetKind,
) -> Result<Document, RecallError> {
    crate::presets::validate_name(name).map_err(RecallError::Refused)?;
    let route = route(link.session, kind, terminal.is_some()).map_err(RecallError::Refused)?;
    let model = link.session.info.model;
    let expected = match route {
        Route::UploadUser => {
            let users = link.user_presets().await.map_err(RecallError::Refused)?;
            let plain = users.get(name).ok_or_else(|| RecallError::Refused(format!("The processor lists no user preset {name}")))?;
            let text = Zeroizing::new(String::from_utf8_lossy(plain).replace("\r\n", "\n"));
            let doc = file_document(plain, model).map_err(RecallError::Refused)?;
            link.upload_on_air(&text).await.map_err(|e| RecallError::Uncertain(format!("The recall of {name} was sent, its outcome is unknown: {e}")))?;
            Some(doc)
        }
        Route::UploadFactory => {
            let emphasis = link.system_value("PRE-E").await.map_err(RecallError::Refused)?
                .ok_or_else(|| RecallError::Refused("The pre-emphasis setting is not available".into()))?;
            let files = link.session.adapter().wire.factory_files.unwrap_or("{}");
            let text = factory_text(files, name, &emphasis).map_err(RecallError::Refused)?;
            let doc = file_document(text.as_bytes(), model).map_err(RecallError::Refused)?;
            link.upload_on_air(&text).await.map_err(|e| RecallError::Uncertain(format!("The recall of {name} was sent, its outcome is unknown: {e}")))?;
            Some(doc)
        }
        Route::StatusPort => {
            let code = link.code.clone();
            let terminal = terminal.ok_or_else(|| RecallError::Refused("no status port".into()))?;
            crate::terminal::send_recall_for_model(terminal, &code, name, RECALL_DEADLINE, model)
                .await
                .map_err(|e| RecallError::Uncertain(format!("The recall of {name} could not be sent completely: {e}")))?;
            None
        }
    };
    let on_air_name = expected.as_ref().map_or(name, |d| d.name.as_str()).to_owned();
    let confirmed = link
        .read_back(|d| confirms(&on_air_name, expected.as_ref(), d), FILE_READBACK_LIMIT)
        .await
        .map_err(|e| RecallError::Uncertain(format!("The recall of {name} could not be confirmed: {e}")))?;
    if !confirms(&on_air_name, expected.as_ref(), &confirmed) {
        return Err(RecallError::Uncertain(format!("The processor did not confirm the recall of {name}; it reports {} on air", confirmed.name)));
    }
    Ok(confirmed)
}

/// The 8400 (its own PC Remote session, `crate::o8400`): a Recall is the
/// client's PEG message 0x400B with the preset's name; the unit writes the
/// recalled preset into its on-air file within a few seconds.
pub mod o8400 {
    use super::RecallError;
    use crate::{adapter::DeviceModel, document::Document, o8400::{self, Session}};
    use std::time::Duration;

    pub const RECALL_EVENT: u16 = 0x400B;
    pub const RECALL_LIMIT: Duration = Duration::from_secs(15);

    /// A file of the unit's user folder as a document.
    pub fn document(file: &[u8]) -> Result<Document, String> {
        Document::parse_for_model(o8400::document_text(file)?, DeviceModel::Optimod8400)
    }

    pub async fn on_air(session: &mut Session) -> Result<Document, String> {
        document(&session.read(o8400::USER_DIR, "onair.orb").await?)
    }

    /// The factory folder for the unit's pre-emphasis (system control PRE-E), as the original client chooses it.
    pub fn factory_folder(system: &Document) -> &'static str {
        match system.fields.get("PRE-E").map(|f| &f.value) {
            Some(crate::document::Value::Choice(v)) if v == "50us" => "c:\\8400\\presets\\50us presets\\",
            _ => "c:\\8400\\presets\\75us presets\\",
        }
    }

    /// Factory and user preset names as the unit lists its files.
    pub async fn presets(session: &mut Session) -> Result<(Vec<String>, Vec<String>), String> {
        let system = document(&session.read(o8400::USER_DIR, "system.orb").await?)?;
        let strip = |f: &str| f.strip_suffix(".ORB").or_else(|| f.strip_suffix(".orb")).map(str::to_owned);
        let factory = session.list(factory_folder(&system), "*.orb").await?.iter().filter_map(|f| strip(f)).collect();
        const STATE: [&str; 5] = ["NEXT.ORB", "ONAIR.ORB", "SYSTEM.ORB", "TEST.ORB", "TIME.ORB"];
        let user = session.list(o8400::USER_DIR, "*.orb").await?.iter()
            .filter(|f| !STATE.contains(&f.to_ascii_uppercase().as_str())).filter_map(|f| strip(f)).collect();
        Ok((factory, user))
    }

    /// Reads the on-air file until it names `name` or the limit passes; the last one read is returned.
    pub async fn read_back(session: &mut Session, name: &str, limit: Duration) -> Result<Document, String> {
        let end = tokio::time::Instant::now() + limit;
        loop {
            let d = on_air(session).await?;
            if d.name == name || tokio::time::Instant::now() >= end {
                return Ok(d);
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    /// One Recall, confirmed by the on-air file. Sent once.
    pub async fn recall(session: &mut Session, name: &str) -> Result<Document, RecallError> {
        crate::presets::validate_name(name).map_err(RecallError::Refused)?;
        if session.login.access_level != 0 {
            return Err(RecallError::Refused("Preset recall is not enabled for this access level".into()));
        }
        let (factory, user) = presets(session).await.map_err(RecallError::Refused)?;
        if !factory.iter().chain(user.iter()).any(|p| p == name) {
            return Err(RecallError::Refused(format!("The processor lists no preset {name}")));
        }
        session.event(RECALL_EVENT, 0, name).await
            .map_err(|e| RecallError::Uncertain(format!("The recall of {name} was sent, its outcome is unknown: {e}")))?;
        let d = read_back(session, name, RECALL_LIMIT).await
            .map_err(|e| RecallError::Uncertain(format!("The recall of {name} could not be confirmed: {e}")))?;
        if d.name != name {
            return Err(RecallError::Uncertain(format!("The processor did not confirm the recall of {name}; it reports {} on air", d.name)));
        }
        Ok(d)
    }
}
