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
}
impl Profile {
    pub fn embedded() -> Result<Self, String> {
        let profile: Self = serde_json::from_str(include_str!(
            "../../../profiles/5700i/3.0.1.20/parameters.json"
        ))
        .map_err(|e| e.to_string())?;
        for (i, field) in profile.fields.iter().enumerate() {
            if if let Some(d) = &field.sample_delay {
                !field.values.is_empty()
                    || field.integer_range.is_some()
                    || d.rate != 64000
                    || d.offset != 23361
                    || d.max_index != 0xf833d
            } else {
                match &field.integer_range {
                    Some(r) => !field.values.is_empty() || r.min > r.max || r.max > i32::MAX as u32,
                    None => field.values.is_empty(),
                }
            } || profile.fields[..i]
                .iter()
                .any(|f| f.scope == field.scope && f.name == field.name)
            {
                return Err("Invalid embedded profile".into());
            }
        }
        Ok(profile)
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
