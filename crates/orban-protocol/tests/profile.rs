use orban_protocol::{control::Scope, document::Value, profile::Profile};
#[test]
fn contrast_boundaries_and_agc_quarter_db_steps_are_exact() {
    let p = Profile::embedded().unwrap();
    assert_eq!(
        p.value(Scope::System, "CONTRAST", 2).unwrap(),
        Value::Int(2)
    );
    assert!(p.value(Scope::System, "CONTRAST", 4).is_err());
    assert!(p.value(Scope::Processing, "CONTRAST", 2).is_err());
    assert_eq!(
        p.value(Scope::Processing, "AGC BASS TH", 0).unwrap(),
        Value::Cent(-1200)
    );
    assert_eq!(
        p.value(Scope::Processing, "AGC BASS TH", 1).unwrap(),
        Value::Cent(-1175)
    );
}
#[test]
fn choices_and_off_sentinels_keep_index_order() {
    let p = Profile::embedded().unwrap();
    assert_eq!(
        p.value(Scope::System, "INPUT A OR D", 1).unwrap(),
        Value::Choice("Digital".into())
    );
    assert!(p.value(Scope::System, "UNKNOWN", 0).is_err());
}

#[test]
fn equalizer_frequency_ranges_preserve_piecewise_steps_and_units() {
    let p = Profile::embedded().unwrap();
    for prefix in ["", "HD "] {
        for (suffix, index, expected) in [
            ("LOW BASS FREQ", 0, Value::Int(80)),
            ("LOW BASS FREQ", 13, Value::Int(145)),
            ("LOW BASS FREQ", 14, Value::Int(150)),
            ("LOW BASS FREQ", 24, Value::Int(250)),
            ("LOW BASS FREQ", 29, Value::Int(350)),
            ("LOW BASS FREQ", 34, Value::Int(500)),
            ("PEQ LOW FREQ", 0, Value::Cent(2000)),
            ("PEQ LOW FREQ", 37, Value::Cent(26630)),
            ("PEQ LOW FREQ", 46, Value::Cent(50000)),
            ("PEQ MID FREQ", 0, Value::Int(250)),
            ("PEQ MID FREQ", 33, Value::Int(2570)),
            ("PEQ MID FREQ", 45, Value::Int(6000)),
            ("PEQ HIGH FREQ", 0, Value::Cent(100)),
            ("PEQ HIGH FREQ", 20, Value::Cent(410)),
            ("PEQ HIGH FREQ", 38, Value::Cent(1500)),
        ] {
            assert_eq!(
                p.value(Scope::Processing, &format!("{prefix}{suffix}"), index)
                    .unwrap(),
                expected
            );
        }
        assert!(
            p.value(Scope::Processing, &format!("{prefix}LOW BASS FREQ"), 35)
                .is_err()
        );
    }
    let json = serde_json::to_value(p).unwrap();
    let fields = json["fields"].as_array().unwrap();
    for (name, unit) in [
        ("PEQ LOW FREQ", "Hz"),
        ("PEQ HIGH FREQ", "kHz"),
        ("AGC BASS TH", "dB"),
    ] {
        assert_eq!(
            fields.iter().find(|f| f["name"] == name).unwrap()["unit"],
            unit
        );
    }
}

#[test]
fn large_integer_ranges_do_not_need_large_enumeration_arrays() {
    let p = Profile::embedded().unwrap();
    assert_eq!(
        p.value(Scope::System, "NETWORK PORT", 6201).unwrap(),
        Value::Int(6201)
    );
    assert!(p.value(Scope::System, "NETWORK PORT", 0).is_err());
    assert!(p.value(Scope::System, "NETWORK PORT", 65535).is_err());
    assert_eq!(
        p.value(Scope::System, "RDS UECP SITE", 1022).unwrap(),
        Value::Int(1022)
    );
    assert!(p.value(Scope::System, "RDS UECP SITE", 1023).is_err());
}

