use core::cell::RefCell;
use rand::{rngs::StdRng, Rng};
use std::rc::Rc;

use crate::{
    contracts,
    types::{AcceptInput, AcceptOutput, PrepareInput, PrepareOutput, ReplicaId},
};

use super::{activity_log::ActivityLog, oracle::Oracle};

#[derive(Debug, Clone)]
pub enum PendingMessage {
    StartProposal(ReplicaId, String),
    Prepare(ReplicaId, PrepareInput),
    PrepareResponse(ReplicaId, PrepareOutput),
    Accept(ReplicaId, AcceptInput),
    AcceptResponse(ReplicaId, AcceptOutput),
}

#[derive(Debug)]
pub enum EventType {
    Queue,
    Receive,
    Drop,
    Duplicate,
}

impl PendingMessage {
    pub fn get_to_replica_id(&self) -> ReplicaId {
        match self {
            PendingMessage::StartProposal(to_replica_id, _) => *to_replica_id,
            PendingMessage::Prepare(to_replica_id, _prepare_input) => *to_replica_id,
            PendingMessage::PrepareResponse(to_replica_id, _prepare_output) => *to_replica_id,
            PendingMessage::Accept(to_replica_id, _accept_input) => *to_replica_id,
            PendingMessage::AcceptResponse(to_replica_id, _accept_output) => *to_replica_id,
        }
    }

    pub fn to_activity_log_event(&self, event_type: EventType) -> String {
        match self {
            PendingMessage::StartProposal(to_replica_id, value) => match event_type {
                EventType::Queue => format!(
                    "[BUS] Simulator -> Replica({}) QUEUED StartProposal({}) ",
                    to_replica_id, value,
                ),
                EventType::Receive => format!(
                    "[BUS] Simulator -> Replica({}) RECEIVED StartProposal({})",
                    to_replica_id, value
                ),
                EventType::Drop => format!(
                    "[SIMULATOR] DROP Simulator -> Replica({}) RECEIVED StartProposal({})",
                    to_replica_id, value
                ),
                EventType::Duplicate => format!(
                    "[SIMULATOR] DUPLICATE Simulator -> Replica({}) RECEIVED StartProposal({})",
                    to_replica_id, value
                ),
            },
            PendingMessage::Prepare(to_replica_id, msg) => match event_type {
                EventType::Queue => format!(
                    "[BUS] Replica({}) -> Replica({}) QUEUED Prepare({})",
                    msg.from_replica_id, to_replica_id, msg.request_id,
                ),
                EventType::Receive => format!(
                    "[BUS] Replica({}) -> Replica({}) RECEIVED Prepare({})",
                    msg.from_replica_id, to_replica_id, msg.request_id,
                ),
                EventType::Drop => format!(
                    "[SIMULATOR] DROP Replica({}) -> Replica({}) Prepare({})",
                    msg.from_replica_id, to_replica_id, msg.request_id,
                ),
                EventType::Duplicate => format!(
                    "[SIMULATOR] DUPLICATE Replica({}) -> Replica({}) Prepare({})",
                    msg.from_replica_id, to_replica_id, msg.request_id,
                ),
            },

            PendingMessage::PrepareResponse(to_replica_id, msg) => match event_type {
                EventType::Queue => format!(
                    "[BUS] Replica({}) -> Replica({}) QUEUED PrepareResponse({}, {:?}, {:?})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.accepted_proposal_number,
                    msg.accepted_value,
                ),
                EventType::Receive => format!(
                    "[BUS] Replica({}) -> Replica({}) RECEIVED PrepareResponse({}, {:?}, {:?})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.accepted_proposal_number,
                    msg.accepted_value,
                ),
                EventType::Drop => format!(
                    "[SIMULATOR] DROP Replica({}) -> Replica({}) PrepareResponse({}, {:?}, {:?})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.accepted_proposal_number,
                    msg.accepted_value,
                ),
                EventType::Duplicate => format!(
                "[SIMULATOR] DUPLICATE Replica({}) -> Replica({}) PrepareResponse({}, {:?}, {:?})",
                msg.from_replica_id,
                to_replica_id,
                msg.request_id,
                msg.accepted_proposal_number,
                msg.accepted_value,
            ),
            },
            PendingMessage::Accept(to_replica_id, msg) => match event_type {
                EventType::Queue => format!(
                    "[BUS] Replica({}) -> Replica({}) QUEUED Accept({}, {}, {})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.proposal_number,
                    msg.value,
                ),
                EventType::Receive => format!(
                    "[BUS] Replica({}) -> Replica({}) RECEIVED Accept({}, {}, {})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.proposal_number,
                    msg.value,
                ),
                EventType::Drop => format!(
                    "[SIMULATOR] DROP Replica({}) -> Replica({}) Accept({}, {}, {})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.proposal_number,
                    msg.value,
                ),
                EventType::Duplicate => format!(
                    "[SIMULATOR] DUPLICATE Replica({}) -> Replica({}) Accept({}, {}, {})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.proposal_number,
                    msg.value,
                ),
            },
            PendingMessage::AcceptResponse(to_replica_id, msg) => match event_type {
                EventType::Queue => format!(
                    "[BUS] Replica({}) -> Replica({}) QUEUED AcceptResponse({}, {})",
                    msg.from_replica_id, to_replica_id, msg.request_id, msg.proposal_number
                ),
                EventType::Receive => format!(
                    "[BUS] Replica({}) -> Replica({}) RECEIVED AcceptResponse({}, {})",
                    msg.from_replica_id, to_replica_id, msg.request_id, msg.proposal_number,
                ),
                EventType::Drop => format!(
                    "[SIMULATOR] DROP Replica({}) -> Replica({}) AcceptResponse({}, {})",
                    msg.from_replica_id, to_replica_id, msg.request_id, msg.proposal_number,
                ),
                EventType::Duplicate => format!(
                    "[SIMULATOR] DUPLICATE Replica({}) -> Replica({}) AcceptResponse({}, {})",
                    msg.from_replica_id, to_replica_id, msg.request_id, msg.proposal_number,
                ),
            },
        }
    }
}

