use orban_protocol::adapter::{AdapterRegistry, DeviceModel, MeterProfile, SkinId, TransportKind};

#[test]
fn official_pc_remote_banners_resolve_to_distinct_models_and_skins() {
    let cases = [
        (
            "5700i V 3.0.1.20",
            DeviceModel::Optimod5700i,
            SkinId::Optimod5700i,
        ),
        (
            "5500i V 3.1.0",
            DeviceModel::Optimod5500i,
            SkinId::Optimod5500i,
        ),
        (
            "5500 V 2.0.0",
            DeviceModel::Optimod5500,
            SkinId::Optimod5500,
        ),
        (
            "5700FM V 1.0.0",
            DeviceModel::Optimod5700Fm,
            SkinId::Optimod5700Fm,
        ),
        (
            "5700HD V 1.0.0",
            DeviceModel::Optimod5700Hd,
            SkinId::Optimod5700Hd,
        ),
        (
            "8500 V 4.0.0",
            DeviceModel::Optimod8500,
            SkinId::Optimod8500,
        ),
        (
            "6300 V 4.1.1",
            DeviceModel::Optimod6300,
            SkinId::Optimod6300,
        ),
        (
            "8600 V 4.5.5",
            DeviceModel::Optimod8600,
            SkinId::Optimod8600,
        ),
        (
            "8700i V 1.5.1",
            DeviceModel::Optimod8700i,
            SkinId::Optimod8700i,
        ),
        (
            "9300 V 2.1.1",
            DeviceModel::Optimod9300,
            SkinId::Optimod9300,
        ),
        (
            "9400 V 2.0.1",
            DeviceModel::Optimod9400,
            SkinId::Optimod9400,
        ),
    ];

    for (banner, model, skin) in cases {
        let adapter = AdapterRegistry::identify_banner(banner).unwrap();
        assert_eq!(adapter.model, model);
        assert_eq!(adapter.skin, skin);
        assert_eq!(adapter.transport, TransportKind::PcRemoteTcp);
        assert_eq!(adapter.default_port, 6201);
        assert_ne!(adapter.product_mark, "");
    }
}

#[test]
fn only_the_verified_5700i_profile_enables_writes_and_live_meter_mapping() {
    let verified = AdapterRegistry::identify_banner("5700i V 3.0.1.20").unwrap();
    assert!(verified.capabilities.parameter_writes);
    assert!(verified.capabilities.preset_recall);
    assert_eq!(
        verified.meter_profile,
        Some(MeterProfile {
            banks: &[1, 2],
            values_per_bank: 112,
        })
    );

    for banner in [
        "5700i V 3.0.1.21",
        "5500i V 3.1.0",
        "5500 V 2.0.0",
        "5700FM V 1.0.0",
        "5700HD V 1.0.0",
        "8500 V 4.0.0",
        "6300 V 4.1.1",
        "8600 V 4.5.5",
        "8700i V 1.5.1",
        "9300 V 2.1.1",
        "9400 V 2.0.1",
    ] {
        let adapter = AdapterRegistry::identify_banner(banner).unwrap();
        assert!(!adapter.capabilities.parameter_writes, "{banner}");
        assert!(!adapter.capabilities.preset_recall, "{banner}");
        assert_eq!(adapter.meter_profile, None, "{banner}");
    }
}

#[test]
fn unknown_or_ambiguous_banners_never_inherit_a_known_write_profile() {
    for banner in ["", "OPTIMOD V 1.0", "5700 V 1.0", "Orban processor"] {
        assert!(
            AdapterRegistry::identify_banner(banner).is_err(),
            "{banner}"
        );
    }
}

#[test]
fn saved_model_choices_have_stable_wire_names_and_auto_default() {
    assert_eq!(DeviceModel::default(), DeviceModel::Auto);
    assert_eq!(
        serde_json::to_string(&DeviceModel::Optimod5500i).unwrap(),
        "\"optimod-5500i\""
    );
    assert_eq!(
        serde_json::from_str::<DeviceModel>("\"optimod-8500\"").unwrap(),
        DeviceModel::Optimod8500
    );
    assert_eq!(
        serde_json::to_string(&DeviceModel::Optimod8700i).unwrap(),
        "\"optimod-8700i\""
    );
}
