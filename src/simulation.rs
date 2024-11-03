use std::{cell::RefCell, collections::HashSet, panic::UnwindSafe, rc::Rc};

use rand::{
    rngs::StdRng,
    seq::{IteratorRandom, SliceRandom},
    Rng,
};

use crate::{
    activity_log::ActivityLog,
    contracts::{self, MessageBus},
    oracle::Oracle,
    types::{AcceptInput, AcceptOutput, PrepareInput, PrepareOutput, ReplicaId},
    Replica,
};

#[derive(Debug)]
struct ActionSimulator {
    config: ActionSimulatorConfig,
    metrics: ActionSimulatorMetrics,
    action_set: ActionSet,
    rng: Rc<RefCell<StdRng>>,
    bus: Rc<SimMessageBus>,
    replicas: Vec<Replica>,
    oracle: Oracle,
    activity_log: Rc<RefCell<ActivityLog>>,
    healthy_replicas: HashSet<ReplicaId>,
    failed_replicas: HashSet<ReplicaId>,
}

#[derive(Debug)]
struct ActionSet {
    items: Vec<Action>,
}

impl ActionSet {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    fn choose(&self, rng: &mut StdRng) -> (usize, Action) {
        let i = rng.gen_range(0..self.items.len());
        (i, self.items[i])
    }

    fn insert(&mut self, value: Action) {
        if !self.items.contains(&value) {
            self.items.push(value);
        }
    }

    fn remove_index(&mut self, i: usize) {
        self.items.swap_remove(i);
    }
}

impl UnwindSafe for ActionSimulator {}

#[derive(Debug)]
pub struct ActionSimulatorConfig {
    max_actions: u32,
    max_user_requests: u32,
    max_replica_crashes: u32,
    max_replica_restarts: u32,
}

#[derive(Debug)]
struct ActionSimulatorMetrics {
    num_user_requests_sent: u32,
    num_replica_crashes: u32,
    num_replica_restarts: u32,
    num_messages_delivered: u32,
    num_messages_dropped: u32,
    num_messages_duplicated: u32,
}

