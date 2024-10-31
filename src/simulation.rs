use std::{cell::RefCell, collections::HashSet, rc::Rc};

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
    rng: Rc<RefCell<StdRng>>,
    bus: Rc<SimMessageBus>,
    replicas: Vec<Replica>,
    oracle: Oracle,
    healthy_replicas: HashSet<ReplicaId>,
    failed_replicas: HashSet<ReplicaId>,
}

#[derive(Debug)]
pub(crate) struct ActionSimulatorConfig {
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
}

impl ActionSimulatorMetrics {
    fn new() -> Self {
        Self {
            num_user_requests_sent: 0,
            num_replica_crashes: 0,
            num_replica_restarts: 0,
        }
    }
}

#[derive(Debug)]
enum Action {
    SendUserRequest,
    CrashReplica,
    RestartReplica,
    DeliverMessage,
}

impl ActionSimulator {
    fn new(
        config: ActionSimulatorConfig,
        rng: Rc<RefCell<StdRng>>,
        replicas: Vec<Replica>,
        bus: Rc<SimMessageBus>,
        oracle: Oracle,
    ) -> Self {
        Self {
            config,
            rng,
            bus,
            oracle,
            healthy_replicas: HashSet::from_iter(replicas.iter().map(|r| r.config.id)),
            failed_replicas: HashSet::new(),
            replicas,
            metrics: ActionSimulatorMetrics::new(),
        }
    }

    fn majority(&self) -> usize {
        self.replicas.len() / 2 + 1
    }