#[derive(Debug)]
pub struct SimMessageBus {
    queue: RefCell<MessageQueue>,
    oracle: RefCell<Oracle>,
    activity_log: Rc<RefCell<ActivityLog>>,
}

#[derive(Debug)]
struct MessageQueue {
    rng: Rc<RefCell<StdRng>>,
    items: Vec<PendingMessage>,
}

impl MessageQueue {
    fn new(rng: Rc<RefCell<StdRng>>) -> Self {
        Self {
            rng,
            items: Vec::new(),
        }
    }

    fn push(&mut self, message: PendingMessage) {
        self.items.push(message);
    }

    // Pops a message from the queue. Messages may be delivered in any order.
    fn pop(&mut self) -> Option<PendingMessage> {
        if self.items.is_empty() {
            return None;
        }

        let i = self
            .rng
            .as_ref()
            .borrow_mut()
            .gen_range(0..self.items.len());
        let item = self.items.swap_remove(i);
        Some(item)
    }
}

impl SimMessageBus {
    pub fn new(
        rng: Rc<RefCell<StdRng>>,
        oracle: Oracle,
        activity_log: Rc<RefCell<ActivityLog>>,
    ) -> Self {
        Self {
            queue: RefCell::new(MessageQueue::new(Rc::clone(&rng))),

            oracle: RefCell::new(oracle),
            activity_log,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.borrow().items.is_empty()
    }

    pub fn next_message(&self) -> Option<PendingMessage> {
        let message = self.queue.borrow_mut().pop()?;
        match &message {
            PendingMessage::Accept(to_replica_id, input) => {
                self.oracle
                    .borrow_mut()
                    .on_accept_sent(*to_replica_id, input);
            }
            PendingMessage::AcceptResponse(to_replica_id, input) => {
                self.oracle
                    .borrow_mut()
                    .on_proposal_accepted(*to_replica_id, input);
            }
            PendingMessage::StartProposal(_, _)
            | PendingMessage::Prepare(_, _)
            | PendingMessage::PrepareResponse(_, _) => {
                // no-op.
            }
        }
        Some(message)
    }

    pub fn add_message(&self, message: PendingMessage) {
        let mut queue = self.queue.borrow_mut();
        queue.push(message);
    }

    pub fn num_messages_in_flight(&self) -> usize {
        self.queue.borrow().items.len()
    }
}

impl contracts::MessageBus for SimMessageBus {
    fn send_start_proposal(&self, to_replica_id: ReplicaId, value: String) {
        let message = PendingMessage::StartProposal(to_replica_id, value);
        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_prepare(&self, to_replica_id: ReplicaId, input: PrepareInput) {
        let message = PendingMessage::Prepare(to_replica_id, input);
        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_prepare_response(&self, to_replica_id: ReplicaId, input: PrepareOutput) {
        let message = PendingMessage::PrepareResponse(to_replica_id, input);
        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_accept(&self, to_replica_id: ReplicaId, input: AcceptInput) {
        let message = PendingMessage::Accept(to_replica_id, input);
        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_accept_response(&self, to_replica_id: ReplicaId, input: AcceptOutput) {
        let message = PendingMessage::AcceptResponse(to_replica_id, input);
        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }
}
