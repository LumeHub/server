use application::{SceneSnapshot, StateEventBus};
use domain::Rgb;

fn snapshot(brightness: u8) -> SceneSnapshot {
    SceneSnapshot {
        on: true,
        brightness,
        color: Rgb::new(10, 20, 30),
        active_effect: None,
        light_effect_end_unix_timestamp_sec: None,
    }
}

#[test]
fn subscriber_receives_notified_snapshot() {
    let bus = StateEventBus::new();
    let mut rx = bus.subscribe();

    bus.notify(snapshot(200));

    let received = rx.try_recv().expect("should have received snapshot");
    assert!(received.on);
    assert_eq!(received.brightness, 200);
    assert_eq!(received.color.r, 10);
}

#[test]
fn multiple_subscribers_each_receive_snapshot() {
    let bus = StateEventBus::new();
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();

    bus.notify(snapshot(1));

    assert!(rx1.try_recv().is_some());
    assert!(rx2.try_recv().is_some());
}

#[test]
fn subscriber_created_after_notify_misses_past_events() {
    let bus = StateEventBus::new();
    bus.notify(snapshot(1));

    let mut late_rx = bus.subscribe();
    assert!(late_rx.try_recv().is_none());
}

#[test]
fn slow_subscriber_skips_to_newer_snapshots_instead_of_blocking() {
    let bus = StateEventBus::new();
    let mut rx = bus.subscribe();

    // Far more than the channel capacity; notify must never block or panic.
    for b in 0..=255u8 {
        bus.notify(snapshot(b));
    }

    let first = rx.try_recv().expect("oldest retained snapshot");
    assert!(first.brightness > 0, "oldest snapshots were dropped");
    let mut last = first.brightness;
    while let Some(s) = rx.try_recv() {
        last = s.brightness;
    }
    assert_eq!(last, 255);
}

#[test]
fn bus_keeps_working_after_all_subscribers_drop() {
    let bus = StateEventBus::new();
    drop(bus.subscribe());
    bus.notify(snapshot(1));

    let mut rx = bus.subscribe();
    bus.notify(snapshot(2));
    assert_eq!(rx.try_recv().map(|s| s.brightness), Some(2));
}

#[test]
fn recv_awaits_next_snapshot() {
    let bus = StateEventBus::new();
    let mut rx = bus.subscribe();
    bus.notify(snapshot(7));
    let got = smol::block_on(rx.recv());
    assert_eq!(got.map(|s| s.brightness), Some(7));
}
