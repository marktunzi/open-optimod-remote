//! Model identity, transport and safety capabilities for supported OPTIMODs.
//!
//! A family resemblance is not write compatibility. Every login banner maps to
//! its own adapter and visual skin. Capabilities become writable only for an
//! exact firmware/profile combination with its own parameter profile. The
//! `evidence` field records whether that profile was verified on hardware or
//! derived statically from the matching official PC Remote and firmware.
//!
//! Adding a model: add a `DeviceModel`/`SkinId`, a read-only family adapter for
//! its banner prefix and, once a profile exists, an exact-banner entry in
//! `EXACT` plus its files in `crate::profile::EMBEDDED`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceModel {
    #[default]
    Auto,
    #[serde(rename = "optimod-5700i")]
    Optimod5700i,
    #[serde(rename = "optimod-5500i")]
    Optimod5500i,
    #[serde(rename = "optimod-5500")]
    Optimod5500,
    #[serde(rename = "optimod-5700-fm")]
    Optimod5700Fm,
    #[serde(rename = "optimod-5700-hd")]
    Optimod5700Hd,
    #[serde(rename = "optimod-8500")]
    Optimod8500,
    #[serde(rename = "optimod-2300")]
    Optimod2300,
    #[serde(rename = "optimod-6300")]
    Optimod6300,
    #[serde(rename = "optimod-8400")]
    Optimod8400,
    #[serde(rename = "optimod-8600")]
    Optimod8600,
    #[serde(rename = "optimod-8700i")]
    Optimod8700i,
    #[serde(rename = "optimod-8700hd")]
    Optimod8700Hd,
    #[serde(rename = "optimod-9300")]
    Optimod9300,
    #[serde(rename = "optimod-9400")]
    Optimod9400,
}

