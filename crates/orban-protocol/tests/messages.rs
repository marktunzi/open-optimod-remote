use orban_protocol::message::{Message, decode};

#[test]
fn meter_values_remain_binary_and_banks_are_not_mislabeled_as_fm_hd() {
    for bank in [1, 2] {
        let mut frame = b"      24       6".to_vec();
        frame.push(bank);
        frame.extend_from_slice(b"112\n");
        let values: Vec<u8> = (0..112).map(|x| if x == 1 { 255 } else { x }).collect();
        frame.extend_from_slice(&values);
        assert_eq!(decode(&frame).unwrap(), Message::Meters { bank, values });
        frame.pop();
        assert!(decode(&frame).is_err());
        frame.push(0);
        frame.push(0);
        assert!(decode(&frame).is_err());
    }
}

#[test]
fn file_data_is_length_delimited_even_when_it_contains_newlines() {
    let bytes = b"      25       6123\n456\nOnAir.orb57onair\n5\n\0\n\xffab";
    assert_eq!(
        decode(bytes).unwrap(),
        Message::File {
            kind: 229,
            name: "OnAir.orb57onair".into(),
            data: b"\0\n\xffab".to_vec()
        }
    );
    assert!(decode(&bytes[..bytes.len() - 1]).is_err());
    assert_eq!(
        decode(b"      25       6123\n456\nC:\\Orban\\Presets\\OnAir.orb57onair\n1\nx").unwrap(),
        Message::File {
            kind: 229,
            name: "C:\\Orban\\Presets\\OnAir.orb57onair".into(),
            data: b"x".to_vec(),
        }
    );
    assert_eq!(
        decode(b"      25       6123\n456\n\n1\nx").unwrap(),
        Message::File {
            kind: 229,
            name: String::new(),
            data: b"x".to_vec(),
        }
    );
    assert!(decode(b"      25       6123\n456\nBad\rName\n1\nx").is_err());
    assert!(decode(b"      25       6123\n456\nOnAir.orb57onair\n9999999999\nx").is_err());
}

#[test]
fn malformed_envelope_does_not_become_a_valid_control_message() {
    assert_eq!(decode(b"      16       6").unwrap(), Message::Heartbeat);
    for b in [
        b"      16       7".as_slice(),
        b"      16       6extra",
        b"       0",
        b"  garbage      6",
    ] {
        assert!(decode(b).is_err());
    }
    assert_eq!(
        decode(b"      43       6\x01\x02").unwrap(),
        Message::Other {
            kind: 247,
            data: vec![1, 2]
        }
    );
}
