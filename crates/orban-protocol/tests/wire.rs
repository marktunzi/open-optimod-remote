use orban_protocol::{
    auth::login_request,
    framing::{Decoder, encode},
};

#[test]
fn heartbeat_matches_the_observed_wire_frame() {
    assert_eq!(
        encode(b"      15       6").unwrap(),
        b"\x1a\xc2\x77\x58\x55      15       6"
    );
}

#[test]
fn fragments_and_combined_frames_produce_exactly_the_original_messages() {
    let frames = [
        encode(b"      16       6").unwrap(),
        encode(&[0, 255, 10, 195]).unwrap(),
    ]
    .concat();
    for split in 0..=frames.len() {
        let mut decoder = Decoder::default();
        let mut messages = decoder.feed(&frames[..split]).unwrap();
        messages.extend(decoder.feed(&frames[split..]).unwrap());
        assert_eq!(
            messages,
            vec![b"      16       6".to_vec(), vec![0, 255, 10, 195]]
        );
        decoder.finish().unwrap();
    }
}

#[test]
fn every_possible_wire_length_round_trips() {
    for n in 1..=65535 {
        let bytes = vec![0xa5; n];
        let mut decoder = Decoder::default();
        assert_eq!(decoder.feed(&encode(&bytes).unwrap()).unwrap(), vec![bytes]);
    }
    assert!(encode(&vec![0; 65536]).is_err());
    assert!(encode(&[]).is_err());
}

#[test]
fn malformed_checksum_and_length_complements_fail_closed() {
    let good = encode(b"      16       6").unwrap();
    for byte in 0..5 {
        let mut bad = good.clone();
        bad[byte] ^= 1;
        assert!(Decoder::default().feed(&bad).is_err());
    }
    let mut bad = good.clone();
    bad[0] ^= 1;
    bad[4] = bad[4].wrapping_sub(1);
    assert!(Decoder::default().feed(&bad).is_err());
}

#[test]
fn truncated_stream_is_not_a_successful_message() {
    let frame = encode(b"      16       6").unwrap();
    for n in 1..frame.len() {
        let mut decoder = Decoder::default();
        assert!(decoder.feed(&frame[..n]).unwrap().is_empty());
        assert!(decoder.finish().is_err());
    }
}

#[test]
fn login_preserves_delimiters_and_uses_legacy_obfuscation() {
    // Synthetic access code; independently calculated from the documented LCG.
    assert_eq!(
        login_request("9876").unwrap(),
        b"connect\n\xe8\x4c\xe0\x32\n\0"
    );
    for code in ["", "12\n34", "12\r34", "12\0", "12é", "12[34"] {
        assert!(login_request(code).is_err(), "invalid input was accepted");
    }
}
