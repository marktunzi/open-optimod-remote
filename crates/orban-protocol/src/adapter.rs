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
    #[serde(rename = "optimod-6300")]
    Optimod6300,
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
    pub const SELECTABLE: [Self; 13] = [
        Self::Auto,
        Self::Optimod5700i,
        Self::Optimod5500i,
        Self::Optimod5500,
        Self::Optimod5700Fm,
        Self::Optimod5700Hd,
        Self::Optimod8500,
        Self::Optimod6300,
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
            Self::Optimod6300 => "OPTIMOD 6300",
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
    #[serde(rename = "optimod-6300")]
    Optimod6300,
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
    pub evidence: Evidence,
}

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
        // The 5700i firmware has no terminal save or delete command; PC Remote
        // syncs a local folder through file messages whose effect is unverified.
        preset_store: false,
        preset_delete: false,
    },
    meter_profile: Some(MeterProfile {
        banks: &[1, 2],
        min_values: 112,
        max_values: 112,
    }),
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
const EXACT: [(&str, AdapterDescriptor); 3] = [
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
            evidence: Evidence::Static,
        },
    ),
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
            evidence: Evidence::Static,
        },
    ),
];

const ADAPTERS: [AdapterDescriptor; 12] = [
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
            ("orban optimod 8600", DeviceModel::Optimod8600),
            ("orban optimod 8700i", DeviceModel::Optimod8700i),
            ("orban optimod-fm 8700hd", DeviceModel::Optimod8700Hd),
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
