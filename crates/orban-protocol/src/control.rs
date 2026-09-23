use crate::document::{Field, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Scope {
    Processing,
    System,
}
pub fn encode_change(scope: Scope, name: &str, field: &Field) -> Result<Vec<u8>, String> {
    if name.is_empty()
        || name.len() > 99
        || !name.is_ascii()
        || name
            .bytes()
            .any(|b| b.is_ascii_control() || b";<>".contains(&b))
    {
        return Err("Invalid control name".into());
    }
    let value = match &field.value {
        Value::Int(v) => format!("Int:{v};"),
        Value::Cent(v) => format!("Cent:{v};"),
        Value::Choice(v) | Value::Text(v) => {
            if v.len() > 255
                || !v.is_ascii()
                || v.bytes()
                    .any(|b| b.is_ascii_control() || b";<>".contains(&b))
            {
                // FM->HD is a documented enum value; the arrow is not a closing delimiter.
                if !matches!(&field.value,Value::Choice(s) if s=="FM->HD") {
                    return Err("Unsupported characters in control value".into());
                }
            }
            format!(
                "{}:<{v}>;",
                if matches!(field.value, Value::Choice(_)) {
                    "String"
                } else {
                    "UserString"
                }
            )
        }
    };
    let scope = match scope {
        Scope::Processing => 1,
        Scope::System => 2,
    };
    crate::message::encode(
        227,
        format!("{name};{};{value}{scope};", field.index).as_bytes(),
    )
}