impl DeviceModel {
    pub const SELECTABLE: [Self; 15] = [
        Self::Auto,
        Self::Optimod5700i,
        Self::Optimod5500i,
        Self::Optimod5500,
        Self::Optimod5700Fm,
        Self::Optimod5700Hd,
        Self::Optimod8500,
        Self::Optimod2300,
        Self::Optimod6300,
        Self::Optimod8400,
        Self::Optimod8600,
        Self::Optimod8700i,
        Self::Optimod8700Hd,
        Self::Optimod9300,
        Self::Optimod9400,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto Detect",
            Self::Optimod5700i => "OPTIMOD 5700i",
            Self::Optimod5500i => "OPTIMOD 5500i",
            Self::Optimod5500 => "OPTIMOD 5500",
            Self::Optimod5700Fm => "OPTIMOD 5700 FM",
            Self::Optimod5700Hd => "OPTIMOD 5700 HD",
            Self::Optimod8500 => "OPTIMOD 8500",
            Self::Optimod2300 => "OPTIMOD 2300",
            Self::Optimod6300 => "OPTIMOD 6300",
            Self::Optimod8400 => "OPTIMOD-FM 8400",
            Self::Optimod8600 => "OPTIMOD 8600",
            Self::Optimod8700i => "OPTIMOD 8700i",
            Self::Optimod8700Hd => "OPTIMOD-FM 8700HD",
            Self::Optimod9300 => "OPTIMOD 9300",
            Self::Optimod9400 => "OPTIMOD 9400",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkinId {
    #[serde(rename = "optimod-5700i")]
    Optimod5700i,
    #[serde(rename = "optimod-5500i")]
    Optimod5500i,
    #[serde(rename = "optimod-5500")]
    Optimod5500,
    #[serde(rename = "optimod-5700-fm")]
    Optimod5700Fm,
    #[serde(rename = "optimod-5700-hd")]
    Optimod5700Hd,
    #[serde(rename = "optimod-8500")]
    Optimod8500,
    #[serde(rename = "optimod-2300")]
    Optimod2300,
    #[serde(rename = "optimod-6300")]
    Optimod6300,
    #[serde(rename = "optimod-8400")]
    Optimod8400,
    #[serde(rename = "optimod-8600")]
    Optimod8600,
    #[serde(rename = "optimod-8700i")]
    Optimod8700i,
    #[serde(rename = "optimod-8700hd")]
    Optimod8700Hd,
    #[serde(rename = "optimod-9300")]
    Optimod9300,
    #[serde(rename = "optimod-9400")]
    Optimod9400,
}

/// How the parameter profile of an adapter was established.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Evidence {
    /// Mappings cross-checked against a real processor with this firmware.
    Hardware,
    /// Derived from the official PC Remote and firmware of this exact version,
    /// not yet confirmed on hardware. Every write is confirmed by a full readback.
    Static,
    /// Identification only; no parameter profile.
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransportKind {
    PcRemoteTcp,
    /// The 8400's own PC Remote session (TCP 51200, meters over UDP); see `crate::o8400`.
    Pc8400Tcp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Capabilities {
    pub connect: bool,
    pub parameter_reads: bool,
    pub parameter_writes: bool,
    pub live_meters: bool,
    pub preset_catalog: bool,
    pub preset_recall: bool,
    /// Save the on-air processing as a user preset on the processor.
    pub preset_store: bool,
    /// Delete a user preset on the processor.
    pub preset_delete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeterProfile {
    pub banks: &'static [u8],
    /// Accepted number of values in one meter record. The 5700i sends exactly
    /// 112; PC Remote for other models reads a variable count.
    pub min_values: usize,
    pub max_values: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct AdapterDescriptor {
    pub id: &'static str,
    pub model: DeviceModel,
    pub skin: SkinId,
    pub transport: TransportKind,
    pub default_port: u16,
    pub default_terminal_port: u16,
    pub product_mark: &'static str,
    pub firmware_prefix: &'static str,
    pub capabilities: Capabilities,
    pub meter_profile: Option<MeterProfile>,
    /// Whether the firmware ends frames early; see `framing::Decoder::with_cut`. `None` where this was not observed.
    pub frame_cut: Option<crate::framing::FrameCut>,
    /// Opcode 239 returns the on-air processing document whole, the same
    /// fields as the terminal AP, over the PC Remote session itself.
    pub on_air_archive: bool,
    /// The opcodes of this model's PC Remote session.
    pub wire: Wire,
    pub evidence: Evidence,
}

/// The last line of the login answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginSeed {
    Always,
    Never,
    /// Read when the unit sends it with the rest of the answer. The original
    /// client reads the answer in one buffer and drops what it does not use,
    /// so its code does not show whether this firmware sends the line.
    IfSent,
}

/// Opcodes that differ between PC Remote generations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wire {
    /// The ordered response boundary: the request and the unit's answer.
    pub boundary: (u16, u16),
    /// The request answered with the on-air processing file (opcode 229).
    pub on_air_request: u16,
    /// The request answered with the system settings file, and that answer's opcode.
    pub setup_request: Option<(u16, u16)>,
    /// The request answered with one file per user preset, and those answers' opcode.
    pub user_presets_request: Option<(u16, u16)>,
    /// The file extension of a user preset; other files in that answer are skipped.
    pub user_extension: &'static str,
    /// Settings, presets, readback and Recall use files over the PC Remote
    /// session; the status (terminal) port is used only for a factory Recall.
    pub file_sync: bool,
    /// Factory preset names, where the firmware's list cannot be read over
    /// the PC Remote session.
    pub factory_presets: &'static [&'static str],
    /// The opcode that ends a file sent without a request, in either
    /// direction; the receiver answers it with the same opcode, which is also
    /// the boundary answer. `None` where this was not observed.
    pub file_end: Option<u16>,
    /// Whether the login answer ends with a session number line.
    pub login_seed: LoginSeed,
    /// File messages carry a file name line; without it, a requested file
    /// is told from a pushed one only by the request in flight.
    pub file_names: bool,
    /// A factory preset can be recalled with the terminal's `RP`.
    pub factory_recall: bool,
    /// Presets are saved as an opcode-229 upload of the on-air file under a
    /// new name and deleted with this opcode and the file name, as the
    /// original PC Remote does. `None` where this was not captured.
    pub preset_file_delete: Option<u16>,
    /// The unit's automation and passcode files, as the original client
    /// reads and writes them. `None` where this was not captured.
    pub tool_files: Option<ToolFiles>,
    /// What the client's Backup to PC requests besides the preset files,
    /// with each answer's opcode.
    pub backup_requests: &'static [(u16, u16)],
    /// The preset-file answers it keeps, by extension; empty keeps all.
    pub backup_kept: &'static [&'static str],
    /// Further files its Restore sends back as preset files (231).
    pub restore_extensions: &'static [&'static str],
}

/// Requests for the files behind the client's Automation and Security
/// dialogs. The unit answers with 235 (automation) and 230 (passcodes) on
/// every model captured so far; the requests differ.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolFiles {
    /// Sent first, as the original client does; without it the 5700i leaves
    /// its passcode request unanswered.
    pub prelude: Option<u16>,
    pub automation_request: u16,
    pub passcodes_request: u16,
    /// The name the client gives the automation file it uploads.
    pub automation_name: &'static str,
}

/// The PC Remote generation of the 5700i, where nothing else was captured.
pub const WIRE_PC_REMOTE: Wire = Wire {
    boundary: (250, 251),
    on_air_request: 239,
    setup_request: None,
    user_presets_request: None,
    user_extension: "",
    file_sync: false,
    factory_presets: &[],
    file_end: None,
    login_seed: LoginSeed::Always,
    file_names: true,
    factory_recall: true,
    preset_file_delete: None,
    tool_files: None,
    backup_requests: &[(240, 233)],
    backup_kept: &[],
    restore_extensions: &[],
};

/// 5700i 3.0.1.20 (reference unit and virtual 5700i).
///
/// The terminal `LP` lists user presets among the factory presets; F3h is
/// answered by the unit's preset files (E7h), which tell them apart. The
/// original PC Remote 3.0.1.20 saves (Save and Save As) by uploading the
/// on-air file with its `Preset Name` and `Preset File Name` replaced, as
/// opcode 229 without a file name: the unit stores it and puts it on air.
/// Delete sends EDh with `NAME.orb57user` and a newline; the unit does not
/// answer (captures, 28 October 2026).
pub const WIRE_5700I: Wire = Wire {
    user_presets_request: Some((243, 231)),
    user_extension: ".orb57user",
    preset_file_delete: Some(237),
    tool_files: Some(ToolFiles {
        prelude: Some(244),
        automation_request: 242,
        passcodes_request: 241,
        automation_name: "Automation.orb57autom",
    }),
    ..WIRE_PC_REMOTE
};

