use std::collections::BTreeMap;
#[derive(Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    Int(i32),
    Cent(i32),
    Choice(String),
    Text(String),
}
#[derive(Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Field {
    pub value: Value,
    pub index: u32,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct Document {
    pub name: String,
    pub fields: BTreeMap<String, Field>,
    #[serde(skip)]
    pub raw: String,
}
impl Document {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        Self::parse_with_prefix(bytes, "OptimodVersion=<5700.")
    }

    pub fn parse_for_model(
        bytes: &[u8],
        model: crate::adapter::DeviceModel,
    ) -> Result<Self, String> {
        let prefix = match model {
            crate::adapter::DeviceModel::Optimod5700i
            | crate::adapter::DeviceModel::Optimod5700Fm
            | crate::adapter::DeviceModel::Optimod5700Hd => "OptimodVersion=<5700.",
            crate::adapter::DeviceModel::Optimod5500i
            | crate::adapter::DeviceModel::Optimod5500 => "OptimodVersion=<8300.",
            crate::adapter::DeviceModel::Optimod8500 => "OptimodVersion=<8500.",
            crate::adapter::DeviceModel::Optimod6300 => "OptimodVersion=<6300.",
            crate::adapter::DeviceModel::Optimod8600 => "OptimodVersion=<8600.",
            crate::adapter::DeviceModel::Optimod8700i => "OptimodVersion=<8700.",
            crate::adapter::DeviceModel::Optimod9300 => "OptimodVersion=<9300.",
            crate::adapter::DeviceModel::Optimod9400 => "OptimodVersion=<9400.",
            crate::adapter::DeviceModel::Auto => {
                return Err("A detected processor model is required".into());
            }
        };
        Self::parse_with_prefix(bytes, prefix)
    }

    fn parse_with_prefix(bytes: &[u8], version_prefix: &str) -> Result<Self, String> {
        if bytes.len() > 65535 {
            return Err("Oversized document".into());
        }
        let raw = std::str::from_utf8(bytes).map_err(|_| "Unsupported document encoding")?;
        if !raw.starts_with(version_prefix) || !raw.ends_with("End Preset<end>\r\n") {
            return Err("Invalid or unsupported Optimod document".into());
        }
        let mut fields = BTreeMap::new();
        let mut name = None;
        for line in raw.lines() {
            if let Some(rest) = line.strip_prefix("Preset Name=<") {
                name = Some(
                    rest.rsplit_once("> size=")
                        .ok_or("Invalid preset name")?
                        .0
                        .to_owned(),
                );
            }
            if let Some(rest) = line.strip_prefix("C:<") {
                let (key, rest) = rest.split_once('>').ok_or("Missing field name")?;
                if key.is_empty() || key.len() > 99 {
                    return Err("Invalid field name".into());
                }
                let (value, index) = rest.rsplit_once(";D:").ok_or("Missing field index")?;
                let index = index
                    .strip_suffix(';')
                    .ok_or("Invalid index terminator")?
                    .parse::<u32>()
                    .map_err(|_| "Invalid field index")?;
                let value = if let Some(v) = value.strip_prefix("Int:") {
                    Value::Int(v.parse().map_err(|_| "Invalid integer")?)
                } else if let Some(v) = value.strip_prefix("Cent:") {
                    Value::Cent(v.parse().map_err(|_| "Invalid cent value")?)
                } else if let Some(v) = value.strip_prefix("String:<") {
                    Value::Choice(v.strip_suffix('>').ok_or("Invalid choice")?.into())
                } else if let Some(v) = value.strip_prefix("UserString:<") {
                    Value::Text(v.strip_suffix('>').ok_or("Invalid text")?.into())
                } else {
                    return Err("Unsupported field type".into());
                };
                if fields.insert(key.into(), Field { value, index }).is_some() {
                    return Err("Duplicate field".into());
                }
            }
        }
        if fields.is_empty() {
            return Err("Document has no fields".into());
        }
        Ok(Self {
            name: name.ok_or("Missing preset name")?,
            fields,
            raw: raw.into(),
        })
    }
}
