use mqtt::wire::{self, Incoming};

#[test]
fn connect_without_credentials_matches_spec_bytes() {
    let bytes = wire::connect("ab", 60, true, None, None);
    assert_eq!(
        bytes,
        vec![
            0x10,
            14, // CONNECT, remaining length
            0,
            4,
            b'M',
            b'Q',
            b'T',
            b'T',
            4,           // protocol name + level 4 (3.1.1)
            0b0000_0010, // clean session
            0,
            60, // keep alive
            0,
            2,
            b'a',
            b'b', // client id
        ]
    );
}

#[test]
fn connect_with_credentials_sets_flags_and_appends_fields() {
    let bytes = wire::connect("c", 30, true, Some("u"), Some("p"));
    assert_eq!(bytes[9], 0b1100_0010); // username + password + clean session
    assert_eq!(&bytes[bytes.len() - 6..], &[0, 1, b'u', 0, 1, b'p']);
}

#[test]
fn publish_qos1_round_trips_through_decode() {
    let bytes = wire::publish("a/b", b"hi", true, Some(7));
    let (packet, used) = wire::decode(&bytes).unwrap().unwrap();
    assert_eq!(used, bytes.len());
    assert_eq!(
        packet,
        Incoming::Publish {
            topic: "a/b".into(),
            payload: b"hi".to_vec(),
            pid: Some(7),
        }
    );
    assert_eq!(bytes[0], 0x30 | 0b0010 | 0b0001); // PUBLISH, QoS1, retain
}

#[test]
fn large_payload_uses_multi_byte_remaining_length() {
    let payload = vec![b'x'; 1_000_000];
    let bytes = wire::publish("t", &payload, false, None);
    let (packet, used) = wire::decode(&bytes).unwrap().unwrap();
    assert_eq!(used, bytes.len());
    let Incoming::Publish { payload: got, .. } = packet else {
        panic!("expected publish");
    };
    assert_eq!(got.len(), 1_000_000);
}

#[test]
fn partial_frame_decodes_to_none() {
    let bytes = wire::publish("topic", b"payload", false, None);
    for cut in 0..bytes.len() {
        assert_eq!(wire::decode(&bytes[..cut]).unwrap(), None, "cut at {cut}");
    }
}

#[test]
fn decode_reports_length_of_first_frame_only() {
    let mut bytes = wire::publish("a", b"1", false, None);
    let first = bytes.len();
    bytes.extend(wire::pingreq());
    let (_, used) = wire::decode(&bytes).unwrap().unwrap();
    assert_eq!(used, first);
}

#[test]
fn decodes_broker_control_packets() {
    assert_eq!(
        wire::decode(&[0x20, 2, 0, 0]).unwrap(),
        Some((Incoming::ConnAck { code: 0 }, 4))
    );
    assert_eq!(
        wire::decode(&[0x20, 2, 0, 5]).unwrap(),
        Some((Incoming::ConnAck { code: 5 }, 4))
    );
    assert_eq!(
        wire::decode(&[0xD0, 0]).unwrap(),
        Some((Incoming::PingResp, 2))
    );
    assert_eq!(
        wire::decode(&[0x40, 2, 0, 9]).unwrap(),
        Some((Incoming::PubAck { pid: 9 }, 4))
    );
    assert_eq!(
        wire::decode(&[0x90, 3, 0, 4, 1]).unwrap(),
        Some((Incoming::SubAck { pid: 4 }, 5))
    );
}

#[test]
fn subscribe_and_acks_encode_per_spec() {
    assert_eq!(
        wire::subscribe(3, &["x/+"], 1),
        vec![0x82, 8, 0, 3, 0, 3, b'x', b'/', b'+', 1]
    );
    assert_eq!(wire::puback(258), vec![0x40, 2, 1, 2]);
    assert_eq!(wire::pingreq(), vec![0xC0, 0]);
    assert_eq!(wire::disconnect(), vec![0xE0, 0]);
}

#[test]
fn malformed_remaining_length_is_an_error() {
    assert!(wire::decode(&[0x30, 0xFF, 0xFF, 0xFF, 0xFF, 0x01]).is_err());
}