/// 8700HD 1.0.2.161: captured from the original PC Remote 1.0.2.161 and the
/// original firmware (virtual 8700, 20-21 October 2026). F9h is answered by
/// FAh, EEh by `OnAir.orb86onair`, EFh by `Setup.orb86setup` (E8h) and F2h by
/// every `.orb86user` file (E7h). The terminal `LP` labels user presets as
/// factory presets, so factory names come from the PC Remote installer.
pub const WIRE_8700HD: Wire = Wire {
    boundary: (249, 250),
    on_air_request: 238,
    setup_request: Some((239, 232)),
    user_presets_request: Some((242, 231)),
    user_extension: ".orb86user",
    file_sync: true,
    factory_presets: &FACTORY_8700HD,
    // The unit follows the on-air file it pushes after a terminal `RP` with
    // FAh; the original PC Remote follows its Recall upload with FAh, and
    // the unit answers it with FAh (captures, 21 October 2026).
    file_end: Some(250),
    login_seed: LoginSeed::Always,
    file_names: true,
    factory_recall: true,
    preset_file_delete: None,
    tool_files: None,
    backup_requests: &[(240, 233)],
    backup_kept: &[],
    restore_extensions: &[],
};

/// 5500 1.2.8.24: the 8700HD generation. `5500PC.exe` 1.2.8.24 sends the same
/// opcode sequences as `8700PC.exe` 1.0.2.161 (sync: EFh, F3h, F0h, F1h, EEh,
/// F9h; presets: DFh, E2h, F2h, F9h) and names the 5500i's files:
/// `OnAir.orb55onair`, `Setup.orb55setup`, `.orb55user`. Its installer holds
/// the same 27 factory presets as the 5500i's. Derived statically (issue #1:
/// the 5700i boundary FAh/FBh never got an answer from a 5500).
pub const WIRE_5500: Wire = Wire {
    user_extension: ".orb55user",
    factory_presets: &FACTORY_5500I,
    login_seed: LoginSeed::IfSent,
    ..WIRE_8700HD
};

/// 5500i 3.1.0.2: captured from the original PC Remote 3.1.0.2 and the
/// original firmware (virtual 5500i, 21 October 2026). The same generation as
/// the 5700i: FAh is answered by FBh and EFh by `OnAir.orb55onair`. F0h is
/// answered by `Setup.orbsetup` (E9h), F3h by the unit's preset files (E7h).
/// A Recall upload ends with FBh, which the unit answers with FBh. Save and
/// Save As upload the on-air file (229) and Delete sends EDh with
/// `NAME.orb55user` and a newline, as on the 5700i; the passcodes come with
/// F4h and the automation file with F2h (captures, 28 October 2026).
pub const WIRE_5500I: Wire = Wire {
    boundary: (250, 251),
    on_air_request: 239,
    setup_request: Some((240, 233)),
    user_presets_request: Some((243, 231)),
    user_extension: ".orb55user",
    file_sync: true,
    factory_presets: &FACTORY_5500I,
    file_end: Some(251),
    login_seed: LoginSeed::Always,
    file_names: true,
    factory_recall: true,
    preset_file_delete: Some(237),
    tool_files: Some(ToolFiles {
        prelude: None,
        automation_request: 242,
        passcodes_request: 244,
        automation_name: "Automation.orb55autom",
    }),
    // Backup to PC keeps setup, network, automation, user presets and setup
    // presets; Restore sends any of them back as 231 (captures, 28 October).
    backup_requests: &[(240, 233), (241, 234), (242, 235)],
    backup_kept: &[".orb55user", ".orb55setup"],
    restore_extensions: &[".orb55setup", ".orbsetup", ".orb55autom", ".orb55net"],
};

/// Preset names of the factory `.orb55f50` files of PC Remote 3.1.0.2.
const FACTORY_5500I: [&str; 27] = [
    "BASS-PUNCH",
    "CLASSICAL-2 BAND",
    "CLASSICAL-2B+AGC",
    "CLASSICAL-5 BAND",
    "CLASSICAL-5B+AGC",
    "COUNTRY UL",
    "COUNTRY-LIGHT",
    "COUNTRY-MEDIUM",
    "EXTREME",
    "JAZZ",
    "NEWS-TALK",
    "OLDIES",
    "PROTECT-0DB",
    "ROCK-GEN UL",
    "ROCK-GENERAL",
    "ROCK-LIGHT",
    "ROCK-LOUD",
    "ROCK-MEDIUM",
    "ROCK-OPEN",
    "ROCK-OPEN UL",
    "ROCK-SMOOTH",
    "ROCK-SOFT",
    "SPORTS",
    "ST ENC NO LIM",
    "URBAN",
    "URBAN UL",
    "URBAN-LOUD",
];

