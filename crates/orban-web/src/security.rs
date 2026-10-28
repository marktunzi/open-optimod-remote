//! The 5700i passcode file (`Password.orb57pas`) as the original PC Remote
//! 3.0.1.20 writes it: one line per passcode with its access level
//! (captured 28 October 2026). Passcodes never go to logs.
use serde::{Deserialize, Serialize};

/// The client's access levels; the index is the level number.
pub const LEVELS: [&str; 6] = [
    "All screens",
    "All screens except Security",
    "All screens except Modify and Security",
    "Presets, Modify, Save, Memory and Automation",
    "Presets and Automation",
    "Presets",
];

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Passcode {
    pub code: String,
    pub level: String,
}

pub fn parse(text: &str) -> Result<Vec<Passcode>, String> {
    text.lines()
        .filter_map(|line| line.trim_end().strip_prefix("C:<"))
        .map(|rest| {
            let (code, rest) = rest
                .split_once(">String:<")
                .ok_or("Invalid passcode line")?;
            let (level, _) = rest.split_once(">;D:").ok_or("Invalid passcode line")?;
            Ok(Passcode {
                code: code.to_owned(),
                level: level.to_owned(),
            })
        })
        .collect()
}

pub fn render(passcodes: &[Passcode]) -> Result<String, String> {
    if passcodes.is_empty() || passcodes.len() > 20 {
        return Err("Program between 1 and 20 passcodes".into());
    }
    if !passcodes.iter().any(|p| p.level == LEVELS[0]) {
        return Err("At least one passcode needs access to all screens".into());
    }
    let mut out = format!(
        "OptimodVersion=<5700.51>\nPreset Name=<Password Preset> size={}\nFactory Preset Name=<Password Preset>\nPreset Type= <PASSWORD PRESET>\nPreset File Name= <Password.orb57pas>\n",
        passcodes.len()
    );
    for (i, p) in passcodes.iter().enumerate() {
        let level = LEVELS
            .iter()
            .position(|l| *l == p.level)
            .ok_or("Unknown access level")?;
        if p.code.is_empty()
            || p.code.len() > 20
            || !p.code.bytes().all(|c| c.is_ascii_alphanumeric())
        {
            return Err("A passcode uses 1 to 20 letters and digits".into());
        }
        if passcodes[..i]
            .iter()
            .any(|q| q.code.eq_ignore_ascii_case(&p.code))
        {
            return Err("Each passcode can be programmed once".into());
        }
        out.push_str(&format!("C:<{}>String:<{}>;D:{level};\n", p.code, p.level));
    }
    out.push_str("End Preset<end>\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    /// The file the original client uploaded with a second passcode at level 5.
    const CAPTURED: &str = "OptimodVersion=<5700.51>\nPreset Name=<Password Preset> size=2\nFactory Preset Name=<Password Preset>\nPreset Type= <PASSWORD PRESET>\nPreset File Name= <Password.orb57pas>\nC:<1234>String:<All screens>;D:0;\nC:<OOR9>String:<Presets>;D:5;\nEnd Preset<end>\n";

    #[test]
    fn the_captured_file_parses_and_renders_byte_for_byte() {
        let codes = super::parse(CAPTURED).unwrap();
        assert_eq!(codes.len(), 2);
        assert_eq!(super::render(&codes).unwrap(), CAPTURED);
        assert!(
            super::render(&codes[1..]).is_err(),
            "a full-access passcode must stay"
        );
        assert!(super::render(&[]).is_err());
    }
}
