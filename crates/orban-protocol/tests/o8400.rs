use orban_protocol::{
    document::{Field, Value},
    o8400,
};

// Bodies captured from the original 8400 PC Remote 3.0 and firmware 3.0.5
// (virtual 8400, 9 and 21 October 2026).

#[test]
fn login_answer_names_the_unit_and_its_access_level() {
    let login = o8400::parse_login(
        "\n\n8400 Station Name: undefined\n8400 Software Version: 3.0.5.7\nAccess Level: 0\nHD Digital Radio Hardware Installed\n",
    )
    .unwrap();
    assert_eq!(login.station, "undefined");
    assert_eq!(login.software, "3.0.5.7");
    assert_eq!(login.access_level, 0);
    assert!(login.hd_radio);
    assert_eq!(
        o8400::parse_login("Invalid Passcode").unwrap_err(),
        "password failed"
    );
    assert!(o8400::parse_login("8400 Station Name: x\n").is_err());
}

#[test]
fn messages_use_the_shared_frame_and_the_8400_types() {
    let bytes = o8400::encode(o8400::LOGIN, b"ADMIN").unwrap();
    assert_eq!(&bytes[5..], b"       0       6ADMIN");
    let (kind, payload) =
        o8400::decode(b"      25       6\x05\x00\x04\x00\x03\x00\x00\x00\x00\x00").unwrap();
    assert_eq!(kind, o8400::CALL_ANSWER);
    assert_eq!(payload.len(), 10);
}

#[test]
fn file_calls_encode_length_prefixed_arguments() {
    let chdir = o8400::call(4, &[&o8400::text_argument(o8400::USER_DIR).unwrap()]).unwrap();
    assert_eq!(chdir, b"\x04\x00\x0e\x00c:\\8400\\user\\\x00\x00\x00");
    let open = o8400::call(5, &[b"onair.orb\0", &[0, 0, 0, 0]]).unwrap();
    assert_eq!(
        open,
        b"\x05\x00\n\x00onair.orb\x00\x04\x00\x00\x00\x00\x00\x00\x00"
    );
    let (function, values) =
        o8400::parse_answer(b"\x05\x00\x04\x00\x03\x00\x00\x00\x00\x00").unwrap();
    assert_eq!(function, 5);
    assert_eq!(o8400::word(&values[0]).unwrap(), 3);
    assert!(o8400::parse_answer(b"\x05\x00\x04\x00\x03\x00").is_err());
    assert!(o8400::text_argument("a\0b").is_err());
}

#[test]
fn changes_match_the_original_client() {
    let field = |value, index| Field { value, index };
    assert_eq!(
        o8400::change_payload("SE AMOUNT", &field(Value::Cent(275), 11)).unwrap(),
        b"SE AMOUNT;11;Cent:275;"
    );
    assert_eq!(
        o8400::change_payload("PHASE ROTATOR", &field(Value::Choice("Out".into()), 1)).unwrap(),
        b"PHASE ROTATOR;1;String:<Out>;"
    );
    assert_eq!(
        o8400::change_payload("DI REF PPM LEVEL", &field(Value::Cent(-450), 37)).unwrap(),
        b"DI REF PPM LEVEL;37;Cent:-450;"
    );
    assert!(o8400::change_payload("A;B", &field(Value::Int(1), 1)).is_err());
    assert!(o8400::change_payload("X", &field(Value::Choice("a>b".into()), 1)).is_err());
}

#[test]
fn recall_is_a_gui_event_with_the_preset_name() {
    assert_eq!(
        o8400::event_payload(16395, 28, "GREGG LL").unwrap(),
        b"16395;28;GREGG LL;"
    );
    assert!(o8400::event_payload(16395, 28, "A;B").is_err());
}

#[test]
fn stored_documents_end_at_end_preset() {
    let mut file =
        b"OptimodVersion=<00.08>\nPreset Name=<X> size=1\nC:<AGC>String:<On>;D:1;\nEnd Preset\n"
            .to_vec();
    let length = file.len();
    file.extend_from_slice(&[0xff; 40]);
    assert_eq!(o8400::document_text(&file).unwrap().len(), length);
    assert!(o8400::document_text(b"OptimodVersion=<00.08>\n").is_err());
}