/// 6300 4.1.1.19: probed on the original firmware (virtual 6300, 21 October
/// 2026). The same generation as the 8700HD: F9h is answered by FAh, EEh by
/// `OnAir.orb63onair`, EFh by `Setup.orb63setup` (E8h) and F2h by every
/// `.orb63user` file (E7h).
pub const WIRE_6300: Wire = Wire {
    boundary: (249, 250),
    on_air_request: 238,
    setup_request: Some((239, 232)),
    user_presets_request: Some((242, 231)),
    user_extension: ".orb63user",
    file_sync: true,
    factory_presets: &FACTORY_6300,
    file_end: Some(250),
    login_seed: LoginSeed::Always,
    file_names: true,
    factory_recall: true,
    preset_file_delete: None,
    tool_files: None,
    backup_requests: &[(240, 233)],
    backup_kept: &[],
    restore_extensions: &[],
};

/// Preset names of the factory `.orb63f` files of PC Remote 4.1.1.19.
const FACTORY_6300: [&str; 68] = [
    "AGC+50us LIMIT",
    "AGC+75us LIMIT",
    "AGC+FLAT LIMIT",
    "CLASSICAL-2 BAND",
    "CLASSICAL-2B SFTKN",
    "CLASSICAL-2B+AGC",
    "CLASSICAL-5 BAND",
    "CLASSICAL-5B+AGC",
    "COUNTRY-LIGHT",
    "COUNTRY-MEDIUM",
    "CRISP",
    "DANCE ENERGY",
    "EDGE",
    "FOLK-TRADITIONAL",
    "GOLD",
    "GREGG",
    "GREGG OPEN",
    "IMPACT",
    "INSTRUMENTAL",
    "JAZZ",
    "LOOK-AHEAD LIMITER",
    "LOUD-BIG",
    "LOUD-COMPRESSED",
    "LOUD-FAT",
    "LOUD-HOT",
    "LOUD-HOT+BASS",
    "LOUD-PUNCHY",
    "LOUD+SLAM",
    "NEWS-TALK",
    "PROTECT",
    "ROCK-DENSE",
    "ROCK-LIGHT",
    "ROCK-MEDIUM",
    "ROCK-MEDIUM+LOWBASS",
    "ROCK-MEDIUM+MIDBASS",
    "ROCK-OPEN",
    "ROCK-SMOOTH",
    "ROCK-SOFT",
    "SMOOTH JAZZ",
    "SOFT KNEE 2B",
    "SOFT KNEE 5B",
    "SPORTS",
    "TV 2B DRAMA",
    "TV 2B GEN PURP -LC",
    "TV 2B GEN PURPOSE",
    "TV 5B DRAMA",
    "TV 5B DRAMA COUPLED",
    "TV 5B GEN PUR W NR",
    "TV 5B GEN PURP -LC",
    "TV 5B GEN PURPOSE",
    "TV 5B NEWS",
    "TV 5B OPTICAL FILM",
    "TV 5B SPORTS",
    "TVA 2B DRAMA",
    "TVA 2B GEN PURP -LC",
    "TVA 2B GEN PURPOSE",
    "TVA 5B DRAMA",
    "TVA 5B DRAMA COUPLED",
    "TVA 5B GEN PUR W NR",
    "TVA 5B GEN PURP -LC",
    "TVA 5B GEN PURPOSE",
    "TVA 5B NEWS",
    "TVA 5B OPTICAL FILM",
    "TVA 5B SPORTS",
    "URBAN-HEAVY",
    "URBAN-LIGHT",
    "WMA MUSIC",
    "WMA NEWS-TALK",
];

/// 2300 2.0.3.85: probed on the original firmware and read from a recorded
/// session of the original PC Remote 2.0.3.85 (virtual 2300, October 2026).
/// An older generation: the login has no session number line, F7h is
/// answered by F8h, EDh by the on-air file, EEh by the setup file (E8h) and
/// F0h by the user preset files (E7h); a file message has no name line and a
/// meter record gives its count as one byte. The unit has no TCP terminal, so
/// a factory preset can only be recalled by uploading a document that the
/// original client converts from its installer's factory file.
pub const WIRE_2300: Wire = Wire {
    boundary: (247, 248),
    on_air_request: 237,
    setup_request: Some((238, 232)),
    user_presets_request: Some((240, 231)),
    user_extension: ".orb23user",
    file_sync: true,
    factory_presets: &FACTORY_2300,
    file_end: None,
    login_seed: LoginSeed::Never,
    file_names: false,
    factory_recall: false,
    preset_file_delete: None,
    tool_files: None,
    backup_requests: &[(240, 233)],
    backup_kept: &[],
    restore_extensions: &[],
};

/// Preset names of the factory `.orb23f75` files of PC Remote 2.0.3.85.
const FACTORY_2300: [&str; 10] = [
    "CLASSICAL",
    "CLASSICAL+AGC",
    "GENERAL PURPOSE",
    "MUSIC HEAVY",
    "MUSIC LIGHT",
    "MUSIC MEDIUM",
    "MUSIC+BASS HEAVY",
    "MUSIC+BASS MEDIUM",
    "ST ENC NO LIM",
    "TALK",
];

