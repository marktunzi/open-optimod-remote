use crate::{control::Scope, document::Value};
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct Definition {
    pub scope: Scope,
    pub name: String,
    pub reference_id: u32,
    pub values: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integer_range: Option<IntegerRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_delay: Option<SampleDelay>,
    #[serde(default)]
    pub unit: String,
    /// How a statically derived mapping relates to the factory presets
    /// ("confirmed", "transform …" or "unobserved"). Absent for hardware profiles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct IntegerRange {
    pub min: u32,
    pub max: u32,
}
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct SampleDelay {
    pub max_index: u32,
    pub offset: u32,
    pub rate: u32,
}
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct Profile {
    pub model: String,
    pub firmware: String,
    pub fields: Vec<Definition>,
    /// Processing pages for this model. Absent: the interface uses its built-in pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layouts: Option<serde_json::Value>,
    /// Meter groups and conversions. Absent: the interface uses its built-in meters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meters: Option<serde_json::Value>,
    /// Statically derived pages for a model with built-in pages. The interface
    /// adds those its built-in set lacks, such as other processing structures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplement_layouts: Option<serde_json::Value>,
}

/// One embedded model profile: adapter id, parameters, optional pages and meters.
struct Embedded {
    adapter_id: &'static str,
    parameters: &'static str,
    layouts: Option<&'static str>,
    meters: Option<&'static str>,
    /// Statically derived fields added to a hardware-verified profile. A field
    /// that already exists in `parameters` is never replaced.
    supplement: Option<&'static str>,
    supplement_layouts: Option<&'static str>,
}

/// Every profile the app can write with. A new model adds one entry here and
/// an exact-banner adapter in `crate::adapter`.
const EMBEDDED: [Embedded; 3] = [
    Embedded {
        adapter_id: "pc-remote-5700i-3.0.1.20",
        parameters: include_str!("../../../profiles/5700i/3.0.1.20/parameters.json"),
        layouts: None,
        meters: None,
        supplement: Some(include_str!(
            "../../../profiles/5700i/3.0.1.20/static-parameters.json"
        )),
        supplement_layouts: Some(include_str!(
            "../../../profiles/5700i/3.0.1.20/static-layouts.json"
        )),
    },
    Embedded {
        adapter_id: "pc-remote-5500-1.2.8.24",
        parameters: include_str!("../../../profiles/5500/1.2.8.24/parameters.json"),
        layouts: Some(include_str!("../../../profiles/5500/1.2.8.24/layouts.json")),
        meters: Some(include_str!("../../../profiles/5500/1.2.8.24/meters.json")),
        supplement: None,
        supplement_layouts: None,
    },
    Embedded {
        adapter_id: "pc-remote-8700hd-1.0.2.161",
        parameters: include_str!("../../../profiles/8700hd/1.0.2.161/parameters.json"),
        layouts: Some(include_str!(
            "../../../profiles/8700hd/1.0.2.161/layouts.json"
        )),
        meters: Some(include_str!(
            "../../../profiles/8700hd/1.0.2.161/meters.json"
        )),
        supplement: None,
        supplement_layouts: None,
    },
];

impl Profile {
    /// The 5700i 3.0.1.20 reference profile.
    pub fn embedded() -> Result<Self, String> {
        Self::for_adapter("pc-remote-5700i-3.0.1.20")?.ok_or_else(|| "Missing 5700i profile".into())
    }

    /// The profile of an exact-banner adapter, or `None` when the adapter has none.
    pub fn for_adapter(adapter_id: &str) -> Result<Option<Self>, String> {
        let Some(entry) = EMBEDDED.iter().find(|e| e.adapter_id == adapter_id) else {
            return Ok(None);
        };
        let mut profile: Self =
            serde_json::from_str(entry.parameters).map_err(|e| e.to_string())?;
        if let Some(text) = entry.supplement {
            let supplement: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
            for field in supplement.fields {
                if field.evidence.is_some()
                    && !profile
                        .fields
                        .iter()
                        .any(|f| f.scope == field.scope && f.name == field.name)
                {
                    profile.fields.push(field);
                }
            }
        }
        profile.validate()?;
        let parse = |text: Option<&str>| {
            text.map(serde_json::from_str::<serde_json::Value>)
                .transpose()
                .map_err(|e| e.to_string())
        };
        profile.layouts = parse(entry.layouts)?;
        profile.meters = parse(entry.meters)?;
        profile.supplement_layouts = parse(entry.supplement_layouts)?;
        Ok(Some(profile))
    }

    /// Adapter ids with an embedded profile.
    pub fn adapter_ids() -> impl Iterator<Item = &'static str> {
        EMBEDDED.iter().map(|e| e.adapter_id)
    }

    fn validate(&self) -> Result<(), String> {
        for (i, field) in self.fields.iter().enumerate() {
            let invalid = if let Some(d) = &field.sample_delay {
                !field.values.is_empty()
                    || field.integer_range.is_some()
                    || d.rate == 0
                    || d.max_index > 0x1f_ffff
            } else {
                match &field.integer_range {
                    Some(r) => !field.values.is_empty() || r.min > r.max || r.max > i32::MAX as u32,
                    None => field.values.is_empty(),
                }
            };
            if invalid
                || self.fields[..i]
                    .iter()
                    .any(|f| f.scope == field.scope && f.name == field.name)
            {
                return Err(format!("Invalid embedded profile field {}", field.name));
            }
        }
        Ok(())
    }

    /// True when a field's mapping is statically derived rather than verified on
    /// hardware. Writes to such a field are always confirmed by a full readback.
    pub fn is_static(&self, scope: Scope, name: &str) -> bool {
        self.fields
            .iter()
            .find(|f| f.scope == scope && f.name == name)
            .is_none_or(|f| f.evidence.is_some())
    }

    pub fn value(&self, scope: Scope, name: &str, index: u32) -> Result<Value, String> {
        self.fields
            .iter()
            .find(|f| f.scope == scope && f.name == name)
            .and_then(|f| {
                if let Some(d) = &f.sample_delay {
                    return (index <= d.max_index).then(|| {
                        Value::Choice(format!(
                            "{:.9}",
                            (index as f64 + d.offset as f64) / d.rate as f64
                        ))
                    });
                }
                match &f.integer_range {
                    Some(r) if (r.min..=r.max).contains(&index) => Some(Value::Int(index as i32)),
                    Some(_) => None,
                    None => f.values.get(index as usize).cloned(),
                }
            })
            .ok_or("Unverified parameter or value outside reference range".into())
    }
}
