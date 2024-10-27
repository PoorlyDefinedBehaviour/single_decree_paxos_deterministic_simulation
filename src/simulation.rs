use std::{cell::RefCell, fmt::Display, rc::Rc};

use rand::{rngs::StdRng, Rng};

use crate::{
    activity_log::ActivityLog,
    contracts::{self, MessageBus},
    oracle::Oracle,
    types::{AcceptInput, AcceptOutput, PrepareInput, PrepareOutput, ReplicaId},
    Replica,
};

#[derive(Debug)]
pub(crate) struct Simulator {
    config: SimulatorConfig,
    rng: Rc<RefCell<StdRng>>,
    replicas: Vec<Rc<RefCell<Replica>>>,
    bus: Rc<SimMessageBus>,
    num_client_requests_sent: u32,
}

#[derive(Debug)]
pub(crate) struct SimulatorConfig {
    num_client_requests: u32,
}

impl Simulator {
    fn new(
        config: SimulatorConfig,
        rng: Rc<RefCell<StdRng>>,
        replicas: Vec<Rc<RefCell<Replica>>>,
        bus: Rc<SimMessageBus>,
    ) -> Self {
        Self {
            config,
            rng,
            replicas,
            bus,
            num_client_requests_sent: 0,
        }
    }

    fn tick(&mut self) {
        if self.num_client_requests_sent < self.config.num_client_requests {
            let i = self.rng.borrow_mut().gen_range(0..self.replicas.len());
            let replica_id = self.replicas[i].borrow().config.id;
            self.bus
                .send_start_proposal(replica_id, format!("V({replica_id})",));
            self.num_client_requests_sent += 1;
        }

        self.bus.tick();
    }
}

#[derive(Debug)]
pub(crate) struct SimMessageBus {
    rng: Rc<RefCell<StdRng>>,
    replicas: RefCell<Vec<Rc<RefCell<Replica>>>>,
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

        let i = self.rng.borrow_mut().gen_range(0..self.items.len());
        let item = self.items.remove(i);
        Some(item)
    }
}

#[derive(Debug)]
pub(crate) enum PendingMessage {
    StartProposal(ReplicaId, String),
    Prepare(ReplicaId, PrepareInput),
    PrepareResponse(ReplicaId, PrepareOutput),
    Accept(ReplicaId, AcceptInput),
    AcceptResponse(ReplicaId, AcceptOutput),
}

#[derive(Debug)]
enum EventType {
    Queue,
    Receive,
}

impl PendingMessage {
    fn to_activity_log_event(&self, event_type: EventType) -> String {
        match self {
            PendingMessage::StartProposal(to_replica_id, value) => match event_type {
                EventType::Queue => format!(
                    "[BUS] Simulator QUEUED StartProposal({}) to replica {}",
                    value, to_replica_id,
                ),
                EventType::Receive => format!(
                    "[BUS] replica {} RECEIVED StartProposal({}) from Simulator",
                    to_replica_id, value
                ),
            },

            PendingMessage::Prepare(to_replica_id, msg) => match event_type {
                EventType::Queue => format!(
                    "[BUS] replica {} QUEUED Prepare({}, {}) to replica {}",
                    msg.from_replica_id, msg.request_id, msg.proposal_number, to_replica_id
                ),
                EventType::Receive => format!(
                    "[BUS] replica {} RECEIVED Prepare({}, {}) from replica {}",
                    to_replica_id, msg.request_id, msg.proposal_number, msg.from_replica_id
                ),
            },

            PendingMessage::PrepareResponse(to_replica_id, msg) => match event_type {
                EventType::Queue => format!(
                    "[BUS] replica {} QUEUED PrepareResponse({}, {:?}, {:?}) to replica {}",
                    msg.from_replica_id,
                    msg.request_id,
                    msg.accepted_proposal_number,
                    msg.accepted_value,
                    to_replica_id,
                ),
                EventType::Receive => format!(
                    "[BUS] replica {} RECEIVED PrepareResponse({}, {:?}, {:?}) from replica {}",
                    to_replica_id,
                    msg.request_id,
                    msg.accepted_proposal_number,
                    msg.accepted_value,
                    msg.from_replica_id,
                ),
            },
            PendingMessage::Accept(to_replica_id, msg) => match event_type {
                EventType::Queue => format!(
                    "[BUS] replica {} QUEUED Accept({}, {}, {}) to replica {}",
                    msg.from_replica_id,
                    msg.request_id,
                    msg.proposal_number,
                    msg.value,
                    to_replica_id
                ),
                EventType::Receive => format!(
                    "[BUS] replica {} RECEIVED Accept({}, {}, {}) from replica {}",
                    to_replica_id,
                    msg.request_id,
                    msg.proposal_number,
                    msg.value,
                    msg.from_replica_id
                ),
            },
            PendingMessage::AcceptResponse(to_replica_id, msg) => match event_type {
                EventType::Queue => format!(
                    "[BUS] replica {} QUEUED AcceptResponse({}, {}) to replica {}",
                    msg.from_replica_id, msg.request_id, msg.min_proposal_number, to_replica_id
                ),
                EventType::Receive => format!(
                    "[BUS] replica {} RECEIVED AcceptResponse({}, {}) from replica {}",
                    to_replica_id, msg.request_id, msg.min_proposal_number, msg.from_replica_id
                ),
            },
        }
    }
}

impl SimMessageBus {
    fn new(
        rng: Rc<RefCell<StdRng>>,
        oracle: Oracle,
        activity_log: Rc<RefCell<ActivityLog>>,
    ) -> Self {
        Self {
            replicas: RefCell::new(Vec::new()),
            queue: RefCell::new(MessageQueue::new(Rc::clone(&rng))),
            rng,
            oracle: RefCell::new(oracle),
            activity_log,
        }
    }
}