/// 8700i 1.5.1.11: captured from the original PC Remote 1.5.1.11 and the
/// original firmware (virtual 8700i, 21 October 2026). The same requests as
/// the 5500i: FAh/FBh, EFh `OnAir.orb87onair`, F0h `Setup.orb87setup` (E9h),
/// F3h the preset files (E7h).
pub const WIRE_8700I: Wire = Wire {
    user_extension: ".orb87user",
    factory_presets: &FACTORY_8700I,
    // The 5500i's file messages are not captured for the 8700i yet.
    preset_file_delete: None,
    tool_files: None,
    backup_requests: &[(240, 233)],
    backup_kept: &[],
    restore_extensions: &[],
    ..WIRE_5500I
};

/// Preset names of the factory `.orb87f50` files of PC Remote 1.5.1.11.
const FACTORY_8700I: [&str; 58] = [
    "CLASSICAL-2 BAND",
    "CLASSICAL-2B MX",
    "CLASSICAL-2B+AGC",
    "CLASSICAL-2B+AGC MX",
    "CLASSICAL-5 BAND",
    "CLASSICAL-5B MX",
    "CLASSICAL-5B+AGC",
    "CLASSICAL-5B+AGC MX",
    "COUNTRY UL",
    "COUNTRY-LIGHT",
    "COUNTRY-MEDIUM",
    "CRISP",
    "CRISP+LOWBASS MX",
    "CRISP+MIDBASS MX",
    "EDGE MX",
    "FOLK-TRADITIONAL",
    "GOLD",
    "GOLD HEAVY BASS MX",
    "GOLD HEAVY MX",
    "GOLD OPEN MX",
    "GREGG",
    "GREGG LL",
    "GREGG MX",
    "GREGG MX XLF",
    "GREGG OPEN",
    "GREGG OPEN MX",
    "IMPACT MX",
    "INSTRUMENTAL",
    "JAZZ",
    "LOUD-COMPRESSED",
    "LOUD-HOT+BASS",
    "LOUD-HOT+BASS LL",
    "LOUD-PUNCHY",
    "LOUD+SLAM",
    "MAINSTREAM 1 MX",
    "MAINSTREAM 2 MX",
    "MAINSTREAM 3 MX",
    "MAINSTREAM 4 MX",
    "MELLOW JAZZ MX",
    "NEWS-TALK",
    "PROTECT-0DB",
    "ROCK-DENSE",
    "ROCK-GEN UL",
    "ROCK-MEDIUM",
    "ROCK-MEDIUM MX",
    "ROCK-MEDIUM+LOWBASS",
    "ROCK-MEDIUM+MIDBASS",
    "ROCK-OPEN",
    "ROCK-OPEN UL",
    "ROCK-SMOOTH",
    "ROCK-SMOOTH MX",
    "ROCK-SOFT",
    "ROCK-SOFT MX",
    "SMOOTH JAZZ",
    "SMOOTH JAZZ MX",
    "SPORTS",
    "URBAN UL",
    "URBAN-LIGHT",
];

/// Preset names of the factory `.orb86f50` files of PC Remote 1.0.2.161.
const FACTORY_8700HD: [&str; 64] = [
    "CLASSICAL-2 BAND",
    "CLASSICAL-2B MX",
    "CLASSICAL-2B+AGC",
    "CLASSICAL-2B+AGC MX",
    "CLASSICAL-5 BAND",
    "CLASSICAL-5B MX",
    "CLASSICAL-5B+AGC",
    "CLASSICAL-5B+AGC MX",
    "COUNTRY UL",
    "COUNTRY-LIGHT",
    "COUNTRY-MEDIUM",
    "CRISP",
    "CRISP+LOWBASS MX",
    "CRISP+MIDBASS MX",
    "DANCE ENERGY",
    "EDGE",
    "EDGE MX",
    "FOLK-TRADITIONAL",
    "GOLD",
    "GOLD HEAVY BASS MX",
    "GOLD HEAVY MX",
    "GOLD OPEN MX",
    "GREGG",
    "GREGG LL",
    "GREGG MX",
    "GREGG OPEN",
    "GREGG OPEN MX",
    "IMPACT",
    "IMPACT LL",
    "IMPACT MX",
    "INSTRUMENTAL",
    "JAZZ",
    "LOUD-BIG",
    "LOUD-COMPRESSED",
    "LOUD-FAT",
    "LOUD-HOT",
    "LOUD-HOT LL",
    "LOUD-HOT+BASS",
    "LOUD-HOT+BASS LL",
    "LOUD-PUNCHY",
    "LOUD-WIDE",
    "LOUD+SLAM",
    "MELLOW JAZZ MX",
    "NEWS-TALK",
    "PROTECT-0DB",
    "ROCK-DENSE",
    "ROCK-GEN UL",
    "ROCK-LIGHT",
    "ROCK-MEDIUM",
    "ROCK-MEDIUM MX",
    "ROCK-MEDIUM+LOWBASS",
    "ROCK-MEDIUM+MIDBASS",
    "ROCK-OPEN",
    "ROCK-OPEN UL",
    "ROCK-SMOOTH",
    "ROCK-SMOOTH MX",
    "ROCK-SOFT",
    "ROCK-SOFT MX",
    "SMOOTH JAZZ",
    "SMOOTH JAZZ MX",
    "SPORTS",
    "URBAN UL",
    "URBAN-HEAVY",
    "URBAN-LIGHT",
];

