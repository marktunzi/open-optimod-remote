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
