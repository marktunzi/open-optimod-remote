//! A user preset onto a running processor, as the original PC Remote's Restore sends one (moved from `orban-web`'s
//! `restore_preset_files`): the document sealed with the access code, uploaded as a NAMED preset file (opcode 231) - the on-air
//! processing is not touched - then confirmed by the processor's own list of user preset files. Never over a preset the processor
//! already lists (no silent overwrite), never sent twice.

use crate::recall::{FileLink, RecallError, file_document};
use std::time::Duration;
use zeroize::Zeroizing;

/// The body of a named preset-file upload: FILETIME (low, high), the file's name line where the model's file messages carry one
/// (the 2300's have none), the sealed size, the sealed document.
pub fn upload_message(name: &str, document: &str, code: &str, modified_ms: u64, file_names: bool, iv: [u8; 16]) -> Result<Vec<u8>, String> {
    let filetime = modified_ms.saturating_mul(10_000).saturating_add(116_444_736_000_000_000);
    let sealed = crate::archive::encrypt(document.as_bytes(), code, iv)?;
    let name_line = if file_names { format!("{name}\n") } else { String::new() };
    let mut m = format!("{}\n{}\n{name_line}{}\n", filetime & 0xffff_ffff, filetime >> 32, sealed.len()).into_bytes();
    m.extend(sealed);
    Ok(m)
}

/// The file name a user preset gets on this model: `NAME<user extension>`, checked as the client checks it.
pub fn file_name(link: &FileLink<'_>, preset: &str) -> Result<String, String> {
    let model = link.session.info.model;
    crate::presets::validate_store_name(preset, crate::presets::max_store_name_length(model))?;
    Ok(format!("{preset}{}", link.session.adapter().wire.user_extension))
}

/// Uploads `document` as user preset `preset` and confirms it by the processor's list (read again for up to `limit`: the unit
/// saves the file a few seconds later). Refused, with nothing sent, when the model has no named preset files, the access level does
/// not allow it, the document is not this model's, or the processor already lists that name.
pub async fn transfer(link: &mut FileLink<'_>, preset: &str, document: &str, modified_ms: u64, limit: Duration) -> Result<(), RecallError> {
    let refuse = |e: String| RecallError::Refused(e);
    let wire = link.session.adapter().wire;
    if wire.preset_file_delete.is_none() || wire.user_presets_request.is_none() {
        return Err(refuse(format!("Preset files cannot be sent to the {} over PC Remote", link.session.info.model.label())));
    }
    if !link.session.info.access().memory {
        return Err(refuse("Preset changes are not enabled for this access level".into()));
    }
    let file = file_name(link, preset).map_err(refuse)?;
    let model = link.session.info.model;
    // the document must be this model's, and name itself `preset`
    let doc = file_document(document.as_bytes(), model).map_err(|e| refuse(format!("not a {} preset: {e}", model.label())))?;
    if doc.name != preset {
        return Err(refuse(format!("the document names itself {}, not {preset}", doc.name)));
    }
    let listed = link.user_presets().await.map_err(refuse)?;
    if listed.contains_key(preset) {
        return Err(refuse(format!("The processor already has a user preset {preset}: nothing is overwritten")));
    }
    let iv = *uuid::Uuid::new_v4().as_bytes();
    let code = link.code().to_owned();
    let message = Zeroizing::new(upload_message(&file, document, &code, modified_ms, wire.file_names, iv).map_err(refuse)?);
    link.session.send(231, &message).await
        .map_err(|e| RecallError::Uncertain(format!("{preset} was sent, its outcome is unknown: {e}")))?;
    if wire.restore_answered {
        link.expect_boundary();
    }
    let end = tokio::time::Instant::now() + limit;
    loop {
        match link.user_presets().await {
            Ok(u) if u.contains_key(preset) => return Ok(()),
            Ok(_) | Err(_) if tokio::time::Instant::now() < end => tokio::time::sleep(Duration::from_secs(1)).await,
            Ok(_) => return Err(RecallError::Uncertain(format!("The processor's preset list does not show {preset}"))),
            Err(e) => return Err(RecallError::Uncertain(format!("{preset} could not be confirmed: {e}"))),
        }
    }
}