const READ_ONLY: Capabilities = Capabilities {
    connect: true,
    parameter_reads: true,
    parameter_writes: false,
    live_meters: false,
    preset_catalog: true,
    preset_recall: false,
    preset_store: false,
    preset_delete: false,
};

const VERIFIED_5700I: AdapterDescriptor = AdapterDescriptor {
    id: "pc-remote-5700i-3.0.1.20",
    model: DeviceModel::Optimod5700i,
    skin: SkinId::Optimod5700i,
    transport: TransportKind::PcRemoteTcp,
    default_port: 6201,
    default_terminal_port: 23,
    product_mark: "5700i DIGITAL",
    firmware_prefix: "5700i V ",
    capabilities: Capabilities {
        connect: true,
        parameter_reads: true,
        parameter_writes: true,
        live_meters: true,
        preset_catalog: true,
        preset_recall: true,
        // No terminal save or delete command: presets are saved and deleted
        // with the original PC Remote's file messages (see WIRE_5700I).
        preset_store: true,
        preset_delete: true,
    },
    meter_profile: Some(MeterProfile {
        banks: &[1, 2],
        min_values: 112,
        max_values: 112,
    }),
    // The firmware ends frames early when they do not fit its send buffer:
    // the reference unit cut the document it pushes after a terminal RP at
    // 4096 wire bytes, the virtual 5700i (same firmware) at 4075 bytes and
    // meter records at arbitrary points (docs/protocol/5700i.md).
    frame_cut: Some(crate::framing::FrameCut),
    // Reference unit (archive) and the virtual 5700i with original firmware
    // (archive equals AP field for field), 20 October 2026.
    on_air_archive: true,
    wire: WIRE_5700I,
    evidence: Evidence::Hardware,
};

const STATIC_FULL: Capabilities = Capabilities {
    connect: true,
    parameter_reads: true,
    parameter_writes: true,
    live_meters: true,
    preset_catalog: true,
    preset_recall: true,
    preset_store: false,
    preset_delete: false,
};

/// The 5500 firmware documents `SP 'preset name'` and `DP 'preset name'` in its
/// terminal help, next to the `RP` command the app already uses for recall.
const STATIC_FULL_WITH_PRESET_FILES: Capabilities = Capabilities {
    preset_store: true,
    preset_delete: true,
    ..STATIC_FULL
};

/// Static meter records: PC Remote accepts any count and ignores channels it
/// does not store, so the decoder only enforces a sane upper bound.
const STATIC_METERS: MeterProfile = MeterProfile {
    banks: &[1, 2],
    min_values: 1,
    max_values: 512,
};

/// Exact banners with a parameter profile. Only these can write.
const EXACT: [(&str, AdapterDescriptor); 8] = [
    ("5700i V 3.0.1.20", VERIFIED_5700I),
    (
        "5500 V 1.2.8.24",
        AdapterDescriptor {
            id: "pc-remote-5500-1.2.8.24",
            model: DeviceModel::Optimod5500,
            skin: SkinId::Optimod5500,
            transport: TransportKind::PcRemoteTcp,
            default_port: 6201,
            default_terminal_port: 23,
            product_mark: "5500 DIGITAL",
            firmware_prefix: "5500 V ",
            capabilities: STATIC_FULL_WITH_PRESET_FILES,
            meter_profile: Some(STATIC_METERS),
            frame_cut: None,
            on_air_archive: true,
            wire: WIRE_5500,
            evidence: Evidence::Static,
        },
    ),
    (
        "5500i V 3.1.0.2",
        AdapterDescriptor {
            id: "pc-remote-5500i-3.1.0.2",
            model: DeviceModel::Optimod5500i,
            skin: SkinId::Optimod5500i,
            transport: TransportKind::PcRemoteTcp,
            default_port: 6201,
            default_terminal_port: 23,
            product_mark: "5500i DIGITAL",
            firmware_prefix: "5500i V ",
            capabilities: STATIC_FULL_WITH_PRESET_FILES,
            meter_profile: Some(STATIC_METERS),
            frame_cut: None,
            on_air_archive: true,
            wire: WIRE_5500I,
            evidence: Evidence::Static,
        },
    ),
    (
        "6300 V 4.1.1.19",
        AdapterDescriptor {
            id: "pc-remote-6300-4.1.1.19",
            model: DeviceModel::Optimod6300,
            skin: SkinId::Optimod6300,
            transport: TransportKind::PcRemoteTcp,
            default_port: 6201,
            default_terminal_port: 23,
            product_mark: "6300 DIGITAL",
            firmware_prefix: "6300 V ",
            capabilities: STATIC_FULL,
            meter_profile: Some(STATIC_METERS),
            frame_cut: None,
            on_air_archive: true,
            wire: WIRE_6300,
            evidence: Evidence::Static,
        },
    ),
    (
        "2300 V 2.0.3.85",
        AdapterDescriptor {
            id: "pc-remote-2300-2.0.3.85",
            model: DeviceModel::Optimod2300,
            skin: SkinId::Optimod2300,
            transport: TransportKind::PcRemoteTcp,
            default_port: 6201,
            default_terminal_port: 23,
            product_mark: "2300 DIGITAL",
            firmware_prefix: "2300 V ",
            capabilities: STATIC_FULL,
            meter_profile: Some(STATIC_METERS),
            frame_cut: None,
            on_air_archive: true,
            wire: WIRE_2300,
            evidence: Evidence::Static,
        },
    ),
    (
        "8700i V 1.5.1.11",
        AdapterDescriptor {
            id: "pc-remote-8700i-1.5.1.11",
            model: DeviceModel::Optimod8700i,
            skin: SkinId::Optimod8700i,
            transport: TransportKind::PcRemoteTcp,
            default_port: 6201,
            default_terminal_port: 23,
            product_mark: "8700i FM+HD",
            firmware_prefix: "8700i V ",
            capabilities: STATIC_FULL,
            meter_profile: Some(STATIC_METERS),
            // Like the 5700i, the 8700i cuts the on-air document it pushes after a
            // terminal RP while session meters are on (8.4 KB; virtual 8700i,
            // 21 October 2026): the next frame starts inside it.
            frame_cut: Some(crate::framing::FrameCut),
            on_air_archive: true,
            wire: WIRE_8700I,
            evidence: Evidence::Static,
        },
    ),
    ("8400 V 3.0.5.7", PC_8400),
    (
        "8700HD V 1.0.2.161",
        AdapterDescriptor {
            id: "pc-remote-8700hd-1.0.2.161",
            model: DeviceModel::Optimod8700Hd,
            skin: SkinId::Optimod8700Hd,
            transport: TransportKind::PcRemoteTcp,
            default_port: 6201,
            default_terminal_port: 23,
            product_mark: "8700HD FM+HD",
            firmware_prefix: "8700HD V ",
            capabilities: STATIC_FULL,
            meter_profile: Some(STATIC_METERS),
            frame_cut: None,
            on_air_archive: true,
            wire: WIRE_8700HD,
            evidence: Evidence::Static,
        },
    ),
];

