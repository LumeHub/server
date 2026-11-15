//! Background persistence of the device state (`state.json`).
//!
//! `save` only records the newest state and wakes the writer; bursts of
//! changes (e.g. a brightness slider) collapse into a single write of the
//! latest value, and callers never wait on disk I/O.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use persistence::PersistedState;
use smol::channel::{Receiver, Sender};

#[derive(Clone)]
pub struct StateSaver {
    latest: Arc<Mutex<Option<PersistedState>>>,
    wake: Sender<()>,
}

impl StateSaver {
    /// Start the writer task for `path`; it runs until every handle is dropped.
    pub fn spawn(path: PathBuf) -> Self {
        let latest = Arc::new(Mutex::new(None));
        // Capacity 1: a pending wake-up already covers any newer state.
        let (wake, rx) = smol::channel::bounded(1);
        smol::spawn(write_loop(path, Arc::clone(&latest), rx)).detach();
        Self { latest, wake }
    }

    pub fn save(&self, state: PersistedState) {
        *self.latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(state);
        let _ = self.wake.try_send(());
    }
}

async fn write_loop(path: PathBuf, latest: Arc<Mutex<Option<PersistedState>>>, wake: Receiver<()>) {
    while wake.recv().await.is_ok() {
        let Some(state) = latest.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            continue;
        };
        let path = path.clone();
        if let Err(e) = smol::unblock(move || persistence::save(&path, &state)).await {
            eprintln!("warning: failed to save state: {e}");
        }
    }
}