    fn next_action(&mut self) -> Action {
        loop {
            let n: u8 = self.rng.borrow_mut().gen::<u8>() % 4;
            match n {
                0 => {
                    if self.metrics.num_user_requests_sent < self.config.max_user_requests {
                        self.metrics.num_user_requests_sent += 1;
                        return Action::SendUserRequest;
                    }
                }
                1 => {
                    if self.metrics.num_replica_crashes < self.config.max_replica_crashes {
                        self.metrics.num_replica_crashes += 1;
                        return Action::CrashReplica;
                    }
                }
                2 => {
                    if self.metrics.num_replica_restarts < self.config.max_replica_restarts {
                        self.metrics.num_replica_restarts += 1;
                        return Action::RestartReplica;
                    }
                }
                3 => return Action::DeliverMessage,
                _ => unreachable!(),
            }
        }
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

    fn get_healthy_replica_mut(&mut self, replica_id: ReplicaId) -> Option<&mut Replica> {
        self.replicas
            .iter_mut()
            .find(|r| self.healthy_replicas.contains(&r.config.id))
    }

    fn run(&mut self) {
        for i in 0..self.config.max_actions {
            match dbg!(self.next_action()) {
                Action::SendUserRequest => {
                    let replica_id = {
                        let replica = self.choose_healthy_replica();
                        replica.config.id
                    };
                    let value = format!("V({}-{})", replica_id, i);
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
                    }
                }
                Action::RestartReplica => {
                    let replica_id = {
                        let replica = self.choose_any_replica();
                        replica.config.id
                    };
                    self.failed_replicas.remove(&replica_id);
                    self.healthy_replicas.insert(replica_id);
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
        match message {
            PendingMessage::StartProposal(to_replica_id, value) => {
                if let Some(replica) = self.get_healthy_replica_mut(to_replica_id) {
                    replica.on_start_proposal(value);
                }
            }
            PendingMessage::Prepare(to_replica_id, input) => {
                if let Some(replica) = self.get_healthy_replica_mut(to_replica_id) {
                    replica.on_prepare(input);
                }
            }
            PendingMessage::PrepareResponse(to_replica_id, input) => {
                if let Some(replica) = self.get_healthy_replica_mut(to_replica_id) {
                    replica.on_prepare_response(input);
                }
            }
            PendingMessage::Accept(to_replica_id, input) => {
                self.oracle.on_accept_sent(to_replica_id, &input);
                if let Some(replica) = self.get_healthy_replica_mut(to_replica_id) {
                    replica.on_accept(input);
                }
            }
            PendingMessage::AcceptResponse(to_replica_id, input) => {
                self.oracle.on_proposal_accepted(to_replica_id, &input);
                if let Some(replica) = self.get_healthy_replica_mut(to_replica_id) {
                    replica.on_accept_response(input);
                }
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Simulator {
    config: SimulatorConfig,
    rng: Rc<RefCell<StdRng>>,
    bus: Rc<SimMessageBus>,
    healthy_replicas: Vec<Replica>,
    failed_replicas: Vec<Replica>,
    failure_injector: FailureInjector,
    num_user_requests_sent: u32,
}

#[derive(Debug)]
pub(crate) struct SimulatorConfig {
    max_user_requests: u32,
    send_user_request_probability: f64,
    crash_replica_probability: f64,
    restart_replica_probability: f64,
}

impl Simulator {
    fn new(
        config: SimulatorConfig,
        rng: Rc<RefCell<StdRng>>,
        replicas: Vec<Replica>,
        bus: Rc<SimMessageBus>,
        failure_injector: FailureInjector,
    ) -> Self {
        Self {
            config,
            rng,
            bus,
            healthy_replicas: replicas,
            failed_replicas: Vec::new(),
            failure_injector,
            num_user_requests_sent: 0,
        }
    }

    fn tick(&mut self) {
        // if self.num_user_requests_sent < self.config.max_user_requests
        //     && self
        //         .rng
        //         .borrow_mut()
        //         .gen_bool(self.config.send_user_request_probability)
        // {
        //     let i = self
        //         .rng
        //         .as_ref()
        //         .borrow_mut()
        //         .gen_range(0..self.healthy_replicas.len());
        //     let replica_id = self.healthy_replicas[i].config.id;
        //     self.bus.send_start_proposal(
        //         replica_id,
        //         format!("V({replica_id}-{})", self.num_user_requests_sent),
        //     );
        //     self.num_user_requests_sent += 1;
        // }

        // self.failure_injector
        //     .tick(&mut self.healthy_replicas, &mut self.failed_replicas);

        // self.bus.tick(&mut self.healthy_replicas);
    }
}

#[derive(Debug)]
pub(crate) struct SimMessageBus {
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

        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Receive));

        Some(message)
    }

    fn tick(&self, healthy_replicas: &mut [Replica]) {
        let Some(message) = self.queue.borrow_mut().pop() else {
            return;
        };

        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Receive));

        match message {
            PendingMessage::StartProposal(to_replica_id, value) => {
                if let Some(replica) = self.find_replica(healthy_replicas, to_replica_id) {
                    replica.on_start_proposal(value);
                }
            }
            PendingMessage::Prepare(to_replica_id, input) => {
                if let Some(replica) = self.find_replica(healthy_replicas, to_replica_id) {
                    replica.on_prepare(input);
                }
            }
            PendingMessage::PrepareResponse(to_replica_id, input) => {
                if let Some(replica) = self.find_replica(healthy_replicas, to_replica_id) {
                    replica.on_prepare_response(input);
                }
            }
            PendingMessage::Accept(to_replica_id, input) => {
                self.oracle
                    .borrow_mut()
                    .on_accept_sent(to_replica_id, &input);
                if let Some(replica) = self.find_replica(healthy_replicas, to_replica_id) {
                    replica.on_accept(input);
                }
            }
            PendingMessage::AcceptResponse(to_replica_id, input) => {
                self.oracle
                    .borrow_mut()
                    .on_proposal_accepted(to_replica_id, &input);
                if let Some(replica) = self.find_replica(healthy_replicas, to_replica_id) {
                    replica.on_accept_response(input);
                }
            }
        }
    }
}

#[derive(Debug)]
struct FailureInjector {
    activity_log: Rc<RefCell<ActivityLog>>,
    rng: Rc<RefCell<StdRng>>,
    majority: usize,
}

impl FailureInjector {
    fn new(
        activity_log: Rc<RefCell<ActivityLog>>,
        rng: Rc<RefCell<StdRng>>,
        majority: usize,
    ) -> Self {
        Self {
            activity_log,
            rng,
            majority,
        }
    }

    fn tick(&mut self, healthy_replicas: &mut Vec<Replica>, failed_replicas: &mut Vec<Replica>) {
        return;
        if self.rng.as_ref().borrow_mut().gen_bool(0.8) && !failed_replicas.is_empty() {
            let i = self
                .rng
                .as_ref()
                .borrow_mut()
                .gen_range(0..failed_replicas.len());

            self.activity_log.as_ref().borrow_mut().record(format!(
                "[FailureInjector] RESTART replica {}",
                failed_replicas[i].config.id
            ));

            // Pretend the replica restarted by re-creating it.
            let replica = failed_replicas.remove(i);
            healthy_replicas.push(Replica::new(replica.config, replica.bus))
        }

        if self.rng.as_ref().borrow_mut().gen_bool(0.05) && healthy_replicas.len() > self.majority {
            let i = self
                .rng
                .as_ref()
                .borrow_mut()
                .gen_range(0..healthy_replicas.len());

            self.activity_log.as_ref().borrow_mut().record(format!(
                "[FailureInjector] CRASH replica {}",
                healthy_replicas[i].config.id
            ));

            failed_replicas.push(healthy_replicas.remove(i));
        }
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

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use quickcheck::quickcheck;
    use rand::{Rng, SeedableRng};

    use crate::{Config, Replica};

    use super::*;

    #[test]
    fn action_simulation() -> Result<()> {
        let seed: u64 = rand::thread_rng().gen();
        println!("seed={seed}");

        let rng = Rc::new(RefCell::new(rand::rngs::StdRng::seed_from_u64(seed)));

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
                )
            })
            .collect();

        let mut sim = ActionSimulator::new(
            ActionSimulatorConfig {
                max_actions: 10_000,
                max_user_requests: 10,
                max_replica_crashes: 3,
                max_replica_restarts: 10,
            },
            Rc::clone(&rng),
            replicas,
            Rc::clone(&bus),
            Oracle::new(majority, Rc::clone(&activity_log)),
        );

        sim.run();

        assert!(sim.bus.queue.borrow().items.is_empty());

        Ok(())
    }