/// The 8400 3.0.5 (software 3.0.5.7): captured from the original 8400 PC
/// Remote 3.0 and the original firmware (virtual 8400, 21 October 2026).
/// Its session is `crate::o8400`; the banner is made from the login answer's
/// software version. Meters arrive over UDP, not as opcode 228.
const PC_8400: AdapterDescriptor = AdapterDescriptor {
    id: "pc-remote-8400-3.0.5.7",
    model: DeviceModel::Optimod8400,
    skin: SkinId::Optimod8400,
    transport: TransportKind::Pc8400Tcp,
    default_port: crate::o8400::DEFAULT_PORT,
    default_terminal_port: 23,
    product_mark: "OPTIMOD-FM 8400",
    firmware_prefix: "8400 V ",
    capabilities: STATIC_FULL,
    meter_profile: None,
    frame_cut: None,
    on_air_archive: false,
    wire: WIRE_8400,
    evidence: Evidence::Static,
};

/// Not used by the 8400 session; its messages are in `crate::o8400`.
const WIRE_8400: Wire = Wire {
    file_sync: true,
    factory_recall: false,
    ..WIRE_PC_REMOTE
};

const ADAPTERS: [AdapterDescriptor; 14] = [
    AdapterDescriptor {
        id: "pc-remote-5700i-family",
        model: DeviceModel::Optimod5700i,
        skin: SkinId::Optimod5700i,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "5700i DIGITAL",
        firmware_prefix: "5700i V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_PC_REMOTE,
        evidence: Evidence::None,
    },
    AdapterDescriptor {
        id: "pc-remote-5500i-family",
        model: DeviceModel::Optimod5500i,
        skin: SkinId::Optimod5500i,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "5500i DIGITAL",
        firmware_prefix: "5500i V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_PC_REMOTE,
        evidence: Evidence::None,
    },
    AdapterDescriptor {
        id: "pc-remote-5500-family",
        model: DeviceModel::Optimod5500,
        skin: SkinId::Optimod5500,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "5500 DIGITAL",
        firmware_prefix: "5500 V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: true,
        wire: WIRE_5500,
        evidence: Evidence::None,
    },
    AdapterDescriptor {
        id: "pc-remote-5700fm-family",
        model: DeviceModel::Optimod5700Fm,
        skin: SkinId::Optimod5700Fm,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "5700 FM",
        firmware_prefix: "5700FM V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_PC_REMOTE,
        evidence: Evidence::None,
    },
    AdapterDescriptor {
        id: "pc-remote-5700hd-family",
        model: DeviceModel::Optimod5700Hd,
        skin: SkinId::Optimod5700Hd,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "5700 HD",
        firmware_prefix: "5700HD V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_PC_REMOTE,
        evidence: Evidence::None,
    },
    AdapterDescriptor {
        id: "pc-remote-8500-family",
        model: DeviceModel::Optimod8500,
        skin: SkinId::Optimod8500,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "8500 FM+HD",
        firmware_prefix: "8500 V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_PC_REMOTE,
        evidence: Evidence::None,
    },
    AdapterDescriptor {
        id: "pc-remote-2300-family",
        model: DeviceModel::Optimod2300,
        skin: SkinId::Optimod2300,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "2300 DIGITAL",
        firmware_prefix: "2300 V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_2300,
        evidence: Evidence::None,
    },
    AdapterDescriptor {
        id: "pc-remote-6300-family",
        model: DeviceModel::Optimod6300,
        skin: SkinId::Optimod6300,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "6300 DIGITAL",
        firmware_prefix: "6300 V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_PC_REMOTE,
        evidence: Evidence::None,
    },
    // Other 8400 software: identification only. The 8400 PC Remote 3.0
    // needs software 3.0.0.0 or later.
    AdapterDescriptor {
        id: "pc-remote-8400-family",
        model: DeviceModel::Optimod8400,
        skin: SkinId::Optimod8400,
        transport: TransportKind::Pc8400Tcp,
        default_port: crate::o8400::DEFAULT_PORT,
        default_terminal_port: 23,
        product_mark: "OPTIMOD-FM 8400",
        firmware_prefix: "8400 V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_8400,
        evidence: Evidence::None,
    },
    AdapterDescriptor {
        id: "pc-remote-8600-family",
        model: DeviceModel::Optimod8600,
        skin: SkinId::Optimod8600,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "8600 FM+HD",
        firmware_prefix: "8600 V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_PC_REMOTE,
        evidence: Evidence::None,
    },
    AdapterDescriptor {
        id: "pc-remote-8700i-family",
        model: DeviceModel::Optimod8700i,
        skin: SkinId::Optimod8700i,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "8700i FM+HD",
        firmware_prefix: "8700i V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_PC_REMOTE,
        evidence: Evidence::None,
    },
    AdapterDescriptor {
        id: "pc-remote-8700hd-family",
        model: DeviceModel::Optimod8700Hd,
        skin: SkinId::Optimod8700Hd,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "8700HD FM+HD",
        firmware_prefix: "8700HD V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_8700HD,
        evidence: Evidence::None,
    },
    AdapterDescriptor {
        id: "pc-remote-9300-family",
        model: DeviceModel::Optimod9300,
        skin: SkinId::Optimod9300,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "9300 AM",
        firmware_prefix: "9300 V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_PC_REMOTE,
        evidence: Evidence::None,
    },
    AdapterDescriptor {
        id: "pc-remote-9400-family",
        model: DeviceModel::Optimod9400,
        skin: SkinId::Optimod9400,
        transport: TransportKind::PcRemoteTcp,
        default_port: 6201,
        default_terminal_port: 23,
        product_mark: "9400 AM+HD",
        firmware_prefix: "9400 V ",
        capabilities: READ_ONLY,
        meter_profile: None,
        frame_cut: None,
        on_air_archive: false,
        wire: WIRE_PC_REMOTE,
        evidence: Evidence::None,
    },
];

