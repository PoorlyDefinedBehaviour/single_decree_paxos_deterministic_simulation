use crate::{simulation::PendingMessage, types::ReplicaId};

#[derive(Debug)]
pub(crate) struct ActivityLog {
    pub events: Vec<String>,
}

impl ActivityLog {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn debug(&self) {
        for event in self.events.iter() {
            println!("{event}");
        }
    }

    pub fn record(&mut self, event: String) {
        self.events.push(event);
    }
}

impl Drop for ActivityLog {
    fn drop(&mut self) {
        self.debug();
    }
}