    #[test]
    fn basic_simulation() -> Result<()> {
        let seed: u64 = rand::thread_rng().gen();
        println!("seed={seed}");

        let rng = Rc::new(RefCell::new(rand::rngs::StdRng::seed_from_u64(seed)));

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
                )
            })
            .collect();

        let mut sim = Simulator::new(
            SimulatorConfig {
                max_user_requests: u32::MAX,
                send_user_request_probability: 0.7,
                crash_replica_probability: 0.08,
                restart_replica_probability: 0.7,
            },
            Rc::clone(&rng),
            replicas,
            Rc::clone(&bus),
            FailureInjector::new(Rc::clone(&activity_log), Rc::clone(&rng), majority),
        );

        for _ in 0..10_000_000 {
            sim.tick();
        }

        assert!(sim.bus.queue.borrow().items.is_empty());

        Ok(())
    }

    // quickcheck! {
    //   #[test]
    //   fn basic_quickcheck(seed: u64, num_client_requests: u32) -> bool {
    //     // Cap the number of client requests.
    //     let num_client_requests = num_client_requests % 100 + 1;

    //     let rng = Rc::new(RefCell::new(rand::rngs::StdRng::seed_from_u64(seed)));

    //     let servers = vec![1, 2, 3];

    //     let majority = servers.len()/2+1;

    //     let activity_log = Rc::new(RefCell::new(ActivityLog::new()));

    //     let bus: Rc<SimMessageBus> = Rc::new(
    //       SimMessageBus::new(
    //         Rc::clone(&rng),
    //         Oracle::new(servers.len(),Rc::clone(&activity_log)),
    //         Rc::clone(&activity_log),
    //       )
    //     );

    //     let replicas: Vec<_> = servers
    //         .iter()
    //         .map(|id| {
    //             Replica::new(
    //                 Config {
    //                     id: *id,
    //                     replicas: servers.clone(),
    //                 },
    //                 Rc::clone(&bus) as Rc<dyn contracts::MessageBus>,
    //             )
    //         })
    //         .collect();

    //     let mut sim = Simulator::new(
    //         SimulatorConfig {
    //             num_client_requests,
    //             send_user_request_probability: 0.4,
    //             crash_replica_probability: 0.05,
    //             restart_replica_probability: 0.7,
    //         },
    //         Rc::clone(&rng),
    //         replicas,
    //         bus,
    //         FailureInjector::new(Rc::clone(&activity_log), Rc::clone(&rng), majority),
    //     );

    //     for _ in 0..100_000 {
    //         sim.tick();
    //     }

    //     true
    //   }
    // }
}