pub struct AdapterRegistry;

impl AdapterRegistry {
    pub fn identify_banner(banner: &str) -> Result<&'static AdapterDescriptor, String> {
        if let Some((_, adapter)) = EXACT.iter().find(|(exact, _)| *exact == banner) {
            return Ok(adapter);
        }
        ADAPTERS
            .iter()
            .find(|adapter| banner.starts_with(adapter.firmware_prefix))
            .ok_or_else(|| "Unsupported device model".into())
    }

    pub fn for_model(model: DeviceModel) -> Option<&'static AdapterDescriptor> {
        match model {
            DeviceModel::Auto => None,
            DeviceModel::Optimod5700i => Some(&ADAPTERS[0]),
            _ => ADAPTERS.iter().find(|adapter| adapter.model == model),
        }
    }

    pub fn identify_terminal_banner(banner: &str) -> Result<DeviceModel, String> {
        let normalized = banner.to_ascii_lowercase();
        for (needle, model) in [
            ("orban optimod 5700i", DeviceModel::Optimod5700i),
            ("orban optimod 5500i", DeviceModel::Optimod5500i),
            ("orban optimod 5500", DeviceModel::Optimod5500),
            ("orban optimod 5700fm", DeviceModel::Optimod5700Fm),
            ("orban optimod 5700hd", DeviceModel::Optimod5700Hd),
            ("orban optimod 8500", DeviceModel::Optimod8500),
            ("orban optimod 6300", DeviceModel::Optimod6300),
            ("orban optimod 2300", DeviceModel::Optimod2300),
            ("orban optimod 8600", DeviceModel::Optimod8600),
            ("orban optimod 8700i", DeviceModel::Optimod8700i),
            // The 8700HD 1.0.2.161 terminal banner: "Orban Optimod 8700HD V 1.0.2.161".
            ("orban optimod 8700hd", DeviceModel::Optimod8700Hd),
            ("orban optimod 9300", DeviceModel::Optimod9300),
            ("orban optimod 9400", DeviceModel::Optimod9400),
        ] {
            if normalized.contains(needle) {
                return Ok(model);
            }
        }
        Err("Unsupported terminal model".into())
    }

    pub fn all() -> &'static [AdapterDescriptor] {
        &ADAPTERS
    }

    /// Adapters that carry a parameter profile, keyed by their exact banner.
    pub fn exact() -> impl Iterator<Item = (&'static str, &'static AdapterDescriptor)> {
        EXACT.iter().map(|(banner, adapter)| (*banner, adapter))
    }
}
