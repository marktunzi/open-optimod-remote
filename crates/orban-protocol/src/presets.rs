use serde::Serialize;
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum PresetKind {
    Factory,
    User,
    Unsaved,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Preset {
    pub name: String,
    pub kind: PresetKind,
}
pub fn parse_list(text: &str) -> Result<Vec<Preset>, String> {
    if text.len() > 65535 {
        return Err("Preset list exceeds the supported size".into());
    }
    let mut presets = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (name, kind) = line.rsplit_once(' ').ok_or("Malformed preset list")?;
        let kind = match kind {
            "factory" => PresetKind::Factory,
            "user" => PresetKind::User,
            "unsaved" => PresetKind::Unsaved,
            _ => return Err("Unknown preset category".into()),
        };
        validate_name(name)?;
        if presets.iter().any(|p: &Preset| p.name == name) {
            return Err("Ambiguous duplicate preset name".into());
        }
        presets.push(Preset {
            name: name.into(),
            kind,
        });
        if presets.len() > 1024 {
            return Err("Too many presets".into());
        }
    }
    if presets.is_empty() {
        return Err("Device returned no presets".into());
    }
    Ok(presets)
}
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 80
        || !name.is_ascii()
        || name
            .bytes()
            .any(|c| c < 32 || c == 127 || c == b'[' || c == b']')
    {
        return Err("Unsupported preset name".into());
    }
    Ok(())
}
pub fn recall_command(name: &str, code: &str) -> Result<zeroize::Zeroizing<String>, String> {
    validate_name(name)?;
    let _ = zeroize::Zeroizing::new(crate::auth::login_request(code)?);
    Ok(zeroize::Zeroizing::new(format!(
        "RP {name}[{}]\r\n",
        code.to_ascii_uppercase()
    )))
}

/// Longest preset name the model's own PC Remote accepts: 18 characters for
/// the 5500, 20 for the 5700i and 8700HD.
pub fn max_store_name_length(model: crate::adapter::DeviceModel) -> usize {
    match model {
        crate::adapter::DeviceModel::Optimod5500 => 18,
        _ => 20,
    }
}

/// A name for a new user preset. The firmware reserves the `modif ` prefix for
/// the unsaved state and PC Remote trims surrounding spaces.
pub fn validate_store_name(name: &str, max: usize) -> Result<(), String> {
    validate_name(name)?;
    if name.len() > max {
        return Err(format!("A preset name can have at most {max} characters"));
    }
    if name.trim() != name {
        return Err("A preset name cannot start or end with a space".into());
    }
    if name.to_ascii_lowercase().starts_with("modif ") {
        return Err("A preset name cannot start with \"modif \"".into());
    }
    Ok(())
}

fn terminal_command(
    verb: &str,
    name: &str,
    code: &str,
) -> Result<zeroize::Zeroizing<String>, String> {
    validate_name(name)?;
    let _ = zeroize::Zeroizing::new(crate::auth::login_request(code)?);
    Ok(zeroize::Zeroizing::new(format!(
        "{verb} {name}[{}]\r\n",
        code.to_ascii_uppercase()
    )))
}

/// `SP NAME[CODE]`: save the on-air processing as a user preset.
pub fn store_command(name: &str, code: &str) -> Result<zeroize::Zeroizing<String>, String> {
    terminal_command("SP", name, code)
}

/// `DP NAME[CODE]`: delete a user preset.
pub fn delete_command(name: &str, code: &str) -> Result<zeroize::Zeroizing<String>, String> {
    terminal_command("DP", name, code)
}

/// A refusal in the processor's reply to `SP` or `DP`. These are the 5500
/// firmware's own messages; the preset list decides success.
pub fn reply_refusal(reply: &str) -> Option<String> {
    const REFUSALS: [&str; 5] = [
        "already exists",
        "A factory preset named",
        "Maximum number of characters",
        "preset does not exist",
        "Cannot delete",
    ];
    reply
        .lines()
        .map(str::trim)
        .find(|line| REFUSALS.iter().any(|refusal| line.contains(refusal)))
        .map(str::to_owned)
}
