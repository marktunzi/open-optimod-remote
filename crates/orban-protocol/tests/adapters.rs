use orban_protocol::adapter::{
    AdapterRegistry, DeviceModel, Evidence, MeterProfile, SkinId, TransportKind,
};
use orban_protocol::profile::Profile;

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
            "8700HD V 1.0.2.160",
            DeviceModel::Optimod8700Hd,
            SkinId::Optimod8700Hd,
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
fn only_exact_profile_banners_enable_writes_and_live_meter_mapping() {
    let verified = AdapterRegistry::identify_banner("5700i V 3.0.1.20").unwrap();
    assert!(verified.capabilities.parameter_writes);
    assert!(verified.capabilities.preset_recall);
    assert_eq!(verified.evidence, Evidence::Hardware);
    assert_eq!(
        verified.meter_profile,
        Some(MeterProfile {
            banks: &[1, 2],
            min_values: 112,
            max_values: 112,
        })
    );

    for (banner, model) in [
        ("5500 V 1.2.8.24", DeviceModel::Optimod5500),
        ("8700HD V 1.0.2.161", DeviceModel::Optimod8700Hd),
    ] {
        let adapter = AdapterRegistry::identify_banner(banner).unwrap();
        assert_eq!(adapter.model, model);
        assert_eq!(adapter.evidence, Evidence::Static, "{banner}");
        assert!(adapter.capabilities.parameter_writes, "{banner}");
        assert!(adapter.capabilities.preset_recall, "{banner}");
        assert!(adapter.capabilities.live_meters, "{banner}");
    }

    for banner in [
        "5700i V 3.0.1.21",
        "5500 V 1.2.8.23",
        "5500 V 1.2.8.24.1",
        "8700HD V 1.0.2.160",
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

#[test]
fn every_writable_adapter_has_a_valid_embedded_profile() {
    for (banner, adapter) in AdapterRegistry::exact() {
        assert!(adapter.capabilities.parameter_writes, "{banner}");
        let profile = Profile::for_adapter(adapter.id)
            .unwrap()
            .unwrap_or_else(|| panic!("{banner} has no embedded profile"));
        assert!(!profile.fields.is_empty(), "{banner}");
        if adapter.evidence == Evidence::Static {
            assert!(profile.layouts.is_some(), "{banner} needs its own pages");
            assert!(profile.meters.is_some(), "{banner} needs its own meters");
        }
    }
    for adapter in AdapterRegistry::all() {
        assert!(!adapter.capabilities.parameter_writes, "{}", adapter.id);
        assert!(
            Profile::for_adapter(adapter.id).unwrap().is_none(),
            "{}",
            adapter.id
        );
    }
}

#[test]
fn the_8700hd_is_its_own_model_and_is_found_by_its_terminal_banner() {
    assert_eq!(
        serde_json::to_string(&DeviceModel::Optimod8700Hd).unwrap(),
        "\"optimod-8700hd\""
    );
    assert_eq!(
        AdapterRegistry::identify_terminal_banner("Welcome to the Orban Optimod-FM 8700HD.")
            .unwrap(),
        DeviceModel::Optimod8700Hd
    );
    assert_ne!(
        AdapterRegistry::identify_banner("8700HD V 1.0.2.161")
            .unwrap()
            .model,
        DeviceModel::Optimod8700i
    );
}
