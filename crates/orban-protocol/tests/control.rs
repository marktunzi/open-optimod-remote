use orban_protocol::{
    control::{Scope, encode_change},
    document::{Field, Value},
};
#[test]
fn typed_write_matches_verified_contrast_frame_and_preserves_scope() {
    let frame = encode_change(
        Scope::System,
        "CONTRAST",
        &Field {
            index: 2,
            value: Value::Int(2),
        },
    )
    .unwrap();
    assert_eq!(
        frame,
        orban_protocol::message::encode(227, b"CONTRAST;2;Int:2;2;").unwrap()
    );
    let frame = encode_change(
        Scope::Processing,
        "FM HD COUPLING",
        &Field {
            index: 0,
            value: Value::Choice("FM->HD".into()),
        },
    )
    .unwrap();
    assert!(frame.ends_with(b"FM HD COUPLING;0;String:<FM->HD>;1;"));
}
#[test]
fn delimiters_cannot_inject_additional_control_changes() {
    for (name, value) in [
        ("CONTRAST;", Value::Int(2)),
        ("LABEL", Value::Text("x>;2;".into())),
        ("LABEL", Value::Text("x\nY".into())),
    ] {
        assert!(encode_change(Scope::System, name, &Field { index: 0, value }).is_err());
    }
}
