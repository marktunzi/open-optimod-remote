//! The passcode file (`Password.orb57pas`, `Password.orb55pas`) as the
//! original PC Remote writes it: one line per passcode with its access level
//! (5700i 3.0.1.20 and 5500i 3.1.0.2, 28 October 2026; `5500PC.exe` 1.2.8.24
//! has the 5500i's level names and writes the same 5500.50 file). Passcodes
//! never go to logs.
use orban_protocol::adapter::DeviceModel;
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

/// The 5500i client's level names, which are also what its file holds.
pub const LEVELS_5500I: [&str; 6] = [
    "all access",
    "all access except security",
    "all screens except modify and security",
    "recall, modify & automation",
    "recall presets program automation",
    "recall presets",
];

/// The model's level names, its document version and its file extension.
pub fn flavour(model: DeviceModel) -> (&'static [&'static str; 6], &'static str, &'static str) {
    match model {
        DeviceModel::Optimod5500i | DeviceModel::Optimod5500 => {
            (&LEVELS_5500I, "5500.50", "orb55pas")
        }
        // `8700PC.exe` 1.0.2.161 has the 5700i's level names; the 8700 document version.
        DeviceModel::Optimod8700Hd => (&LEVELS, "8700.51", "orb86pas"),
        _ => (&LEVELS, "5700.51", "orb57pas"),
    }
}

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

pub fn render(passcodes: &[Passcode], model: DeviceModel) -> Result<String, String> {
    let (levels, version, extension) = flavour(model);
    if passcodes.is_empty() || passcodes.len() > 20 {
        return Err("Program between 1 and 20 passcodes".into());
    }
    if !passcodes.iter().any(|p| p.level == levels[0]) {
        return Err("At least one passcode needs access to all screens".into());
    }
    let mut out = format!(
        "OptimodVersion=<{version}>\nPreset Name=<Password Preset> size={}\nFactory Preset Name=<Password Preset>\nPreset Type= <PASSWORD PRESET>\nPreset File Name= <Password.{extension}>\n",
        passcodes.len()
    );
    for (i, p) in passcodes.iter().enumerate() {
        let level = levels
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
        assert_eq!(
            super::render(&codes, super::DeviceModel::Optimod5700i).unwrap(),
            CAPTURED
        );
        assert!(
            super::render(&codes[1..], super::DeviceModel::Optimod5700i).is_err(),
            "a full-access passcode must stay"
        );
        assert!(super::render(&[], super::DeviceModel::Optimod5700i).is_err());
    }

    /// The 5500i's own file, as its firmware keeps it.
    #[test]
    fn the_5500i_file_parses_and_renders_byte_for_byte() {
        const FILE: &str = "OptimodVersion=<5500.50>\nPreset Name=<Password Preset> size=1\nFactory Preset Name=<Password Preset>\nPreset Type= <PASSWORD PRESET>\nPreset File Name= <Password.orb55pas>\nC:<1234>String:<all access>;D:0;\nEnd Preset<end>\n";
        let codes = super::parse(FILE).unwrap();
        assert_eq!(
            super::render(&codes, super::DeviceModel::Optimod5500i).unwrap(),
            FILE
        );
    }
}