#[test]
fn bass_coupling_loudness_thresholds_and_sample_delay_are_exact() {
    let p = Profile::embedded().unwrap();
    assert_eq!(
        p.value(Scope::Processing, "AGC BASS COUPLE", 0).unwrap(),
        Value::Choice("Off".into())
    );
    assert_eq!(
        p.value(Scope::Processing, "AGC BASS COUPLE", 10).unwrap(),
        Value::Int(3)
    );
    assert!(p.value(Scope::Processing, "AGC BASS COUPLE", 14).is_err());
    for name in ["BS1770 LDNES CTRL THR", "FM BS1770 LDNES CTRL THR"] {
        assert_eq!(p.value(Scope::System, name, 0).unwrap(), Value::Cent(-3100));
        assert_eq!(
            p.value(Scope::System, name, 200).unwrap(),
            Value::Cent(-1100)
        );
        assert!(p.value(Scope::System, name, 201).is_err());
    }
    assert_eq!(
        p.value(Scope::System, "DIVERSITY DELAY ADJ", 497590)
            .unwrap(),
        Value::Choice("8.139859375".into())
    );
    assert_eq!(
        p.value(Scope::System, "DIVERSITY DELAY ADJ", 0).unwrap(),
        Value::Choice("0.365015625".into())
    );
    assert_eq!(
        p.value(Scope::System, "DIVERSITY DELAY ADJ", 0xf833d)
            .unwrap(),
        Value::Choice("16.249968750".into())
    );
    assert!(
        p.value(Scope::System, "DIVERSITY DELAY ADJ", 0xf833e)
            .is_err()
    );
}

fn static_profile(adapter: &str) -> Profile {
    Profile::for_adapter(adapter).unwrap().unwrap()
}

#[test]
fn optimod_5500_values_follow_the_firmware_wire_format() {
    let p = static_profile("pc-remote-5500-1.2.8.24");
    // Confirmed by factory presets: index 7 is Cent 1100, the last index is Off.
    assert_eq!(
        p.value(Scope::Processing, "2B BASS ATTACK", 7).unwrap(),
        Value::Cent(1100)
    );
    assert_eq!(
        p.value(Scope::Processing, "2B BASS ATTACK", 47).unwrap(),
        Value::Choice("Off".into())
    );
    // PC Remote shows Int 4; the processor documents Cent 400.
    assert_eq!(
        p.value(Scope::Processing, "B1 ATTACK", 0).unwrap(),
        Value::Cent(400)
    );
    assert_eq!(
        p.value(Scope::Processing, "AGC BASS COUPLE", 13).unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        p.value(Scope::System, "DIVERSITY DELAY ADJ", 0).unwrap(),
        Value::Choice("0.011890625".into())
    );
    assert_eq!(
        p.value(Scope::System, "NETWORK PORT", 6201).unwrap(),
        Value::Int(6201)
    );
    // Conflicting with the factory presets: never writable.
    assert!(p.value(Scope::Processing, "PEQ MID WIDTH", 0).is_err());
    // A 5700i-only HD field does not exist on the 5500.
    assert!(p.value(Scope::Processing, "HD COUPLING", 0).is_err());
}

#[test]
fn optimod_8700hd_values_follow_the_firmware_wire_format() {
    let p = static_profile("pc-remote-8700hd-1.0.2.161");
    assert_eq!(
        p.value(Scope::Processing, "HD COUPLING", 0).unwrap(),
        Value::Choice("FM->HD".into())
    );
    assert_eq!(
        p.value(Scope::Processing, "AGC BASS COUPLE", 10).unwrap(),
        Value::Int(3)
    );
    assert_eq!(
        p.value(Scope::System, "BS1770 LDNES CTRL THR", 0).unwrap(),
        Value::Cent(-3100)
    );
    assert_eq!(
        p.value(Scope::System, "DIVERSITY DELAY ADJ", 0).unwrap(),
        Value::Choice("0.000015625".into())
    );
    assert!(
        p.value(Scope::System, "DIVERSITY DELAY ADJ", 524_288)
            .is_err()
    );
    assert!(p.value(Scope::Processing, "MPX PWR OFFSET", 0).is_err());
}

#[test]
fn static_profiles_carry_their_own_pages_and_meters() {
    for adapter in ["pc-remote-5500-1.2.8.24", "pc-remote-8700hd-1.0.2.161"] {
        let p = static_profile(adapter);
        let pages = p.layouts.as_ref().unwrap().as_array().unwrap();
        assert!(pages.len() >= 10, "{adapter}");
        for page in pages {
            for control in page["controls"].as_array().unwrap() {
                let name = control["name"].as_str().unwrap();
                assert!(
                    p.fields.iter().any(|f| f.name == name),
                    "{adapter}: page control {name} has no profile entry"
                );
            }
        }
        let meters = p.meters.as_ref().unwrap();
        assert!(meters["groups"].as_array().unwrap().len() >= 7, "{adapter}");
        assert!(p.fields.iter().all(|f| f.evidence.is_some()), "{adapter}");
    }
    assert!(Profile::embedded().unwrap().layouts.is_none());
}
