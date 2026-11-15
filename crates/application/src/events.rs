use async_broadcast::{InactiveReceiver, Receiver, Sender, TryRecvError};

use crate::SceneSnapshot;

const CAPACITY: usize = 16;

/// Fan-out of device state snapshots to every interested adapter (SSE, MQTT).
///
/// Notifying never blocks: when a subscriber falls behind, its oldest
/// snapshots are dropped, since only the latest state matters.
#[derive(Clone)]
pub struct StateEventBus {
    tx: Sender<SceneSnapshot>,
    // Keeps the channel open while nobody is subscribed.
    keepalive: InactiveReceiver<SceneSnapshot>,
}

/// One subscriber's view of the bus.
pub struct Subscription {
    rx: Receiver<SceneSnapshot>,
}

impl StateEventBus {
    pub fn new() -> Self {
        let (mut tx, rx) = async_broadcast::broadcast(CAPACITY);
        tx.set_overflow(true);
        tx.set_await_active(false);
        Self {
            tx,
            keepalive: rx.deactivate(),
        }
    }

    pub fn notify(&self, snapshot: SceneSnapshot) {
        // Only fails when nobody is subscribed, which is fine.
        let _ = self.tx.try_broadcast(snapshot);
    }

    pub fn subscribe(&self) -> Subscription {
        Subscription {
            rx: self.keepalive.activate_cloned(),
        }
    }
}

impl Default for StateEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Subscription {
    /// Wait for the next snapshot. `None` once the bus is gone.
    pub async fn recv(&mut self) -> Option<SceneSnapshot> {
        loop {
            match self.rx.recv().await {
                Ok(s) => return Some(s),
                Err(async_broadcast::RecvError::Overflowed(_)) => continue,
                Err(async_broadcast::RecvError::Closed) => return None,
            }
        }
    }

    /// Next already-queued snapshot, if any.
    pub fn try_recv(&mut self) -> Option<SceneSnapshot> {
        loop {
            match self.rx.try_recv() {
                Ok(s) => return Some(s),
                Err(TryRecvError::Overflowed(_)) => continue,
                Err(_) => return None,
            }
        }
    }
}
