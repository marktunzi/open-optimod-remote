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
