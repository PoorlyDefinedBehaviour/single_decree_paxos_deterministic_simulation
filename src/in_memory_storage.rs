use std::cell::RefCell;

use crate::contracts;

#[derive(Debug)]
pub struct InMemoryStorage {
    state: RefCell<Option<contracts::DurableState>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            state: RefCell::new(None),
        }
    }
}

impl contracts::Storage for InMemoryStorage {
    fn load(&self) -> contracts::DurableState {
        self.state
            .borrow()
            .clone()
            .unwrap_or(contracts::DurableState {
                min_proposal_number: 0,
                accepted_proposal_number: None,
                accepted_value: None,
            })
    }

    fn store(&self, state: &contracts::DurableState) {
        *self.state.borrow_mut() = Some(state.clone());
    }
}