impl ActionSimulatorMetrics {
    fn new() -> Self {
        Self {
            num_user_requests_sent: 0,
            num_replica_crashes: 0,
            num_replica_restarts: 0,
            num_messages_delivered: 0,
            num_messages_dropped: 0,
            num_messages_duplicated: 0,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Action {
    SendUserRequest,
    CrashReplica,
    RestartReplica,
    DeliverMessage,
    DropMessage,
    DuplicateMessage,
}

impl ActionSimulator {
    fn new(
        config: ActionSimulatorConfig,
        rng: Rc<RefCell<StdRng>>,
        replicas: Vec<Replica>,
        bus: Rc<SimMessageBus>,
        oracle: Oracle,
        activity_log: Rc<RefCell<ActivityLog>>,
    ) -> Self {
        Self {
            config,
            rng,
            bus,
            action_set: {
                let mut set = ActionSet::new();
                set.insert(Action::SendUserRequest);
                set.insert(Action::CrashReplica);
                set.insert(Action::RestartReplica);
                set
            },
            oracle,
            activity_log,
            healthy_replicas: HashSet::from_iter(replicas.iter().map(|r| r.config.id)),
            failed_replicas: HashSet::new(),
            replicas,
            metrics: ActionSimulatorMetrics::new(),
        }
    }

    fn majority(&self) -> usize {
        self.replicas.len() / 2 + 1
    }

    fn next_action(&mut self, metrics: TODO) {
        let action = self
            .action_set
            .filter(|action| action.passes_constraints(&metrics))
            .choose(rng);

        match action {}
    }

    fn next_action(&mut self) -> Action {
        dbg!(&self.action_set);
        let (i, action) = self.action_set.choose(&mut self.rng.borrow_mut());

        match action {
            Action::SendUserRequest => {
                self.metrics.num_user_requests_sent += 1;

                if self.metrics.num_user_requests_sent >= self.config.max_user_requests {
                    self.action_set.remove_index(i);
                }

                self.action_set.insert(Action::DeliverMessage);
                self.action_set.insert(Action::DropMessage);
                self.action_set.insert(Action::DuplicateMessage);
            }
            Action::CrashReplica => {
                self.metrics.num_replica_crashes += 1;

                if self.metrics.num_replica_crashes >= self.config.max_replica_crashes {
                    self.action_set.remove_index(i);
                }
            }
            Action::RestartReplica => {
                self.metrics.num_replica_restarts += 1;

                if self.metrics.num_replica_restarts >= self.config.max_replica_restarts {
                    self.action_set.remove_index(i);
                }
            }
            Action::DeliverMessage => {
                self.metrics.num_messages_delivered += 1;

                if self.metrics.num_messages_delivered >= self.metrics.num_user_requests_sent {
                    self.action_set.remove_index(i);
                }
            }
            Action::DropMessage => {
                self.metrics.num_messages_dropped += 1;

                if self.metrics.num_messages_dropped >= self.metrics.num_messages_delivered {
                    self.action_set.remove_index(i);
                }
            }
            Action::DuplicateMessage => {
                self.metrics.num_messages_duplicated += 1;

                if self.metrics.num_messages_duplicated >= self.metrics.num_messages_delivered {
                    self.action_set.remove_index(i);
                }
            }
        }

        action
    }

    fn choose_healthy_replica(&mut self) -> &Replica {
        let replicas = self
            .replicas
            .iter()
            .filter(|r| self.healthy_replicas.contains(&r.config.id));
        replicas.choose(unsafe { &mut *self.rng.as_ptr() }).unwrap()
    }

    fn choose_any_replica(&mut self) -> &Replica {
        self.replicas
            .choose(unsafe { &mut *self.rng.as_ptr() })
            .unwrap()
    }

    fn get_healthy_replicas_mut(&mut self) -> Vec<&mut Replica> {
        self.replicas
            .iter_mut()
            .filter(|r| self.healthy_replicas.contains(&r.config.id))
            .collect()
    }

    fn get_healthy_replica_index(&mut self, replica_id: ReplicaId) -> Option<usize> {
        self.replicas.iter().enumerate().find_map(|(i, r)| {
            if r.config.id == replica_id && self.healthy_replicas.contains(&r.config.id) {
                Some(i)
            } else {
                None
            }
        })
    }

    fn recreate_replica(&mut self, replica_id: ReplicaId) {
        for i in 0..self.replicas.len() {
            if self.replicas[i].config.id == replica_id {
                let replica = self.replicas.swap_remove(i);
                self.replicas
                    .push(Replica::new(replica.config, replica.bus, replica.storage));
                return;
            }
        }
    }

    fn run(&mut self) {
        for i in 0..self.config.max_actions {
            let action = self.next_action();

            match action {
                Action::SendUserRequest => {
                    let replica_id = {
                        let replica = self.choose_healthy_replica();
                        replica.config.id
                    };
                    let value = format!("V({}, {})", replica_id, i);
                    self.bus.send_start_proposal(replica_id, value);
                }
                Action::CrashReplica => {
                    if self.healthy_replicas.len() > self.majority() {
                        let replica_id = {
                            let replica = self.choose_healthy_replica();
                            replica.config.id
                        };
                        self.healthy_replicas.remove(&replica_id);
                        self.failed_replicas.insert(replica_id);
                        self.activity_log
                            .borrow_mut()
                            .record(format!("[SIMULATOR] CRASH Replica({replica_id})"));
                    }
                }
                Action::RestartReplica => {
                    let replica_id = {
                        let replica = self.choose_any_replica();
                        replica.config.id
                    };
                    self.recreate_replica(replica_id);
                    self.failed_replicas.remove(&replica_id);
                    self.healthy_replicas.insert(replica_id);
                    self.activity_log
                        .borrow_mut()
                        .record(format!("[SIMULATOR] RESTART Replica({replica_id})"));
                }
                Action::DropMessage => {
                    if let Some(message) = self.bus.next_message() {
                        self.activity_log
                            .borrow_mut()
                            .record(message.to_activity_log_event(EventType::Drop));
                    }
                }
                Action::DuplicateMessage => {
                    if let Some(message) = self.bus.next_message() {
                        self.activity_log
                            .borrow_mut()
                            .record(message.to_activity_log_event(EventType::Duplicate));
                        self.bus.add_message(message.clone());
                        self.bus.add_message(message);
                    }
                }
                Action::DeliverMessage => {
                    if let Some(message) = self.bus.next_message() {
                        self.deliver_message(message);
                    }
                }
            }
        }

        while let Some(message) = self.bus.next_message() {
            self.deliver_message(message);
        }
    }

    fn deliver_message(&mut self, message: PendingMessage) {
        let Some(i) = self.get_healthy_replica_index(message.get_to_replica_id()) else {
            return;
        };

        let replica = &mut self.replicas[i];

        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Receive));

        match message {
            PendingMessage::StartProposal(_to_replica_id, value) => {
                replica.on_start_proposal(value);
            }
            PendingMessage::Prepare(_to_replica_id, input) => {
                replica.on_prepare(input);
            }
            PendingMessage::PrepareResponse(_to_replica_id, input) => {
                replica.on_prepare_response(input);
            }
            PendingMessage::Accept(to_replica_id, input) => {
                self.oracle.on_accept_sent(to_replica_id, &input);
                replica.on_accept(input);
            }
            PendingMessage::AcceptResponse(to_replica_id, input) => {
                self.oracle.on_proposal_accepted(to_replica_id, &input);
                replica.on_accept_response(input);
            }
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
        let item = self.items.remove(i);
        Some(item)
    }
}

#[derive(Debug, Clone)]
pub enum PendingMessage {
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
    Drop,
    Duplicate,
}

impl PendingMessage {
    fn get_to_replica_id(&self) -> ReplicaId {
        match self {
            PendingMessage::StartProposal(to_replica_id, _) => *to_replica_id,
            PendingMessage::Prepare(to_replica_id, prepare_input) => *to_replica_id,
            PendingMessage::PrepareResponse(to_replica_id, prepare_output) => *to_replica_id,
            PendingMessage::Accept(to_replica_id, accept_input) => *to_replica_id,
            PendingMessage::AcceptResponse(to_replica_id, accept_output) => *to_replica_id,
        }
    }
    fn to_activity_log_event(&self, event_type: EventType) -> String {
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
                    msg.from_replica_id, to_replica_id, msg.request_id, msg.min_proposal_number
                ),
                EventType::Receive => format!(
                    "[BUS] Replica({}) -> Replica({}) RECEIVED AcceptResponse({}, {})",
                    msg.from_replica_id, to_replica_id, msg.request_id, msg.min_proposal_number,
                ),
                EventType::Drop => format!(
                    "[SIMULATOR] DROP Replica({}) -> Replica({}) AcceptResponse({}, {})",
                    msg.from_replica_id, to_replica_id, msg.request_id, msg.min_proposal_number,
                ),
                EventType::Duplicate => format!(
                  "[SIMULATOR] DUPLICATE Replica({}) -> Replica({}) AcceptResponse({}, {})",
                  msg.from_replica_id, to_replica_id, msg.request_id, msg.min_proposal_number,
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
            queue: RefCell::new(MessageQueue::new(Rc::clone(&rng))),

            oracle: RefCell::new(oracle),
            activity_log,
        }
    }
}

impl SimMessageBus {
    fn find_replica<'a>(
        &self,
        replicas: &'a mut [Replica],
        replica_id: ReplicaId,
    ) -> Option<&'a mut Replica> {
        replicas.iter_mut().find(|r| r.config.id == replica_id)
    }

    fn next_message(&self) -> Option<PendingMessage> {
        let message = self.queue.borrow_mut().pop()?;

        Some(message)
    }

    fn add_message(&self, message: PendingMessage) {
        let mut queue = self.queue.borrow_mut();
        queue.push(message);
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
        // self.activity_log
        //     .as_ref()
        //     .borrow_mut()
        //     .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_prepare_response(&self, to_replica_id: ReplicaId, input: PrepareOutput) {
        let message = PendingMessage::PrepareResponse(to_replica_id, input);
        // self.activity_log
        //     .as_ref()
        //     .borrow_mut()
        //     .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_accept(&self, to_replica_id: ReplicaId, input: AcceptInput) {
        let message = PendingMessage::Accept(to_replica_id, input);
        // self.activity_log
        //     .as_ref()
        //     .borrow_mut()
        //     .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_accept_response(&self, to_replica_id: ReplicaId, input: AcceptOutput) {
        let message = PendingMessage::AcceptResponse(to_replica_id, input);
        // self.activity_log
        //     .as_ref()
        //     .borrow_mut()
        //     .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use rand::{Rng, SeedableRng};

    use crate::{in_memory_storage::InMemoryStorage, Config, Replica};

    use super::*;
    #[test]
    fn action_simulation() -> Result<()> {
        let count = if std::env::var("SEED").is_ok() {
            1
        } else {
            std::thread::available_parallelism()?.get()
        };

        eprintln!("Spawning {count} threads");

        let handles: Vec<_> = (0..count)
            .map(|_| {
                std::thread::spawn(|| {
                    let seed: u64 = std::env::var("SEED")
                        .map(|v| v.parse::<u64>().unwrap())
                        .unwrap_or_else(|_| rand::thread_rng().gen());

                    let max_iters = std::env::var("MAX_ITERS")
                        .map(|v| v.parse::<u64>().unwrap())
                        .unwrap_or_else(|_| 10_000);

                    let max_actions = std::env::var("MAX_ACTIONS")
                        .map(|v| v.parse::<u32>().unwrap())
                        .unwrap_or_else(|_| 1000);

                    eprintln!("SEED={seed}");

                    let rng = Rc::new(RefCell::new(rand::rngs::StdRng::seed_from_u64(seed)));

                    for i in 0..max_iters {
                        if i % 1_000 == 0 {
                            eprintln!("Running simulation {i}");
                        }
                        let simulator_config = {
                            let mut rng = rng.borrow_mut();
                            ActionSimulatorConfig {
                                max_actions,
                                max_user_requests: rng.gen::<u32>() % 100 + 1,
                                max_replica_crashes: rng.gen::<u32>() % 100,
                                max_replica_restarts: rng.gen::<u32>() % 100,
                            }
                        };

                        let activity_log = Rc::new(RefCell::new(ActivityLog::new()));

                        let servers = vec![1, 2, 3];

                        let majority = servers.len() / 2 + 1;

                        let bus: Rc<SimMessageBus> = Rc::new(SimMessageBus::new(
                            Rc::clone(&rng),
                            Oracle::new(majority, Rc::clone(&activity_log)),
                            Rc::clone(&activity_log),
                        ));

                        let replicas: Vec<_> = servers
                            .iter()
                            .map(|id| {
                                Replica::new(
                                    Config {
                                        id: *id,
                                        replicas: servers.clone(),
                                    },
                                    Rc::clone(&bus) as Rc<dyn contracts::MessageBus>,
                                    Rc::new(InMemoryStorage::new()),
                                )
                            })
                            .collect();

                        let mut sim = ActionSimulator::new(
                            simulator_config,
                            Rc::clone(&rng),
                            replicas,
                            Rc::clone(&bus),
                            Oracle::new(majority, Rc::clone(&activity_log)),
                            Rc::clone(&activity_log),
                        );

                        let result = std::panic::catch_unwind(move || {
                            sim.run();
                            assert!(sim.bus.queue.borrow().items.is_empty());
                        });
                        if result.is_err() {
                            activity_log.borrow_mut().print_events();
                            eprintln!("SEED={seed}");
                            std::process::exit(1);
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        Ok(())
    }
}
