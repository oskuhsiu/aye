//! Periodic local ref observation. Object loading runs away from terminal input.
use aye::{
    error::Result,
    reader::{Reader, ReaderSnapshot},
};
use std::{
    io,
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Debug)]
pub enum Update {
    Snapshot(ReaderSnapshot),
    Failed(String),
}

trait Source {
    fn current_oid(&mut self) -> Result<Option<String>>;
    fn load(&mut self, oid: &str) -> Result<ReaderSnapshot>;
}
impl Source for Reader {
    fn current_oid(&mut self) -> Result<Option<String>> {
        Reader::current_oid(self)
    }
    fn load(&mut self, oid: &str) -> Result<ReaderSnapshot> {
        Reader::load(self, oid)
    }
}
struct Poller<S> {
    source: S,
    attempted_oid: Option<String>,
    error: Option<String>,
}
impl<S: Source> Poller<S> {
    fn new(source: S, initial_oid: String) -> Self {
        Self {
            source,
            attempted_oid: Some(initial_oid),
            error: None,
        }
    }
    fn failure(&mut self, message: String, force: bool) -> Option<Update> {
        if !force && self.error.as_ref() == Some(&message) {
            return None;
        }
        self.error = Some(message.clone());
        Some(Update::Failed(message))
    }
    fn poll(&mut self, force: bool) -> Option<Update> {
        let oid = match self.source.current_oid() {
            Ok(Some(oid)) => oid,
            Ok(None) => {
                self.attempted_oid = None;
                return self.failure(
                    "NOT_INITIALIZED: No aye task state found. Run aye init.".into(),
                    force,
                );
            }
            Err(error) => {
                self.attempted_oid = None;
                return self.failure(error.to_string(), force);
            }
        };
        if !force && self.attempted_oid.as_ref() == Some(&oid) {
            return None;
        }
        self.attempted_oid = Some(oid.clone());
        // Loading this immutable OID remains consistent if the ref moves now.
        match self.source.load(&oid) {
            Ok(snapshot) => {
                self.error = None;
                Some(Update::Snapshot(snapshot))
            }
            Err(error) => self.failure(error.to_string(), force),
        }
    }
}

/// Holds only the newest pending update, so a slow renderer cannot accumulate
/// a queue of full snapshots. Dropping it stops the watcher without ref writes.
pub struct Watcher {
    wake: Option<Sender<()>>,
    latest: Arc<Mutex<Option<Update>>>,
    worker: Option<JoinHandle<()>>,
}
impl Watcher {
    pub fn start(reader: Reader, initial_oid: String) -> io::Result<Self> {
        let (wake, requests) = mpsc::channel();
        let latest = Arc::new(Mutex::new(None));
        let updates = Arc::clone(&latest);
        let worker = thread::Builder::new()
            .name("aye-view-ref".into())
            .spawn(move || {
                let mut poller = Poller::new(reader, initial_oid);
                loop {
                    let force = match requests.recv_timeout(Duration::from_millis(500)) {
                        Ok(()) => {
                            while requests.try_recv().is_ok() {}
                            true
                        }
                        Err(RecvTimeoutError::Timeout) => false,
                        Err(RecvTimeoutError::Disconnected) => break,
                    };
                    if let Some(update) = poller.poll(force) {
                        let old = updates
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .replace(update);
                        drop(old);
                    }
                }
            })?;
        Ok(Self {
            wake: Some(wake),
            latest,
            worker: Some(worker),
        })
    }
    pub fn refresh(&self) {
        if let Some(wake) = &self.wake {
            let _ = wake.send(());
        }
    }
    pub fn take_update(&self) -> Option<Update> {
        self.latest.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}
impl Drop for Watcher {
    fn drop(&mut self) {
        self.wake.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
