use mqtt::publish::active_scene_state_payload;

// HA's MQTT select compares the raw state payload against its options list.
// A JSON-encoded string ("\"Sunset\"") never matches, so HA shows "unknown".
#[test]
fn active_scene_state_is_raw_name_not_json() {
    assert_eq!(active_scene_state_payload("Sunset"), b"Sunset".to_vec());
}

#[test]
fn active_scene_state_empty_is_empty_payload() {
    assert_eq!(active_scene_state_payload(""), Vec::<u8>::new());
}