impl SimMessageBus {
    fn set_replicas(&self, new_replicas: Vec<Rc<RefCell<Replica>>>) {
        let mut replicas = self.replicas.borrow_mut();
        replicas.clear();
        replicas.extend(new_replicas);
    }

    fn find_replica(&self, replica_id: ReplicaId) -> Rc<RefCell<Replica>> {
        let replicas = self.replicas.borrow();

        let replica = replicas
            .iter()
            .find(|r| r.borrow().config.id == replica_id)
            .expect("unknown replica id");

        Rc::clone(replica)
    }

    fn tick(&self) {
        // let message = {
        //     let mut queue = self.queue.borrow_mut();
        //     if queue.is_empty() {
        //         return;
        //     }
        //     queue.remove(0)
        // };
        let Some(message) = self.queue.borrow_mut().pop() else {
            return;
        };

        self.activity_log
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Receive));

        match message {
            PendingMessage::StartProposal(to_replica_id, value) => {
                let replica = self.find_replica(to_replica_id);
                replica.borrow_mut().on_start_proposal(value);
            }
            PendingMessage::Prepare(to_replica_id, input) => {
                let replica = self.find_replica(to_replica_id);
                replica.borrow_mut().on_prepare(input);
            }
            PendingMessage::PrepareResponse(to_replica_id, input) => {
                self.oracle
                    .borrow()
                    .on_prepare_response_sent(to_replica_id, &input);
                let replica = self.find_replica(to_replica_id);
                replica.borrow_mut().on_prepare_response(input);
            }
            PendingMessage::Accept(to_replica_id, input) => {
                self.oracle
                    .borrow_mut()
                    .on_accept_sent(to_replica_id, &input);
                let replica = self.find_replica(to_replica_id);
                replica.borrow_mut().on_accept(input);
            }
            PendingMessage::AcceptResponse(to_replica_id, input) => {
                self.oracle
                    .borrow_mut()
                    .on_proposal_accepted(to_replica_id, &input);
                let replica = self.find_replica(to_replica_id);
                replica.borrow_mut().on_accept_response(input);
            }
        }
    }
}

impl contracts::MessageBus for SimMessageBus {
    fn send_start_proposal(&self, to_replica_id: ReplicaId, value: String) {
        let message = PendingMessage::StartProposal(to_replica_id, value);
        self.activity_log
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_prepare(&self, to_replica_id: ReplicaId, input: PrepareInput) {
        let message = PendingMessage::Prepare(to_replica_id, input);
        self.activity_log
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_prepare_response(&self, to_replica_id: ReplicaId, input: PrepareOutput) {
        let message = PendingMessage::PrepareResponse(to_replica_id, input);
        self.activity_log
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_accept(&self, to_replica_id: ReplicaId, input: AcceptInput) {
        let message = PendingMessage::Accept(to_replica_id, input);
        self.activity_log
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_accept_response(&self, to_replica_id: ReplicaId, input: AcceptOutput) {
        let message = PendingMessage::AcceptResponse(to_replica_id, input);
        self.activity_log
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::quickcheck;
    use rand::SeedableRng;

    use crate::{Config, Replica};

    use super::*;

    #[test]
    fn basic_simulation() {
        let seed: u64 = rand::thread_rng().gen();
        println!("seed={seed}");

        let rng = Rc::new(RefCell::new(rand::rngs::StdRng::seed_from_u64(seed)));

        let activity_log = Rc::new(RefCell::new(ActivityLog::new()));

        let servers = vec![1, 2, 3];

        let bus: Rc<SimMessageBus> = Rc::new(SimMessageBus::new(
            Rc::clone(&rng),
            Oracle::new(servers.len(), Rc::clone(&activity_log)),
            Rc::clone(&activity_log),
        ));

        let replicas: Vec<_> = servers
            .iter()
            .map(|id| {
                Rc::new(RefCell::new(Replica::new(
                    Config {
                        id: *id,
                        replicas: servers.clone(),
                    },
                    Rc::clone(&bus) as Rc<dyn contracts::MessageBus>,
                )))
            })
            .collect();

        bus.set_replicas(replicas.clone());

        let mut sim = Simulator::new(
            SimulatorConfig {
                num_client_requests: 3,
            },
            rng,
            replicas,
            Rc::clone(&bus),
        );

        for _ in 0..1000 {
            sim.tick();
        }

        activity_log.borrow().debug();
    }

    quickcheck! {
      #[test]
      fn basic_quickcheck(seed: u64, num_client_requests: u32) -> bool {
        // Number of client requests between 1 and 10.
        let num_client_requests = num_client_requests % 3 + 1;
        dbg!(num_client_requests);

        let rng = Rc::new(RefCell::new(rand::rngs::StdRng::seed_from_u64(seed)));

        let servers = vec![1, 2, 3];

        let activity_log = Rc::new(RefCell::new(ActivityLog::new()));

        let bus: Rc<SimMessageBus> = Rc::new(
          SimMessageBus::new(
            Rc::clone(&rng),
            Oracle::new(servers.len(),Rc::clone(&activity_log)),
            Rc::clone(&activity_log),
          )
        );

        let replicas: Vec<_> = servers
            .iter()
            .map(|id| {
                Rc::new(RefCell::new(Replica::new(
                    Config {
                        id: *id,
                        replicas: servers.clone(),
                    },
                    Rc::clone(&bus) as Rc<dyn contracts::MessageBus>,
                )))
            })
            .collect();

        bus.set_replicas(replicas.clone());

        let mut sim = Simulator::new(
            SimulatorConfig {
                num_client_requests,
            },
            rng,
            replicas,
            bus,
        );

        for _ in 0..100 {
            sim.tick();
        }

        true
      }
    }
}
