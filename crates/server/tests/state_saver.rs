use std::time::{Duration, Instant};

use domain::Rgb;
use lumehub_server::state_saver::StateSaver;
use persistence::PersistedState;

fn state(brightness: u8) -> PersistedState {
    PersistedState {
        on: true,
        brightness,
        color: Rgb::BLACK,
        active_effect: None,
    }
}

fn wait_for(path: &std::path::Path, brightness: u8) -> PersistedState {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(s) = persistence::load(path)
            && s.brightness == brightness
        {
            return s;
        }
        assert!(
            Instant::now() < deadline,
            "state never reached brightness {brightness}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn saves_state_in_the_background() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let saver = StateSaver::spawn(path.clone());
    saver.save(state(42));
    assert_eq!(wait_for(&path, 42).brightness, 42);
}

#[test]
fn bursts_coalesce_and_the_latest_state_wins() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let saver = StateSaver::spawn(path.clone());
    // save() must never block the render/runtime path, however fast it's called.
    for b in 0..=200u8 {
        saver.save(state(b));
    }
    assert_eq!(wait_for(&path, 200).brightness, 200);
}

#[test]
fn clones_share_one_writer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let saver = StateSaver::spawn(path.clone());
    let other = saver.clone();
    other.save(state(7));
    wait_for(&path, 7);
    saver.save(state(8));
    wait_for(&path, 8);
}
