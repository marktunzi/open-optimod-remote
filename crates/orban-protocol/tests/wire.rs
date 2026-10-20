use orban_protocol::{
    auth::login_request,
    framing::{Decoder, Frame, FrameCut, encode},
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

const CUT: FrameCut = FrameCut;

/// The on-air document a 5700i pushes after a terminal `RP`, as captured on the
/// reference unit: the header announces the whole file, the wire carries only
/// the first 4096 bytes, then the next frame starts.
fn pushed_document(declared: usize) -> (Vec<u8>, Vec<u8>) {
    let mut payload = format!("{:8}{:8}0\n0\n\n{}\n", 229 - 204, 6, declared - 26).into_bytes();
    payload.extend((0..declared - payload.len()).map(|i| (i * 131 % 251) as u8));
    let frame = encode(&payload).unwrap();
    (frame[..4096].to_vec(), payload[..4091].to_vec())
}

#[test]
fn a_pushed_document_cut_at_4096_bytes_keeps_the_following_frames() {
    let (cut, sent) = pushed_document(7417);
    let meter = [b"      24       6\x01112\n".as_slice(), &[7; 112]].concat();
    let stream = [
        cut,
        encode(&meter).unwrap(),
        encode(b"      47       6clock").unwrap(),
    ]
    .concat();
    for split in (0..=stream.len())
        .step_by(7)
        .chain([4095, 4096, 4097, 4101, 4117])
    {
        let mut decoder = Decoder::with_cut(CUT);
        let mut frames = decoder.feed_frames(&stream[..split]).unwrap();
        frames.extend(decoder.feed_frames(&stream[split..]).unwrap());
        assert_eq!(
            frames,
            vec![
                Frame::Truncated {
                    payload: sent.clone(),
                    declared: 7417
                },
                Frame::Complete(meter.clone()),
                Frame::Complete(b"      47       6clock".to_vec()),
            ],
            "split at {split}"
        );
        decoder.finish().unwrap();
    }
}

#[test]
fn a_requested_archive_longer_than_4096_bytes_arrives_whole() {
    // Opcode 239 returns the on-air archive complete (7718 bytes on the unit).
    for declared in [4092, 4093, 4100, 4112, 7713] {
        let mut payload = format!("{:8}{:8}0\n0\n\n{}\n", 229 - 204, 6, declared - 26).into_bytes();
        payload.extend((0..declared - payload.len()).map(|i| (i * 7 % 256) as u8));
        let stream = [
            encode(&payload).unwrap(),
            encode(b"      47       6").unwrap(),
        ]
        .concat();
        for split in (0..=stream.len())
            .step_by(97)
            .chain([4096, 4101, stream.len() - 1])
        {
            let mut decoder = Decoder::with_cut(CUT);
            let mut frames = decoder.feed_frames(&stream[..split]).unwrap();
            frames.extend(decoder.feed_frames(&stream[split..]).unwrap());
            assert_eq!(
                frames,
                vec![
                    Frame::Complete(payload.clone()),
                    Frame::Complete(b"      47       6".to_vec())
                ],
                "{declared} split at {split}"
            );
        }
    }
}

#[test]
fn a_header_without_a_message_prefix_at_the_cut_does_not_end_the_frame() {
    // A body may contain bytes that happen to form a valid header at 4096;
    // without the opcode and protocol fields that follow a real one, the frame
    // is read to its announced length.
    let mut payload = format!("{:8}{:8}", 229 - 204, 6).into_bytes();
    payload.resize(4091, 0x55);
    payload.extend(&encode(b"x").unwrap()[..5]);
    payload.extend(b"not a prefix....");
    payload.resize(5000, 0x33);
    let stream = encode(&payload).unwrap();
    let mut decoder = Decoder::with_cut(CUT);
    assert_eq!(
        decoder.feed_frames(&stream).unwrap(),
        vec![Frame::Complete(payload)]
    );
}

#[test]
fn without_a_cut_the_decoder_still_reads_the_announced_length() {
    let (cut, _) = pushed_document(7417);
    let stream = [cut, encode(b"      47       6clock").unwrap()].concat();
    let mut decoder = Decoder::default();
    assert!(decoder.feed(&stream).unwrap().is_empty());
}

#[test]
fn any_frame_ends_where_a_real_frame_starts_inside_it() {
    // The virtual 5700i cut meter records at arbitrary points while it
    // switched presets; the next frame started where the bytes stopped.
    let meter = [b"      24       6\x01112\n".as_slice(), &[7; 112]].concat();
    let whole = encode(&meter).unwrap();
    for sent in [6, 22, 97, 116, whole.len() - 1] {
        let stream = [
            &whole[..sent],
            &whole[..],
            &encode(b"      47       6").unwrap(),
        ]
        .concat();
        let mut decoder = Decoder::with_cut(CUT);
        let frames = decoder.feed_frames(&stream).unwrap();
        assert_eq!(frames.len(), 3, "sent {sent}");
        assert!(
            matches!(&frames[0], Frame::Truncated { declared: 133, .. }),
            "sent {sent}"
        );
        assert_eq!(frames[1], Frame::Complete(meter.clone()));
    }
}

#[test]
fn a_push_cut_before_4096_bytes_keeps_the_boundary_behind_it() {
    // Virtual 5700i, 20 October 2026: the pushed document ended at 4075 wire
    // bytes, a 21-byte 251 followed, and the next frame began at 4096. A rule
    // that cuts only at 4096 swallowed that 251.
    let (cut, _) = pushed_document(7381);
    let boundary = encode(b"      47       6").unwrap();
    let meter = [b"      24       6\x01112\n".as_slice(), &[7; 112]].concat();
    let stream = [&cut[..4075], &boundary[..], &encode(&meter).unwrap()].concat();
    for split in [4075, 4080, 4096, 4100, stream.len()] {
        let mut decoder = Decoder::with_cut(CUT);
        let mut frames = decoder.feed_frames(&stream[..split]).unwrap();
        frames.extend(decoder.feed_frames(&stream[split..]).unwrap());
        assert_eq!(
            frames,
            vec![
                Frame::Truncated {
                    payload: cut[5..4075].to_vec(),
                    declared: 7381
                },
                Frame::Complete(b"      47       6".to_vec()),
                Frame::Complete(meter.clone()),
            ],
            "split at {split}"
        );
    }
}

#[test]
fn a_frame_not_followed_by_a_frame_is_corrupt() {
    let mut decoder = Decoder::with_cut(CUT);
    let stream = [encode(b"      47       6").unwrap(), vec![0; 30]].concat();
    assert!(decoder.feed_frames(&stream).is_err());
    assert!(decoder.is_failed());
}

/// A raw PC Remote capture of the virtual 5700i (original firmware 3.0.1.20)
/// while a terminal RP recalled GREGG: the pushed document (announced 7381
/// bytes) is cut at 4075 wire bytes and five later meter records are cut
/// short; two of those cuts (4075, 117) end exactly 21 bytes, one 251, before
/// the announced end, so only a search inside the frame finds them.
#[test]
fn the_virtual_5700i_recall_capture_decodes_without_losing_a_frame() {
    let stream = include_bytes!("fixtures/5700i-virtual-recall-capture.bin");
    for chunk in [1, 7, 1460, stream.len()] {
        let mut decoder = Decoder::with_cut(CUT);
        let mut frames = Vec::new();
        for part in stream.chunks(chunk) {
            frames.extend(decoder.feed_frames(part).unwrap());
        }
        let cut: Vec<_> = frames
            .iter()
            .filter_map(|f| match f {
                Frame::Truncated { payload, declared } => Some((payload.len() + 5, *declared)),
                Frame::Complete(_) => None,
            })
            .collect();
        assert_eq!(
            cut,
            [
                (4075, 7381),
                (97, 133),
                (117, 133),
                (22, 133),
                (116, 133),
                (76, 133)
            ],
            "chunk {chunk}"
        );
        // Counted independently: 193 whole meter records, 98 boundaries and
        // one 247 between the cuts; the capture ends on a frame boundary.
        let count = |prefix: &[u8]| {
            frames
                .iter()
                .filter(|f| matches!(f, Frame::Complete(p) if p.starts_with(prefix)))
                .count()
        };
        assert_eq!(
            (
                count(b"      24       6"),
                count(b"      47       6"),
                count(b"      43       6")
            ),
            (193, 98, 1),
            "chunk {chunk}"
        );
        decoder.finish().unwrap();
    }
}
