use orban_protocol::presets::{PresetKind, parse_list, recall_command};
#[test]
fn names_with_spaces_and_modified_states_remain_distinct() {
    let p = parse_list("FACTORY OPEN factory\r\nMy Station user\r\nmodif My Station unsaved\r\n\n")
        .unwrap();
    assert_eq!(p.len(), 3);
    assert_eq!(p[1].name, "My Station");
    assert_eq!(p[2].kind, PresetKind::Unsaved);
    assert_eq!(
        &**recall_command("FACTORY OPEN", "9876").unwrap(),
        "RP FACTORY OPEN[9876]\r\n"
    );
}
#[test]
fn preset_names_cannot_inject_terminal_commands() {
    for name in ["", "A\r\nRP B", "A[9876]", "A\0B"] {
        assert!(recall_command(name, "9876").is_err());
    }
    assert!(parse_list("NAME arbitrary").is_err());
    assert!(parse_list("NAME factory\nNAME user").is_err());
    assert!(parse_list("").is_err());
}

#[test]
fn save_and_delete_commands_follow_the_5500_terminal_help() {
    use orban_protocol::{
        adapter::DeviceModel,
        presets::{
            delete_command, max_store_name_length, reply_refusal, store_command,
            validate_store_name,
        },
    };
    assert_eq!(
        &**store_command("MY STATION", "9876").unwrap(),
        "SP MY STATION[9876]\r\n"
    );
    assert_eq!(
        &**delete_command("MY STATION", "9876").unwrap(),
        "DP MY STATION[9876]\r\n"
    );
    assert!(store_command("X]\r\nRP Y[", "9876").is_err());

    assert_eq!(max_store_name_length(DeviceModel::Optimod5500), 18);
    assert_eq!(max_store_name_length(DeviceModel::Optimod5700i), 20);
    assert!(validate_store_name("EIGHTEEN CHARS 123", 18).is_ok());
    assert!(validate_store_name("NINETEEN CHARS 1234", 18).is_err());
    assert!(validate_store_name(" LEADING", 18).is_err());
    assert!(validate_store_name("Modif ROCK", 18).is_err());

    assert_eq!(
        reply_refusal(" SP: ROCK already exists. Please choose another name.\r\n").as_deref(),
        Some("SP: ROCK already exists. Please choose another name.")
    );
    assert!(reply_refusal(" DP: preset does not exist").is_some());
    assert_eq!(reply_refusal(" SP: ROCK\r\n"), None);
}
