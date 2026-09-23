use orban_protocol::{
    adapter::DeviceModel,
    document::{Document, Value},
};
const DOC: &str = "OptimodVersion=<5700.51>\r\nPreset Name=<DEMO> size=3\r\nC:<COUPLING>String:<FM->HD>;D:0;\r\nC:<GAIN>Cent:-125;D:7;\r\nC:<LABEL>UserString:<Example>;D:0;\r\nFuture Metadata=<retain me>\r\nEnd Preset<end>\r\n";
#[test]
fn values_keep_their_types_and_device_indices() {
    let d = Document::parse(DOC.as_bytes()).unwrap();
    assert_eq!(d.name, "DEMO");
    assert_eq!(d.fields.len(), 3);
    assert_eq!(d.fields["COUPLING"].value, Value::Choice("FM->HD".into()));
    assert_eq!(d.fields["GAIN"].value, Value::Cent(-125));
    assert_eq!(d.fields["GAIN"].index, 7);
    assert_eq!(d.raw, DOC);
    assert_eq!(d.fields["LABEL"].value, Value::Text("Example".into()));
}

#[test]
fn document_families_are_parsed_only_for_the_detected_processor() {
    for (model, version) in [
        (DeviceModel::Optimod5500i, "8300.10"),
        (DeviceModel::Optimod5500, "8300.10"),
        (DeviceModel::Optimod5700Fm, "5700.50"),
        (DeviceModel::Optimod5700Hd, "5700.50"),
        (DeviceModel::Optimod8500, "8500.40"),
        (DeviceModel::Optimod6300, "6300.50"),
        (DeviceModel::Optimod8600, "8600.40"),
        (DeviceModel::Optimod8700i, "8700.51"),
        (DeviceModel::Optimod9300, "9300.30"),
        (DeviceModel::Optimod9400, "9400.30"),
    ] {
        let text = format!(
            "OptimodVersion=<{version}>\r\nPreset Name=<DEMO> size=1\r\nC:<AGC>String:<On>;D:1;\r\nEnd Preset<end>\r\n"
        );
        assert!(
            Document::parse_for_model(text.as_bytes(), model).is_ok(),
            "{model:?}"
        );
        if model != DeviceModel::Optimod5700Fm && model != DeviceModel::Optimod5700Hd {
            assert!(Document::parse(text.as_bytes()).is_err(), "{model:?}");
        }
    }
}
#[test]
fn malformed_decrypted_documents_never_become_device_state() {
    for doc in [
        DOC.replace("End Preset<end>", ""),
        DOC.replace("D:7", "D:-1"),
        DOC.replace("C:<LABEL>", "C:<GAIN>"),
        DOC.replace("OptimodVersion", "InvalidMagic"),
    ] {
        assert!(Document::parse(doc.as_bytes()).is_err());
    }
}
